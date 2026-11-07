use serde_json::Value;

use crate::bridge::protocol::ProtocolError;
use crate::providers::map_provider;

/// Map an oh-my-pi (OMP) extension event into a desk-pet event.
///
/// Event names match <https://omp.sh/docs/hooks>. OMP is the `omp` CLI, not a
/// separate OpenAI product. OpenAI's hook surface is Codex (`codex::events`).
///
/// Extension point: message streams, compaction, and `user_bash` / `user_python`
/// are intentionally unmapped. Those payloads carry prompts or commands. Add a
/// match arm here, then the same name in `scripts/agent-hook.sh`, only when the
/// signal is a short label.
pub fn map_incoming(body: Value) -> Result<Value, ProtocolError> {
    map_provider("omp", body, map_hook)
}

fn map_hook(hook: &str, outcome: Option<&str>) -> Option<(&'static str, Option<&'static str>)> {
    Some(match hook {
        "session_start" | "session_switch" => ("sessionStart", Some("coding")),
        "session_shutdown" => ("sessionEnd", None),
        "agent_start" | "before_agent_start" | "turn_start" => ("stateUpdate", Some("thinking")),
        "tool_call" | "tool_execution_start" => ("stateUpdate", Some("coding")),
        "tool_execution_end" => match outcome {
            Some("error") => ("stateUpdate", Some("error")),
            _ => ("stateUpdate", Some("coding")),
        },
        "agent_end" => match outcome {
            Some("continue") => ("stateUpdate", Some("thinking")),
            _ => ("stateUpdate", Some("waiting_for_user")),
        },
        "session_stop" => ("stateUpdate", Some("waiting_for_user")),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bridge::protocol::parse_incoming;
    use serde_json::json;

    #[test]
    fn maps_session_tools_and_turn_end() {
        let cases = [
            ("session_start", None, "sessionStart", Some("coding")),
            ("session_switch", None, "sessionStart", Some("coding")),
            ("session_shutdown", None, "sessionEnd", None),
            ("agent_start", None, "stateUpdate", Some("thinking")),
            ("before_agent_start", None, "stateUpdate", Some("thinking")),
            ("turn_start", None, "stateUpdate", Some("thinking")),
            ("tool_call", None, "stateUpdate", Some("coding")),
            ("tool_execution_start", None, "stateUpdate", Some("coding")),
            (
                "tool_execution_end",
                Some("ok"),
                "stateUpdate",
                Some("coding"),
            ),
            (
                "tool_execution_end",
                Some("error"),
                "stateUpdate",
                Some("error"),
            ),
            (
                "agent_end",
                Some("done"),
                "stateUpdate",
                Some("waiting_for_user"),
            ),
            (
                "agent_end",
                Some("continue"),
                "stateUpdate",
                Some("thinking"),
            ),
            (
                "session_stop",
                None,
                "stateUpdate",
                Some("waiting_for_user"),
            ),
        ];
        for (hook, outcome, event, state) in cases {
            let mut body = json!({ "hook": hook });
            if let Some(outcome) = outcome {
                body["outcome"] = json!(outcome);
            }
            let mapped = map_incoming(body).unwrap();
            assert_eq!(mapped["event"], event, "{hook}");
            assert_eq!(mapped["source"], "omp", "{hook}");
            match state {
                Some(state) => assert_eq!(mapped["state"], state, "{hook}"),
                None => assert!(mapped.get("state").is_none(), "{hook}"),
            }
        }
    }

    #[test]
    fn raw_tool_error_boolean_drops_result_text() {
        let mapped = map_incoming(json!({
            "hook": "tool_execution_end",
            "isError": true,
            "toolName": "bash",
            "args": { "command": "SECRET_COMMAND" },
            "result": "SECRET_RESULT /Users/me/SECRET_PATH",
            "prompt": "SECRET_PROMPT"
        }))
        .unwrap();
        let event = parse_incoming(mapped.clone()).unwrap();
        assert_eq!(event.state.as_deref(), Some("error"));
        let ble = String::from_utf8(event.to_ble_bytes().unwrap()).unwrap();
        let rendered = format!("{mapped}{ble}");
        for secret in [
            "SECRET_COMMAND",
            "SECRET_RESULT",
            "SECRET_PATH",
            "SECRET_PROMPT",
            "bash",
        ] {
            assert!(!rendered.contains(secret), "{secret} leaked in {rendered}");
        }
    }

    #[test]
    fn continuing_agent_stays_thinking_without_messages() {
        let mapped = map_incoming(json!({
            "hook": "agent_end",
            "willContinue": true,
            "messages": [{ "text": "SECRET_REPLY" }]
        }))
        .unwrap();
        assert_eq!(mapped["state"], "thinking");
        assert!(!mapped.to_string().contains("SECRET_REPLY"));
    }

    #[test]
    fn rejects_unmapped_high_volume_events() {
        for hook in [
            "message_update",
            "tool_execution_update",
            "user_bash",
            "turn_end",
        ] {
            assert!(map_incoming(json!({ "hook": hook })).is_err(), "{hook}");
        }
    }
}
