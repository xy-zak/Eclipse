//! Peer discovery via the OS Tailscale CLI.
//!
//! Flow: `tailscale status --json`, skip Self, TCP-probe Sunshine's health port
//! on each peer's Tailscale IPv4. CLI details stay here so the UI never shells out.

use std::collections::HashMap;
use std::process::Command;
use std::time::Duration;

use crate::sidecar::SUNSHINE;

const PROBE_TIMEOUT: Duration = Duration::from_millis(500);

/// A Tailscale peer the dashboard can offer as a Connect target.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Peer {
    pub hostname: String,
    pub ip: String,
    pub host_online: bool,
    pub tailscale_online: bool,
}

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoverySnapshot {
    pub peers: Vec<Peer>,
    pub tailscale_ok: bool,
    pub message: Option<String>,
}

impl DiscoverySnapshot {
    fn fail(message: impl Into<String>) -> Self {
        Self {
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
}

struct Draft {
    id: Option<String>,
    hostname: String,
    ip: String,
    tailscale_online: bool,
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
        })
        .collect();

    DiscoverySnapshot {
        peers,
        tailscale_ok: true,
        message: None,
    }
}

fn draft_from_node(node: NodeJson) -> Option<Draft> {
    let ip = pick_ipv4(node.tailscale_ips.as_deref().unwrap_or(&[]))?;
    let hostname = node
        .host_name
        .filter(|s| !s.is_empty())
        .or_else(|| {
            node.dns_name
                .map(|d| d.trim_end_matches('.').to_string())
                .filter(|s| !s.is_empty())
        })?;
    Some(Draft {
        id: node.id,
        hostname,
        ip,
        tailscale_online: node.online.unwrap_or(false),
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

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = r#"{
      "BackendState": "Running",
      "Self": {
        "ID": "self1",
        "HostName": "omarchy",
        "TailscaleIPs": ["100.64.0.1", "fd7a:115c:a1e0::1"],
        "Online": true
      },
      "Peer": {
        "n1": {
          "ID": "peer1",
          "HostName": "work-pc",
          "DNSName": "work-pc.tailnet.ts.net.",
          "TailscaleIPs": ["100.64.0.2", "fd7a:115c:a1e0::2"],
          "Online": true
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
        let laptop = snap.peers.iter().find(|p| p.hostname == "laptop").unwrap();
        assert_eq!(laptop.ip, "100.64.0.9");
        assert!(!laptop.tailscale_online);
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
}
