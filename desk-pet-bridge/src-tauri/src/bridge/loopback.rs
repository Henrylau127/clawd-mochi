use std::fs;
use std::path::PathBuf;

use serde::Deserialize;
use serde_json::json;

pub const DEFAULT_PORT: u16 = 8765;
pub const HOST: &str = "127.0.0.1";

#[derive(Debug, Deserialize)]
struct Sidecar {
    port: Option<u16>,
}

pub fn validate_port(port: u16) -> Result<u16, String> {
    if port < 1024 {
        return Err("Use a port from 1024 to 65535.".to_string());
    }
    Ok(port)
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
    let port = validate_port(port)?;
    let body = json!({
        "host": HOST,
        "port": port,
        "url": format!("http://{HOST}:{port}/v1/cursor/event"),
    });
    fs::write(sidecar_path(), format!("{body}\n")).map_err(|err| err.to_string())
}

pub fn load_saved_port() -> Option<u16> {
    let raw = fs::read_to_string(sidecar_path()).ok()?;
    let sidecar: Sidecar = serde_json::from_str(&raw).ok()?;
    sidecar.port.filter(|port| validate_port(*port).is_ok())
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
    fn rejects_system_ports() {
        assert!(validate_port(80).is_err());
        assert!(validate_port(1023).is_err());
        assert_eq!(validate_port(1024).unwrap(), 1024);
        assert_eq!(validate_port(8765).unwrap(), 8765);
    }
}
