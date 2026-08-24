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
const PAIR_START_GRACE: Duration = Duration::from_millis(400);
pub const STREAM_WIDTH: u32 = 1920;
pub const STREAM_HEIGHT: u32 = 1080;
pub const STREAM_FPS: u32 = 60;

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
    // Windows Moonlight otherwise uses the local monitor (often 4K HEVC). Linux
    // VAAPI hosts frequently encode audio but never emit video at that size, which
    // Moonlight reports as a UDP 47998 timeout even when the path is open.
    vec![
        "stream".into(),
        "--1080".into(),
        "--fps".into(),
        STREAM_FPS.to_string(),
        "--video-codec".into(),
        "H.264".into(),
        "--no-hdr".into(),
        "--packet-size".into(),
        "1024".into(),
        peer_ip.into(),
        DESKTOP_APP.into(),
    ]
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

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionStatus {
    pub connected: bool,
    pub ip: Option<String>,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
}

/// Holds Moonlight pair/stream children. One Connect at a time.
pub struct SessionProcess {
    pair: Mutex<Option<Child>>,
    stream: Mutex<Option<Child>>,
    connected_ip: Mutex<Option<String>>,
}

impl SessionProcess {
    pub fn new() -> Self {
        Self {
            pair: Mutex::new(None),
            stream: Mutex::new(None),
            connected_ip: Mutex::new(None),
        }
    }

    pub fn stop(&self) {
        let pair = self.pair.lock().unwrap_or_else(|e| e.into_inner()).take();
        kill_child(pair);
        let stream = self.stream.lock().unwrap_or_else(|e| e.into_inner()).take();
        kill_child(stream);
        *self.connected_ip.lock().unwrap_or_else(|e| e.into_inner()) = None;
    }

    pub fn status(&self) -> SessionStatus {
        let connected = self.stream_still_running();
        let ip = if connected {
            self.connected_ip
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clone()
        } else {
            *self.connected_ip.lock().unwrap_or_else(|e| e.into_inner()) = None;
            None
        };
        SessionStatus {
            connected,
            ip,
            width: STREAM_WIDTH,
            height: STREAM_HEIGHT,
            fps: STREAM_FPS,
        }
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

        pairing::ensure_host_video(ip).map_err(|err| err.into_message())?;

        let child = MOONLIGHT
            .spawn_args(&stream_args(ip))
            .map_err(|err| format!("Failed to start Moonlight stream: {err}"))?;
        *self.stream.lock().unwrap_or_else(|e| e.into_inner()) = Some(child);
        *self.connected_ip.lock().unwrap_or_else(|e| e.into_inner()) = Some(ip.to_string());
        Ok(())
    }

    fn pair_host(&self, ip: &str) -> Result<(), String> {
        let pin = generate_pin();
        let child = MOONLIGHT
            .spawn_args(&pair_args(ip, &pin))
            .map_err(|err| format!("Failed to start Moonlight pair: {err}"))?;
        *self.pair.lock().unwrap_or_else(|e| e.into_inner()) = Some(child);

        thread::sleep(PAIR_START_GRACE);
        if !self.pair_still_running() {
            self.kill_pair();
            return Err(
                "Moonlight pair failed to start. On Windows, rebuild sidecars so Qt/SDL/FFmpeg DLLs sit next to moonlight.exe."
                    .into(),
            );
        }

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

    fn pair_still_running(&self) -> bool {
        let mut slot = self.pair.lock().unwrap_or_else(|e| e.into_inner());
        match slot.as_mut() {
            Some(child) => matches!(child.try_wait(), Ok(None)),
            None => false,
        }
    }

    fn stream_still_running(&self) -> bool {
        let mut slot = self.stream.lock().unwrap_or_else(|e| e.into_inner());
        match slot.as_mut() {
            Some(child) => match child.try_wait() {
                Ok(None) => true,
                Ok(Some(_)) => {
                    *slot = None;
                    false
                }
                Err(_) => {
                    *slot = None;
                    false
                }
            },
            None => false,
        }
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
            [
                "stream",
                "--1080",
                "--fps",
                "60",
                "--video-codec",
                "H.264",
                "--no-hdr",
                "--packet-size",
                "1024",
                "100.64.0.1",
                "Desktop"
            ]
        );
    }

    #[test]
    fn already_paired_detects_moonlight_message() {
        assert!(already_paired("Work-PC is already paired"));
        assert!(!already_paired("Failed to connect"));
    }

    #[test]
    fn pair_still_running_is_false_without_child() {
        let session = SessionProcess::new();
        assert!(!session.pair_still_running());
        let status = session.status();
        assert!(!status.connected);
        assert!(status.ip.is_none());
        assert_eq!(status.width, STREAM_WIDTH);
        assert_eq!(status.height, STREAM_HEIGHT);
        assert_eq!(status.fps, STREAM_FPS);
    }
}
