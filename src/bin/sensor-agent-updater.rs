#[path = "../updater.rs"]
mod updater;
#[path = "../health.rs"]
mod health;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().with_target(false).init();
    updater::watch_for_updates().await
}