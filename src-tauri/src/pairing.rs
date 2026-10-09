//! Automated GameStream PIN pairing and local host session control.
//!
//! Moonlight starts the handshake (`moonlight pair --pin`); this module POSTs
//! the same PIN to the remote Sunshine Web UI at `/api/pin`. Inbound stream
//! status is `GET /api/sessions` on localhost. HTTP details stay here so an
//! API change does not leak into React.

#[cfg(test)]
use std::thread;
use std::time::Duration;
#[cfg(test)]
use std::time::Instant;

use crate::display::{parse_host_identity, DisplayInfo, HostInventory};
use crate::hostconf::{self, is_allowed_peer_ip};
use crate::sidecar::{probe_tcp, SUNSHINE};

const PIN_RETRY: Duration = Duration::from_secs(45);
const PIN_INTERVAL: Duration = Duration::from_millis(300);
const PIN_REQUEST_TIMEOUT: Duration = Duration::from_secs(5);
const RTSP_PORT: u16 = 48010;
/// GameStream HTTP. Moonlight pair/list use this, not the Sunshine Web UI (47990).
pub const GAMESTREAM_HTTP_PORT: u16 = 47989;

const CLIENT_NAME: &str = "Eclipse";

/// Stable prefix so the UI can offer “Trust this Host” instead of a file-delete hint.
pub const TLS_CERT_CHANGED_PREFIX: &str = "ECLIPSE_TLS_CERT_CHANGED:";

#[derive(Debug, PartialEq, Eq)]
pub enum PinError {
    Unauthorized,
    Unconfigured,
    TlsCertChanged(String),
    Other(String),
}

impl PinError {
    pub fn into_message(self) -> String {
        match self {
            PinError::Unauthorized => {
                "Host Sunshine rejected pairing (HTTP 401). Rebuild that PC with this Eclipse version so pairing APIs allow Tailscale CGNAT, or check the host is on the tailnet."
                    .into()
            }
            PinError::Unconfigured => {
                "Host Sunshine has no Web UI credentials. Open Eclipse on that PC first.".into()
            }
            PinError::TlsCertChanged(host) => format!("{TLS_CERT_CHANGED_PREFIX}{host}"),
            PinError::Other(msg) => msg,
        }
    }

    pub fn is_fatal(&self) -> bool {
        matches!(
            self,
            PinError::Unauthorized | PinError::Unconfigured | PinError::TlsCertChanged(_)
        )
    }
}

pub fn pin_url(host: &str) -> String {
    format!("https://{host}:{}/api/pin", sunshine_ui_port())
}

#[cfg(test)]
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

#[cfg(test)]
fn latest_sunshine_log(log: &str) -> &str {
    log.rfind("Sunshine version:")
        .map(|i| &log[i..])
        .unwrap_or(log)
}

#[cfg(test)]
fn host_rtsp_bind_failed(log: &str) -> bool {
    latest_sunshine_log(log).contains("Couldn't bind RTSP server to port")
}

#[cfg(test)]
fn host_capture_failed(log: &str) -> bool {
    let slice = latest_sunshine_log(log);
    slice.contains("Unable to initialize capture method")
        || slice.contains("Platform failed to initialize")
        || slice.contains("Unable to find display or encoder")
        || slice.contains("Video failed to find working encoder")
}

#[cfg(test)]
fn host_kms_missing_caps(log: &str) -> bool {
    let slice = latest_sunshine_log(log);
    slice.contains("config: 'capture' = kms") && slice.contains("Failed to gain CAP_SYS_ADMIN")
}

/// Fail Connect on capture/encoder problems. An RTSP bind log line alone is
/// not enough: Sunshine often logs that after it already owns :48010.
#[cfg(test)]
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
    let mut bytes = [0u8; 2];
    getrandom::getrandom(&mut bytes).expect("CSPRNG for GameStream PIN");
    let n = u16::from_le_bytes(bytes) % 10_000;
    format!("{n:04}")
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
    if !is_allowed_peer_ip(host) {
        return Err(PinError::Other(hostconf::peer_ip_rejected_message(host)));
    }
    if gamestream_http_reachable(host) {
        Ok(())
    } else {
        Err(PinError::Other(gamestream_unreachable_message(host)))
    }
}

/// Retry `op` until it returns `Ok(true)` or the budget is exhausted.
#[cfg(test)]
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
                if err.is_fatal() {
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
        .tls_info(true)
        .redirect(reqwest::redirect::Policy::none())
        .timeout(PIN_REQUEST_TIMEOUT)
        .build()
        .map_err(|err| PinError::Other(format!("HTTPS client failed: {err}")))
}

fn verify_host_cert(
    host: &str,
    response: &reqwest::blocking::Response,
    replace: bool,
) -> Result<(), PinError> {
    let Some(info) = response.extensions().get::<reqwest::tls::TlsInfo>() else {
        return Err(PinError::Other(format!(
            "No TLS certificate info from {host}."
        )));
    };
    let Some(der) = info.peer_certificate() else {
        return Err(PinError::Other(format!("No TLS certificate from {host}.")));
    };
    let pin = hostconf::sha256_hex(der);
    match hostconf::load_host_cert_pin(host) {
        Some(stored) if stored != pin => {
            if replace {
                hostconf::store_host_cert_pin(host, &pin);
                Ok(())
            } else {
                Err(PinError::TlsCertChanged(host.to_string()))
            }
        }
        Some(_) => Ok(()),
        None => {
            hostconf::store_host_cert_pin(host, &pin);
            Ok(())
        }
    }
}

/// HTTPS TOFU check (GET `/api/displays`). Run before pair so already-paired hosts still prompt.
pub fn ensure_host_tls(host: &str, replace_cert: bool) -> Result<(), PinError> {
    if !is_allowed_peer_ip(host) {
        return Err(PinError::Other(hostconf::peer_ip_rejected_message(host)));
    }
    let client = http_client()?;
    let response = client
        .get(displays_url(host))
        .timeout(PIN_REQUEST_TIMEOUT)
        .send()
        .map_err(|err| PinError::Other(format!("Host HTTPS failed: {err}")))?;
    verify_host_cert(host, &response, replace_cert)
}

/// POST `/api/pin`. `Ok(true)` means Sunshine accepted the PIN.
pub fn submit_pin(host: &str, pin: &str, name: &str) -> Result<bool, PinError> {
    if !is_allowed_peer_ip(host) {
        return Err(PinError::Other(hostconf::peer_ip_rejected_message(host)));
    }
    let client = http_client()?;
    let response = client
        .post(pin_url(host))
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .json(&pin_payload(pin, name))
        .send()
        .map_err(|err| PinError::Other(format!("PIN POST failed: {err}")))?;
    verify_host_cert(host, &response, false)?;

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

/// Probe TCP 48010 so Connect fails before Moonlight if RTSP is down.
pub fn ensure_host_video(host: &str) -> Result<(), PinError> {
    if !is_allowed_peer_ip(host) {
        return Err(PinError::Other(hostconf::peer_ip_rejected_message(host)));
    }
    if !probe_tcp(host, RTSP_PORT, Duration::from_millis(500)) {
        return Err(PinError::Other(
            "Host RTSP port 48010 is not reachable. Stop extra Sunshine processes (distro package or leftover Eclipse) and restart Eclipse.".into(),
        ));
    }
    Ok(())
}

/// GET `/api/displays`. Missing endpoint (old Sunshine) yields `None`.
pub fn fetch_host_inventory(host: &str) -> Option<HostInventory> {
    if !is_allowed_peer_ip(host) {
        return None;
    }
    let client = http_client().ok()?;
    let response = client
        .get(displays_url(host))
        .timeout(Duration::from_secs(2))
        .send()
        .ok()?;
    verify_host_cert(host, &response, false).ok()?;
    if !response.status().is_success() {
        return None;
    }
    let value: serde_json::Value = response.json().ok()?;
    Some(parse_host_identity(&value))
}

/// GET `/api/displays`. Missing endpoint (old Sunshine) yields an empty list.
pub fn fetch_displays(host: &str) -> Option<Vec<DisplayInfo>> {
    fetch_host_inventory(host).map(|h| h.displays)
}

/// POST `/api/apps/close` so the host Desktop app ends with the last client window.
pub fn close_host_app(host: &str) {
    let _ = close_host_app_result(host);
}

/// POST `/api/apps/close` and surface HTTP failures (used by force-disconnect).
pub fn close_host_app_result(host: &str) -> Result<(), String> {
    if !is_allowed_peer_ip(host) {
        return Err(hostconf::peer_ip_rejected_message(host));
    }
    let client = http_client().map_err(|err| err.into_message())?;
    let response = client
        .post(apps_close_url(host))
        .timeout(CLOSE_REQUEST_TIMEOUT)
        .send()
        .map_err(|err| format!("Force disconnect failed: {err}"))?;
    verify_host_cert(host, &response, false).map_err(|err| err.into_message())?;
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

/// Active inbound streams on this machine's Sunshine; None when unreachable.
pub fn local_session_count() -> Option<usize> {
    fetch_session_ips(LOCAL_HOST).map(|ips| ips.len())
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
    if !is_allowed_peer_ip(host) {
        return None;
    }
    let client = http_client().ok()?;
    let response = client
        .get(sessions_url(host))
        .timeout(SESSION_REQUEST_TIMEOUT)
        .send()
        .ok()?;
    verify_host_cert(host, &response, false).ok()?;
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
    fn fetch_displays_rejects_rfc1918() {
        assert!(fetch_displays("192.168.1.1").is_none());
        assert!(fetch_host_inventory("192.168.1.1").is_none());
    }

    #[test]
    fn ensure_host_video_rejects_rfc1918() {
        let err = ensure_host_video("10.0.0.1").unwrap_err();
        match err {
            PinError::Other(msg) => assert!(msg.contains("100.64.0.0/10"), "{msg}"),
            other => panic!("expected Other, got {other:?}"),
        }
    }

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
    fn generate_pin_is_not_unix_nanos_modulo() {
        use std::time::{SystemTime, UNIX_EPOCH};
        let time_pin = format!(
            "{:04}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
                % 10_000
        );
        let pins: Vec<String> = (0..40).map(|_| generate_pin()).collect();
        assert!(
            pins.iter().any(|p| p != &time_pin),
            "PIN matched wall-clock nanos % 10000"
        );
    }

    #[test]
    fn submit_pin_rejects_rfc1918_without_http() {
        let err = submit_pin("192.168.1.1", "1234", "Eclipse").unwrap_err();
        match err {
            PinError::Other(msg) => assert!(msg.contains("100.64.0.0/10"), "{msg}"),
            other => panic!("expected Other, got {other:?}"),
        }
    }

    #[test]
    fn pin_payload_has_no_credentials() {
        let body = pin_payload("1234", "Eclipse");
        assert!(body.get("username").is_none());
        assert!(body.get("password").is_none());
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
    fn tls_cert_changed_message_is_parseable() {
        let msg = PinError::TlsCertChanged("100.64.0.9".into()).into_message();
        assert_eq!(msg, format!("{TLS_CERT_CHANGED_PREFIX}100.64.0.9"));
    }

    #[test]
    fn retry_until_ok_stops_on_tls_cert_changed() {
        let err = retry_until_ok(
            || Err(PinError::TlsCertChanged("100.64.0.9".into())),
            Duration::from_secs(2),
            Duration::from_millis(1),
        )
        .unwrap_err();
        assert_eq!(err, PinError::TlsCertChanged("100.64.0.9".into()));
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
