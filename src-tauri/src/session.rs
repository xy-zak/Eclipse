//! Client session launch: pair if needed, then spawn Moonlight on Desktop.
//!
//! CLI mapping lives here so a Moonlight flag change does not touch discovery
//! or the dashboard.

use std::process::Child;
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

use crate::pairing::{self, generate_pin};
use crate::sidecar::MOONLIGHT;

const DESKTOP_APP: &str = "Desktop";
const POST_PIN_SETTLE: Duration = Duration::from_secs(3);

pub fn list_args(peer_ip: &str) -> Vec<String> {
    vec!["list".into(), peer_ip.into()]
}

pub fn pair_args(peer_ip: &str, pin: &str) -> Vec<String> {
    vec![
        "pair".into(),
        peer_ip.into(),
        "--pin".into(),
        pin.into(),
    ]
}

pub fn stream_args(peer_ip: &str) -> Vec<String> {
    vec!["stream".into(), peer_ip.into(), DESKTOP_APP.into()]
}

pub fn already_paired(text: &str) -> bool {
    text.to_ascii_lowercase().contains("already paired")
}

fn list_has_desktop(peer_ip: &str) -> bool {
    let Ok(out) = MOONLIGHT.run_args(&list_args(peer_ip)) else {
        return false;
    };
    if !out.status.success() {
        return false;
    }
    let stdout = String::from_utf8_lossy(&out.stdout);
    stdout
        .lines()
        .any(|line| line.trim().eq_ignore_ascii_case(DESKTOP_APP))
}

fn kill_child(child: Option<Child>) {
    if let Some(mut child) = child {
        let _ = child.kill();
        let _ = child.wait();
    }
}

/// Holds Moonlight pair/stream children. One Connect at a time.
pub struct SessionProcess {
    pair: Mutex<Option<Child>>,
    stream: Mutex<Option<Child>>,
}

impl SessionProcess {
    pub fn new() -> Self {
        Self {
            pair: Mutex::new(None),
            stream: Mutex::new(None),
        }
    }

    pub fn stop(&self) {
        let pair = self.pair.lock().unwrap_or_else(|e| e.into_inner()).take();
        kill_child(pair);
        let stream = self.stream.lock().unwrap_or_else(|e| e.into_inner()).take();
        kill_child(stream);
    }

    pub fn connect(&self, peer_ip: &str) -> Result<(), String> {
        let ip = peer_ip.trim();
        if ip.is_empty() {
            return Err("No peer IP.".into());
        }
        if !MOONLIGHT.is_present() {
            return Err("Moonlight sidecar is not built.".into());
        }

        self.stop();

        if !list_has_desktop(ip) {
            self.pair_host(ip)?;
        }

        let child = MOONLIGHT
            .spawn_args(&stream_args(ip))
            .map_err(|err| format!("Failed to start Moonlight stream: {err}"))?;
        *self.stream.lock().unwrap_or_else(|e| e.into_inner()) = Some(child);
        Ok(())
    }

    fn pair_host(&self, ip: &str) -> Result<(), String> {
        let pin = generate_pin();
        let child = MOONLIGHT
            .spawn_args(&pair_args(ip, &pin))
            .map_err(|err| format!("Failed to start Moonlight pair: {err}"))?;
        *self.pair.lock().unwrap_or_else(|e| e.into_inner()) = Some(child);

        let pin_result = pairing::submit_pin_until_accepted(ip, &pin, pairing::client_name());
        if let Err(err) = pin_result {
            self.kill_pair();
            return Err(err.into_message());
        }

        thread::sleep(POST_PIN_SETTLE);
        self.kill_pair();
        Ok(())
    }

    fn kill_pair(&self) {
        let pair = self.pair.lock().unwrap_or_else(|e| e.into_inner()).take();
        kill_child(pair);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn moonlight_binary_is_the_session_engine() {
        assert_eq!(MOONLIGHT.name, "moonlight");
    }

    #[test]
    fn list_pair_stream_args_match_moonlight_cli() {
        assert_eq!(list_args("100.64.0.1"), ["list", "100.64.0.1"]);
        assert_eq!(
            pair_args("100.64.0.1", "1234"),
            ["pair", "100.64.0.1", "--pin", "1234"]
        );
        assert_eq!(
            stream_args("100.64.0.1"),
            ["stream", "100.64.0.1", "Desktop"]
        );
    }

    #[test]
    fn already_paired_detects_moonlight_message() {
        assert!(already_paired("Work-PC is already paired"));
        assert!(!already_paired("Failed to connect"));
    }
}
