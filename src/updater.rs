//! 单向 scp 发布更新：按 release ID 校验、安装并重启服务。
use anyhow::{Context, Result};
use notify::Watcher;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use tokio::time::{interval, Duration};
use tracing::{info, warn};

const INCOMING_DIR: &str = "/var/lib/sensor-agent/incoming";
const CURRENT_BIN: &str = "/usr/local/bin/sensor-agent";
const CURRENT_UPDATER: &str = "/usr/local/bin/sensor-agent-updater";

/// 监听、轮询并恢复 incoming/ 中的已提交发布。
pub async fn watch_for_updates() -> Result<()> {
    let (tx, mut rx) = tokio::sync::mpsc::channel(8);
    let mut watcher = notify::recommended_watcher(move |res| {
        if let Ok(event) = res {
            let _ = tx.blocking_send(event);
        }
    })
    .context("create watcher")?;

    watcher
        .watch(Path::new(INCOMING_DIR), notify::RecursiveMode::NonRecursive)
        .context("watch incoming dir")?;

    try_ready_releases();
    let mut retry = interval(Duration::from_secs(5));

    loop {
        tokio::select! {
            Some(_) = rx.recv() => try_ready_releases(),
            _ = retry.tick() => try_ready_releases(),
        }
    }
}

fn try_ready_releases() {
    let release_ids = match ready_release_ids() {
        Ok(ids) => ids,
        Err(error) => {
            warn!(%error, "failed to scan incoming releases");
            return;
        }
    };

    for release_id in release_ids {
        info!(%release_id, "ready release detected, trying to apply");
        if let Err(error) = try_apply_update(&release_id) {
            warn!(%error, release_id, "release is incomplete or invalid, will retry");
        }
    }
}

fn ready_release_ids() -> Result<Vec<String>> {
    let mut release_ids = Vec::new();
    for entry in std::fs::read_dir(INCOMING_DIR)? {
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

fn release_path(release_id: &str, suffix: &str) -> PathBuf {
    Path::new(INCOMING_DIR).join(format!("release.{release_id}.{suffix}"))
}

/// 校验、安装并重启服务。
fn try_apply_update(release_id: &str) -> Result<()> {
    let ready_path = release_path(release_id, "ready");
    let service_path = release_path(release_id, "sensor-agent");
    let updater_path = release_path(release_id, "sensor-agent-updater");
    let manifest_path = release_path(release_id, "json");

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

    if let Ok(old_manifest) = std::fs::read_to_string("/var/lib/sensor-agent/release.json")
        && let (Some(old), Some(new)) = (j_parse(&old_manifest), manifest["version"].as_str())
        && new <= old.as_str()
    {
        info!(%release_id, "release version {} is not newer than {}, skipping", new, old.as_str());
        cleanup(release_id);
        return Ok(());
    }

    std::fs::copy(CURRENT_BIN, format!("{CURRENT_BIN}.prev")).ok();
    install_binary(&service_path, CURRENT_BIN).context("install sensor-agent")?;
    install_binary(&updater_path, CURRENT_UPDATER).context("install updater")?;
    atomic_copy(&manifest_path, "/var/lib/sensor-agent/release.json")?;

    std::process::Command::new("systemctl")
        .args(["restart", "sensor-agent.service"])
        .status()
        .context("restart sensor-agent.service")?
        .success()
        .then_some(())
        .ok_or_else(|| anyhow::anyhow!("systemctl restart failed"))?;

    cleanup(release_id);
    info!(%release_id, "release applied");
    Ok(())
}

fn file_sha256(path: &Path) -> Result<String> {
    let data = std::fs::read(path)?;
    let mut hasher = Sha256::new();
    hasher.update(data);
    Ok(hex(&hasher.finalize()))
}

fn install_binary(source: &Path, target: &str) -> Result<()> {
    let staged = format!("{target}.new");
    std::fs::copy(source, &staged)?;
    std::fs::set_permissions(&staged, mode(0o755))?;
    set_root_owner(&staged)?;
    sync_file(&staged)?;
    std::fs::rename(&staged, target)?;
    sync_parent(target)?;
    Ok(())
}

fn atomic_copy(source: &Path, target: &str) -> Result<()> {
    let staged = format!("{target}.tmp");
    std::fs::copy(source, &staged)?;
    sync_file(&staged)?;
    std::fs::rename(&staged, target)?;
    sync_parent(target)?;
    Ok(())
}

fn set_root_owner(path: &str) -> Result<()> {
    std::process::Command::new("chown")
        .args(["root:root", path])
        .status()
        .context("set root owner")?
        .success()
        .then_some(())
        .ok_or_else(|| anyhow::anyhow!("chown failed for {path}"))
}

fn sync_file(path: &str) -> Result<()> {
    std::fs::File::open(path)?.sync_all()?;
    Ok(())
}

fn sync_parent(path: &str) -> Result<()> {
    let parent = Path::new(path)
        .parent()
        .context("missing parent directory")?;
    std::fs::File::open(parent)?.sync_all()?;
    Ok(())
}

fn cleanup(release_id: &str) {
    for suffix in ["sensor-agent", "sensor-agent-updater", "json", "ready"] {
        let _ = std::fs::remove_file(release_path(release_id, suffix));
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
