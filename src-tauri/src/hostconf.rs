//! Sunshine host config merge, Tailscale CGNAT checks, and Web UI credential helpers.

use std::fs;
use std::net::{IpAddr, Ipv4Addr};
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::sidecar::sunshine_state_path;

pub const SUNSHINE_CONF_TEMPLATE: &str = include_str!("../../packaging/sunshine.conf");
pub const SUNSHINE_UI_USER: &str = "eclipse";

const CONF_KEYS: [(&str, &str); 5] = [
    ("lan_encryption_mode", "2"),
    ("wan_encryption_mode", "2"),
    ("origin_web_ui_allowed", "lan"),
    ("upnp", "disabled"),
    ("system_tray", "disabled"),
];

/// Strip IPv4-mapped IPv6 and brackets so `::ffff:100.64.0.2` classifies as CGNAT.
pub fn normalize_ip(ip: &str) -> String {
    let ip = ip.trim();
    if let Some(v4) = ip.strip_prefix("::ffff:") {
        return v4.to_string();
    }
    if let Some(rest) = ip.strip_prefix("[::ffff:") {
        return rest.trim_end_matches(']').to_string();
    }
    ip.trim_matches(|c| c == '[' || c == ']').to_string()
}

/// Tailscale CGNAT: 100.64.0.0/10 (100.64.0.0–100.127.255.255).
pub fn is_tailscale_cgnat(ip: &str) -> bool {
    let ip = normalize_ip(ip);
    let Ok(addr) = ip.parse::<Ipv4Addr>() else {
        return false;
    };
    let oct = addr.octets();
    oct[0] == 100 && (oct[1] & 0xC0) == 0x40
}

pub fn is_loopback_ip(ip: &str) -> bool {
    let ip = normalize_ip(ip);
    match ip.parse::<IpAddr>() {
        Ok(IpAddr::V4(v4)) => v4.is_loopback(),
        Ok(IpAddr::V6(v6)) => v6.is_loopback(),
        Err(_) => ip == "localhost",
    }
}

/// Connect, latency, and remote Web UI calls: loopback (smoke) or Tailscale CGNAT.
pub fn is_allowed_peer_ip(ip: &str) -> bool {
    is_loopback_ip(ip) || is_tailscale_cgnat(ip)
}

pub fn peer_ip_rejected_message(ip: &str) -> String {
    format!(
        "Refusing {ip}: Eclipse only connects to Tailscale CGNAT addresses (100.64.0.0/10)."
    )
}

pub fn sunshine_conf_path() -> PathBuf {
    sunshine_state_path().with_file_name("sunshine.conf")
}

pub fn web_ui_password_path() -> PathBuf {
    sunshine_state_path()
        .parent()
        .map(|p| p.join("web-ui-password"))
        .unwrap_or_else(|| PathBuf::from("web-ui-password"))
}

/// Upsert encryption/UPnP keys without removing capture or other user settings.
pub fn merge_sunshine_conf(existing: &str) -> String {
    let mut lines: Vec<String> = if existing.trim().is_empty() {
        SUNSHINE_CONF_TEMPLATE
            .lines()
            .map(str::to_string)
            .collect()
    } else {
        existing.lines().map(str::to_string).collect()
    };
    for (key, val) in CONF_KEYS {
        upsert_conf_line(&mut lines, key, val);
    }
    let mut out = lines.join("\n");
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

fn upsert_conf_line(lines: &mut Vec<String>, key: &str, val: &str) {
    let replacement = format!("{key} = {val}");
    if let Some(i) = lines.iter().position(|l| conf_line_key(l) == Some(key)) {
        lines[i] = replacement;
    } else {
        lines.push(replacement);
    }
}

fn conf_line_key(line: &str) -> Option<&str> {
    let t = line.trim_start();
    if t.is_empty() || t.starts_with('#') {
        return None;
    }
    t.split_once('=').map(|(k, _)| k.trim())
}

pub fn ensure_sunshine_conf() {
    let path = sunshine_conf_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let existing = fs::read_to_string(&path).unwrap_or_default();
    let merged = merge_sunshine_conf(&existing);
    if merged != existing {
        if let Err(err) = fs::write(&path, merged) {
            eprintln!("Eclipse: could not write {}: {err}", path.display());
        }
    }
}

pub fn random_ui_password() -> String {
    const CHARS: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz23456789";
    let mut bytes = [0u8; 20];
    getrandom::getrandom(&mut bytes).expect("CSPRNG for Web UI password");
    bytes
        .iter()
        .map(|b| CHARS[*b as usize % CHARS.len()] as char)
        .collect()
}

/// Sunshine stores SHA-256(password||salt) as uppercase hex with digest bytes reversed.
pub fn sunshine_password_hex(password: &str, salt: &str) -> String {
    let digest = Sha256::digest(format!("{password}{salt}").as_bytes());
    digest.iter().rev().map(|b| format!("{b:02X}")).collect()
}

pub fn sunshine_creds_are_legacy_eclipse(path: &Path) -> bool {
    let Ok(text) = fs::read_to_string(path) else {
        return false;
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else {
        return false;
    };
    let username = value.get("username").and_then(|v| v.as_str()).unwrap_or("");
    if !username.eq_ignore_ascii_case(SUNSHINE_UI_USER) {
        return false;
    }
    let salt = value.get("salt").and_then(|v| v.as_str()).unwrap_or("");
    let stored = value.get("password").and_then(|v| v.as_str()).unwrap_or("");
    if salt.is_empty() || stored.is_empty() {
        return false;
    }
    stored.eq_ignore_ascii_case(&sunshine_password_hex("eclipse", salt))
}

pub fn write_web_ui_password_file(password: &str) {
    let path = web_ui_password_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if fs::write(&path, format!("{password}\n")).is_ok() {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
        }
    }
}

pub fn read_web_ui_password() -> Result<String, String> {
    let path = web_ui_password_path();
    let text = fs::read_to_string(&path).map_err(|_| {
        format!(
            "No Web UI password file yet. Start Host streaming once, then try again ({})",
            path.display()
        )
    })?;
    let pass = text.trim();
    if pass.is_empty() {
        return Err("Web UI password file is empty.".into());
    }
    Ok(pass.to_string())
}

pub fn host_certs_path() -> PathBuf {
    if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
        return PathBuf::from(xdg).join("eclipse").join("host-certs.json");
    }
    if let Some(home) = std::env::var_os("HOME") {
        return PathBuf::from(home)
            .join(".config")
            .join("eclipse")
            .join("host-certs.json");
    }
    #[cfg(windows)]
    {
        if let Some(appdata) = std::env::var_os("APPDATA") {
            return PathBuf::from(appdata).join("Eclipse").join("host-certs.json");
        }
    }
    std::env::temp_dir().join("eclipse-host-certs.json")
}

pub fn load_host_cert_pin(host: &str) -> Option<String> {
    let text = fs::read_to_string(host_certs_path()).ok()?;
    let value: serde_json::Value = serde_json::from_str(&text).ok()?;
    value
        .get(host)
        .and_then(|v| v.as_str())
        .map(str::to_string)
}

pub fn store_host_cert_pin(host: &str, pin: &str) {
    let path = host_certs_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let mut map = fs::read_to_string(&path)
        .ok()
        .and_then(|t| serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(&t).ok())
        .unwrap_or_default();
    map.insert(host.to_string(), serde_json::Value::String(pin.to_string()));
    let _ = fs::write(path, serde_json::Value::Object(map).to_string());
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cgnat_is_100_64_through_127() {
        assert!(is_tailscale_cgnat("100.64.0.1"));
        assert!(is_tailscale_cgnat("100.127.255.254"));
        assert!(is_tailscale_cgnat("::ffff:100.64.0.2"));
        assert!(is_tailscale_cgnat("[::ffff:100.64.0.2]"));
        assert!(!is_tailscale_cgnat("100.63.255.255"));
        assert!(!is_tailscale_cgnat("100.128.0.1"));
        assert!(!is_tailscale_cgnat("192.168.1.1"));
        assert!(!is_tailscale_cgnat("10.0.0.1"));
        assert!(!is_tailscale_cgnat("127.0.0.1"));
    }

    #[test]
    fn loopback_and_allowed_peer() {
        assert!(is_loopback_ip("127.0.0.1"));
        assert!(is_loopback_ip("::1"));
        assert!(is_allowed_peer_ip("127.0.0.1"));
        assert!(is_allowed_peer_ip("100.64.0.9"));
        assert!(!is_allowed_peer_ip("192.168.0.5"));
        assert!(!is_allowed_peer_ip(""));
        assert!(peer_ip_rejected_message("8.8.8.8").contains("100.64.0.0/10"));
    }

    #[test]
    fn merge_upserts_keys_and_keeps_capture() {
        let existing = "capture = kms\nlan_encryption_mode = 0\nsystem_tray = enabled\n";
        let merged = merge_sunshine_conf(existing);
        assert!(merged.contains("capture = kms"));
        assert!(merged.contains("lan_encryption_mode = 2"));
        assert!(merged.contains("wan_encryption_mode = 2"));
        assert!(merged.contains("origin_web_ui_allowed = lan"));
        assert!(merged.contains("upnp = disabled"));
        assert!(merged.contains("system_tray = disabled"));
        assert!(!merged.contains("system_tray = enabled"));
        assert!(!merged.contains("lan_encryption_mode = 0"));
    }

    #[test]
    fn merge_empty_uses_template() {
        let merged = merge_sunshine_conf("");
        assert!(merged.contains("lan_encryption_mode = 2"));
        assert!(merged.contains("upnp = disabled"));
        assert!(merged.contains("system_tray = disabled"));
    }

    #[test]
    fn random_ui_password_is_not_shared_secret() {
        let p = random_ui_password();
        assert_eq!(p.len(), 20);
        assert_ne!(p, "eclipse");
        assert_ne!(p, random_ui_password());
    }

    #[test]
    fn sunshine_password_hex_reverses_sha256_bytes() {
        // SHA-256("") then reversed uppercase hex.
        let empty = sunshine_password_hex("", "");
        assert_eq!(empty.len(), 64);
        assert!(empty.chars().all(|c| c.is_ascii_hexdigit()));
        assert_eq!(empty, empty.to_uppercase());
        assert_ne!(
            empty,
            "E3B0C44298FC1C149AFBF4C8996FB92427AE41E4649B934CA495991B7852B855"
        );
    }
}
