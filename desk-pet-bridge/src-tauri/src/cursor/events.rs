use serde_json::{json, Value};

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
        for hook in [
            "preToolUse",
            "afterFileEdit",
            "beforeReadFile",
            "postToolUse",
        ] {
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
    fn leaves_codex_and_omp_hook_names_unmapped() {
        assert!(map_incoming(json!({"hook": "SessionStart"})).is_err());
        assert!(map_incoming(json!({"hook": "session_start"})).is_err());
    }
}
