//! 单向 scp 发布更新：按 release ID 校验、安装并重启服务。
use crate::health::{self, RollbackInfo};
use crate::paths::InstallPaths;
use anyhow::{Context, Result};
use notify::Watcher;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use tokio::time::{interval, Duration};
use tracing::{info, warn};

/// 监听、轮询并恢复 incoming/ 中的已提交发布。
pub async fn watch_for_updates(paths: &InstallPaths) -> Result<()> {
    let (tx, mut rx) = tokio::sync::mpsc::channel(8);
    let mut watcher = notify::recommended_watcher(move |res| {
        if let Ok(event) = res {
            let _ = tx.blocking_send(event);
        }
    })
    .context("create watcher")?;

    watcher
        .watch(&paths.incoming_dir, notify::RecursiveMode::NonRecursive)
        .context("watch incoming dir")?;

    try_ready_releases(paths).await;
    let mut retry = interval(Duration::from_secs(5));

    loop {
        tokio::select! {
            Some(_) = rx.recv() => try_ready_releases(paths).await,
            _ = retry.tick() => try_ready_releases(paths).await,
        }
    }
}

async fn try_ready_releases(paths: &InstallPaths) {
    let release_ids = match ready_release_ids(paths) {
        Ok(ids) => ids,
        Err(error) => {
            warn!(%error, "failed to scan incoming releases");
            return;
        }
    };

    for release_id in release_ids {
        info!(%release_id, "ready release detected, trying to apply");
        if let Err(error) = try_apply_update(paths, &release_id).await {
            warn!(%error, release_id, "release is incomplete or invalid, will retry");
        }
    }
}

fn ready_release_ids(paths: &InstallPaths) -> Result<Vec<String>> {
    let mut release_ids = Vec::new();
    for entry in std::fs::read_dir(&paths.incoming_dir)? {
        let path = entry?.path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let Some(release_id) = name
            .strip_prefix("release.")
            .and_then(|name| name.strip_suffix(".ready"))
        else {
            continue;
        };
        if valid_release_id(release_id) {
            release_ids.push(release_id.to_owned());
        }
    }
    release_ids.sort();
    Ok(release_ids)
}

fn valid_release_id(release_id: &str) -> bool {
    !release_id.is_empty()
        && release_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

fn release_path(paths: &InstallPaths, release_id: &str, suffix: &str) -> PathBuf {
    paths
        .incoming_dir
        .join(format!("release.{release_id}.{suffix}"))
}

/// 校验、安装并重启服务。
async fn try_apply_update(paths: &InstallPaths, release_id: &str) -> Result<()> {
    let ready_path = release_path(paths, release_id, "ready");
    let service_path = release_path(paths, release_id, "sensor-agent");
    let updater_path = release_path(paths, release_id, "sensor-agent-updater");
    let manifest_path = release_path(paths, release_id, "json");

    for path in [&ready_path, &service_path, &updater_path, &manifest_path] {
        if !path.exists() {
            anyhow::bail!("incomplete release, missing {}", path.display());
        }
    }

    let ready_id = std::fs::read_to_string(&ready_path)?.trim().to_owned();
    if ready_id != release_id {
        anyhow::bail!("ready marker release ID mismatch");
    }

    let manifest: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&manifest_path)?)
            .context("parse release manifest")?;
    let expected_service = manifest["sha256"]
        .as_str()
        .filter(|value| !value.is_empty())
        .context("manifest has no service sha256")?;
    let expected_updater = manifest["updater_sha256"]
        .as_str()
        .filter(|value| !value.is_empty())
        .context("manifest has no updater sha256")?;

    if file_sha256(&service_path)? != expected_service {
        anyhow::bail!("sensor-agent sha256 mismatch");
    }
    if file_sha256(&updater_path)? != expected_updater {
        anyhow::bail!("sensor-agent-updater sha256 mismatch");
    }

    let new_seq = manifest["release_seq"]
        .as_u64()
        .context("manifest has no valid release_seq")?;

    let old_seq = std::fs::read_to_string(&paths.manifest_file).context("read current release manifest")
        .and_then(|data| serde_json::from_str::<serde_json::Value>(&data).context("parse current release manifest"))
        .and_then(|old_manifest| {
            old_manifest["release_seq"]
                .as_u64()
                .context("current manifest has no valid release_seq")
        })?;

    if new_seq <= old_seq {
        info!(%release_id, new_seq, old_seq, "release is not newer, skipping");
        cleanup(paths, release_id);
        return Ok(());
    }

    health::clear_healthy(paths)?;
    std::fs::copy(&paths.agent_bin, paths.agent_previous_file()).context("backup sensor-agent")?;
    std::fs::copy(&paths.updater_bin, paths.updater_previous_file()).ok();
    if paths.manifest_file.exists() {
        std::fs::copy(&paths.manifest_file, paths.manifest_previous_file()).context("backup release manifest")?;
    }
    install_binary(&service_path, &paths.agent_bin).context("install sensor-agent")?;
    install_binary(&updater_path, &paths.updater_bin).context("install updater")?;
    atomic_copy(&manifest_path, &paths.manifest_file)?;

    restart_agent(&paths.service_name)?;
    tokio::time::sleep(Duration::from_secs(30)).await;

    if health::is_healthy(paths) {
        cleanup(paths, release_id);
        info!(%release_id, "release applied and became healthy");
        return Ok(());
    }

    warn!(%release_id, "release did not become healthy, rolling back");
    rollback_release(paths, release_id, &manifest).context("rollback release")
}

fn rollback_release(paths: &InstallPaths, release_id: &str, new_manifest: &serde_json::Value) -> Result<()> {
    let from = new_manifest["version"]
        .as_str()
        .unwrap_or("unknown")
        .to_owned();
    let old_manifest = std::fs::read_to_string(paths.manifest_previous_file()).unwrap_or_default();
    let to = j_parse(&old_manifest).unwrap_or_else(|| "unknown".to_owned());
    let bad_binary = paths.bad_agent_file();

    let _ = std::fs::remove_file(&bad_binary);
    std::fs::rename(&paths.agent_bin, &bad_binary).context("keep bad binary")?;
    std::fs::rename(paths.agent_previous_file(), &paths.agent_bin).context("restore sensor-agent")?;

    if paths.updater_previous_file().exists() {
        std::fs::remove_file(&paths.updater_bin).ok();
        std::fs::rename(paths.updater_previous_file(), &paths.updater_bin)
            .context("restore updater")?;
    }

    if paths.manifest_previous_file().exists() {
        std::fs::remove_file(&paths.manifest_file).ok();
        std::fs::rename(paths.manifest_previous_file(), &paths.manifest_file)
            .context("restore release manifest")?;
    } else {
        std::fs::remove_file(&paths.manifest_file).ok();
    }

    health::clear_healthy(paths)?;
    health::write_rollback(paths, &RollbackInfo {
        device_id: device_id_from_state(paths),
        event: "rollback".to_owned(),
        from,
        to,
        bad_binary_kept: bad_binary.display().to_string(),
    })?;
    cleanup(paths, release_id);
    restart_agent(&paths.service_name)
}

fn restart_agent(service_name: &str) -> Result<()> {
    std::process::Command::new("systemctl")
        .args(["restart", service_name])
        .status()
        .context("restart sensor-agent.service")?
        .success()
        .then_some(())
        .ok_or_else(|| anyhow::anyhow!("systemctl restart failed"))
}

fn device_id_from_state(paths: &InstallPaths) -> String {
    std::fs::read_to_string(paths.state_file())
        .ok()
        .and_then(|data| serde_json::from_str::<serde_json::Value>(&data).ok())
        .and_then(|state| state["device_id"].as_str().map(str::to_owned))
        .unwrap_or_else(|| "unknown".to_owned())
}

fn file_sha256(path: &Path) -> Result<String> {
    let data = std::fs::read(path)?;
    let mut hasher = Sha256::new();
    hasher.update(data);
    Ok(hex(&hasher.finalize()))
}

fn install_binary(source: &Path, target: &Path) -> Result<()> {
    let staged = target.with_extension("new");
    std::fs::copy(source, &staged)?;
    std::fs::set_permissions(&staged, mode(0o755))?;
    set_root_owner(&staged)?;
    sync_file(&staged)?;
    std::fs::rename(&staged, target)?;
    sync_parent(target)?;
    Ok(())
}

fn atomic_copy(source: &Path, target: &Path) -> Result<()> {
    let staged = target.with_extension("tmp");
    std::fs::copy(source, &staged)?;
    sync_file(&staged)?;
    std::fs::rename(&staged, target)?;
    sync_parent(target)?;
    Ok(())
}

fn set_root_owner(path: &Path) -> Result<()> {
    let path = path.to_str().context("non-UTF8 install path")?;
    std::process::Command::new("chown")
        .args(["root:root", path])
        .status()
        .context("set root owner")?
        .success()
        .then_some(())
        .ok_or_else(|| anyhow::anyhow!("chown failed for {path}"))
}

fn sync_file(path: &Path) -> Result<()> {
    std::fs::File::open(path)?.sync_all()?;
    Ok(())
}

fn sync_parent(path: &Path) -> Result<()> {
    let parent = path
        .parent()
        .context("missing parent directory")?;
    std::fs::File::open(parent)?.sync_all()?;
    Ok(())
}

fn cleanup(paths: &InstallPaths, release_id: &str) {
    for suffix in ["sensor-agent", "sensor-agent-updater", "json", "ready"] {
        let _ = std::fs::remove_file(release_path(paths, release_id, suffix));
    }
}

fn j_parse(json: &str) -> Option<String> {
    serde_json::from_str::<serde_json::Value>(json)
        .ok()?
        .get("version")?
        .as_str()
        .map(str::to_owned)
}

fn hex(digest: &[u8]) -> String {
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn mode(mode: u32) -> std::fs::Permissions {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::Permissions::from_mode(mode)
    }
    #[cfg(not(unix))]
    {
        std::fs::Permissions::readonly()
    }
}
