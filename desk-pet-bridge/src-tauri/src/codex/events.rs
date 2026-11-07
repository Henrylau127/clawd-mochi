use serde_json::Value;

use crate::bridge::protocol::ProtocolError;
use crate::providers::map_provider;

/// Map an OpenAI Codex hook (`hooks.json` command stdin) into a desk-pet event.
///
/// Hook names match Codex's documented set:
/// <https://learn.chatgpt.com/docs/hooks>.
/// The mapper never copies prompt, cwd, transcript, tool input, or error text.
pub fn map_incoming(body: Value) -> Result<Value, ProtocolError> {
    map_provider("codex", body, map_hook)
}

fn map_hook(hook: &str, outcome: Option<&str>) -> Option<(&'static str, Option<&'static str>)> {
    Some(match hook {
        "SessionStart" => ("sessionStart", Some("coding")),
        "SessionEnd" => ("sessionEnd", None),
        "UserPromptSubmit" | "PreModelRequest" | "PreCompact" | "PostCompact" => {
            ("stateUpdate", Some("thinking"))
        }
        "PostModelRequest" => match outcome {
            Some("failed") => ("stateUpdate", Some("error")),
            Some("cancelled") => ("stateUpdate", Some("idle")),
            _ => ("stateUpdate", Some("thinking")),
        },
        "PreToolUse" | "PostToolUse" | "SubagentStart" | "SubagentStop" => {
            ("stateUpdate", Some("coding"))
        }
        "PermissionRequest" | "Stop" => ("stateUpdate", Some("waiting_for_user")),
        "Interrupt" => ("stateUpdate", Some("idle")),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bridge::protocol::parse_incoming;
    use serde_json::json;

    #[test]
    fn maps_session_model_tools_and_stop() {
        let cases = [
            ("SessionStart", None, "sessionStart", Some("coding")),
            ("SessionEnd", None, "sessionEnd", None),
            ("UserPromptSubmit", None, "stateUpdate", Some("thinking")),
            ("PreModelRequest", None, "stateUpdate", Some("thinking")),
            (
                "PostModelRequest",
                Some("completed"),
                "stateUpdate",
                Some("thinking"),
            ),
            (
                "PostModelRequest",
                Some("failed"),
                "stateUpdate",
                Some("error"),
            ),
            (
                "PostModelRequest",
                Some("cancelled"),
                "stateUpdate",
                Some("idle"),
            ),
            ("PreToolUse", None, "stateUpdate", Some("coding")),
            ("PostToolUse", None, "stateUpdate", Some("coding")),
            (
                "PermissionRequest",
                None,
                "stateUpdate",
                Some("waiting_for_user"),
            ),
            ("Stop", None, "stateUpdate", Some("waiting_for_user")),
            ("Interrupt", None, "stateUpdate", Some("idle")),
            ("SubagentStart", None, "stateUpdate", Some("coding")),
            ("SubagentStop", None, "stateUpdate", Some("coding")),
            ("PreCompact", None, "stateUpdate", Some("thinking")),
            ("PostCompact", None, "stateUpdate", Some("thinking")),
        ];
        for (hook, outcome, event, state) in cases {
            let mut body = json!({ "hook_event_name": hook });
            if let Some(outcome) = outcome {
                body["status"] = json!(outcome);
            }
            let mapped = map_incoming(body).unwrap();
            assert_eq!(mapped["event"], event, "{hook}");
            assert_eq!(mapped["source"], "codex", "{hook}");
            match state {
                Some(state) => assert_eq!(mapped["state"], state, "{hook}"),
                None => assert!(mapped.get("state").is_none(), "{hook}"),
            }
        }
    }

    #[test]
    fn drops_prompt_path_command_and_error_text() {
        let mapped = map_incoming(json!({
            "hook_event_name": "PostModelRequest",
            "status": "failed",
            "error": "API key sk-live-SECRET leaked",
            "prompt": "SECRET_PROMPT",
            "cwd": "/Users/me/SECRET_PATH",
            "transcript_path": "/tmp/SECRET_TRANSCRIPT.jsonl",
            "tool_input": { "command": "SECRET_COMMAND" },
            "last_assistant_message": "SECRET_REPLY",
            "session_id": "thr_secret",
            "model": "gpt-secret"
        }))
        .unwrap();
        let event = parse_incoming(mapped.clone()).unwrap();
        assert_eq!(event.state.as_deref(), Some("error"));
        assert_eq!(event.source.as_deref(), Some("codex"));
        let ble = String::from_utf8(event.to_ble_bytes().unwrap()).unwrap();
        let rendered = format!("{mapped}{ble}");
        for secret in [
            "SECRET",
            "sk-live",
            "SECRET_PATH",
            "SECRET_COMMAND",
            "thr_secret",
            "gpt-secret",
        ] {
            assert!(!rendered.contains(secret), "{secret} leaked in {rendered}");
        }
    }

    #[test]
    fn passes_through_normalized_event_without_copying_details() {
        let mapped = map_incoming(json!({
            "event": "stateUpdate",
            "state": "coding",
            "source": "cursor",
            "id": "abc_123",
            "ts": "2026-11-07T21:16:00Z",
            "details": { "message": "SECRET_DETAIL" }
        }))
        .unwrap();
        assert_eq!(mapped["source"], "codex");
        assert_eq!(mapped["state"], "coding");
        assert_eq!(mapped["id"], "abc_123");
        assert_eq!(mapped["ts"], "2026-11-07T21:16:00Z");
        assert!(mapped.get("details").is_none());
        assert!(!mapped.to_string().contains("SECRET_DETAIL"));
    }

    #[test]
    fn rejects_unknown_hook_and_unsafe_state() {
        assert!(map_incoming(json!({"hook": "dropTable"})).is_err());
        assert!(map_incoming(json!({"event": "stateUpdate", "state": "explode"})).is_err());
        assert!(map_incoming(json!({"hook": "../SECRET_PROMPT"})).is_err());
    }
}
