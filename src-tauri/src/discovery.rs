//! Peer discovery via the OS Tailscale CLI.
//!
//! Flow: `tailscale status --json`, skip Self, TCP-probe Sunshine's health port
//! on each peer's Tailscale IPv4. CLI details stay here so the UI never shells out.

use std::collections::HashMap;
#[cfg(target_os = "linux")]
use std::fs;
use std::net::{IpAddr, UdpSocket};
use std::process::Command;
use std::time::Duration;

use crate::sidecar::{probe_tcp_ms, SUNSHINE};

const PROBE_TIMEOUT: Duration = Duration::from_millis(500);

/// A Tailscale peer the dashboard can offer as a Connect target.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Peer {
    pub hostname: String,
    pub ip: String,
    pub host_online: bool,
    pub tailscale_online: bool,
    pub rx_bytes: Option<u64>,
    pub tx_bytes: Option<u64>,
    pub os: String,
}

/// This machine, as shown in the Client panel.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalNode {
    pub hostname: String,
    pub lan_ip: Option<String>,
    pub tailscale_ip: Option<String>,
    pub tailscale_online: bool,
    pub link_speed_mbps: Option<u32>,
    pub rx_bytes: Option<u64>,
    pub tx_bytes: Option<u64>,
    pub display_width: Option<u32>,
    pub display_height: Option<u32>,
    pub os: String,
}

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoverySnapshot {
    pub local: Option<LocalNode>,
    pub peers: Vec<Peer>,
    pub tailscale_ok: bool,
    pub message: Option<String>,
}

impl DiscoverySnapshot {
    fn fail(message: impl Into<String>) -> Self {
        Self {
            local: Some(local_without_tailscale()),
            peers: Vec::new(),
            tailscale_ok: false,
            message: Some(message.into()),
        }
    }
}

#[derive(Debug, serde::Deserialize)]
struct StatusJson {
    #[serde(rename = "BackendState")]
    backend_state: Option<String>,
    #[serde(rename = "Self")]
    self_node: Option<NodeJson>,
    #[serde(rename = "Peer")]
    peer: Option<HashMap<String, NodeJson>>,
}

#[derive(Debug, serde::Deserialize)]
struct NodeJson {
    #[serde(rename = "ID")]
    id: Option<String>,
    #[serde(rename = "HostName")]
    host_name: Option<String>,
    #[serde(rename = "DNSName")]
    dns_name: Option<String>,
    #[serde(rename = "TailscaleIPs")]
    tailscale_ips: Option<Vec<String>>,
    #[serde(rename = "Online")]
    online: Option<bool>,
    #[serde(rename = "RxBytes")]
    rx_bytes: Option<u64>,
    #[serde(rename = "TxBytes")]
    tx_bytes: Option<u64>,
    #[serde(rename = "OS")]
    os: Option<String>,
}

struct Draft {
    id: Option<String>,
    hostname: String,
    ip: String,
    tailscale_online: bool,
    rx_bytes: Option<u64>,
    tx_bytes: Option<u64>,
    os: String,
}

pub fn list_peers() -> DiscoverySnapshot {
    let output = match Command::new("tailscale").args(["status", "--json"]).output() {
        Ok(out) => out,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return DiscoverySnapshot::fail(
                "Tailscale is not installed. Install it from your OS and run tailscale up.",
            );
        }
        Err(err) => {
            return DiscoverySnapshot::fail(format!("Could not run tailscale: {err}"));
        }
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let msg = stderr.trim();
        return DiscoverySnapshot::fail(if msg.is_empty() {
            "tailscale status --json failed. Log in with tailscale up.".to_string()
        } else {
            msg.to_string()
        });
    }

    let json = match String::from_utf8(output.stdout) {
        Ok(s) => s,
        Err(_) => return DiscoverySnapshot::fail("tailscale status --json was not valid UTF-8."),
    };

    snapshot_from_json(&json, true)
}

pub fn peer_latency_ms(ip: &str) -> Option<u32> {
    let Some(health) = SUNSHINE.health() else {
        return None;
    };
    probe_tcp_ms(ip.trim(), health.port, PROBE_TIMEOUT)
}

fn snapshot_from_json(json: &str, probe: bool) -> DiscoverySnapshot {
    let status: StatusJson = match serde_json::from_str(json) {
        Ok(s) => s,
        Err(err) => {
            return DiscoverySnapshot::fail(format!("Could not parse tailscale status: {err}"));
        }
    };

    let backend = status.backend_state.as_deref().unwrap_or("");
    if backend != "Running" {
        let hint = if backend.is_empty() {
            "Tailscale is not running. Run tailscale up.".to_string()
        } else {
            format!("Tailscale state is {backend}. Run tailscale up.")
        };
        return DiscoverySnapshot::fail(hint);
    }

    let local = status.self_node.as_ref().map(local_from_self);
    let self_id = status.self_node.as_ref().and_then(|n| n.id.clone());
    let mut drafts: Vec<Draft> = status
        .peer
        .unwrap_or_default()
        .into_values()
        .filter_map(draft_from_node)
        .filter(|d| d.id.as_ref() != self_id.as_ref())
        .collect();
    drafts.sort_by(|a, b| a.hostname.cmp(&b.hostname));

    let peers = drafts
        .into_iter()
        .map(|d| Peer {
            host_online: probe && SUNSHINE.probe_at(&d.ip, PROBE_TIMEOUT),
            hostname: d.hostname,
            ip: d.ip,
            tailscale_online: d.tailscale_online,
            rx_bytes: d.rx_bytes,
            tx_bytes: d.tx_bytes,
            os: d.os,
        })
        .collect();

    DiscoverySnapshot {
        local,
        peers,
        tailscale_ok: true,
        message: None,
    }
}

fn local_from_self(node: &NodeJson) -> LocalNode {
    let hostname = node_hostname(node).unwrap_or_else(|| os_hostname());
    let (display_width, display_height) = display_size();
    LocalNode {
        hostname,
        lan_ip: lan_ipv4(),
        tailscale_ip: pick_ipv4(node.tailscale_ips.as_deref().unwrap_or(&[])),
        tailscale_online: node.online.unwrap_or(false),
        link_speed_mbps: link_speed_mbps(),
        rx_bytes: node.rx_bytes,
        tx_bytes: node.tx_bytes,
        display_width,
        display_height,
        os: {
            let from_ts = format_os(node.os.as_deref());
            if from_ts.is_empty() {
                native_os()
            } else {
                from_ts
            }
        },
    }
}

fn local_without_tailscale() -> LocalNode {
    let (display_width, display_height) = display_size();
    LocalNode {
        hostname: os_hostname(),
        lan_ip: lan_ipv4(),
        tailscale_ip: None,
        tailscale_online: false,
        link_speed_mbps: link_speed_mbps(),
        rx_bytes: None,
        tx_bytes: None,
        display_width,
        display_height,
        os: native_os(),
    }
}

fn node_hostname(node: &NodeJson) -> Option<String> {
    node.host_name
        .clone()
        .filter(|s| !s.is_empty())
        .or_else(|| {
            node.dns_name
                .as_ref()
                .map(|d| d.trim_end_matches('.').to_string())
                .filter(|s| !s.is_empty())
        })
}

fn draft_from_node(node: NodeJson) -> Option<Draft> {
    let ip = pick_ipv4(node.tailscale_ips.as_deref().unwrap_or(&[]))?;
    let hostname = node_hostname(&node)?;
    Some(Draft {
        id: node.id,
        hostname,
        ip,
        tailscale_online: node.online.unwrap_or(false),
        rx_bytes: node.rx_bytes,
        tx_bytes: node.tx_bytes,
        os: format_os(node.os.as_deref()),
    })
}

fn pick_ipv4(ips: &[String]) -> Option<String> {
    let v4: Vec<&String> = ips
        .iter()
        .filter(|ip| ip.contains('.') && !ip.contains(':'))
        .collect();
    v4.iter()
        .find(|ip| ip.starts_with("100."))
        .or_else(|| v4.first())
        .map(|s| (*s).clone())
}

fn os_hostname() -> String {
    if let Ok(out) = Command::new("hostname").output() {
        if out.status.success() {
            let name = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !name.is_empty() {
                return name;
            }
        }
    }
    "this PC".into()
}

fn native_os() -> String {
    format_os(Some(std::env::consts::OS))
}

fn format_os(os: Option<&str>) -> String {
    let Some(raw) = os.map(str::trim).filter(|s| !s.is_empty()) else {
        return String::new();
    };
    match raw.to_ascii_lowercase().as_str() {
        "linux" => "Linux".into(),
        "windows" => "Windows".into(),
        "macos" | "darwin" => "macOS".into(),
        "ios" => "iOS".into(),
        "android" => "Android".into(),
        "freebsd" => "FreeBSD".into(),
        other => {
            let mut chars = other.chars();
            match chars.next() {
                Some(first) => format!("{}{}", first.to_uppercase(), chars.as_str()),
                None => String::new(),
            }
        }
    }
}

fn lan_ipv4() -> Option<String> {
    if let Some(ip) = udp_local_ipv4() {
        if is_usable_lan(&ip) {
            return Some(ip);
        }
    }
    parse_host_ipv4()
}

fn is_usable_lan(ip: &str) -> bool {
    !ip.starts_with("100.") && !ip.starts_with("127.") && ip != "0.0.0.0"
}

fn udp_local_ipv4() -> Option<String> {
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("1.1.1.1:80").ok()?;
    match socket.local_addr().ok()?.ip() {
        IpAddr::V4(v4) => Some(v4.to_string()),
        IpAddr::V6(_) => None,
    }
}

fn parse_host_ipv4() -> Option<String> {
    #[cfg(windows)]
    {
        let out = Command::new("ipconfig").output().ok()?;
        if !out.status.success() {
            return None;
        }
        let text = String::from_utf8_lossy(&out.stdout);
        for line in text.lines() {
            let line = line.trim();
            let Some((_, rest)) = line.split_once(':') else {
                continue;
            };
            if !line.to_ascii_lowercase().contains("ipv4") {
                continue;
            }
            let ip = rest.trim();
            if is_usable_lan(ip) {
                return Some(ip.to_string());
            }
        }
        None
    }
    #[cfg(not(windows))]
    {
        let out = Command::new("ip")
            .args(["-4", "-o", "addr", "show"])
            .output()
            .ok()?;
        if !out.status.success() {
            return None;
        }
        let text = String::from_utf8_lossy(&out.stdout);
        for line in text.lines() {
            let cols: Vec<&str> = line.split_whitespace().collect();
            if cols.len() < 4 || cols[2] != "inet" {
                continue;
            }
            let iface = cols[1].trim_end_matches(':');
            if skip_iface(iface) {
                continue;
            }
            let ip = cols[3].split('/').next().unwrap_or("");
            if is_usable_lan(ip) {
                return Some(ip.to_string());
            }
        }
        None
    }
}

#[cfg(not(windows))]
fn skip_iface(iface: &str) -> bool {
    iface == "lo"
        || iface.starts_with("tailscale")
        || iface.starts_with("docker")
        || iface.starts_with("veth")
        || iface.starts_with("br-")
        || iface.starts_with("virbr")
}

fn link_speed_mbps() -> Option<u32> {
    #[cfg(target_os = "linux")]
    {
        let route = fs::read_to_string("/proc/net/route").ok()?;
        let iface = route.lines().skip(1).find_map(|line| {
            let cols: Vec<&str> = line.split_whitespace().collect();
            if cols.len() >= 2 && cols[1] == "00000000" && !skip_iface(cols[0]) {
                Some(cols[0].to_string())
            } else {
                None
            }
        })?;
        let speed = fs::read_to_string(format!("/sys/class/net/{iface}/speed")).ok()?;
        let n: i64 = speed.trim().parse().ok()?;
        if n <= 0 || n > 1_000_000 {
            return None;
        }
        Some(n as u32)
    }
    #[cfg(not(target_os = "linux"))]
    {
        None
    }
}

/// Physical pixel size of the focused (or largest) display.
fn display_size() -> (Option<u32>, Option<u32>) {
    if let Some((w, h)) = os_display_size() {
        (Some(w), Some(h))
    } else {
        (None, None)
    }
}

fn os_display_size() -> Option<(u32, u32)> {
    #[cfg(target_os = "linux")]
    {
        hyprctl_focused_size().or_else(drm_preferred_mode)
    }
    #[cfg(not(target_os = "linux"))]
    {
        None
    }
}

#[cfg(target_os = "linux")]
fn hyprctl_focused_size() -> Option<(u32, u32)> {
    let out = Command::new("hyprctl").args(["monitors", "-j"]).output().ok()?;
    if !out.status.success() {
        return None;
    }
    parse_hyprctl_monitors(&String::from_utf8_lossy(&out.stdout))
}

fn parse_hyprctl_monitors(json: &str) -> Option<(u32, u32)> {
    let monitors: Vec<serde_json::Value> = serde_json::from_str(json).ok()?;
    let focused = monitors
        .iter()
        .find(|m| m.get("focused").and_then(|v| v.as_bool()) == Some(true))
        .or_else(|| monitors.first())?;
    let w = focused.get("width")?.as_u64()? as u32;
    let h = focused.get("height")?.as_u64()? as u32;
    if w == 0 || h == 0 {
        return None;
    }
    Some((w, h))
}

#[cfg(target_os = "linux")]
fn drm_preferred_mode() -> Option<(u32, u32)> {
    let drm = fs::read_dir("/sys/class/drm").ok()?;
    let mut best: Option<(u32, u32)> = None;
    for entry in drm {
        let path = entry.ok()?.path();
        let name = path.file_name()?.to_string_lossy();
        if !name.contains('-') {
            continue;
        }
        let status = fs::read_to_string(path.join("status")).unwrap_or_default();
        if status.trim() != "connected" {
            continue;
        }
        let modes = fs::read_to_string(path.join("modes")).ok()?;
        let first = modes.lines().next()?;
        let (w_s, h_s) = first.split_once('x')?;
        let w: u32 = w_s.parse().ok()?;
        let h: u32 = h_s
            .split(|c: char| !c.is_ascii_digit())
            .next()?
            .parse()
            .ok()?;
        if w == 0 || h == 0 {
            continue;
        }
        if best.map_or(true, |(bw, bh)| w.saturating_mul(h) > bw.saturating_mul(bh)) {
            best = Some((w, h));
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = r#"{
      "BackendState": "Running",
      "Self": {
        "ID": "self1",
        "HostName": "omarchy",
        "OS": "linux",
        "TailscaleIPs": ["100.64.0.1", "fd7a:115c:a1e0::1"],
        "Online": true,
        "RxBytes": 10,
        "TxBytes": 20
      },
      "Peer": {
        "n1": {
          "ID": "peer1",
          "HostName": "work-pc",
          "DNSName": "work-pc.tailnet.ts.net.",
          "OS": "windows",
          "TailscaleIPs": ["100.64.0.2", "fd7a:115c:a1e0::2"],
          "Online": true,
          "RxBytes": 100,
          "TxBytes": 200
        },
        "n2": {
          "ID": "self1",
          "HostName": "omarchy-dup",
          "TailscaleIPs": ["100.64.0.1"],
          "Online": true
        },
        "n3": {
          "ID": "peer2",
          "HostName": "phone",
          "TailscaleIPs": ["fd7a:115c:a1e0::3"],
          "Online": true
        },
        "n4": {
          "ID": "peer3",
          "HostName": "laptop",
          "TailscaleIPs": ["10.0.0.9", "100.64.0.9"],
          "Online": false
        }
      }
    }"#;

    #[test]
    fn parse_skips_self_and_ipv6_only() {
        let snap = snapshot_from_json(FIXTURE, false);
        assert!(snap.tailscale_ok);
        assert!(snap.message.is_none());
        let names: Vec<_> = snap.peers.iter().map(|p| p.hostname.as_str()).collect();
        assert_eq!(names, ["laptop", "work-pc"]);
        let work = snap.peers.iter().find(|p| p.hostname == "work-pc").unwrap();
        assert_eq!(work.ip, "100.64.0.2");
        assert!(work.tailscale_online);
        assert!(!work.host_online);
        assert_eq!(work.rx_bytes, Some(100));
        assert_eq!(work.tx_bytes, Some(200));
        assert_eq!(work.os, "Windows");
        let laptop = snap.peers.iter().find(|p| p.hostname == "laptop").unwrap();
        assert_eq!(laptop.ip, "100.64.0.9");
        assert!(!laptop.tailscale_online);
        let local = snap.local.expect("self node");
        assert_eq!(local.hostname, "omarchy");
        assert_eq!(local.tailscale_ip.as_deref(), Some("100.64.0.1"));
        assert!(local.tailscale_online);
        assert_eq!(local.rx_bytes, Some(10));
        assert_eq!(local.tx_bytes, Some(20));
        assert_eq!(local.os, "Linux");
    }

    #[test]
    fn stopped_backend_is_not_ok() {
        let snap = snapshot_from_json(r#"{"BackendState":"Stopped"}"#, false);
        assert!(!snap.tailscale_ok);
        assert!(snap.peers.is_empty());
        assert!(snap.message.unwrap().contains("Stopped"));
    }

    #[test]
    fn needs_login_is_not_ok() {
        let snap = snapshot_from_json(r#"{"BackendState":"NeedsLogin"}"#, false);
        assert!(!snap.tailscale_ok);
        assert!(snap.message.unwrap().contains("NeedsLogin"));
    }

    #[test]
    fn invalid_json_is_not_ok() {
        let snap = snapshot_from_json("not-json", false);
        assert!(!snap.tailscale_ok);
    }

    #[test]
    fn pick_ipv4_prefers_cgnet() {
        assert_eq!(
            pick_ipv4(&["fd7a::1".into(), "10.0.0.1".into(), "100.1.2.3".into()]).as_deref(),
            Some("100.1.2.3")
        );
        assert_eq!(pick_ipv4(&["fd7a::1".into()]), None);
    }

    #[test]
    fn format_os_normalizes_tailscale_names() {
        assert_eq!(format_os(Some("linux")), "Linux");
        assert_eq!(format_os(Some("windows")), "Windows");
        assert_eq!(format_os(Some("darwin")), "macOS");
        assert_eq!(format_os(None), "");
    }

    #[test]
    fn hyprctl_uses_focused_physical_size() {
        let json = r#"[{"id":0,"name":"HDMI-A-1","width":1920,"height":1080,"focused":false},{"id":1,"name":"eDP-1","width":2880,"height":1920,"focused":true}]"#;
        assert_eq!(parse_hyprctl_monitors(json), Some((2880, 1920)));
    }
}
