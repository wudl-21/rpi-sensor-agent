//! 主动拉取式 OTA：Pi 出网，不依赖 GitHub / 不被 SSH 进来
use anyhow::{Context, Result};
use crate::paths::InstallPaths;
use serde::Deserialize;
use sha2::{Digest, Sha256};

#[derive(Debug, Deserialize)]
struct Release {
    version: String,
    release_seq: u64,
    url: String,
    sha256: String,
    #[serde(default)]
    min_version: Option<String>,
}

fn is_newer(remote_seq: u64, local_seq: u64) -> bool {
    remote_seq > local_seq
}

pub async fn check_and_apply(
    paths: &InstallPaths,
    ota_base_url: &str,
    local_release_seq: u64,
) -> Result<bool> {
    // 1. 拉 manifest（超时别卡死）
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()?;

    let latest: Release = client
        .get(format!("{ota_base_url}/latest.json"))
        .send()
        .await
        .context("ota: fetch latest.json")?
        .bytes()
        .await
        .context("ota: read latest.json")
        .and_then(|bytes| serde_json::from_slice(&bytes).context("ota: parse latest.json"))?;

    if !is_newer(latest.release_seq, local_release_seq) {
        return Ok(false);
    }

    tracing::info!(new_version = %latest.version, "ota: newer release found");

    // 2. 下载二进制
    let bytes = client
        .get(&latest.url)
        .send()
        .await
        .context("ota: download binary")?
        .bytes()
        .await
        .context("ota: read binary")?;

    // 3. sha256 校验
    let mut h = Sha256::new();
    h.update(&bytes);
    let actual = hex(&h.finalize());

    if actual != latest.sha256 {
        anyhow::bail!("ota: sha256 mismatch, firmware rejected");
    }

    // 4. 写 .next（不碰正在跑的自己）
    let next_bin = paths.agent_bin.with_extension("next");
    std::fs::write(&next_bin, &bytes).context("ota: write next binary")?;
    std::fs::set_permissions(
        &next_bin,
        std::os::unix::fs::PermissionsExt::from_mode(0o755),
    )
    .context("ota: chmod next")?;

    // 5. 原子替换
    std::fs::rename(&next_bin, &paths.agent_bin).context("ota: replace binary")?;

    tracing::info!(version = %latest.version, "ota: binary replaced, exiting for systemd");
    Ok(true)
}

fn hex(d: &[u8]) -> String {
    d.iter().map(|b| format!("{:02x}", b)).collect()
}
