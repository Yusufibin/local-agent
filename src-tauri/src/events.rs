use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProcessStatus {
    Spawned,
    Exited,
    Crashed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum HostEvent {
    Rpc { event: Value },
    Process {
        status: ProcessStatus,
        #[serde(skip_serializing_if = "Option::is_none")]
        code: Option<i32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        message: Option<String>,
    },
    UiRequest { request: Value },
    Log {
        level: String,
        message: String,
    },
    Watchdog {
        #[serde(rename = "silenceMs")]
        silence_ms: u64,
    },
}

impl HostEvent {
    pub fn log(level: &str, message: impl Into<String>) -> Self {
        Self::Log {
            level: level.to_string(),
            message: message.into(),
        }
    }

    pub fn process(status: ProcessStatus, code: Option<i32>) -> Self {
        Self::Process {
            status,
            code,
            message: None,
        }
    }

    pub fn process_msg(
        status: ProcessStatus,
        code: Option<i32>,
        message: impl Into<String>,
    ) -> Self {
        Self::Process {
            status,
            code,
            message: Some(message.into()),
        }
    }

    pub fn watchdog(silence_ms: u64) -> Self {
        Self::Watchdog { silence_ms }
    }
}

pub fn classify_stdout(value: &Value) -> StdoutClass {
    match value.get("type").and_then(|v| v.as_str()).unwrap_or("") {
        "response" => StdoutClass::Response,
        "extension_ui_request" => StdoutClass::UiRequest,
        _ => StdoutClass::RpcEvent,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StdoutClass {
    Response,
    UiRequest,
    RpcEvent,
}

pub fn wrap_stdout(value: Value) -> HostEvent {
    match classify_stdout(&value) {
        StdoutClass::UiRequest => HostEvent::UiRequest { request: value },
        _ => HostEvent::Rpc { event: value },
    }
}
