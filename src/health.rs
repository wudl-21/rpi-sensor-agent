use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

pub const HEALTHY_PATH: &str = "/var/lib/sensor-agent/healthy";
pub const ROLLBACK_PATH: &str = "/var/lib/sensor-agent/rollback.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RollbackInfo {
    pub device_id: String,
    pub event: String,
    pub from: String,
    pub to: String,
    pub bad_binary_kept: String,
}

pub fn is_healthy() -> bool {
    Path::new(HEALTHY_PATH).exists()
}

pub fn mark_healthy() -> Result<()> {
    let tmp = format!("{HEALTHY_PATH}.tmp");
    std::fs::write(&tmp, b"healthy\n").context("write healthy marker")?;
    std::fs::rename(&tmp, HEALTHY_PATH).context("publish healthy marker")?;
    Ok(())
}

pub fn clear_healthy() -> Result<()> {
    match std::fs::remove_file(HEALTHY_PATH) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error).context("remove healthy marker"),
    }
}

pub fn read_rollback() -> Result<Option<RollbackInfo>> {
    if !Path::new(ROLLBACK_PATH).exists() {
        return Ok(None);
    }

    let data = std::fs::read_to_string(ROLLBACK_PATH).context("read rollback marker")?;
    Ok(Some(serde_json::from_str(&data).context("parse rollback marker")?))
}

pub fn write_rollback(info: &RollbackInfo) -> Result<()> {
    let tmp = format!("{ROLLBACK_PATH}.tmp");
    let data = serde_json::to_vec_pretty(info).context("serialize rollback marker")?;
    std::fs::write(&tmp, data).context("write rollback marker")?;
    std::fs::rename(&tmp, ROLLBACK_PATH).context("publish rollback marker")?;
    Ok(())
}

pub fn clear_rollback() -> Result<()> {
    match std::fs::remove_file(ROLLBACK_PATH) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error).context("remove rollback marker"),
    }
}
