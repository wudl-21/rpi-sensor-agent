//! 主循环：采样 / 发 MQTT / 重传 / 优雅退出
use crate::{
    command::{Command, CommandEnvelope},
    config::Config,
    health,
    mqtt::Mqtt,
    state, store,
};
use anyhow::Result;
use crate::paths::InstallPaths;
use tokio::signal;
use tokio::time::{Duration, interval};
use tracing::{info, warn};

pub const RESTART_EXIT_CODE: i32 = 75;

pub enum ExitAction {
    Stop,
    Restart,
}

pub async fn run(cfg: Config, paths: InstallPaths) -> Result<ExitAction> {
    // ---- 状态 ----
    let mut st = state::load(&paths).unwrap_or_default();
    st.boot_count += 1;
    state::save(&paths, &st)?;
    info!(boot_count = st.boot_count, device_id = %st.device_id, "agent booted!");

    // ---- 本地库 ----
    let conn = store::open(&paths)?;

    // ---- MQTT（broker 地址从配置来）----
    let (mqtt, mut command_rx) = Mqtt::connect(
        &st.device_id,
        &cfg.mqtt_host,
        cfg.mqtt_port,
        cfg.mqtt_username.as_deref(),
        cfg.mqtt_password.as_deref(),
        cfg.mqtt_tls,
        cfg.mqtt_ca_file.as_deref(),
    )
    .await?;
    mqtt.publish_online().await?;
    let mut pending_rollback = health::read_rollback(&paths)?;

    // ---- 定时器 ----
    let interval_secs = st.interval_secs.unwrap_or(cfg.interval_secs).max(1);
    let mut sample_tick = interval(Duration::from_secs(interval_secs));
    let mut resend_tick = interval(Duration::from_secs(15)); // 每 15s 补发一次

    // ---- Ctrl+C / SIGTERM ----
    let ctrl_c = signal::ctrl_c();
    tokio::pin!(ctrl_c);

    let exit_action = loop {
        tokio::select! {
            // 定时采样
            _ = sample_tick.tick() => {
                let temp = 23.5;
                let hum: Option<f64> = Some(55.0);

                match store::insert(&conn, &st.device_id, temp, hum) {
                    Ok(id) => {
                        st.sample_seq += 1;
                        if let Err(error) = state::save(&paths, &st) {
                            warn!(%error, "save state after sample failed");
                        } else if !health::is_healthy(&paths) {
                            health::mark_healthy(&paths)?;
                            info!("agent is healthy");
                        }

                        if let Some(rollback) = pending_rollback.as_ref() {
                            match mqtt.publish_rollback(rollback).await {
                                Ok(()) => {
                                    health::clear_rollback(&paths)?;
                                    pending_rollback = None;
                                    info!("rollback event published");
                                }
                                Err(error) => warn!(%error, "rollback event publish failed"),
                            }
                        }

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

            // MQTT 命令
            Some(payload) = command_rx.recv() => {
                if let Some(action) = handle_command(
                    &payload,
                    &paths,
                    &cfg.ota_base_url,
                    &mut st,
                    &mut sample_tick,
                    &mqtt,
                    &conn,
                )
                .await
                {
                    break action;
                }
            }

            // 关机
            _ = &mut ctrl_c => {
                warn!("shutdown signal received");
                mqtt.publish_offline().await.ok(); // 失败不管
                state::save(&paths, &st).ok(); // 失败不管
                info!("agent stopped cleanly");
                break ExitAction::Stop;
            }
        }
    };

    Ok(exit_action)
}

async fn handle_command(
    payload: &[u8],
    paths: &InstallPaths,
    ota_base_url: &str,
    st: &mut state::State,
    sample_tick: &mut tokio::time::Interval,
    mqtt: &Mqtt,
    conn: &rusqlite::Connection,
) -> Option<ExitAction> {
    let envelope: CommandEnvelope = match serde_json::from_slice(payload) {
        Ok(command) => command,
        Err(error) => {
            warn!(%error, "invalid MQTT command");
            return None;
        }
    };

    let command_id = envelope.command_id.clone();
    let op = envelope.command.op_name();
    if st.last_command_id.as_deref() == Some(command_id.as_str()) {
        publish_ack(
            mqtt,
            &command_id,
            op,
            "succeeded",
            Some("already_processed"),
        )
        .await;
        return None;
    }

    if let Command::SetInterval { value } = &envelope.command
        && !(1..=3600).contains(value)
    {
        publish_ack(
            mqtt,
            &command_id,
            op,
            "rejected",
            Some("interval_out_of_range"),
        )
        .await;
        return None;
    }

    st.last_command_id = Some(command_id.clone());
    if let Err(error) = state::save(paths, st) {
        warn!(%error, %command_id, "save command state failed");
        publish_ack(mqtt, &command_id, op, "failed", Some("state_save_failed")).await;
        return None;
    }

    match envelope.command {
        Command::SetInterval { value } => {
            st.interval_secs = Some(value);
            if let Err(error) = state::save(paths, st) {
                warn!(%error, "save interval state failed");
                publish_ack(mqtt, &command_id, op, "failed", Some("state_save_failed")).await;
                return None;
            }
            *sample_tick = interval(Duration::from_secs(value));
            info!(interval = value, "interval changed via command");
            publish_ack(mqtt, &command_id, op, "succeeded", None).await;
        }

        Command::PublishNow => {
            let temp = 23.5;
            let hum = Some(55.0);
            match store::insert(conn, &st.device_id, temp, hum) {
                Ok(id) => {
                    st.sample_seq += 1;
                    if let Err(error) = state::save(paths, st) {
                        warn!(%error, "save state after manual sample failed");
                    }
                    match mqtt.publish_telemetry(st.sample_seq, temp, hum).await {
                        Ok(()) => {
                            let _ = store::mark_sent(conn, id);
                            publish_ack(mqtt, &command_id, op, "succeeded", None).await;
                        }
                        Err(error) => {
                            warn!(%error, "manual sample publish failed");
                            publish_ack(mqtt, &command_id, op, "failed", Some("publish_failed"))
                                .await;
                        }
                    }
                }
                Err(error) => {
                    warn!(%error, "manual sample store failed");
                    publish_ack(mqtt, &command_id, op, "failed", Some("store_failed")).await;
                }
            }
        }

        Command::TriggerOta => {
            match crate::ota::check_and_apply(
                paths,
                ota_base_url,
                crate::version::release_seq(),
            )
            .await
            {
                Ok(true) => {
                    publish_ack(mqtt, &command_id, op, "accepted", None).await;
                    tokio::time::sleep(Duration::from_millis(300)).await;
                    return Some(ExitAction::Restart);
                }
                Ok(false) => {
                    publish_ack(mqtt, &command_id, op, "succeeded", Some("already_latest")).await;
                }
                Err(error) => {
                    warn!(%error, "command OTA failed");
                    publish_ack(mqtt, &command_id, op, "failed", Some("ota_failed")).await;
                }
            }
        }

        Command::Reboot => {
            publish_ack(mqtt, &command_id, op, "accepted", None).await;
            tokio::time::sleep(Duration::from_millis(300)).await;
            return Some(ExitAction::Restart);
        }

        Command::Stop => {
            publish_ack(mqtt, &command_id, op, "accepted", None).await;
            tokio::time::sleep(Duration::from_millis(300)).await;
            return Some(ExitAction::Stop);
        }
    }

    None
}

async fn publish_ack(mqtt: &Mqtt, command_id: &str, op: &str, status: &str, error: Option<&str>) {
    if let Err(publish_error) = mqtt.publish_ack(command_id, op, status, error).await {
        warn!(error = %publish_error, %command_id, "publish command ack failed");
    }
}
