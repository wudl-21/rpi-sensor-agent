//! 版本模块：编译期写入，运行时可读

/// 发布版本号，由构建脚本注入，未使用构建脚本时回退到 Cargo 版本。
pub const VERSION: &str = match option_env!("BUILD_VERSION") {
    Some(value) => value,
    None => env!("CARGO_PKG_VERSION"),
};

/// 发布序号，由 scripts/build.sh 写入构建环境。
pub const RELEASE_SEQ: &str = match option_env!("RELEASE_SEQ") {
    Some(value) => value,
    None => "0",
};

pub fn release_seq() -> u64 {
    RELEASE_SEQ.parse().unwrap_or(0)
}

/// 构建时间（编译那一刻）
pub const BUILD_TIME: &str = match option_env!("BUILD_TIME") {
    Some(t) => t,
    None => "dev-build",
};

/// 设备端自报身份
pub fn version_info() -> serde_json::Value {
    serde_json::json!({
        "version": VERSION,
        "release_seq": release_seq(),
        "build_time": BUILD_TIME,
        "runtime": "tokio",
        "target": "aarch64-linux-gnu",
    })
}
