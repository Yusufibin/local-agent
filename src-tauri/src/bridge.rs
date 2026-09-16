//! Async extension-UI bridge. The stdout reader never waits on the GUI.

use serde_json::{json, Value};
use std::collections::HashMap;
use std::time::{Duration, Instant};

const DIALOG_METHODS: &[&str] = &["select", "confirm", "input", "editor"];
const FIRE_FORGET: &[&str] = &[
    "notify",
    "setStatus",
    "setWidget",
    "setTitle",
    "set_editor_text",
];

#[derive(Debug, Clone)]
struct OpenDialog {
    deadline: Option<Instant>,
}

#[derive(Debug, Default)]
pub struct UiBridge {
    open: HashMap<String, OpenDialog>,
}

impl UiBridge {
    pub fn new() -> Self {
        Self {
            open: HashMap::new(),
        }
    }

    pub fn is_dialog_method(method: &str) -> bool {
        DIALOG_METHODS.contains(&method)
    }

    pub fn is_fire_and_forget(method: &str) -> bool {
        FIRE_FORGET.contains(&method)
    }

    pub fn on_request(&mut self, request: &Value) {
        self.expire();
        let Some(id) = request.get("id").and_then(|v| v.as_str()) else {
            return;
        };
        let method = request
            .get("method")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if !Self::is_dialog_method(method) {
            return;
        }
        let deadline = request
            .get("timeout")
            .and_then(|v| v.as_u64())
            .map(|ms| Instant::now() + Duration::from_millis(ms));
        self.open
            .insert(id.to_string(), OpenDialog { deadline });
    }

    pub fn expire(&mut self) {
        let now = Instant::now();
        self.open.retain(|_, d| match d.deadline {
            Some(t) => t > now,
            None => true,
        });
    }

    /// Returns true if the host should write `extension_ui_response`.
    /// Late responses after timeout (or unknown ids) are ignored.
    pub fn accept_response(&mut self, id: &str) -> bool {
        self.expire();
        self.open.remove(id).is_some()
    }

    pub fn open_ids(&mut self) -> Vec<String> {
        self.expire();
        self.open.keys().cloned().collect()
    }

    pub fn build_response(id: &str, payload: &Value) -> Value {
        let mut body = json!({
            "type": "extension_ui_response",
            "id": id,
        });
        if let Some(obj) = body.as_object_mut() {
            if let Some(payload_obj) = payload.as_object() {
                for (k, v) in payload_obj {
                    if k == "id" || k == "type" {
                        continue;
                    }
                    obj.insert(k.clone(), v.clone());
                }
            }
        }
        body
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn late_response_after_timeout_is_ignored() {
        let mut ui = UiBridge::new();
        ui.on_request(&json!({
            "type": "extension_ui_request",
            "id": "t1",
            "method": "select",
            "timeout": 20
        }));
        assert!(ui.accept_response("t1"));
        ui.on_request(&json!({
            "type": "extension_ui_request",
            "id": "t2",
            "method": "confirm",
            "timeout": 20
        }));
        thread::sleep(Duration::from_millis(40));
        assert!(!ui.accept_response("t2"));
    }

    #[test]
    fn fire_and_forget_is_not_tracked() {
        let mut ui = UiBridge::new();
        ui.on_request(&json!({
            "type": "extension_ui_request",
            "id": "n1",
            "method": "notify",
            "message": "hi"
        }));
        assert!(!ui.accept_response("n1"));
    }
}
