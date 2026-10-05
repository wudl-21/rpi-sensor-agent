//! 版本模块：编译期写入，运行时可读

/// 语义版本号（和 Cargo.toml 保持一致）
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// 构建时间（编译那一刻）
pub const BUILD_TIME: &str = match option_env!("BUILD_TIME") {
    Some(t) => t,
    None => "dev-build",
};

/// 设备端自报身份
pub fn version_info() -> serde_json::Value {
    serde_json::json!({
        "version": VERSION,
        "build_time": BUILD_TIME,
        "runtime": "tokio",
        "target": "aarch64-linux-gnu",
    })
}
