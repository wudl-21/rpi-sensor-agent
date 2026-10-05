//! OTA 检查 + 下载 + 校验 + 替换
use anyhow::{Context, Result};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::path::Path;

const CURRENT_BIN: &str = "/usr/local/bin/sensor-agent";
const NEXT_BIN: &str = "/usr/local/bin/sensor-agent.next";

#[derive(Debug, Deserialize)]
struct Release {
    version: String,
    url: String,
    sha256: String,
}

/// 版本号比较："0.4.0" > "0.3.1"
fn is_newer(remote: &str, local: &str) -> bool {
    let parse = |s: &str| {
        s.split('.')
            .map(|x| x.parse::<u32>().unwrap_or(0))
            .collect::<Vec<_>>()
    };
    parse(remote) > parse(local)
}

/// 主入口：该不该更新？
pub async fn maybe_update(current_version: &str) -> Result<bool> {
    // 1. 查最新版本
    let resp = reqwest::get("https://ota.example.com/latest.json")
        .await
        .context("ota: fetch latest.json")?;

    let body = resp
        .bytes()
        .await
        .context("ota: read latest.json body")?;
    let release: Release = serde_json::from_slice(&body).context("ota: parse latest.json")?;

    if !is_newer(&release.version, current_version) {
        return Ok(false); // 已经是最新
    }

    tracing::info!(new_version = %release.version, "ota: newer version found");

    // 2. 下载二进制
    let bytes = reqwest::get(&release.url)
        .await
        .context("ota: download binary")?
        .bytes()
        .await
        .context("ota: read binary body")?;

    // 3. sha256 校验
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    let actual = hex_encode(&hasher.finalize());
    if actual != release.sha256 {
        anyhow::bail!("ota: sha256 mismatch (possible tampered firmware)");
    }

    // 4. 写到 .next（不碰正在跑的自己）
    std::fs::write(NEXT_BIN, &bytes).context("ota: write next binary")?;
    std::fs::set_permissions(NEXT_BIN, std::os::unix::fs::PermissionsExt::from_mode(0o755))
        .context("ota: chmod next binary")?;

    // 5. 原子替换
    std::fs::rename(NEXT_BIN, CURRENT_BIN).context("ota: atomically replace binary")?;

    tracing::info!(version = %release.version, "ota: binary replaced, exiting for systemd");
    Ok(true)
}

fn hex_encode(data: &[u8]) -> String {
    data.iter()
        .map(|b| format!("{:02x}", b))
        .collect()
}
