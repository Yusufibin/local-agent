//! Bound the IPC payload while exposing the full transcript in real pages.
use serde_json::{json, Value};
pub const PAGE_SIZE: usize = 200;

pub fn page(mut data: Value, before: Option<usize>, limit: Option<usize>) -> Value {
    let messages = data.get_mut("messages").and_then(Value::as_array_mut);
    let Some(messages) = messages else {
        return json!({ "messages": [], "total": 0, "before": 0, "hasMore": false });
    };
    let total = messages.len();
    let end = before.unwrap_or(total).min(total);
    let start = end.saturating_sub(limit.unwrap_or(PAGE_SIZE).clamp(1, PAGE_SIZE));
    let slice = messages.drain(start..end).collect::<Vec<_>>();
    json!({
        "messages": slice, "total": total, "before": start,
        "hasMore": start > 0, "truncatedFrom": start
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pages_have_no_gaps_and_keep_absolute_cursor_when_messages_are_appended() {
        let data = json!({ "messages": (0..450).collect::<Vec<_>>() });
        let newest = page(data.clone(), None, None);
        assert_eq!(newest["messages"][0], 250);
        assert_eq!(newest["before"], 250);
        let older = page(data, Some(250), None);
        assert_eq!(older["messages"][0], 50);
        assert_eq!(older["messages"][199], 249);
        let appended = json!({ "messages": (0..451).collect::<Vec<_>>() });
        assert_eq!(page(appended, Some(250), None)["messages"], older["messages"]);
    }
    #[test]
    fn empty_and_invalid_cursors_are_bounded() {
        assert_eq!(page(json!({}), None, None)["total"], 0);
        let data = json!({ "messages": [1, 2, 3] });
        assert_eq!(page(data.clone(), Some(100), Some(0))["messages"], json!([3]));
        assert_eq!(page(data, Some(0), None)["messages"], json!([]));
    }
}
