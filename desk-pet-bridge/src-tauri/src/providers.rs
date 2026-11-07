//! Shared sanitizer for non-Cursor agent hooks.
//!
//! Codex and OMP rebuild a desk-pet event from an allowlisted hook name and
//! outcome. Prompts, paths, commands, transcripts, and error text are dropped.
//! Claude Code and upstream Pi have no matcher here; accept them in
//! `loopback::validate_provider` only after adding one.

use serde_json::{json, Value};

use crate::bridge::protocol::{is_allowed_event, is_allowed_state, ProtocolError};

pub(crate) fn map_provider(
    source: &'static str,
    body: Value,
    map_hook: fn(&str, Option<&str>) -> Option<(&'static str, Option<&'static str>)>,
) -> Result<Value, ProtocolError> {
    if !body.is_object() {
        return Err(ProtocolError::Malformed);
    }

    let outcome = safe_outcome(&body);
    if let Some(hook) = hook_name(&body)? {
        if let Some((event, state)) = map_hook(&hook, outcome) {
            return Ok(finish(source, event, state, &body));
        }
        if is_allowed_event(&hook) {
            return Ok(finish(source, &hook, body_state(&body)?, &body));
        }
        return Err(ProtocolError::UnknownEvent);
    }

    if let Some(event) = body.get("event").and_then(Value::as_str) {
        if is_allowed_event(event) {
            return Ok(finish(source, event, body_state(&body)?, &body));
        }
    }
    Err(ProtocolError::UnknownEvent)
}

fn hook_name(body: &Value) -> Result<Option<String>, ProtocolError> {
    for key in ["hook", "hook_event_name"] {
        match body.get(key) {
            None | Some(Value::Null) => {}
            Some(Value::String(value)) if value.is_empty() => {}
            Some(Value::String(value)) if is_safe_token(value, 64) => {
                return Ok(Some(value.clone()));
            }
            Some(_) => return Err(ProtocolError::Malformed),
        }
    }
    Ok(None)
}

/// `outcome` is the script's allowlisted token. `status` is Codex
/// `PostModelRequest`. Booleans cover a raw OMP payload posted without the script.
fn safe_outcome(body: &Value) -> Option<&str> {
    if let Some(value) = body.get("outcome").and_then(Value::as_str) {
        if is_outcome(value) {
            return Some(value);
        }
    }
    if let Some(status) = body.get("status").and_then(Value::as_str) {
        if is_outcome(status) && matches!(status, "completed" | "failed" | "cancelled") {
            return Some(status);
        }
    }
    if body.get("isError") == Some(&Value::Bool(true)) {
        return Some("error");
    }
    if body.get("willContinue") == Some(&Value::Bool(true)) {
        return Some("continue");
    }
    None
}

fn is_outcome(value: &str) -> bool {
    matches!(
        value,
        "completed" | "failed" | "cancelled" | "error" | "ok" | "continue" | "done"
    )
}

fn body_state(body: &Value) -> Result<Option<&str>, ProtocolError> {
    match body.get("state") {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(state)) if state.is_empty() => Ok(None),
        Some(Value::String(state)) if is_allowed_state(state) => Ok(Some(state.as_str())),
        Some(Value::String(_)) => Err(ProtocolError::UnknownState),
        Some(_) => Err(ProtocolError::Malformed),
    }
}

fn finish(source: &str, event: &str, state: Option<&str>, body: &Value) -> Value {
    let mut mapped = json!({
        "event": event,
        "source": source,
    });
    if let Some(state) = state {
        mapped["state"] = json!(state);
    }
    if let Some(id) = body.get("id").and_then(Value::as_str) {
        if is_safe_token(id, 32) {
            mapped["id"] = json!(id);
        }
    }
    if let Some(ts) = body.get("ts").and_then(Value::as_str) {
        if is_utc_timestamp(ts) {
            mapped["ts"] = json!(ts);
        }
    }
    mapped
}

fn is_safe_token(value: &str, max: usize) -> bool {
    !value.is_empty()
        && value.len() <= max
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
}

fn is_utc_timestamp(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.len() != 20 {
        return false;
    }
    let digits =
        |range: std::ops::Range<usize>| bytes[range].iter().all(|byte| byte.is_ascii_digit());
    digits(0..4)
        && bytes[4] == b'-'
        && digits(5..7)
        && bytes[7] == b'-'
        && digits(8..10)
        && bytes[10] == b'T'
        && digits(11..13)
        && bytes[13] == b':'
        && digits(14..16)
        && bytes[16] == b':'
        && digits(17..19)
        && bytes[19] == b'Z'
}
