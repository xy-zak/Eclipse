//! Client session launch: pair if needed, then spawn Moonlight window(s) on Desktop.
//!
//! A session is the connection, not a single OS window. Ctrl+Alt+Q closes one
//! window; the last window (or Ctrl+Alt+Esc) disconnects.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::process::Child;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use crate::display::{self, DisplayInfo, ScaleOption};
use crate::pairing::{self, generate_pin};
use crate::sidecar::MOONLIGHT;

const DESKTOP_APP: &str = "Desktop";
const POST_PIN_SETTLE: Duration = Duration::from_secs(3);
const PAIR_START_GRACE: Duration = Duration::from_millis(400);
pub const STREAM_FPS: u32 = 60;
const DEFAULT_WIDTH: u32 = 1920;
const DEFAULT_HEIGHT: u32 = 1080;
const DEFAULT_BITRATE_MBPS: u32 = 20;
const DEFAULT_CODEC: &str = "H.264";

#[cfg(test)]
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

pub fn stream_args(
    peer_ip: &str,
    width: u32,
    height: u32,
    display_index: Option<usize>,
    audio: bool,
    prefs: &StreamPrefs,
) -> Vec<String> {
    let mut args = vec![
        "stream".into(),
        "--resolution".into(),
        format!("{width}x{height}"),
        "--fps".into(),
        prefs.fps.to_string(),
        "--video-codec".into(),
        prefs.codec.clone(),
        "--bitrate".into(),
        (prefs.bitrate_mbps * 1000).to_string(),
        "--no-hdr".into(),
        "--packet-size".into(),
        "1024".into(),
        "--absolute-mouse".into(),
        "--display-mode".into(),
        "windowed".into(),
    ];
    if let Some(index) = display_index {
        args.push("--host-display".into());
        args.push(index.to_string());
    }
    if !audio {
        args.push("--mute-on-focus-loss".into());
    }
    args.push(peer_ip.into());
    args.push(DESKTOP_APP.into());
    args
}

pub fn already_paired(text: &str) -> bool {
    text.to_ascii_lowercase().contains("already paired")
}

struct PairProc {
    child: Child,
    log: Arc<Mutex<String>>,
}

fn drain_pipe(mut pipe: impl Read + Send + 'static, log: Arc<Mutex<String>>) {
    thread::spawn(move || {
        let mut buf = [0u8; 4096];
        loop {
            match pipe.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    if let Ok(mut text) = log.lock() {
                        text.push_str(&String::from_utf8_lossy(&buf[..n]));
                    }
                }
            }
        }
    });
}

fn attach_log_drain(child: &mut Child) -> Arc<Mutex<String>> {
    let log = Arc::new(Mutex::new(String::new()));
    if let Some(out) = child.stdout.take() {
        drain_pipe(out, Arc::clone(&log));
    }
    if let Some(err) = child.stderr.take() {
        drain_pipe(err, Arc::clone(&log));
    }
    log
}

fn snapshot_log(log: &Arc<Mutex<String>>) -> String {
    log.lock().map(|text| text.clone()).unwrap_or_default()
}

fn kill_child(child: Option<Child>) {
    if let Some(mut child) = child {
        let _ = child.kill();
        let _ = child.wait();
    }
}

fn is_engine_log_line(line: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    lower.contains("sdl info")
        || lower.contains("sdl debug")
        || lower.contains("compiled with sdl")
        || lower.contains(" - sdl ")
        || (lower.contains("ffmpeg") && (lower.contains(" info") || lower.contains(" debug")))
}

fn moonlight_user_message(output: &str) -> Option<String> {
    output
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !is_engine_log_line(line))
        .last()
        .map(str::to_string)
}

fn pair_succeeded(ok: bool, output: &str) -> bool {
    ok || already_paired(output) || output.to_ascii_lowercase().contains("pairing completed")
}

fn pair_user_error(output: &str, fallback: &str) -> String {
    moonlight_user_message(output).unwrap_or_else(|| fallback.to_string())
}

fn pair_process_error(output: &str) -> String {
    pair_user_error(
        output,
        "Moonlight pair failed to start. On Windows, rebuild sidecars so Qt/SDL/FFmpeg DLLs sit next to moonlight.exe.",
    )
}

fn pair_timeout_message(output: &str) -> String {
    pair_user_error(
        output,
        "PIN was not accepted before timeout. GameStream on the host may have stalled after Moonlight connected.",
    )
}

#[derive(Debug, PartialEq, Eq)]
enum ControlCmd {
    DisconnectAll,
    OpenAll,
    SwitchDisplay { index: usize, pid: Option<u32> },
    Ignore,
}

fn parse_control_line(cmd: &str) -> ControlCmd {
    let cmd = cmd.trim();
    if cmd == "disconnect-all" || cmd == "esc" {
        return ControlCmd::DisconnectAll;
    }
    if cmd == "open-all" || cmd == "0" {
        return ControlCmd::OpenAll;
    }
    if let Some(rest) = cmd.strip_prefix("switch-display ") {
        let mut parts = rest.split_whitespace();
        let Some(n) = parts.next().and_then(|s| s.parse::<usize>().ok()) else {
            return ControlCmd::Ignore;
        };
        let pid = parts.next().and_then(|s| s.parse::<u32>().ok());
        return ControlCmd::SwitchDisplay {
            index: n.saturating_sub(1),
            pid,
        };
    }
    ControlCmd::Ignore
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamPrefs {
    pub scale: u32,
    pub fps: u32,
    pub codec: String,
    pub bitrate_mbps: u32,
}

impl Default for StreamPrefs {
    fn default() -> Self {
        Self {
            scale: 1,
            fps: STREAM_FPS,
            codec: DEFAULT_CODEC.into(),
            bitrate_mbps: DEFAULT_BITRATE_MBPS,
        }
    }
}

fn validate_prefs(prefs: StreamPrefs) -> Result<StreamPrefs, String> {
    if !matches!(prefs.scale, 0 | 1 | 2 | 4) {
        return Err("Stream scale must be Quarter, Half, or Native.".into());
    }
    if !matches!(prefs.fps, 30 | 60) {
        return Err("FPS must be 30 or 60.".into());
    }
    if !matches!(prefs.codec.as_str(), "H.264" | "HEVC" | "AV1") {
        return Err("Codec must be H.264, HEVC, or AV1.".into());
    }
    if !matches!(prefs.bitrate_mbps, 10 | 20 | 50) {
        return Err("Max bandwidth must be 10, 20, or 50 Mb/s.".into());
    }
    Ok(prefs)
}

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamWindowStatus {
    pub display_index: u32,
    pub name: String,
    pub native_width: u32,
    pub native_height: u32,
    pub stream_width: u32,
    pub stream_height: u32,
    pub window_open: bool,
}

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionStatus {
    pub connected: bool,
    pub ip: Option<String>,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub scale: u32,
    pub codec: String,
    pub bitrate_mbps: u32,
    pub displays: Vec<StreamWindowStatus>,
    pub scale_options: Vec<ScaleOption>,
}

struct StreamChild {
    child: Child,
    display_index: usize,
    audio: bool,
}

struct SessionInner {
    pair: Mutex<Option<PairProc>>,
    windows: Mutex<Vec<StreamChild>>,
    connected_ip: Mutex<Option<String>>,
    host_displays: Mutex<Vec<DisplayInfo>>,
    prefs: Mutex<StreamPrefs>,
    control_port: Mutex<Option<u16>>,
}

/// Holds Moonlight pair/stream children. One Connect at a time; N stream windows.
pub struct SessionProcess {
    inner: Arc<SessionInner>,
}

impl SessionProcess {
    pub fn new() -> Self {
        let inner = Arc::new(SessionInner {
            pair: Mutex::new(None),
            windows: Mutex::new(Vec::new()),
            connected_ip: Mutex::new(None),
            host_displays: Mutex::new(Vec::new()),
            prefs: Mutex::new(load_prefs()),
            control_port: Mutex::new(None),
        });
        start_control_listener(Arc::clone(&inner));
        Self { inner }
    }

    pub fn stop(&self) {
        self.inner.stop();
    }

    pub fn status(&self) -> SessionStatus {
        self.inner.status()
    }

    pub fn set_prefs(&self, prefs: StreamPrefs) -> Result<(), String> {
        self.inner.set_prefs(prefs)
    }

    pub fn open_all_displays(&self) -> Result<(), String> {
        self.inner.open_all_displays()
    }

    pub fn connect(&self, peer_ip: &str, trust_new_cert: bool) -> Result<(), String> {
        self.inner.connect(peer_ip, trust_new_cert)
    }
}

impl SessionInner {
    fn stop(&self) {
        self.kill_pair();
        let mut windows = self.windows.lock().unwrap_or_else(|e| e.into_inner());
        for slot in windows.drain(..) {
            kill_child(Some(slot.child));
        }
        let ip = self
            .connected_ip
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take();
        if let Some(ip) = ip {
            pairing::close_host_app(&ip);
        }
    }

    fn reap_windows(&self) {
        let mut windows = self.windows.lock().unwrap_or_else(|e| e.into_inner());
        windows.retain_mut(|slot| match slot.child.try_wait() {
            Ok(None) => true,
            _ => false,
        });
        if windows.is_empty() {
            if let Some(ip) = self
                .connected_ip
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .take()
            {
                pairing::close_host_app(&ip);
            }
        }
    }

    fn status(&self) -> SessionStatus {
        self.reap_windows();
        let windows = self.windows.lock().unwrap_or_else(|e| e.into_inner());
        let connected = !windows.is_empty();
        let ip = if connected {
            self.connected_ip
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clone()
        } else {
            None
        };
        let host = self.host_displays.lock().unwrap_or_else(|e| e.into_inner());
        let prefs = self.prefs.lock().unwrap_or_else(|e| e.into_inner()).clone();
        let scale = prefs.scale;
        let open_indices: Vec<usize> = windows.iter().map(|w| w.display_index).collect();
        drop(windows);

        let mut displays = Vec::new();
        let mut width = DEFAULT_WIDTH;
        let mut height = DEFAULT_HEIGHT;
        let mut options = Vec::new();
        for (i, host_d) in host.iter().enumerate() {
            let (sw, sh) = display::apply_scale(host_d.width, host_d.height, scale);
            if i == 0 || open_indices.contains(&i) {
                width = sw;
                height = sh;
            }
            if i == 0 {
                options = display::scale_options(host_d.width, host_d.height);
            }
            displays.push(StreamWindowStatus {
                display_index: i as u32,
                name: host_d.name.clone(),
                native_width: host_d.width,
                native_height: host_d.height,
                stream_width: sw,
                stream_height: sh,
                window_open: open_indices.contains(&i),
            });
        }
        if displays.is_empty() {
            options = display::scale_options(DEFAULT_WIDTH, DEFAULT_HEIGHT);
        }
        SessionStatus {
            connected,
            ip,
            width,
            height,
            fps: prefs.fps,
            scale,
            codec: prefs.codec,
            bitrate_mbps: prefs.bitrate_mbps,
            displays,
            scale_options: options,
        }
    }

    fn set_prefs(&self, prefs: StreamPrefs) -> Result<(), String> {
        let prefs = validate_prefs(prefs)?;
        *self.prefs.lock().unwrap_or_else(|e| e.into_inner()) = prefs.clone();
        save_prefs(&prefs);
        let ip = self
            .connected_ip
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        let Some(ip) = ip else {
            return Ok(());
        };
        let indices: Vec<(usize, bool)> = self
            .windows
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .map(|w| (w.display_index, w.audio))
            .collect();
        if indices.is_empty() {
            return Ok(());
        }
        for (index, audio) in indices {
            self.respawn_window(&ip, index, audio)?;
        }
        Ok(())
    }

    fn open_all_displays(&self) -> Result<(), String> {
        let ip = self
            .connected_ip
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
            .ok_or_else(|| "Not connected.".to_string())?;
        self.refresh_host_displays(&ip);
        let mut count = self
            .host_displays
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .len()
            .max(1);
        const MAX_DISPLAY_WINDOWS: usize = 4;
        if count > MAX_DISPLAY_WINDOWS {
            eprintln!(
                "Eclipse: opening first {MAX_DISPLAY_WINDOWS} of {count} host displays (GPU encode limit)"
            );
            count = MAX_DISPLAY_WINDOWS;
        }
        let open: Vec<usize> = self
            .windows
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .map(|w| w.display_index)
            .collect();
        for index in 0..count {
            if !open.contains(&index) {
                self.spawn_window(&ip, index, index == 0)?;
            }
        }
        Ok(())
    }

    fn switch_display(&self, index: usize, pid: Option<u32>) -> Result<(), String> {
        let ip = self
            .connected_ip
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
            .ok_or_else(|| "Not connected.".to_string())?;
        self.refresh_host_displays(&ip);
        let count = self
            .host_displays
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .len()
            .max(1);
        if index >= count {
            return Ok(());
        }
        let retarget = {
            let mut windows = self.windows.lock().unwrap_or_else(|e| e.into_inner());
            let pos = if let Some(pid) = pid {
                windows.iter().position(|w| w.child.id() == pid)
            } else {
                windows.len().checked_sub(1)
            };
            pos.map(|p| windows.remove(p))
        };
        let Some(slot) = retarget else {
            return Ok(());
        };
        if slot.display_index == index {
            self.windows
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .push(slot);
            return Ok(());
        }
        kill_child(Some(slot.child));
        self.spawn_window(&ip, index, slot.audio)?;
        Ok(())
    }

    fn connect(&self, peer_ip: &str, trust_new_cert: bool) -> Result<(), String> {
        let ip = peer_ip.trim();
        if ip.is_empty() {
            return Err("No peer IP.".into());
        }
        if !crate::hostconf::is_allowed_peer_ip(ip) {
            return Err(crate::hostconf::peer_ip_rejected_message(ip));
        }
        if !MOONLIGHT.is_present() {
            return Err("Moonlight sidecar is not built.".into());
        }
        if pairing::fetch_local_session_ip().is_some() {
            return Err(
                "This PC is being streamed to. Force disconnect from the streaming client first."
                    .into(),
            );
        }

        self.stop();

        pairing::ensure_gamestream_http(ip).map_err(|err| err.into_message())?;
        pairing::ensure_host_tls(ip, trust_new_cert).map_err(|err| err.into_message())?;
        // `moonlight list` waits up to 30s when the host is missing from Moonlight's
        // cache. `pair` already succeeds immediately if the host is paired.
        self.pair_host(ip)?;

        pairing::ensure_host_video(ip).map_err(|err| err.into_message())?;

        self.refresh_host_displays(ip);
        let host = self.host_displays.lock().unwrap_or_else(|e| e.into_inner());
        let native = host.first().cloned();
        let start_index = host.iter().position(|d| d.focused).unwrap_or(0);
        drop(host);

        let (nw, nh) = native
            .map(|d| (d.width, d.height))
            .unwrap_or((DEFAULT_WIDTH, DEFAULT_HEIGHT));
        let client = display::local_displays();
        let (cw, ch) = display::focused_size(&client);
        let mut prefs = self.prefs.lock().unwrap_or_else(|e| e.into_inner());
        if prefs.scale == 0 {
            prefs.scale = display::default_denominator(nw, nh, cw.unwrap_or(1920), ch.unwrap_or(1080));
            save_prefs(&prefs);
        }
        drop(prefs);
        *self.connected_ip.lock().unwrap_or_else(|e| e.into_inner()) = Some(ip.to_string());
        self.spawn_window(ip, start_index, true)?;
        Ok(())
    }

    fn refresh_host_displays(&self, ip: &str) {
        let displays = pairing::fetch_displays(ip).unwrap_or_default();
        *self.host_displays.lock().unwrap_or_else(|e| e.into_inner()) = displays;
    }

    fn stream_size_for(&self, display_index: usize) -> (u32, u32) {
        let host = self.host_displays.lock().unwrap_or_else(|e| e.into_inner());
        let scale = self
            .prefs
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .scale
            .max(1);
        let native = host.get(display_index).or_else(|| host.first());
        match native {
            Some(d) => display::apply_scale(d.width, d.height, scale.max(1)),
            None => (DEFAULT_WIDTH, DEFAULT_HEIGHT),
        }
    }

    fn spawn_window(&self, ip: &str, display_index: usize, audio: bool) -> Result<(), String> {
        let (width, height) = self.stream_size_for(display_index);
        let prefs = self.prefs.lock().unwrap_or_else(|e| e.into_inner()).clone();
        let args = stream_args(ip, width, height, Some(display_index), audio, &prefs);
        let port = *self.control_port.lock().unwrap_or_else(|e| e.into_inner());
        let display_name = self
            .host_displays
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(display_index)
            .map(|d| d.name.clone())
            .filter(|n| !n.is_empty());
        let mut env = Vec::new();
        if let Some(port) = port {
            env.push(("ECLIPSE_CONTROL", format!("127.0.0.1:{port}")));
        }
        if let Some(name) = display_name {
            env.push(("ECLIPSE_DISPLAY_NAME", name));
        }
        let child = MOONLIGHT
            .spawn_args_env(&args, &env)
            .map_err(|err| format!("Failed to start Moonlight stream: {err}"))?;
        self.windows
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(StreamChild {
                child,
                display_index,
                audio,
            });
        Ok(())
    }

    fn respawn_window(&self, ip: &str, display_index: usize, audio: bool) -> Result<(), String> {
        {
            let mut windows = self.windows.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(pos) = windows.iter().position(|w| w.display_index == display_index)
            {
                let slot = windows.remove(pos);
                kill_child(Some(slot.child));
            }
        }
        self.spawn_window(ip, display_index, audio)
    }

    fn pair_host(&self, ip: &str) -> Result<(), String> {
        let pin = generate_pin();
        let mut child = MOONLIGHT
            .spawn_args_captured(&pair_args(ip, &pin))
            .map_err(|err| format!("Failed to start Moonlight pair: {err}"))?;
        let log = attach_log_drain(&mut child);
        *self.pair.lock().unwrap_or_else(|e| e.into_inner()) = Some(PairProc { child, log });

        thread::sleep(PAIR_START_GRACE);
        if let Some((ok, output)) = self.pair_exited() {
            if pair_succeeded(ok, &output) {
                return Ok(());
            }
            return Err(pair_process_error(&output));
        }

        let deadline = Instant::now() + pairing::pin_retry_budget();
        loop {
            match pairing::submit_pin(ip, &pin, pairing::client_name()) {
                Ok(true) => break,
                Ok(false) => {}
                Err(err) if err.is_fatal() => {
                    self.kill_pair();
                    return Err(err.into_message());
                }
                Err(_) => {}
            }
            if let Some((ok, output)) = self.pair_exited() {
                if pair_succeeded(ok, &output) {
                    return Ok(());
                }
                return Err(pair_process_error(&output));
            }
            if Instant::now() >= deadline {
                let output = self.take_pair_output();
                if pair_succeeded(false, &output) {
                    return Ok(());
                }
                return Err(pair_timeout_message(&output));
            }
            thread::sleep(pairing::pin_retry_interval());
        }

        thread::sleep(POST_PIN_SETTLE);
        self.kill_pair();
        Ok(())
    }

    fn pair_exited(&self) -> Option<(bool, String)> {
        let mut slot = self.pair.lock().unwrap_or_else(|e| e.into_inner());
        let proc = slot.as_mut()?;
        match proc.child.try_wait() {
            Ok(Some(status)) => {
                let proc = slot.take()?;
                thread::sleep(Duration::from_millis(30));
                Some((status.success(), snapshot_log(&proc.log)))
            }
            _ => None,
        }
    }

    fn take_pair_output(&self) -> String {
        let Some(mut proc) = self.pair.lock().unwrap_or_else(|e| e.into_inner()).take() else {
            return String::new();
        };
        let _ = proc.child.kill();
        let _ = proc.child.wait();
        thread::sleep(Duration::from_millis(30));
        snapshot_log(&proc.log)
    }

    fn kill_pair(&self) {
        if let Some(mut proc) = self.pair.lock().unwrap_or_else(|e| e.into_inner()).take() {
            let _ = proc.child.kill();
            let _ = proc.child.wait();
        }
    }

    #[cfg(test)]
    fn pair_still_running(&self) -> bool {
        let mut slot = self.pair.lock().unwrap_or_else(|e| e.into_inner());
        match slot.as_mut() {
            Some(proc) => matches!(proc.child.try_wait(), Ok(None)),
            None => false,
        }
    }
}

fn start_control_listener(inner: Arc<SessionInner>) {
    let listener = match TcpListener::bind("127.0.0.1:0") {
        Ok(l) => l,
        Err(_) => return,
    };
    let port = match listener.local_addr() {
        Ok(addr) => addr.port(),
        Err(_) => return,
    };
    *inner.control_port.lock().unwrap_or_else(|e| e.into_inner()) = Some(port);
    let _ = listener.set_nonblocking(false);
    thread::spawn(move || {
        for conn in listener.incoming() {
            let Ok(mut stream) = conn else {
                continue;
            };
            let Ok(clone) = stream.try_clone() else {
                continue;
            };
            let mut reader = BufReader::new(clone);
            let mut line = String::new();
            if reader.read_line(&mut line).is_err() {
                continue;
            }
            let cmd = parse_control_line(line.trim());
            let result = match cmd {
                ControlCmd::DisconnectAll => {
                    inner.stop();
                    Ok(())
                }
                ControlCmd::OpenAll => inner.open_all_displays(),
                ControlCmd::SwitchDisplay { index, pid } => inner.switch_display(index, pid),
                ControlCmd::Ignore => Ok(()),
            };
            let _ = writeln!(stream, "{}", if result.is_ok() { "ok" } else { "err" });
        }
    });
}

fn scale_pref_path() -> PathBuf {
    if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
        return PathBuf::from(xdg).join("eclipse").join("stream-scale.json");
    }
    if let Some(home) = std::env::var_os("HOME") {
        return PathBuf::from(home)
            .join(".config")
            .join("eclipse")
            .join("stream-scale.json");
    }
    #[cfg(windows)]
    {
        if let Some(appdata) = std::env::var_os("APPDATA") {
            return PathBuf::from(appdata)
                .join("Eclipse")
                .join("stream-scale.json");
        }
    }
    std::env::temp_dir().join("eclipse-stream-scale.json")
}

fn load_prefs() -> StreamPrefs {
    let mut prefs = StreamPrefs::default();
    let text = std::fs::read_to_string(scale_pref_path()).ok();
    let Some(text) = text else {
        return prefs;
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else {
        return prefs;
    };
    if let Some(n) = value
        .get("denominator")
        .or_else(|| value.get("scale"))
        .and_then(|d| d.as_u64())
        .map(|n| n as u32)
        .filter(|n| matches!(*n, 1 | 2 | 4))
    {
        prefs.scale = n;
    }
    if let Some(n) = value
        .get("fps")
        .and_then(|d| d.as_u64())
        .map(|n| n as u32)
        .filter(|n| matches!(*n, 30 | 60))
    {
        prefs.fps = n;
    }
    if let Some(codec) = value.get("codec").and_then(|d| d.as_str()) {
        if matches!(codec, "H.264" | "HEVC" | "AV1") {
            prefs.codec = codec.to_string();
        }
    }
    if let Some(n) = value
        .get("bitrateMbps")
        .and_then(|d| d.as_u64())
        .map(|n| n as u32)
        .filter(|n| matches!(*n, 10 | 20 | 50))
    {
        prefs.bitrate_mbps = n;
    }
    prefs
}

fn save_prefs(prefs: &StreamPrefs) {
    let path = scale_pref_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let body = serde_json::json!({
        "denominator": prefs.scale,
        "scale": prefs.scale,
        "fps": prefs.fps,
        "codec": prefs.codec,
        "bitrateMbps": prefs.bitrate_mbps,
    });
    let _ = std::fs::write(path, body.to_string());
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
            stream_args(
                "100.64.0.1",
                1440,
                960,
                Some(0),
                true,
                &StreamPrefs::default()
            ),
            [
                "stream",
                "--resolution",
                "1440x960",
                "--fps",
                "60",
                "--video-codec",
                "H.264",
                "--bitrate",
                "20000",
                "--no-hdr",
                "--packet-size",
                "1024",
                "--absolute-mouse",
                "--display-mode",
                "windowed",
                "--host-display",
                "0",
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
    fn pair_timeout_uses_moonlight_stderr() {
        let noisy = "\
00:00:00 - SDL Info (0): Compiled with SDL 2.32.70
Failed to connect to 100.64.0.1
";
        assert_eq!(
            pair_timeout_message(noisy),
            "Failed to connect to 100.64.0.1"
        );
        assert!(pair_timeout_message("   ").contains("PIN was not accepted"));
    }

    #[test]
    fn pair_process_error_skips_sdl_banner() {
        let noisy = "\
00:00:00 - SDL Info (0): Compiled with SDL 2.32.70
no Qt platform plugin
";
        assert_eq!(pair_process_error(noisy), "no Qt platform plugin");
        assert!(pair_process_error("").contains("Qt/SDL/FFmpeg"));
    }

    #[test]
    fn pair_succeeded_treats_exit_zero_and_already_paired() {
        assert!(pair_succeeded(true, "00:00:00 - SDL Info (0): Compiled with SDL 2.32.70"));
        assert!(pair_succeeded(false, "zak-laptop is already paired"));
        assert!(pair_succeeded(false, "Pairing completed successfully"));
        assert!(!pair_succeeded(false, "Failed to connect to 100.64.0.1"));
    }

    #[test]
    fn connect_reports_empty_ip() {
        let session = SessionProcess::new();
        let err = session.connect("  ", false).unwrap_err();
        assert_eq!(err, "No peer IP.");
    }

    #[test]
    fn connect_rejects_rfc1918() {
        let session = SessionProcess::new();
        let err = session.connect("192.168.1.1", false).unwrap_err();
        assert!(err.contains("100.64.0.0/10"), "{err}");
    }

    #[test]
    fn pair_still_running_is_false_without_child() {
        let session = SessionProcess::new();
        assert!(!session.inner.pair_still_running());
        let status = session.status();
        assert!(!status.connected);
        assert!(status.ip.is_none());
        assert_eq!(status.fps, STREAM_FPS);
    }

    #[test]
    fn last_window_clears_connected_ip() {
        let session = SessionProcess::new();
        *session
            .inner
            .connected_ip
            .lock()
            .unwrap() = Some("100.64.0.1".into());
        session.inner.reap_windows();
        assert!(session
            .inner
            .connected_ip
            .lock()
            .unwrap()
            .is_none());
    }

    #[test]
    fn control_line_maps_shortcuts() {
        assert_eq!(parse_control_line("disconnect-all"), ControlCmd::DisconnectAll);
        assert_eq!(parse_control_line("esc"), ControlCmd::DisconnectAll);
        assert_eq!(parse_control_line("open-all"), ControlCmd::OpenAll);
        assert_eq!(
            parse_control_line("switch-display 2 4242"),
            ControlCmd::SwitchDisplay {
                index: 1,
                pid: Some(4242),
            }
        );
        assert_eq!(
            parse_control_line("switch-display 1"),
            ControlCmd::SwitchDisplay {
                index: 0,
                pid: None,
            }
        );
        assert_eq!(parse_control_line("nope"), ControlCmd::Ignore);
    }

    #[cfg(unix)]
    #[test]
    fn first_window_exit_keeps_session() {
        let session = SessionProcess::new();
        *session.inner.connected_ip.lock().unwrap() = Some("100.64.0.1".into());
        let mut finished = std::process::Command::new("true")
            .spawn()
            .expect("true");
        let _ = finished.wait();
        let live = std::process::Command::new("sleep")
            .arg("30")
            .spawn()
            .expect("sleep");
        session.inner.windows.lock().unwrap().push(StreamChild {
            child: finished,
            display_index: 0,
            audio: true,
        });
        session.inner.windows.lock().unwrap().push(StreamChild {
            child: live,
            display_index: 1,
            audio: false,
        });
        session.inner.reap_windows();
        assert!(session.inner.connected_ip.lock().unwrap().is_some());
        assert_eq!(session.inner.windows.lock().unwrap().len(), 1);
        {
            let mut windows = session.inner.windows.lock().unwrap();
            kill_child(windows.pop().map(|w| w.child));
        }
        session.inner.reap_windows();
        assert!(session.inner.connected_ip.lock().unwrap().is_none());
    }
}
