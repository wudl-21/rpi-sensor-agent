use std::time::Duration;
use tracing::{info, warn};

pub fn start() {
    let watchdog_usec = std::env::var("WATCHDOG_USEC")
        .ok()
        .and_then(|value| value.parse::<u64>().ok());

    let Some(watchdog_usec) = watchdog_usec else {
        warn!("systemd watchdog is not enabled");
        return;
    };

    let _ = sd_notify::notify(&[sd_notify::NotifyState::Ready]);

    // 按 watchdog 超时时间的一半发送，留出调度延迟余量
    let period_usec = (watchdog_usec / 2).max(1_000_000);
    let period = Duration::from_micros(period_usec);

    info!(?period, "systemd watchdog enabled");

    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(period);

        loop {
            ticker.tick().await;

            if let Err(error) =
                sd_notify::notify(&[sd_notify::NotifyState::Watchdog])
            {
                warn!(%error, "failed to notify systemd watchdog");
            }
        }
    });
}