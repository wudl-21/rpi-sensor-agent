use anyhow::Context;
use serde::Deserialize;
use crate::paths::InstallPaths;

#[derive(Debug, Deserialize, Default)]
pub struct Config {
    #[serde(default = "Config::default_interval_secs")]
    pub interval_secs: u64,
    #[serde(default = "Config::default_mqtt_host")]
    pub mqtt_host: String,
    #[serde(default = "Config::default_mqtt_port")]
    pub mqtt_port: u16,
    #[serde(default)]
    pub mqtt_username: Option<String>,
    #[serde(default)]
    pub mqtt_password: Option<String>,
    #[serde(default)]
    pub mqtt_tls: bool,
    #[serde(default)]
    pub mqtt_ca_file: Option<String>,
    #[serde(default = "Config::default_ota_base_url")]
    pub ota_base_url: String,
}

impl Config {
    pub fn load(paths: &InstallPaths) -> anyhow::Result<Self> {
        let s = std::fs::read_to_string(&paths.config_file).context("read config file")?;
        let cfg = toml::from_str::<Config>(&s).context("parse config file")?;
        Ok(cfg)
    }

    fn default_interval_secs() -> u64 {
        10
    }

    fn default_mqtt_host() -> String {
        "localhost".into()
    }

    fn default_mqtt_port() -> u16 {
        1883
    }

    fn default_ota_base_url() -> String {
        "https://ota.cn.example.com/releases".to_owned()
    }
}
