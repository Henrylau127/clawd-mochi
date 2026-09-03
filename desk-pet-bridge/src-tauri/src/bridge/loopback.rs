use std::fs;
use std::io::ErrorKind;
use std::net::Ipv4Addr;
use std::path::PathBuf;

use serde::Deserialize;
use serde_json::json;

pub const DEFAULT_PORT: u16 = 8765;
pub const HOST: &str = "127.0.0.1";
pub const MIN_PORT: u16 = 1;
pub const MAX_PORT: u16 = u16::MAX;

#[derive(Debug, Deserialize)]
struct Sidecar {
    port: Option<u16>,
}

pub fn validate_port(port: u16) -> Result<u16, String> {
    if port < MIN_PORT {
        return Err(format!("Use a port from {MIN_PORT} to {MAX_PORT}."));
    }
    Ok(port)
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
    fn accepts_valid_tcp_ports() {
        assert!(validate_port(0).is_err());
        assert_eq!(validate_port(1).unwrap(), 1);
        assert_eq!(validate_port(80).unwrap(), 80);
        assert_eq!(validate_port(1024).unwrap(), 1024);
        assert_eq!(validate_port(8765).unwrap(), 8765);
        assert_eq!(validate_port(65535).unwrap(), 65535);
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
