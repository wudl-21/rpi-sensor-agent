//! MQTT 客户端：长连接 / 上线 / 采样 / 离线
use crate::{command::Topic, health::RollbackInfo, version};
use anyhow::{Context, Result};
use rumqttc::{AsyncClient, Event, Incoming, MqttOptions, QoS, Transport};
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::{info, warn};

pub struct Mqtt {
    client: AsyncClient,
    device_id: String,
}

impl Mqtt {
    /// 连接 broker；eventloop 在后台跑
    pub async fn connect(
        device_id: &str,
        host: &str,
        port: u16,
        username: Option<&str>,
        password: Option<&str>,
        tls: bool,
        ca_file: Option<&str>,
    ) -> Result<(Self, mpsc::Receiver<Vec<u8>>)> {
        let mut mopts = MqttOptions::new(device_id, host, port);
        mopts.set_keep_alive(Duration::from_secs(30));
        mopts.set_clean_session(false);
        mopts.set_inflight(16);

        match (username, password) {
            (Some(username), Some(password)) => {
                mopts.set_credentials(username, password);
            }
            (None, None) => {}
            _ => anyhow::bail!("mqtt_username and mqtt_password must be configured together"),
        }

        if tls {
            let ca_file = ca_file.context("mqtt_ca_file is required when mqtt_tls is true")?;
            let ca = std::fs::read(ca_file)
                .with_context(|| format!("read MQTT CA certificate: {ca_file}"))?;
            mopts.set_transport(Transport::tls(ca, None, None));
        }

        let (client, mut eventloop) = AsyncClient::new(mopts, 16);
        let (command_tx, command_rx) = mpsc::channel(16);
        let command_topic = Topic::Commands.format(device_id);
        client
            .subscribe(command_topic.clone(), QoS::AtLeastOnce)
            .await
            .context("subscribe commands")?;
        let event_device_id = device_id.to_owned();

        // 后台任务：心跳、重连、发布确认和下行命令转发。
        tokio::spawn(async move {
            loop {
                match eventloop.poll().await {
                    Ok(Event::Incoming(Incoming::Publish(packet)))
                        if packet.topic == command_topic =>
                    {
                        if command_tx.send(packet.payload.to_vec()).await.is_err() {
                            break;
                        }
                    }
                    Ok(Event::Incoming(Incoming::PubAck(_))) => {}
                    Ok(_) => {}
                    Err(e) => {
                        warn!(error = %e, "mqtt eventloop error, will retry");
                        tokio::time::sleep(Duration::from_secs(2)).await;
                    }
                }
            }
        });

        info!(device_id, "mqtt client created");
        Ok((
            Self {
                client,
                device_id: event_device_id,
            },
            command_rx,
        ))
    }

    /// 上线：带版本号
    pub async fn publish_online(&self) -> Result<()> {
        let payload = serde_json::json!({
            "device_id": self.device_id,
            "version": version::VERSION,
            "release_seq": version::release_seq(),
            "event": "online",
            "ts": now_unix(),
        });
        self.client
            .publish(
                Topic::Status.format(&self.device_id),
                QoS::AtLeastOnce,
                false,
                payload.to_string(),
            )
            .await
            .map_err(|e| anyhow::anyhow!(e).context("publish online failed"))?;
        Ok(())
    }

    /// 采样上行
    pub async fn publish_telemetry(
        &self,
        seq: u64,
        temperature: f64,
        humidity: Option<f64>,
    ) -> Result<()> {
        let payload = serde_json::json!({
            "device_id": self.device_id,
            "seq": seq,
            "ts": now_unix(),
            "temperature": temperature,
            "humidity": humidity,
        });
        self.client
            .publish(
                Topic::Telemetry.format(&self.device_id),
                QoS::AtLeastOnce,
                false,
                payload.to_string(),
            )
            .await
            .map_err(|e| anyhow::anyhow!(e).context("publish telemetry failed"))?;
        Ok(())
    }

    /// 离线广播
    pub async fn publish_offline(&self) -> Result<()> {
        let payload = serde_json::json!({
            "device_id": self.device_id,
            "version": version::VERSION,
            "release_seq": version::release_seq(),
            "event": "offline",
            "ts": now_unix(),
        });
        // 离线消息也用 QoS1，但失败不致命
        if let Err(e) = self
            .client
            .publish(
                Topic::Status.format(&self.device_id),
                QoS::AtLeastOnce,
                false,
                payload.to_string(),
            )
            .await
        {
            warn!(error = %e, "publish offline failed (ignored)");
        }
        Ok(())
    }

    pub async fn publish_event(&self, event: &str) -> Result<()> {
        let payload = serde_json::json!({
            "device_id": self.device_id,
            "event": event,
            "version": version::VERSION,
            "release_seq": version::release_seq(),
            "ts": now_unix(),
        });
        self.client
            .publish(
                Topic::Events.format(&self.device_id),
                QoS::AtLeastOnce,
                false,
                payload.to_string(),
            )
            .await?;
        Ok(())
    }

    pub async fn publish_rollback(&self, rollback: &RollbackInfo) -> Result<()> {
        let payload = serde_json::json!({
            "device_id": rollback.device_id,
            "event": "rollback",
            "from": rollback.from,
            "to": rollback.to,
            "bad_binary_kept": rollback.bad_binary_kept,
            "ts": now_unix(),
        });

        self.client
            .publish(
                Topic::Events.format(&self.device_id),
                QoS::AtLeastOnce,
                false,
                payload.to_string(),
            )
            .await
            .map_err(|error| anyhow::anyhow!(error).context("publish rollback failed"))?;
        Ok(())
    }

    pub async fn publish_ack(
        &self,
        command_id: &str,
        op: &str,
        status: &str,
        error: Option<&str>,
    ) -> Result<()> {
        let payload = serde_json::json!({
            "device_id": self.device_id,
            "command_id": command_id,
            "op": op,
            "status": status,
            "error": error,
            "version": version::VERSION,
            "release_seq": version::release_seq(),
            "ts": now_unix(),
        });

        self.client
            .publish(
                Topic::CommandAck.format(&self.device_id),
                QoS::AtLeastOnce,
                false,
                payload.to_string(),
            )
            .await
            .context("publish command ack")?;
        Ok(())
    }
}

fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}
