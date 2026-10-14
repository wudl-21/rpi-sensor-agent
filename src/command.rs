use serde::Deserialize;

#[derive(Debug, Clone, Copy)]
pub enum Topic {
    Status,
    Telemetry,
    Events,
    Commands,
    CommandAck,
}

impl Topic {
    pub fn format(self, device_id: &str) -> String {
        match self {
            Self::Status => format!("devices/{device_id}/status"),
            Self::Telemetry => format!("devices/{device_id}/telemetry"),
            Self::Events => format!("devices/{device_id}/events"),
            Self::Commands => format!("server/{device_id}/commands"),
            Self::CommandAck => format!("devices/{device_id}/command_ack"),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case", tag = "op")]
pub enum Command {
    SetInterval { value: u64 },
    PublishNow,
    TriggerOta,
    Reboot,
    Stop,
}

impl Command {
    pub fn op_name(&self) -> &'static str {
        match self {
            Self::SetInterval { .. } => "set_interval",
            Self::PublishNow => "publish_now",
            Self::TriggerOta => "trigger_ota",
            Self::Reboot => "reboot",
            Self::Stop => "stop",
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct CommandEnvelope {
    pub command_id: String,

    #[serde(flatten)]
    pub command: Command,
}

#[cfg(test)]
mod tests {
    use super::{Command, CommandEnvelope, Topic};

    #[test]
    fn parses_set_interval_command() {
        let command: CommandEnvelope =
            serde_json::from_str(r#"{"command_id":"cmd-1","op":"set_interval","value":30}"#)
                .expect("valid command");

        assert_eq!(command.command_id, "cmd-1");
        assert_eq!(command.command.op_name(), "set_interval");
        assert!(matches!(
            command.command,
            Command::SetInterval { value: 30 }
        ));
    }

    #[test]
    fn rejects_unknown_operation() {
        let result =
            serde_json::from_str::<CommandEnvelope>(r#"{"command_id":"cmd-1","op":"format_disk"}"#);

        assert!(result.is_err());
    }

    #[test]
    fn formats_command_topics() {
        assert_eq!(Topic::Commands.format("pi-1"), "server/pi-1/commands");
        assert_eq!(Topic::CommandAck.format("pi-1"), "devices/pi-1/command_ack");
    }
}
