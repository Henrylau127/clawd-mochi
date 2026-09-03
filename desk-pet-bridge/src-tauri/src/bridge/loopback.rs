use std::fs;
use std::io::ErrorKind;
use std::net::Ipv4Addr;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::json;

pub const DEFAULT_PORT: u16 = 8765;
pub const HOST: &str = "127.0.0.1";
pub const MIN_PORT: u16 = 1;
pub const MAX_PORT: u16 = u16::MAX;
pub const DEFAULT_PROVIDER: &str = "cursor";

const ALLOWED_PROVIDERS: &[&str] = &["cursor"];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct KnownPet {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PetRoster {
    pub pets: Vec<KnownPet>,
    pub active_pet_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Sidecar {
    port: Option<u16>,
    #[serde(default)]
    provider: Option<String>,
    #[serde(default)]
    active_pet_id: Option<String>,
    #[serde(default)]
    pets: Option<Vec<KnownPet>>,
}

pub fn validate_port(port: u16) -> Result<u16, String> {
    if port < MIN_PORT {
        return Err(format!("Use a port from {MIN_PORT} to {MAX_PORT}."));
    }
    Ok(port)
}

pub fn validate_provider(provider: &str) -> Result<String, String> {
    if ALLOWED_PROVIDERS.contains(&provider) {
        return Ok(provider.to_string());
    }
    Err("Only Cursor hooks are supported right now.".to_string())
}

pub fn port_is_occupied(port: u16) -> bool {
    port_bind_block_reason(port).is_some()
}

pub fn port_bind_block_reason(port: u16) -> Option<String> {
    match std::net::TcpListener::bind((Ipv4Addr::LOCALHOST, port)) {
        Ok(_) => None,
        Err(err) if err.kind() == ErrorKind::AddrInUse => {
            Some(format!("Port {port} is already in use."))
        }
        Err(err) if err.kind() == ErrorKind::PermissionDenied => {
            Some(format!("Port {port} cannot be used (permission denied)."))
        }
        Err(_) => Some(format!("Port {port} is unavailable.")),
    }
}

pub fn reject_if_occupied(port: u16, currently_listening: bool) -> Result<u16, String> {
    let port = validate_port(port)?;
    if currently_listening {
        return Ok(port);
    }
    match port_bind_block_reason(port) {
        None => Ok(port),
        Some(reason) => Err(reason),
    }
}

pub fn resolve_port() -> u16 {
    if let Ok(raw) = std::env::var("DESKPET_BRIDGE_PORT") {
        if let Ok(port) = raw.parse::<u16>() {
            if validate_port(port).is_ok() {
                return port;
            }
        }
    }
    load_saved_port().unwrap_or(DEFAULT_PORT)
}

pub fn sidecar_path() -> PathBuf {
    home_dir().join(".deskpet-bridge.json")
}

pub fn save_port(port: u16) -> Result<(), String> {
    write_sidecar(port, load_saved_provider(), load_roster())
}

pub fn save_provider(provider: &str) -> Result<String, String> {
    let provider = validate_provider(provider)?;
    write_sidecar(
        load_saved_port().unwrap_or(DEFAULT_PORT),
        provider.clone(),
        load_roster(),
    )?;
    Ok(provider)
}

pub fn save_roster(roster: &PetRoster) -> Result<(), String> {
    write_sidecar(
        load_saved_port().unwrap_or(DEFAULT_PORT),
        load_saved_provider(),
        roster.clone(),
    )
}

pub fn load_saved_port() -> Option<u16> {
    load_sidecar()?
        .port
        .filter(|port| validate_port(*port).is_ok())
}

pub fn load_saved_provider() -> String {
    load_sidecar()
        .and_then(|sidecar| sidecar.provider)
        .and_then(|provider| validate_provider(&provider).ok())
        .unwrap_or_else(|| DEFAULT_PROVIDER.to_string())
}

pub fn load_roster() -> PetRoster {
    let Some(sidecar) = load_sidecar() else {
        return PetRoster::default();
    };
    let pets = sidecar.pets.unwrap_or_default();
    let active_pet_id = sidecar
        .active_pet_id
        .filter(|id| pets.iter().any(|pet| pet.id == *id || pet.name == *id));
    PetRoster {
        pets,
        active_pet_id,
    }
}

fn load_sidecar() -> Option<Sidecar> {
    let raw = fs::read_to_string(sidecar_path()).ok()?;
    serde_json::from_str(&raw).ok()
}

fn write_sidecar(port: u16, provider: String, roster: PetRoster) -> Result<(), String> {
    let port = validate_port(port)?;
    let provider = validate_provider(&provider).unwrap_or_else(|_| DEFAULT_PROVIDER.to_string());
    let body = json!({
        "host": HOST,
        "port": port,
        "url": format!("http://{HOST}:{port}/v1/cursor/event"),
        "provider": provider,
        "active_pet_id": roster.active_pet_id,
        "pets": roster.pets,
    });
    fs::write(sidecar_path(), format!("{body}\n")).map_err(|err| err.to_string())
}

fn home_dir() -> PathBuf {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

#[cfg(test)]
mod tests {
    use super::validate_port;

    #[test]
    fn accepts_valid_tcp_ports() {
        assert!(validate_port(0).is_err());
        assert_eq!(validate_port(1).unwrap(), 1);
        assert_eq!(validate_port(80).unwrap(), 80);
        assert_eq!(validate_port(1024).unwrap(), 1024);
        assert_eq!(validate_port(8765).unwrap(), 8765);
        assert_eq!(validate_port(65535).unwrap(), 65535);
    }

    #[test]
    fn accepts_cursor_provider_only() {
        assert_eq!(super::validate_provider("cursor").unwrap(), "cursor");
        assert!(super::validate_provider("codex").is_err());
        assert!(super::validate_provider("claude-code").is_err());
        assert!(super::validate_provider("pi").is_err());
    }

    #[test]
    fn detects_occupied_loopback_port() {
        let held = std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).unwrap();
        let port = held.local_addr().unwrap().port();
        assert!(super::port_is_occupied(port));
        assert!(super::reject_if_occupied(port, false).is_err());
        assert_eq!(super::reject_if_occupied(port, true).unwrap(), port);
    }
}
