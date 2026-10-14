use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use crate::paths::InstallPaths;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RollbackInfo {
    pub device_id: String,
    pub event: String,
    pub from: String,
    pub to: String,
    pub bad_binary_kept: String,
}

pub fn is_healthy(paths: &InstallPaths) -> bool {
    paths.healthy_file().exists()
}

pub fn mark_healthy(paths: &InstallPaths) -> Result<()> {
    let healthy = paths.healthy_file();
    let tmp = format!("{}.tmp", healthy.display());
    std::fs::write(&tmp, b"healthy\n").context("write healthy marker")?;
    std::fs::rename(&tmp, healthy).context("publish healthy marker")?;
    Ok(())
}

pub fn clear_healthy(paths: &InstallPaths) -> Result<()> {
    match std::fs::remove_file(paths.healthy_file()) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error).context("remove healthy marker"),
    }
}

pub fn read_rollback(paths: &InstallPaths) -> Result<Option<RollbackInfo>> {
    let rollback = paths.rollback_file();
    if !rollback.exists() {
        return Ok(None);
    }

    let data = std::fs::read_to_string(rollback).context("read rollback marker")?;
    Ok(Some(
        serde_json::from_str(&data).context("parse rollback marker")?,
    ))
}

pub fn write_rollback(paths: &InstallPaths, info: &RollbackInfo) -> Result<()> {
    let rollback = paths.rollback_file();
    let tmp = format!("{}.tmp", rollback.display());
    let data = serde_json::to_vec_pretty(info).context("serialize rollback marker")?;
    std::fs::write(&tmp, data).context("write rollback marker")?;
    std::fs::rename(&tmp, rollback).context("publish rollback marker")?;
    Ok(())
}

pub fn clear_rollback(paths: &InstallPaths) -> Result<()> {
    match std::fs::remove_file(paths.rollback_file()) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error).context("remove rollback marker"),
    }
}
