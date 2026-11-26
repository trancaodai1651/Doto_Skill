//! Bus sự kiện realtime (SSE). Mỗi sự kiện có `type` + `data` khớp `ServerEvent` phía web.

use serde::Serialize;
use tokio::sync::broadcast;

#[derive(Debug, Clone, Serialize)]
pub struct Event {
    #[serde(rename = "type")]
    pub kind: &'static str,
    pub data: serde_json::Value,
    /// Viewer chỉ nhận sự kiện có `viewer_visible = true`.
    #[serde(skip)]
    pub viewer_visible: bool,
}

#[derive(Clone)]
pub struct EventBus(broadcast::Sender<Event>);

impl Default for EventBus {
    fn default() -> Self {
        Self(broadcast::channel(512).0)
    }
}

impl EventBus {
    pub fn emit<T: Serialize>(&self, kind: &'static str, data: &T) {
        self.emit_scoped(kind, data, false)
    }

    pub fn emit_scoped<T: Serialize>(&self, kind: &'static str, data: &T, viewer_visible: bool) {
        if let Ok(v) = serde_json::to_value(data) {
            let _ = self.0.send(Event { kind, data: v, viewer_visible });
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.0.subscribe()
    }
}
