//! 主动拉取式 OTA：Pi 出网，不依赖 GitHub / 不被 SSH 进来
use anyhow::{Context, Result};
use serde::Deserialize;
use sha2::{Digest, Sha256};

const OTA_BASE: &str = "https://ota.cn.example.com/releases";
const CURRENT_BIN: &str = "/usr/local/bin/sensor-agent";
const NEXT_BIN: &str = "/usr/local/bin/sensor-agent.next";

#[derive(Debug, Deserialize)]
struct Release {
    version: String,
    url: String,
    sha256: String,
    #[serde(default)]
    min_version: Option<String>,
}

fn is_newer(remote: &str, local: &str) -> bool {
    let parse = |s: &str| s.split('.').map(|x| x.parse::<u32>().unwrap_or(0)).collect::<Vec<_>>();
    parse(remote) > parse(local)
}

pub async fn check_and_apply(local_version: &str) -> Result<bool> {
    // 1. 拉 manifest（超时别卡死）
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()?;

    let latest: Release = client
        .get(format!("{OTA_BASE}/latest.json"))
        .send()
        .await
        .context("ota: fetch latest.json")?
        .bytes()
        .await
        .context("ota: read latest.json")
        .and_then(|bytes| serde_json::from_slice(&bytes).context("ota: parse latest.json"))?;

    if !is_newer(&latest.version, local_version) {
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
    std::fs::write(NEXT_BIN, &bytes).context("ota: write next binary")?;
    std::fs::set_permissions(NEXT_BIN, std::os::unix::fs::PermissionsExt::from_mode(0o755))
        .context("ota: chmod next")?;

    // 5. 原子替换
    std::fs::rename(NEXT_BIN, CURRENT_BIN).context("ota: replace binary")?;

    tracing::info!(version = %latest.version, "ota: binary replaced, exiting for systemd");
    Ok(true)
}

fn hex(d: &[u8]) -> String {
    d.iter().map(|b| format!("{:02x}", b)).collect()
}
