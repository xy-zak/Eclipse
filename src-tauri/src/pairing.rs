//! Automated GameStream PIN pairing and local host session control.
//!
//! Moonlight starts the handshake (`moonlight pair --pin`); this module POSTs
//! the same PIN to the remote Sunshine Web UI at `/api/pin`. Inbound stream
//! status is `GET /api/sessions` on localhost. HTTP details stay here so an
//! API change does not leak into React.

use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::sidecar::{probe_tcp, SUNSHINE, SUNSHINE_UI_PASS, SUNSHINE_UI_USER};

const PIN_RETRY: Duration = Duration::from_secs(45);
const PIN_INTERVAL: Duration = Duration::from_millis(300);
const PIN_REQUEST_TIMEOUT: Duration = Duration::from_secs(5);
const RTSP_PORT: u16 = 48010;
/// GameStream HTTP. Moonlight pair/list use this, not the Sunshine Web UI (47990).
pub const GAMESTREAM_HTTP_PORT: u16 = 47989;

const CLIENT_NAME: &str = "Eclipse";

#[derive(Debug, PartialEq, Eq)]
pub enum PinError {
    Unauthorized,
    Unconfigured,
    Other(String),
}

impl PinError {
    pub fn into_message(self) -> String {
        match self {
            PinError::Unauthorized => {
                "Host Sunshine uses a custom Web UI password. Default eclipse/eclipse was rejected."
                    .into()
            }
            PinError::Unconfigured => {
                "Host Sunshine has no Web UI credentials. Open Eclipse on that PC first.".into()
            }
            PinError::Other(msg) => msg,
        }
    }
}

pub fn pin_url(host: &str) -> String {
    format!("https://{host}:{}/api/pin", sunshine_ui_port())
}

pub fn logs_url(host: &str) -> String {
    format!("https://{host}:{}/api/logs", sunshine_ui_port())
}

pub fn displays_url(host: &str) -> String {
    format!("https://{host}:{}/api/displays", sunshine_ui_port())
}

pub fn sessions_url(host: &str) -> String {
    format!("https://{host}:{}/api/sessions", sunshine_ui_port())
}

pub fn apps_close_url(host: &str) -> String {
    format!("https://{host}:{}/api/apps/close", sunshine_ui_port())
}

const LOCAL_HOST: &str = "127.0.0.1";
const SESSION_REQUEST_TIMEOUT: Duration = Duration::from_millis(400);
const CLOSE_REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

fn sunshine_ui_port() -> u16 {
    SUNSHINE.health().map(|h| h.port).unwrap_or(47990)
}

fn latest_sunshine_log(log: &str) -> &str {
    log.rfind("Sunshine version:")
        .map(|i| &log[i..])
        .unwrap_or(log)
}

#[cfg(test)]
fn host_rtsp_bind_failed(log: &str) -> bool {
    latest_sunshine_log(log).contains("Couldn't bind RTSP server to port")
}

fn host_capture_failed(log: &str) -> bool {
    let slice = latest_sunshine_log(log);
    slice.contains("Unable to initialize capture method")
        || slice.contains("Platform failed to initialize")
        || slice.contains("Unable to find display or encoder")
        || slice.contains("Video failed to find working encoder")
}

fn host_kms_missing_caps(log: &str) -> bool {
    let slice = latest_sunshine_log(log);
    slice.contains("config: 'capture' = kms") && slice.contains("Failed to gain CAP_SYS_ADMIN")
}

/// Fail Connect on capture/encoder problems. An RTSP bind log line alone is
/// not enough: Sunshine often logs that after it already owns :48010.
pub fn host_video_issue(log: &str) -> Option<&'static str> {
    if host_kms_missing_caps(log) {
        Some(
            "Host Sunshine capture is KMS but lacks CAP_SYS_ADMIN. On Linux run ./scripts/linux-host-perms.sh (then restart Eclipse), or ./scripts/linux-host-perms.sh --reset-capture to use auto/wlr. See docs/Video.md.",
        )
    } else if host_capture_failed(log) {
        Some(
            "Host Sunshine could not initialize video capture/encoding. See docs/Video.md. On Linux run ./scripts/linux-host-perms.sh and rebuild sidecars; on Windows run .\\scripts\\windows-firewall.ps1 and confirm GPU drivers.",
        )
    } else {
        None
    }
}

pub fn pin_payload(pin: &str, name: &str) -> serde_json::Value {
    serde_json::json!({ "pin": pin, "name": name })
}

pub fn generate_pin() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{:04}", (nanos % 10_000) as u32)
}

pub fn client_name() -> &'static str {
    CLIENT_NAME
}

/// True when Moonlight can reach GameStream HTTP on the peer.
pub fn gamestream_http_reachable(host: &str) -> bool {
    probe_tcp(host, GAMESTREAM_HTTP_PORT, Duration::from_secs(1))
}

pub fn gamestream_unreachable_message(host: &str) -> String {
    format!(
        "Host {host} does not accept GameStream on TCP {GAMESTREAM_HTTP_PORT}. Pairing cannot start. On the host PC allow TCP 47984-47990 and UDP 47998-48010 (then restart Eclipse there)."
    )
}

/// Fail Connect before spawning Moonlight pair when GameStream HTTP is closed.
pub fn ensure_gamestream_http(host: &str) -> Result<(), PinError> {
    if gamestream_http_reachable(host) {
        Ok(())
    } else {
        Err(PinError::Other(gamestream_unreachable_message(host)))
    }
}

/// Retry `op` until it returns `Ok(true)` or the budget is exhausted.
pub fn retry_until_ok<F>(mut op: F, budget: Duration, interval: Duration) -> Result<(), PinError>
where
    F: FnMut() -> Result<bool, PinError>,
{
    let deadline = Instant::now() + budget;
    let mut last = PinError::Other("PIN was not accepted before timeout.".into());
    loop {
        match op() {
            Ok(true) => return Ok(()),
            Ok(false) => {}
            Err(err) => {
                if matches!(err, PinError::Unauthorized | PinError::Unconfigured) {
                    return Err(err);
                }
                last = err;
            }
        }
        if Instant::now() >= deadline {
            return Err(last);
        }
        thread::sleep(interval);
    }
}

fn http_client() -> Result<reqwest::blocking::Client, PinError> {
    reqwest::blocking::Client::builder()
        .danger_accept_invalid_certs(true)
        .redirect(reqwest::redirect::Policy::none())
        .timeout(PIN_REQUEST_TIMEOUT)
        .build()
        .map_err(|err| PinError::Other(format!("HTTPS client failed: {err}")))
}

/// POST `/api/pin`. `Ok(true)` means Sunshine accepted the PIN.
pub fn submit_pin(host: &str, pin: &str, name: &str) -> Result<bool, PinError> {
    let client = http_client()?;
    let response = client
        .post(pin_url(host))
        .basic_auth(SUNSHINE_UI_USER, Some(SUNSHINE_UI_PASS))
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .json(&pin_payload(pin, name))
        .send()
        .map_err(|err| PinError::Other(format!("PIN POST failed: {err}")))?;

    let status = response.status();
    if status.as_u16() == 401 {
        return Err(PinError::Unauthorized);
    }
    if status.is_redirection() {
        return Err(PinError::Unconfigured);
    }
    if !status.is_success() {
        let body = response.text().unwrap_or_default();
        return Err(PinError::Other(format!(
            "PIN POST returned {status}: {}",
            body.trim()
        )));
    }

    let value: serde_json::Value = response
        .json()
        .map_err(|err| PinError::Other(format!("PIN response was not JSON: {err}")))?;
    Ok(value
        .get("status")
        .and_then(|v| v.as_bool())
        .unwrap_or(false))
}

pub fn pin_retry_budget() -> Duration {
    PIN_RETRY
}

pub fn pin_retry_interval() -> Duration {
    PIN_INTERVAL
}

/// GET `/api/logs`, then probe TCP 48010. Log text can lie about RTSP bind.
pub fn ensure_host_video(host: &str) -> Result<(), PinError> {
    let client = http_client()?;
    let response = client
        .get(logs_url(host))
        .basic_auth(SUNSHINE_UI_USER, Some(SUNSHINE_UI_PASS))
        .send()
        .map_err(|err| PinError::Other(format!("Host log fetch failed: {err}")))?;
    let status = response.status();
    if status.as_u16() == 401 {
        return Err(PinError::Unauthorized);
    }
    if !status.is_success() {
        return Ok(());
    }
    let text = response
        .text()
        .map_err(|err| PinError::Other(format!("Host log fetch was not text: {err}")))?;
    if let Some(msg) = host_video_issue(&text) {
        return Err(PinError::Other(msg.into()));
    }
    if !probe_tcp(host, RTSP_PORT, Duration::from_millis(500)) {
        return Err(PinError::Other(
            "Host RTSP port 48010 is not reachable. Stop extra Sunshine processes (distro package or leftover Eclipse) and restart Eclipse.".into(),
        ));
    }
    Ok(())
}

/// GET `/api/displays`. Missing endpoint (old Sunshine) yields an empty list.
pub fn fetch_displays(host: &str) -> Option<Vec<crate::display::DisplayInfo>> {
    let client = http_client().ok()?;
    let response = client
        .get(displays_url(host))
        .basic_auth(SUNSHINE_UI_USER, Some(SUNSHINE_UI_PASS))
        .timeout(Duration::from_millis(400))
        .send()
        .ok()?;
    if !response.status().is_success() {
        return None;
    }
    let value: serde_json::Value = response.json().ok()?;
    Some(parse_displays_payload(&value))
}

pub fn parse_displays_payload(value: &serde_json::Value) -> Vec<crate::display::DisplayInfo> {
    let Some(arr) = value.get("displays").and_then(|v| v.as_array()) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for (i, item) in arr.iter().enumerate() {
        let width = item.get("width").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
        let height = item.get("height").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
        if width == 0 || height == 0 {
            continue;
        }
        let name = item
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let focused = item
            .get("focused")
            .and_then(|v| v.as_bool())
            .unwrap_or(i == 0);
        out.push(crate::display::DisplayInfo {
            name: if name.is_empty() {
                format!("Display {}", i + 1)
            } else {
                name
            },
            width,
            height,
            focused,
        });
    }
    out
}

/// POST `/api/apps/close` so the host Desktop app ends with the last client window.
pub fn close_host_app(host: &str) {
    let _ = close_host_app_result(host);
}

/// POST `/api/apps/close` and surface HTTP failures (used by force-disconnect).
pub fn close_host_app_result(host: &str) -> Result<(), String> {
    let client = http_client().map_err(|err| err.into_message())?;
    let response = client
        .post(apps_close_url(host))
        .basic_auth(SUNSHINE_UI_USER, Some(SUNSHINE_UI_PASS))
        .timeout(CLOSE_REQUEST_TIMEOUT)
        .send()
        .map_err(|err| format!("Force disconnect failed: {err}"))?;
    let status = response.status();
    if !status.is_success() {
        return Err(format!("Force disconnect returned {status}."));
    }
    Ok(())
}

pub fn normalize_client_ip(ip: &str) -> String {
    let ip = ip.trim();
    if let Some(v4) = ip.strip_prefix("::ffff:") {
        return v4.to_string();
    }
    if let Some(rest) = ip.strip_prefix("[::ffff:") {
        return rest.trim_end_matches(']').to_string();
    }
    ip.to_string()
}

pub fn parse_sessions_payload(value: &serde_json::Value) -> Vec<String> {
    let Some(arr) = value.get("sessions").and_then(|v| v.as_array()) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for item in arr {
        let raw = if let Some(ip) = item.get("ip").and_then(|v| v.as_str()) {
            ip
        } else if let Some(ip) = item.as_str() {
            ip
        } else {
            continue;
        };
        let ip = normalize_client_ip(raw);
        if ip.is_empty() || out.iter().any(|existing| existing == &ip) {
            continue;
        }
        out.push(ip);
    }
    out
}

/// GET `/api/sessions` on this machine's Sunshine. Missing endpoint yields None.
pub fn fetch_local_session_ip() -> Option<String> {
    fetch_session_ips(LOCAL_HOST)?.into_iter().next()
}

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InboundStream {
    pub ip: String,
}

pub fn local_inbound() -> Option<InboundStream> {
    fetch_local_session_ip().map(|ip| InboundStream { ip })
}

fn fetch_session_ips(host: &str) -> Option<Vec<String>> {
    let client = http_client().ok()?;
    let response = client
        .get(sessions_url(host))
        .basic_auth(SUNSHINE_UI_USER, Some(SUNSHINE_UI_PASS))
        .timeout(SESSION_REQUEST_TIMEOUT)
        .send()
        .ok()?;
    if !response.status().is_success() {
        return None;
    }
    let value: serde_json::Value = response.json().ok()?;
    Some(parse_sessions_payload(&value))
}

/// Kick every inbound Moonlight session on this host.
pub fn force_disconnect_inbound() -> Result<(), String> {
    close_host_app_result(LOCAL_HOST)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pin_url_uses_peer_host_and_sunshine_port() {
        assert_eq!(pin_url("100.64.0.2"), "https://100.64.0.2:47990/api/pin");
        assert_eq!(pin_url("127.0.0.1"), "https://127.0.0.1:47990/api/pin");
        assert_eq!(logs_url("100.64.0.2"), "https://100.64.0.2:47990/api/logs");
        assert_eq!(
            displays_url("100.64.0.2"),
            "https://100.64.0.2:47990/api/displays"
        );
        assert_eq!(
            sessions_url("127.0.0.1"),
            "https://127.0.0.1:47990/api/sessions"
        );
        assert_eq!(
            apps_close_url("127.0.0.1"),
            "https://127.0.0.1:47990/api/apps/close"
        );
    }

    #[test]
    fn gamestream_http_port_is_moonlight_serverinfo() {
        assert_eq!(GAMESTREAM_HTTP_PORT, 47989);
        assert!(gamestream_unreachable_message("100.64.0.2").contains("47989"));
        assert_eq!(pin_retry_budget(), Duration::from_secs(45));
    }

    #[test]
    fn parse_sessions_payload_dedupes_and_normalizes() {
        let value = serde_json::json!({
            "status": true,
            "sessions": [
                {"ip": "100.64.0.2"},
                {"ip": "::ffff:100.64.0.2"},
                {"ip": "100.64.0.9"},
                "100.64.0.9"
            ]
        });
        assert_eq!(
            parse_sessions_payload(&value),
            ["100.64.0.2", "100.64.0.9"]
        );
    }

    #[test]
    fn parse_sessions_payload_empty_without_array() {
        assert!(parse_sessions_payload(&serde_json::json!({"status": true})).is_empty());
    }

    #[test]
    fn normalize_client_ip_strips_v4_mapped() {
        assert_eq!(normalize_client_ip("::ffff:100.64.0.2"), "100.64.0.2");
        assert_eq!(normalize_client_ip("[::ffff:100.64.0.2]"), "100.64.0.2");
        assert_eq!(normalize_client_ip(" 100.64.0.2 "), "100.64.0.2");
    }

    #[test]
    fn parse_displays_payload_skips_zero_size() {
        let value = serde_json::json!({
            "status": true,
            "displays": [
                {"index": 0, "name": "eDP-1", "width": 2880, "height": 1920, "focused": true},
                {"index": 1, "name": "HDMI-A-1", "width": 0, "height": 0, "focused": false},
                {"index": 2, "name": "DP-1", "width": 1920, "height": 1080, "focused": false}
            ]
        });
        let displays = parse_displays_payload(&value);
        assert_eq!(displays.len(), 2);
        assert_eq!(displays[0].name, "eDP-1");
        assert_eq!(displays[0].width, 2880);
        assert!(displays[0].focused);
        assert_eq!(displays[1].name, "DP-1");
        assert!(!displays[1].focused);
    }

    #[test]
    fn host_rtsp_bind_failed_uses_latest_sunshine_instance() {
        let log = "\
Sunshine version: old
Couldn't bind RTSP server to port [48010], Address already in use
Sunshine version: new
Info: Found H.264 encoder: h264_vaapi [vaapi]
";
        assert!(!host_rtsp_bind_failed(log));
        let log = "\
Sunshine version: 0.0.0
Info: Found H.264 encoder: h264_vaapi [vaapi]
Fatal: Couldn't bind RTSP server to port [48010], Address already in use
";
        assert!(host_rtsp_bind_failed(log));
    }

    #[test]
    fn host_video_issue_ignores_rtsp_log_when_encoder_came_up() {
        let log = "\
Sunshine version: 0.0.0
Info: Found H.264 encoder: h264_vaapi [vaapi]
Fatal: Couldn't bind RTSP server to port [48010], Address already in use
Info: Starting main loop
";
        assert!(host_rtsp_bind_failed(log));
        assert!(host_video_issue(log).is_none());
    }

    #[test]
    fn host_kms_missing_caps_requires_kms_config_and_setcap_failure() {
        let kms = "\
Sunshine version: 0.0.0
config: 'capture' = kms
Error: Failed to gain CAP_SYS_ADMIN
Error: Unable to initialize capture method
";
        assert!(host_kms_missing_caps(kms));
        assert!(host_capture_failed(kms));
        let msg = host_video_issue(kms).expect("kms without caps is a video issue");
        assert!(msg.contains("linux-host-perms.sh"));

        let wlr = "\
Sunshine version: 0.0.0
Info: Found H.264 encoder: h264_vaapi [vaapi]
";
        assert!(!host_kms_missing_caps(wlr));
        assert!(!host_capture_failed(wlr));
        assert!(host_video_issue(wlr).is_none());
    }

    #[test]
    fn host_capture_failed_ignores_older_sunshine_instance() {
        let log = "\
Sunshine version: old
Unable to initialize capture method
Sunshine version: new
Info: Found H.264 encoder: h264_vaapi [vaapi]
";
        assert!(!host_capture_failed(log));
        assert!(host_video_issue(log).is_none());
    }

    #[test]
    fn pin_payload_uses_sunshine_name_field() {
        let body = pin_payload("1234", "Eclipse");
        assert_eq!(body["pin"], "1234");
        assert_eq!(body["name"], "Eclipse");
        assert!(body.get("hostname").is_none());
    }

    #[test]
    fn generate_pin_is_four_digits() {
        let pin = generate_pin();
        assert_eq!(pin.len(), 4);
        assert!(pin.chars().all(|c| c.is_ascii_digit()));
    }

    #[test]
    fn retry_until_ok_succeeds_on_later_true() {
        let mut n = 0;
        retry_until_ok(
            || {
                n += 1;
                Ok(n >= 3)
            },
            Duration::from_secs(2),
            Duration::from_millis(1),
        )
        .unwrap();
        assert_eq!(n, 3);
    }

    #[test]
    fn retry_until_ok_stops_on_unauthorized() {
        let err = retry_until_ok(
            || Err(PinError::Unauthorized),
            Duration::from_secs(2),
            Duration::from_millis(1),
        )
        .unwrap_err();
        assert_eq!(err, PinError::Unauthorized);
    }
}
