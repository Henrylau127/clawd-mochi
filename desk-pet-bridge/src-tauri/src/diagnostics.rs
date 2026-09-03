const SENSITIVE: &[&str] = &["passkey", "password", "token", "secret", "pin code"];

/// Redact likely secrets from diagnostic strings. Never log GATT keys or passkeys.
pub fn redact(input: &str) -> String {
    let lower = input.to_lowercase();
    if SENSITIVE.iter().any(|key| lower.contains(key)) {
        return "[redacted]".to_string();
    }
    redact_event_message(input)
        .unwrap_or_else(|| input.to_string())
        .chars()
        .take(160)
        .collect()
}

fn redact_event_message(input: &str) -> Option<String> {
    let mut value: serde_json::Value = serde_json::from_str(input).ok()?;
    value
        .get_mut("details")
        .and_then(serde_json::Value::as_object_mut)?
        .remove("message")?;
    serde_json::to_string(&value).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_passkey() {
        assert_eq!(redact("passkey=123456"), "[redacted]");
    }

    #[test]
    fn keeps_ack() {
        assert_eq!(
            redact(r#"{"ok":true,"mode":"agent"}"#),
            r#"{"ok":true,"mode":"agent"}"#
        );
    }

    #[test]
    fn removes_event_detail_message() {
        let output = redact(
            r#"{"event":"securityEvent","details":{"type":"ssh_login","message":"SSH login from 203.0.113.1"}}"#,
        );
        assert!(!output.contains("203.0.113.1"));
        assert!(output.contains(r#""type":"ssh_login""#));
    }
}
