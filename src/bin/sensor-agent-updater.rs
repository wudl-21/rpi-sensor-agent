#[path = "../updater.rs"]
mod updater;
#[path = "../health.rs"]
mod health;
#[path = "../paths.rs"]
mod paths;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().with_target(false).init();
    let paths = paths::InstallPaths::load()?;
    updater::watch_for_updates(&paths).await
}