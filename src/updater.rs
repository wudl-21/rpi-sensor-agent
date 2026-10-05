//! 本地 drop-in 更新：发现 incoming/ 下的新二进制就换自己
use anyhow::{Context, Result};
use notify::Watcher;
use sha2::{Digest, Sha256};
use tracing::info;
use std::path::Path;

const INCOMING: &str = "/var/lib/sensor-agent/incoming/sensor-agent.new";
const SHA_PATH: &str = "/var/lib/sensor-agent/incoming/sensor-agent.new.sha256";
const CURRENT_BIN: &str = "/usr/local/bin/sensor-agent";

/// 阻塞监听：一旦出现新文件就处理
pub async fn watch_for_updates() -> Result<()> {
    let (tx, mut rx) = tokio::sync::mpsc::channel(8);

    let mut watcher = notify::recommended_watcher(move |res| {
        if let Ok(event) = res {
            let _ = tx.blocking_send(event);
        }
    })
    .context("create watcher")?;

    watcher
        .watch(Path::new("/var/lib/sensor-agent/incoming"), notify::RecursiveMode::NonRecursive)
        .context("watch incoming dir")?;

    while rx.recv().await.is_some() {
        if Path::new(INCOMING).exists() {
            info!("drop-in update detected, trying to apply");
            try_apply_update()?;
        }
    }
    Ok(())
}

/// 校验 + 替换 + 退出
fn try_apply_update() -> Result<()> {
    let incoming = "/var/lib/sensor-agent/incoming";

    let bin_path = format!("{incoming}/sensor-agent.new");
    let sha_path = format!("{incoming}/sensor-agent.new.sha256");
    let manifest = format!("{incoming}/release.json");

    if !Path::new(&bin_path).exists() {
        return Ok(());
    }

    // 1. 算文件 hash
    let data = std::fs::read(&bin_path)?;
    let mut h = Sha256::new();
    h.update(&data);
    let actual = hex(&h.finalize());

    // 2. 优先用 release.json 里的 sha256
    let expected = if Path::new(&manifest).exists() {
        let j: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&manifest)?)?;
        j["sha256"].as_str().unwrap_or("").to_string()
    } else if Path::new(&sha_path).exists() {
        std::fs::read_to_string(&sha_path)?.trim().to_string()
    } else {
        anyhow::bail!("no sha256 source");
    };

    if actual != expected {
        anyhow::bail!("sha256 mismatch: drop-in ignored");
    }

    // 3. 版本不降级（避免 CI 回滚到老的）
    if let Ok(j) = std::fs::read_to_string("/var/lib/sensor-agent/release.json")
    && let (Some(old), Some(new)) = (j_parse(&j), j_parse(&std::fs::read_to_string(&manifest)?))
    && new <= old {
        tracing::info!("drop-in version not newer, skipping");
        cleanup(incoming);
        return Ok(());
    }

    // 4. 备份 + 原子替换
    std::fs::copy(CURRENT_BIN, format!("{CURRENT_BIN}.prev")).ok();
    std::fs::rename(&bin_path, CURRENT_BIN)?;
    std::fs::set_permissions(CURRENT_BIN, mode(0o755)).ok();

    // 5. 把 release.json 存到状态目录
    std::fs::copy(&manifest, "/var/lib/sensor-agent/release.json").ok();

    cleanup(incoming);
    tracing::info!("drop-in update applied");
    std::process::exit(0);
}


fn hex(d: &[u8]) -> String {
    d.iter().map(|b| format!("{:02x}", b)).collect()
}

fn j_parse(json: &str) -> Option<String> {
    serde_json::from_str::<serde_json::Value>(json)
        .ok()?
        .get("version")?
        .as_str()
        .map(str::to_owned)
}

fn cleanup(incoming: &str) {
    let _ = std::fs::remove_file(format!("{incoming}/sensor-agent.new"));
    let _ = std::fs::remove_file(format!("{incoming}/sensor-agent.new.sha256"));
    let _ = std::fs::remove_file(format!("{incoming}/release.json"));
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
