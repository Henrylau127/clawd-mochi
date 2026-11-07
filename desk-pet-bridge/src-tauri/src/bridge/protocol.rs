use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::caption;

pub const MAX_PAYLOAD_BYTES: usize = 180;

const ALLOWED_EVENTS: &[&str] = &[
    "sessionStart",
    "sessionEnd",
    "stateUpdate",
    "securityEvent",
    "attentionRequired",
    "attentionCleared",
    "sleep",
    "wake",
    "pairingStart",
    "pairingStop",
    "unbond",
    "quotaUpdate",
];

const ALLOWED_STATES: &[&str] = &[
    "coding",
    "thinking",
    "idle",
    "waiting",
    "waiting_for_user",
    "error",
    "security",
    "celebrate",
];

const ALLOWED_SECURITY_TYPES: &[&str] = &["ssh_login", "github_login", "deployment", "sudo"];

#[derive(Debug, Error)]
pub enum ProtocolError {
    #[error("malformed json")]
    Malformed,
    #[error("unknown event")]
    UnknownEvent,
    #[error("unknown state")]
    UnknownState,
    #[error("payload too large ({0} bytes)")]
    TooLarge(usize),
    #[error("serialize failed")]
    Serialize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EventDetails {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub r#type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DeskPetEvent {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub event: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ts: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub details: Option<EventDetails>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quota: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub caption: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mood: Option<String>,
}

impl DeskPetEvent {
    pub fn new(event: impl Into<String>) -> Self {
        Self {
            id: None,
            event: event.into(),
            state: None,
            source: Some("bridge".to_string()),
            ts: None,
            details: None,
            quota: None,
            caption: None,
            mood: None,
        }
    }

    pub fn with_state(mut self, state: impl Into<String>) -> Self {
        self.state = Some(state.into());
        self
    }

    pub fn ensure_id(&mut self) {
        if self.id.as_ref().map(|s| s.is_empty()).unwrap_or(true) {
            self.id = Some(short_id());
        }
    }

    pub fn validate(&self) -> Result<(), ProtocolError> {
        if !ALLOWED_EVENTS.contains(&self.event.as_str()) {
            return Err(ProtocolError::UnknownEvent);
        }
        if let Some(state) = &self.state {
            if !state.is_empty() && !ALLOWED_STATES.contains(&state.as_str()) {
                return Err(ProtocolError::UnknownState);
            }
        }
        Ok(())
    }

    pub fn sanitized_for_delivery(&self) -> Self {
        let mut sanitized = self.clone();
        sanitized.details = (sanitized.event == "securityEvent")
            .then(|| {
                sanitized.details.and_then(|details| {
                    ALLOWED_SECURITY_TYPES
                        .contains(&details.r#type.as_deref().unwrap_or_default())
                        .then(|| EventDetails {
                            r#type: details.r#type,
                            message: None,
                        })
                })
            })
            .flatten();
        sanitized.caption = sanitized
            .caption
            .and_then(|value| caption::allowed_caption(&value).map(str::to_string));
        sanitized.mood = sanitized
            .mood
            .and_then(|value| caption::allowed_mood(&value).map(str::to_string));
        sanitized
    }

    pub fn to_ble_bytes(&self) -> Result<Vec<u8>, ProtocolError> {
        self.validate()?;
        let sanitized = self.sanitized_for_delivery();
        let json = serde_json::to_vec(&sanitized).map_err(|_| ProtocolError::Serialize)?;
        if json.len() > MAX_PAYLOAD_BYTES {
            return Err(ProtocolError::TooLarge(json.len()));
        }
        Ok(json)
    }
}

pub fn parse_incoming(value: serde_json::Value) -> Result<DeskPetEvent, ProtocolError> {
    let mut event: DeskPetEvent =
        serde_json::from_value(value).map_err(|_| ProtocolError::Malformed)?;
    event.ensure_id();
    if event.source.is_none() {
        event.source = Some("bridge".to_string());
    }
    event.validate()?;
    Ok(event.sanitized_for_delivery())
}

pub fn short_id() -> String {
    uuid::Uuid::new_v4().simple().to_string()[..8].to_string()
}

fn requires_agent_mode(event: &str) -> bool {
    matches!(
        event,
        "stateUpdate" | "attentionRequired" | "attentionCleared"
    )
}

/// Firmware ignores agent events until `sessionStart` moves the pet out of wander mode.
pub fn prepend_session_start_if_needed(
    events: Vec<DeskPetEvent>,
    pet_mode: Option<&str>,
) -> Vec<DeskPetEvent> {
    if pet_mode.is_some_and(|mode| mode != "wander") {
        return events;
    }
    if events.iter().any(|event| event.event == "sessionStart") {
        return events;
    }
    if !events.iter().any(|event| requires_agent_mode(&event.event)) {
        return events;
    }
    let state = events
        .iter()
        .find(|event| event.event == "stateUpdate")
        .and_then(|event| event.state.as_deref())
        .unwrap_or("coding");
    let mut session_start = DeskPetEvent::new("sessionStart").with_state(state);
    session_start.caption = events.iter().find_map(|event| event.caption.clone());
    session_start.mood = events.iter().find_map(|event| event.mood.clone());
    session_start.ensure_id();
    let mut expanded = vec![session_start];
    expanded.extend(events);
    expanded
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_start_fits_mtu_budget() {
        let mut event = DeskPetEvent::new("sessionStart").with_state("coding");
        event.source = Some("cursor".to_string());
        event.ensure_id();
        let bytes = event.to_ble_bytes().expect("serialize");
        assert!(bytes.len() <= MAX_PAYLOAD_BYTES);
        let json = String::from_utf8(bytes).unwrap();
        assert!(json.contains("sessionStart"));
        assert!(json.contains("coding"));
    }

    #[test]
    fn rejects_unknown_event() {
        let event = DeskPetEvent::new("dropTable");
        assert!(matches!(event.validate(), Err(ProtocolError::UnknownEvent)));
    }

    #[test]
    fn rejects_unknown_state() {
        let event = DeskPetEvent::new("stateUpdate").with_state("explode");
        assert!(matches!(event.validate(), Err(ProtocolError::UnknownState)));
    }

    #[test]
    fn prepends_session_start_for_wander_agent_events() {
        let events = vec![DeskPetEvent::new("stateUpdate").with_state("thinking")];
        let expanded = prepend_session_start_if_needed(events, Some("wander"));
        assert_eq!(expanded.len(), 2);
        assert_eq!(expanded[0].event, "sessionStart");
        assert_eq!(expanded[0].state.as_deref(), Some("thinking"));
        assert_eq!(expanded[1].event, "stateUpdate");
    }

    #[test]
    fn skips_session_start_when_already_in_agent_mode() {
        let events = vec![DeskPetEvent::new("stateUpdate").with_state("coding")];
        let expanded = prepend_session_start_if_needed(events, Some("agent"));
        assert_eq!(expanded.len(), 1);
    }

    #[test]
    fn ble_payload_drops_detail_messages_but_keeps_allowed_security_type() {
        let event = DeskPetEvent {
            details: Some(EventDetails {
                r#type: Some("ssh_login".to_string()),
                message: Some("SSH login from 203.0.113.1".to_string()),
            }),
            ..DeskPetEvent::new("securityEvent")
        };

        let value: serde_json::Value =
            serde_json::from_slice(&event.to_ble_bytes().expect("serialize")).expect("valid json");
        assert_eq!(value["details"]["type"], "ssh_login");
        assert!(value["details"].get("message").is_none());
    }

    #[test]
    fn celebrate_caption_fits_the_payload_budget_and_bad_labels_are_dropped() {
        let mut event = DeskPetEvent::new("stateUpdate").with_state("celebrate");
        event.source = Some("cursor".to_string());
        event.ts = Some("2026-11-07T21:15:00Z".to_string());
        event.caption = Some("waiting for you".to_string());
        event.mood = Some("happy".to_string());
        event.ensure_id();
        let bytes = event.to_ble_bytes().expect("serialize");
        assert!(bytes.len() <= MAX_PAYLOAD_BYTES, "{}", bytes.len());

        event.caption = Some("/tmp/secret".to_string());
        event.mood = Some("sudo".to_string());
        let text = String::from_utf8(event.to_ble_bytes().expect("serialize")).unwrap();
        assert!(!text.contains("secret"));
        assert!(!text.contains("sudo"));
        assert!(!text.contains("caption"));
        assert!(!text.contains("mood"));
    }

    #[test]
    fn ble_payload_drops_unrecognized_security_type() {
        let event = DeskPetEvent {
            details: Some(EventDetails {
                r#type: Some("raw_credential_dump".to_string()),
                message: None,
            }),
            ..DeskPetEvent::new("securityEvent")
        };

        let value: serde_json::Value =
            serde_json::from_slice(&event.to_ble_bytes().expect("serialize")).expect("valid json");
        assert!(value.get("details").is_none());
    }
}
