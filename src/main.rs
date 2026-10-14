mod agent;
mod command;
mod config;
mod health;
mod mqtt;
mod ota;
mod paths;
mod state;
mod store;
mod version;

use tracing::error;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().with_target(false).init();

    let paths = match paths::InstallPaths::load() {
        Ok(paths) => paths,
        Err(error) => {
            error!(%error, "install layout load failed");
            std::process::exit(1);
        }
    };

    let cfg = match config::Config::load(&paths) {
        Ok(c) => c,
        Err(e) => {
            error!(error = %e, "config load failed");
            std::process::exit(1);
        }
    };

    match agent::run(cfg, paths).await {
        Ok(agent::ExitAction::Stop) => {}
        Ok(agent::ExitAction::Restart) => std::process::exit(agent::RESTART_EXIT_CODE),
        Err(error) => {
            error!(%error, "agent exited with error");
            std::process::exit(1);
        }
    }
}
