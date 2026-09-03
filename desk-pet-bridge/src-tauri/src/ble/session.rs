use serde::Deserialize;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FirmwareStatus {
    pub mode: Option<String>,
    pub state: Option<String>,
    pub firmware: Option<String>,
    pub secure_session_confirmed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AckStatus {
    pub id: Option<String>,
    pub ok: bool,
    pub mode: Option<String>,
    pub state: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Notification {
    FirmwareStatus(FirmwareStatus),
    Ack(AckStatus),
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum SessionError {
    #[error("status notification is not valid UTF-8")]
    InvalidUtf8,
    #[error("status notification is malformed JSON")]
    MalformedJson,
    #[error("protected status read is required before subscribing")]
    MissingEncryptedStatusRead,
    #[error("firmware secure session confirmation is missing")]
    MissingFirmwareConfirmation,
    #[error("firmware status is missing mode or state")]
    MissingFirmwareStatusFields,
    #[error("firmware status contains an unknown mode or state")]
    InvalidFirmwareStatusValue,
    #[error("firmware secure session is not ready")]
    FirmwareSessionNotReady,
    #[error("status subscription is required before the session is ready")]
    MissingStatusSubscription,
}

#[derive(Debug, Deserialize)]
struct WireNotification {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    ok: Option<bool>,
    #[serde(default)]
    mode: Option<String>,
    #[serde(default)]
    state: Option<String>,
    #[serde(default)]
    ble_connected: Option<bool>,
    #[serde(default)]
    secure_session: Option<bool>,
    #[serde(default)]
    fw: Option<String>,
    #[serde(default)]
    error: Option<String>,
}

#[derive(Debug, Default)]
pub struct Handshake {
    encrypted_status_read: bool,
    status_subscription: bool,
    firmware_confirmation: bool,
}

impl Handshake {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record_encrypted_status_read(
        &mut self,
        payload: &[u8],
    ) -> Result<FirmwareStatus, SessionError> {
        let status = parse_firmware_status(payload)?;
        self.encrypted_status_read = true;
        self.firmware_confirmation = status.secure_session_confirmed;
        Ok(status)
    }

    pub fn record_status_subscription(&mut self) -> Result<(), SessionError> {
        if !self.encrypted_status_read {
            return Err(SessionError::MissingEncryptedStatusRead);
        }
        if !self.firmware_confirmation {
            return Err(SessionError::MissingFirmwareConfirmation);
        }
        self.status_subscription = true;
        Ok(())
    }

    pub fn is_ready(&self) -> bool {
        self.encrypted_status_read && self.status_subscription && self.firmware_confirmation
    }

    pub fn ensure_ready(&self) -> Result<(), SessionError> {
        if !self.encrypted_status_read {
            return Err(SessionError::MissingEncryptedStatusRead);
        }
        if !self.firmware_confirmation {
            return Err(SessionError::MissingFirmwareConfirmation);
        }
        if !self.status_subscription {
            return Err(SessionError::MissingStatusSubscription);
        }
        Ok(())
    }
}

pub fn parse_firmware_status(payload: &[u8]) -> Result<FirmwareStatus, SessionError> {
    let wire = parse_wire(payload)?;
    let secure_session_confirmed = match (wire.ble_connected, wire.secure_session) {
        (Some(true), Some(true)) => true,
        (Some(false), _) | (_, Some(false)) => false,
        _ => return Err(SessionError::MissingFirmwareConfirmation),
    };
    if !secure_session_confirmed {
        return Err(SessionError::FirmwareSessionNotReady);
    }
    validate_status_fields(wire.mode.as_deref(), wire.state.as_deref())?;

    Ok(FirmwareStatus {
        mode: wire.mode,
        state: wire.state,
        firmware: wire.fw,
        secure_session_confirmed,
    })
}

pub fn parse_notification(payload: &[u8]) -> Result<Notification, SessionError> {
    let wire = parse_wire(payload)?;
    if wire.ble_connected.is_some() || wire.secure_session.is_some() {
        return parse_firmware_status(payload).map(Notification::FirmwareStatus);
    }
    if let Some(ok) = wire.ok {
        validate_status_fields(wire.mode.as_deref(), wire.state.as_deref())?;
        return Ok(Notification::Ack(AckStatus {
            id: wire.id,
            ok,
            mode: wire.mode,
            state: wire.state,
            error: wire.error,
        }));
    }
    Err(SessionError::MissingFirmwareConfirmation)
}

fn validate_status_fields(mode: Option<&str>, state: Option<&str>) -> Result<(), SessionError> {
    let mode = mode.ok_or(SessionError::MissingFirmwareStatusFields)?;
    let state = state.ok_or(SessionError::MissingFirmwareStatusFields)?;
    if !matches!(mode, "wander" | "agent" | "sleep")
        || !matches!(
            state,
            "idle" | "coding" | "thinking" | "waiting_for_user" | "error" | "security"
        )
    {
        return Err(SessionError::InvalidFirmwareStatusValue);
    }
    Ok(())
}

fn parse_wire(payload: &[u8]) -> Result<WireNotification, SessionError> {
    std::str::from_utf8(payload).map_err(|_| SessionError::InvalidUtf8)?;
    serde_json::from_slice(payload).map_err(|_| SessionError::MalformedJson)
}

#[cfg(test)]
mod tests {
    use super::{parse_notification, Handshake, Notification, SessionError};

    const READY_STATUS: &[u8] =
        br#"{"mode":"wander","state":"idle","ble_connected":true,"secure_session":true,"fw":"0.2.2-mac2"}"#;

    #[test]
    fn handshake_requires_status_subscription() {
        let mut handshake = Handshake::new();
        let status = handshake
            .record_encrypted_status_read(READY_STATUS)
            .expect("ready status should parse");

        assert_eq!(status.mode.as_deref(), Some("wander"));
        assert!(!handshake.is_ready());

        handshake
            .record_status_subscription()
            .expect("subscription should follow the protected status read");
        assert!(handshake.is_ready());
    }

    #[test]
    fn handshake_rejects_subscription_before_protected_status_read() {
        let mut handshake = Handshake::new();

        assert_eq!(
            handshake.record_status_subscription(),
            Err(SessionError::MissingEncryptedStatusRead)
        );
        assert!(!handshake.is_ready());
    }

    #[test]
    fn handshake_requires_explicit_secure_session_confirmation() {
        let mut handshake = Handshake::new();

        assert_eq!(
            handshake.record_encrypted_status_read(
                br#"{"mode":"wander","state":"idle","ble_connected":true}"#
            ),
            Err(SessionError::MissingFirmwareConfirmation)
        );
        assert!(!handshake.is_ready());
    }

    #[test]
    fn handshake_rejects_unconfirmed_firmware_status() {
        let mut handshake = Handshake::new();
        let error = handshake
            .record_encrypted_status_read(
                br#"{"mode":"wander","state":"idle","ble_connected":false}"#,
            )
            .expect_err("firmware must confirm the secure session");

        assert!(error.to_string().contains("secure session"));
        assert!(!handshake.is_ready());
    }

    #[test]
    fn parses_firmware_status_notifications_for_live_state() {
        let notification = parse_notification(
            br#"{"mode":"agent","state":"thinking","ble_connected":true,"secure_session":true,"fw":"0.2.2-mac2"}"#,
        )
        .expect("status notification should parse");

        match notification {
            Notification::FirmwareStatus(status) => {
                assert_eq!(status.mode.as_deref(), Some("agent"));
                assert_eq!(status.state.as_deref(), Some("thinking"));
                assert!(status.secure_session_confirmed);
            }
            Notification::Ack(_) => panic!("expected firmware status"),
        }
    }

    #[test]
    fn parses_ack_notifications_for_live_state_without_marking_ready() {
        let notification =
            parse_notification(br#"{"id":"abcd1234","ok":true,"mode":"agent","state":"coding"}"#)
                .expect("ack notification should parse");

        match notification {
            Notification::Ack(ack) => {
                assert!(ack.ok);
                assert_eq!(ack.mode.as_deref(), Some("agent"));
                assert_eq!(ack.state.as_deref(), Some("coding"));
            }
            Notification::FirmwareStatus(_) => panic!("expected acknowledgement"),
        }
    }

    #[test]
    fn rejects_incomplete_ack_notifications() {
        assert_eq!(
            parse_notification(br#"{"ok":true}"#),
            Err(SessionError::MissingFirmwareStatusFields)
        );
    }

    #[test]
    fn rejects_unknown_firmware_status_values() {
        assert_eq!(
            parse_notification(
                br#"{"mode":"unknown","state":"idle","ble_connected":true,"secure_session":true}"#
            ),
            Err(SessionError::InvalidFirmwareStatusValue)
        );
    }

    #[test]
    fn malformed_notifications_are_rejected() {
        assert!(parse_notification(br#"{"mode":"agent"}"#).is_err());
        assert!(parse_notification(b"not json").is_err());
    }
}
