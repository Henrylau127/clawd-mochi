use serde_json::{json, Value};

use crate::bridge::caption::{self, ActivityHint};
use crate::bridge::protocol::ProtocolError;

/// Map a Cursor hook name or an already-normalized desk-pet event into the
/// JSON shape firmware `JsonProtocol` accepts.
pub fn map_incoming(mut body: Value) -> Result<Value, ProtocolError> {
    if !body.is_object() {
        return Err(ProtocolError::Malformed);
    }

    let hook = body
        .get("hook")
        .and_then(Value::as_str)
        .or_else(|| body.get("event").and_then(Value::as_str))
        .unwrap_or("")
        .to_string();

    let hint = ActivityHint::from_value(&body);

    if matches!(
        hook.as_str(),
        "sessionStart"
            | "sessionEnd"
            | "stateUpdate"
            | "securityEvent"
            | "attentionRequired"
            | "attentionCleared"
            | "sleep"
            | "wake"
            | "pairingStart"
            | "pairingStop"
            | "unbond"
            | "quotaUpdate"
    ) {
        if let Some(obj) = body.as_object_mut() {
            obj.remove("hook");
        }
        caption::decorate(&mut body, &hook, &hint);
        return Ok(body);
    }

    let (event, state) = match hook.as_str() {
        "afterAgentThought" => ("stateUpdate", Some("thinking")),
        // Leave thinking as soon as tools/edits run; thought hooks alone stick otherwise.
        "beforeShellExecution"
        | "beforeMCPExecution"
        | "preToolUse"
        | "afterFileEdit"
        | "beforeReadFile" => ("stateUpdate", Some("coding")),
        "afterAgentResponse" => ("stateUpdate", Some("waiting_for_user")),
        // Stay in coding between tool rounds instead of flashing idle.
        "afterShellExecution" | "postToolUse" => ("stateUpdate", Some("coding")),
        "postToolUseFailure" => ("stateUpdate", Some("error")),
        "" => return Err(ProtocolError::UnknownEvent),
        _ => return Err(ProtocolError::UnknownEvent),
    };

    let mut mapped = json!({
        "event": event,
        "source": "cursor",
    });
    if let Some(state) = state {
        mapped["state"] = json!(state);
    }
    if let Some(id) = body.get("id") {
        mapped["id"] = id.clone();
    }
    if let Some(ts) = body.get("ts") {
        mapped["ts"] = ts.clone();
    }
    caption::decorate(&mut mapped, &hook, &hint);
    Ok(mapped)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn maps_thought_hook() {
        let mapped = map_incoming(json!({"hook": "afterAgentThought"})).unwrap();
        assert_eq!(mapped["event"], "stateUpdate");
        assert_eq!(mapped["state"], "thinking");
    }

    #[test]
    fn tool_and_edit_hooks_leave_thinking() {
        for hook in ["preToolUse", "afterFileEdit", "beforeReadFile", "postToolUse"] {
            let mapped = map_incoming(json!({ "hook": hook })).unwrap();
            assert_eq!(mapped["event"], "stateUpdate", "{}", hook);
            assert_eq!(mapped["state"], "coding", "{}", hook);
        }
    }

    #[test]
    fn passes_through_session_start() {
        let mapped = map_incoming(json!({"event": "sessionStart", "state": "coding"})).unwrap();
        assert_eq!(mapped["event"], "sessionStart");
        assert_eq!(mapped["state"], "coding");
        assert!(mapped.get("hook").is_none());
    }

    #[test]
    fn rejects_unknown_event_name() {
        assert!(map_incoming(json!({"event": "dropTable"})).is_err());
    }

    #[test]
    fn thought_and_edit_hooks_add_plain_captions() {
        let thought = map_incoming(json!({"hook": "afterAgentThought"})).unwrap();
        assert_eq!(thought["caption"], "thinking");

        let edit = map_incoming(json!({
            "hook": "afterFileEdit",
            "file_path": "/Users/me/secret.env"
        }))
        .unwrap();
        assert_eq!(edit["state"], "coding");
        assert_eq!(edit["caption"], "editing");
        assert!(!edit.to_string().contains("secret"));
    }

    #[test]
    fn response_hook_still_waits() {
        let mapped = map_incoming(json!({"hook": "afterAgentResponse"})).unwrap();
        assert_eq!(mapped["state"], "waiting_for_user");
        assert_eq!(mapped["caption"], "waiting for you");
    }

    #[test]
    fn finished_commit_celebrates_without_the_command() {
        let mapped = map_incoming(json!({
            "hook": "afterShellExecution",
            "command": "git commit -m 'ship /tmp/id_rsa'"
        }))
        .unwrap();
        assert_eq!(mapped["state"], "celebrate");
        assert_eq!(mapped["caption"], "committed");
        let blob = mapped.to_string();
        assert!(!blob.contains("id_rsa"));
        assert!(!blob.contains("git"));
    }

    #[test]
    fn session_start_keeps_coding_and_adds_a_wake_caption() {
        let mapped = map_incoming(
            json!({"event": "sessionStart", "state": "coding", "hook": "sessionStart"}),
        )
        .unwrap();
        assert_eq!(mapped["state"], "coding");
        assert_eq!(mapped["caption"], "waking up");
        assert!(mapped.get("hook").is_none());
    }
}
