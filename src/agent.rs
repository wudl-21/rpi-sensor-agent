//! 主循环：采样 / 发 MQTT / 重传 / 优雅退出
use crate::{
    config::Config, 
    mqtt::Mqtt, 
    state, 
    store,
    //ota, 
    version,
    updater,
    watchdog,
};
use anyhow::Result;
use tokio::signal;
use tokio::time::{interval, Duration};
use tracing::{info, warn};

pub async fn run(cfg: Config) -> Result<()> {
    // ---- 状态 ----
    let mut st = state::load().unwrap_or_default();
    st.boot_count += 1;
    state::save(&st)?;
    info!(boot_count = st.boot_count, device_id = %st.device_id, version = %&version::version_info(), "agent booted!");

    // ---- 本地库 ----
    let conn = store::open()?;

    // ---- MQTT（broker 地址从配置来）----
    let mqtt = Mqtt::connect(&st.device_id, &cfg.mqtt_host, cfg.mqtt_port).await?;
    watchdog::start();  // systemd watchdog
    mqtt.publish_online().await?;

    // ---- 定时器 ----
    let mut sample_tick = interval(Duration::from_secs(cfg.interval_secs));
    let mut resend_tick = interval(Duration::from_secs(15)); // 每 15s 补发一次
    //let mut ota_tick = interval(Duration::from_secs(5 * 60)); // 5 分钟查一次

    // ---- Ctrl+C / SIGTERM ----
    let ctrl_c = signal::ctrl_c();
    tokio::pin!(ctrl_c);

    // ---- Drop-in 更新监听 ----
    tokio::spawn(async {
        if let Err(e) = updater::watch_for_updates().await {
            tracing::warn!(error = %e, "updater watcher exited");
        }
    });

    loop {
        tokio::select! {
            // 定时采样
            _ = sample_tick.tick() => {
                let temp = 23.5;
                let hum: Option<f64> = Some(55.0);

                match store::insert(&conn, &st.device_id, temp, hum) {
                    Ok(id) => {
                        st.sample_seq += 1;
                        state::save(&st).ok(); // 失败不管

                        if let Err(e) = mqtt.publish_telemetry(st.sample_seq, temp, hum).await {
                            warn!(error = %e, row_id = id, "telemetry not sent, will resend later");
                        } else {
                            let _ = store::mark_sent(&conn, id);
                        }
                    }
                    Err(e) => warn!(error = %e, "store insert failed"),
                }
            }

            // 定时补发未确认消息
            _ = resend_tick.tick() => {
                match store::unsent(&conn) {
                    Ok(rows) => {
                        for (id, temp, hum) in rows {
                            if mqtt.publish_telemetry(0, temp, hum).await.is_ok() {
                                let _ = store::mark_sent(&conn, id);
                            }
                        }
                    }
                    Err(e) => warn!(error = %e, "load unsent failed"),
                }
            }

            /* 
            _ = ota_tick.tick() => {
                match ota::maybe_update(version::VERSION).await {
                    Ok(true) => {
                        // 二进制已替换，自己退，systemd 重启新版本
                        mqtt.publish_event("updating").await.ok();
                        tracing::info!("ota: restarting into new version");
                        std::process::exit(0);
                    }
                    Ok(false) => { /* 没有更新 */ }
                    Err(e) => warn!(error = %e, "ota check failed"),
                }
            }
            */

            // 关机
            _ = &mut ctrl_c => {
                warn!("shutdown signal received");
                mqtt.publish_offline().await.ok(); // 失败不管
                state::save(&st).ok(); // 失败不管
                info!("agent stopped cleanly");
                break;
            }
        }
    }

    Ok(())
}
