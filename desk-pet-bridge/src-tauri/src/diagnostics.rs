const SENSITIVE: &[&str] = &["passkey", "password", "token", "secret", "pin code"];

/// Redact likely secrets from diagnostic strings. Never log GATT keys or passkeys.
pub fn redact(input: &str) -> String {
    let lower = input.to_lowercase();
    if SENSITIVE.iter().any(|key| lower.contains(key)) {
        return "[redacted]".to_string();
    }
    input.chars().take(160).collect()
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
        assert_eq!(redact(r#"{"ok":true,"mode":"agent"}"#), r#"{"ok":true,"mode":"agent"}"#);
    }
}
