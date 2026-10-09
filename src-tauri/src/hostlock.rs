//! Lock this PC when an inbound stream ends.
//!
//! Runs in the Eclipse tray as the logged-in user and watches Sunshine's
//! loopback `GET /api/sessions`. Sunshine's own app `undo` hooks miss dropped
//! connections and would need root/admin config writes in packaged installs,
//! so the lock lives here instead.

use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant};

use crate::pairing;

const POLL: Duration = Duration::from_secs(2);
/// Eclipse respawns stream windows on display switches and settings changes;
/// those gaps are a few seconds, so only lock after a longer quiet spell.
const IDLE_BEFORE_LOCK: Duration = Duration::from_secs(8);
const SUPPORTED: bool = cfg!(any(windows, target_os = "linux"));

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HostLockStatus {
    pub on: bool,
    pub supported: bool,
    pub last_error: Option<String>,
}

static LAST_ERROR: Mutex<Option<String>> = Mutex::new(None);

/// Debounce over session-count samples: lock once a stream has ended and
/// nothing reconnected within `IDLE_BEFORE_LOCK`.
#[derive(Debug, Default)]
struct Watch {
    had_session: bool,
    zero_since: Option<Instant>,
}

impl Watch {
    /// `count` is `None` when Sunshine is unreachable; that never locks.
    fn observe(&mut self, count: Option<usize>, now: Instant) -> bool {
        match count {
            None => false,
            Some(n) if n > 0 => {
                self.had_session = true;
                self.zero_since = None;
                false
            }
            Some(_) => {
                if !self.had_session {
                    return false;
                }
                let since = *self.zero_since.get_or_insert(now);
                if now.duration_since(since) < IDLE_BEFORE_LOCK {
                    return false;
                }
                self.had_session = false;
                self.zero_since = None;
                true
            }
        }
    }
}

pub fn start_watcher() {
    if !SUPPORTED {
        return;
    }
    thread::spawn(|| {
        let mut watch = Watch::default();
        loop {
            if !load_on() {
                watch = Watch::default();
            } else if watch.observe(pairing::local_session_count(), Instant::now()) {
                record(lock_now());
            }
            thread::sleep(POLL);
        }
    });
}

pub fn status() -> HostLockStatus {
    HostLockStatus {
        on: load_on(),
        supported: SUPPORTED,
        last_error: LAST_ERROR.lock().unwrap_or_else(|e| e.into_inner()).clone(),
    }
}

pub fn set_on(on: bool) -> HostLockStatus {
    save_on(on);
    *LAST_ERROR.lock().unwrap_or_else(|e| e.into_inner()) = None;
    status()
}

/// Lock right now so the user can check their screen locker responds.
pub fn test_lock() -> HostLockStatus {
    record(lock_now());
    status()
}

fn record(result: Result<(), String>) {
    if let Err(err) = &result {
        eprintln!("Eclipse: lock on stream end failed ({err})");
    }
    *LAST_ERROR.lock().unwrap_or_else(|e| e.into_inner()) = result.err();
}

#[cfg(windows)]
fn lock_now() -> Result<(), String> {
    #[link(name = "user32")]
    extern "system" {
        fn LockWorkStation() -> i32;
    }
    // SAFETY: no arguments; fails (returns 0) when not on the interactive desktop.
    if unsafe { LockWorkStation() } != 0 {
        Ok(())
    } else {
        Err("Windows refused to lock this PC.".into())
    }
}

#[cfg(target_os = "linux")]
fn lock_now() -> Result<(), String> {
    use std::process::Command;

    let session = linux_session_id()
        .ok_or_else(|| "No login session found for this user.".to_string())?;
    let status = Command::new("loginctl")
        .args(["lock-session", &session])
        .status()
        .map_err(|err| format!("loginctl failed: {err}"))?;
    if !status.success() {
        return Err(format!("loginctl lock-session {session} failed."));
    }
    // logind only broadcasts Lock; a screen locker must answer it.
    thread::sleep(Duration::from_secs(2));
    let locked = Command::new("loginctl")
        .args(["show-session", &session, "-p", "LockedHint", "--value"])
        .output()
        .map(|out| String::from_utf8_lossy(&out.stdout).trim() == "yes")
        .unwrap_or(false);
    if locked {
        Ok(())
    } else {
        Err("No screen locker answered the lock request (install hypridle + hyprlock, or your desktop's locker).".into())
    }
}

#[cfg(target_os = "linux")]
fn linux_session_id() -> Option<String> {
    if let Some(id) = std::env::var("XDG_SESSION_ID").ok().filter(|id| !id.is_empty()) {
        return Some(id);
    }
    // Tray started outside the session scope (e.g. a systemd user unit):
    // fall back to the user's graphical session.
    let out = std::process::Command::new("loginctl")
        .args(["show-user", &current_uid(), "-p", "Display", "--value"])
        .output()
        .ok()?;
    let id = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (!id.is_empty()).then_some(id)
}

#[cfg(target_os = "linux")]
fn current_uid() -> String {
    extern "C" {
        fn getuid() -> u32;
    }
    // SAFETY: getuid has no preconditions and cannot fail.
    unsafe { getuid() }.to_string()
}

#[cfg(not(any(windows, target_os = "linux")))]
fn lock_now() -> Result<(), String> {
    Err("Locking on stream end is not supported on this OS yet.".into())
}

fn pref_path() -> PathBuf {
    #[cfg(target_os = "macos")]
    {
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home)
                .join("Library")
                .join("Application Support")
                .join("Eclipse")
                .join("host-lock.json");
        }
    }
    if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
        return PathBuf::from(xdg).join("eclipse").join("host-lock.json");
    }
    if let Some(home) = std::env::var_os("HOME") {
        return PathBuf::from(home)
            .join(".config")
            .join("eclipse")
            .join("host-lock.json");
    }
    #[cfg(windows)]
    {
        if let Some(appdata) = std::env::var_os("APPDATA") {
            return PathBuf::from(appdata).join("Eclipse").join("host-lock.json");
        }
    }
    std::env::temp_dir().join("eclipse-host-lock.json")
}

fn load_on() -> bool {
    fs::read_to_string(pref_path())
        .ok()
        .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
        .and_then(|value| value.get("lockOnEnd").and_then(|v| v.as_bool()))
        .unwrap_or(false)
}

fn save_on(on: bool) {
    let path = pref_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::write(path, serde_json::json!({ "lockOnEnd": on }).to_string());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locks_only_after_a_stream_then_quiet_spell() {
        let t0 = Instant::now();
        let mut watch = Watch::default();
        // No stream yet: an idle host never locks.
        assert!(!watch.observe(Some(0), t0));
        assert!(!watch.observe(Some(0), t0 + Duration::from_secs(30)));
        // Stream, then it ends.
        assert!(!watch.observe(Some(1), t0 + Duration::from_secs(31)));
        assert!(!watch.observe(Some(0), t0 + Duration::from_secs(32)));
        assert!(!watch.observe(Some(0), t0 + Duration::from_secs(36)));
        assert!(watch.observe(Some(0), t0 + Duration::from_secs(40)));
        // Locks once per stream.
        assert!(!watch.observe(Some(0), t0 + Duration::from_secs(60)));
    }

    #[test]
    fn short_gaps_and_unreachable_host_do_not_lock() {
        let t0 = Instant::now();
        let mut watch = Watch::default();
        assert!(!watch.observe(Some(2), t0));
        // Window respawn: a few seconds at zero, then back.
        assert!(!watch.observe(Some(0), t0 + Duration::from_secs(1)));
        assert!(!watch.observe(Some(1), t0 + Duration::from_secs(5)));
        // The quiet timer restarts after the reconnect.
        assert!(!watch.observe(Some(0), t0 + Duration::from_secs(6)));
        assert!(!watch.observe(Some(0), t0 + Duration::from_secs(13)));
        // Sunshine down: no decision either way.
        assert!(!watch.observe(None, t0 + Duration::from_secs(30)));
        assert!(watch.observe(Some(0), t0 + Duration::from_secs(31)));
    }
}
