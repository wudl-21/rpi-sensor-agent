use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const DEFAULT_LAYOUT_FILE: &str = "/etc/sensor-agent/install.toml";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallPaths {
    pub config_file: PathBuf,
    pub state_dir: PathBuf,
    pub incoming_dir: PathBuf,
    pub manifest_file: PathBuf,
    pub agent_bin: PathBuf,
    pub updater_bin: PathBuf,
    #[serde(default = "default_service_name")]
    pub service_name: String,
}

impl Default for InstallPaths {
    fn default() -> Self {
        Self {
            config_file: "/etc/sensor-agent/config.toml".into(),
            state_dir: "/var/lib/sensor-agent".into(),
            incoming_dir: "/var/lib/sensor-agent/incoming".into(),
            manifest_file: "/var/lib/sensor-agent/release.json".into(),
            agent_bin: "/usr/local/bin/sensor-agent".into(),
            updater_bin: "/usr/local/bin/sensor-agent-updater".into(),
            service_name: default_service_name(),
        }
    }
}

impl InstallPaths {
    pub fn load() -> Result<Self> {
        let layout_file = std::env::var_os("SENSOR_AGENT_LAYOUT")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(DEFAULT_LAYOUT_FILE));
        if !layout_file.exists() {
            return Ok(Self::default());
        }

        let data = std::fs::read_to_string(&layout_file)
            .with_context(|| format!("read install layout {}", layout_file.display()))?;
        toml::from_str(&data)
            .with_context(|| format!("parse install layout {}", layout_file.display()))
    }

    pub fn state_file(&self) -> PathBuf {
        self.state_dir.join("state.json")
    }

    pub fn database_file(&self) -> PathBuf {
        self.state_dir.join("samples.db")
    }

    pub fn healthy_file(&self) -> PathBuf {
        self.state_dir.join("healthy")
    }

    pub fn rollback_file(&self) -> PathBuf {
        self.state_dir.join("rollback.json")
    }

    pub fn manifest_previous_file(&self) -> PathBuf {
        with_suffix(&self.manifest_file, ".prev")
    }

    pub fn agent_previous_file(&self) -> PathBuf {
        with_suffix(&self.agent_bin, ".prev")
    }

    pub fn updater_previous_file(&self) -> PathBuf {
        with_suffix(&self.updater_bin, ".prev")
    }

    pub fn bad_agent_file(&self) -> PathBuf {
        with_suffix(&self.agent_bin, ".bad")
    }
}

fn default_service_name() -> String {
    "sensor-agent.service".to_owned()
}

fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut value = path.as_os_str().to_owned();
    value.push(suffix);
    PathBuf::from(value)
}