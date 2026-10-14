//! 设备运行状态：重启后还能恢复“我是谁、我跑到哪了”
use anyhow::{self, Context};
use crate::paths::InstallPaths;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct State {
    /// 设备唯一 ID（首次启动生成）
    pub device_id: String,

    /// 采样序号（每采一次 +1）
    pub sample_seq: u64,

    /// 上次采样时间（UNIX 秒）
    pub last_sample_ts: i64,

    /// 启动次数（看设备跑了多久）
    pub boot_count: u64,

    /// 远程设置的采样间隔；缺省时使用 config.toml
    #[serde(default)]
    pub interval_secs: Option<u64>,

    /// 最近已处理的命令，用于抵御 MQTT QoS 1 重复投递
    #[serde(default)]
    pub last_command_id: Option<String>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            device_id: format!("pi-{:x}", random_u32()),
            sample_seq: 0,
            last_sample_ts: 0,
            boot_count: 1,
            interval_secs: None,
            last_command_id: None,
        }
    }
}

/// 从磁盘加载；没有就新建
pub fn load(paths: &InstallPaths) -> anyhow::Result<State> {
    let state_file = paths.state_file();
    match std::fs::read_to_string(&state_file) {
        Ok(s) => serde_json::from_str::<State>(&s).context("parse state file"),
        Err(_) => {
            let s = State::default();
            save(paths, &s).context("save initial state")?; // 第一次启动：落盘
            Ok(s)
        }
    }
}

/// 原子写：先写临时文件，再 rename
pub fn save(paths: &InstallPaths, st: &State) -> anyhow::Result<()> {
    let state_file = paths.state_file();
    let tmp = format!("{}.tmp", state_file.display());
    let data = serde_json::to_string_pretty(st).context("serialize state")?;

    // 1. 写临时文件
    std::fs::write(&tmp, data).context("write state tmp")?;
    // 2. rename 是原子操作（文件系统级）
    std::fs::rename(&tmp, state_file).context("rename state file")?;
    Ok(())
}

/// 简单随机（不用 rand crate 也行）
fn random_u32() -> u32 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0)
}
