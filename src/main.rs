mod agent;
mod config;
mod mqtt;
mod state;
mod store;
mod version;
mod watchdog;

use tracing::error;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().with_target(false).init();

    let cfg = match config::Config::load() {
        Ok(c) => c,
        Err(e) => {
            error!(error = %e, "config load failed");
            return; // systemd 会看到非 0 / 或 restart
        }
    };

    if let Err(e) = agent::run(cfg).await {
        error!(error = %e, "agent exited with error");
    }
}
