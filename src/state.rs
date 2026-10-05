//! 设备运行状态：重启后还能恢复“我是谁、我跑到哪了”
use serde::{Deserialize, Serialize};
use anyhow::{self, Context};

/// 状态文件路径（Linux 惯例：/var/lib/<app>）
const STATE_PATH: &str = "/var/lib/sensor-agent/state.json";

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
}

impl Default for State {
    fn default() -> Self {
        Self {
            device_id: format!("pi-{:x}", random_u32()),
            sample_seq: 0,
            last_sample_ts: 0,
            boot_count: 1,
        }
    }
}

/// 从磁盘加载；没有就新建
pub fn load() -> anyhow::Result<State> {
    match std::fs::read_to_string(STATE_PATH) {
        Ok(s) => serde_json::from_str::<State>(&s).context("parse state file"),
        Err(_) => {
            let s = State::default();
            save(&s).context("save initial state")?; // 第一次启动：落盘
            Ok(s)
        }
    }
}

/// 原子写：先写临时文件，再 rename
pub fn save(st: &State) -> anyhow::Result<()> {
    let tmp = format!("{}.tmp", STATE_PATH);
    let data = serde_json::to_string_pretty(st).context("serialize state")?;

    // 1. 写临时文件
    std::fs::write(&tmp, data).context("write state tmp")?;
    // 2. rename 是原子操作（文件系统级）
    std::fs::rename(&tmp, STATE_PATH).context("rename state file")?;
    Ok(())
}

/// 简单随机（不用 rand crate 也行）
fn random_u32() -> u32 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0)
}
