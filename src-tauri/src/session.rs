//! Client session launch: pair if needed, then spawn Moonlight window(s) on Desktop.
//!
//! A session is the connection, not a single OS window. Ctrl+Alt+Q (or
//! Ctrl+Alt+Shift+Q) closes one window; the last window (or Esc on the same
//! chord) disconnects.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::Child;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use crate::display::{self, DisplayInfo, HostDisplayCache, ScaleOption};
use crate::pairing::{self, generate_pin};
use crate::sidecar::MOONLIGHT;

const DESKTOP_APP: &str = "Desktop";
const POST_PIN_SETTLE: Duration = Duration::from_secs(3);
const PAIR_START_GRACE: Duration = Duration::from_millis(400);
pub const STREAM_FPS: u32 = 60;
const DEFAULT_WIDTH: u32 = 1920;
const DEFAULT_HEIGHT: u32 = 1080;
const MAX_DISPLAY_WINDOWS: usize = 4;
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
    prefs: &StreamPrefs,
    display_map: &str,
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
        // 4:4:4 keeps text sharp; Moonlight warns and falls back when the host
        // encoder or client decoder can't do it.
        if prefs.yuv444 {
            "--yuv444".into()
        } else {
            "--no-yuv444".into()
        },
        // Stats start hidden; Ctrl+Alt+S toggles them per window.
        "--no-performance-overlay".into(),
        "--packet-size".into(),
        "1024".into(),
        // Desktop mode sends absolute positions, which pin at the window edge.
        // Game mode sends relative deltas so aim keeps turning (raw input).
        if prefs.game_mouse {
            "--no-absolute-mouse".into()
        } else {
            "--absolute-mouse".into()
        },
        "--display-mode".into(),
        "windowed".into(),
        // Pin every input/audio option. Anything left out is read from the
        // Moonlight.conf that stock Moonlight shares, so it varies per machine.
        "--capture-system-keys".into(),
        prefs.system_keys.clone(),
        "--audio-config".into(),
        prefs.audio_channels.clone(),
        if prefs.audio_on_host {
            "--audio-on-host".into()
        } else {
            "--no-audio-on-host".into()
        },
        // Extra display windows are muted with ECLIPSE_AUDIO=0, not by focus.
        "--no-mute-on-focus-loss".into(),
        "--no-mouse-buttons-swap".into(),
        "--no-reverse-scroll-direction".into(),
        "--no-touchscreen-trackpad".into(),
    ];
    if let Some(index) = display_index {
        args.push("--host-display".into());
        args.push(index.to_string());
    }
    // Options must come before host/app. QCommandLineParser treats flags after
    // the positional "Desktop" as extra positionals, so the map would be dropped.
    if !display_map.is_empty() {
        args.push("--eclipse-display-map".into());
        args.push(display_map.to_string());
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
    ShowDisplays,
    RegisterUi { port: u16, pid: u32 },
    SwitchDisplay { index: usize, pid: Option<u32> },
    UiPos { pos: [u16; 2] },
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
    if cmd == "show-displays" {
        return ControlCmd::ShowDisplays;
    }
    if let Some(rest) = cmd.strip_prefix("register-ui ") {
        let mut parts = rest.split_whitespace();
        let Some(port) = parts.next().and_then(|s| s.parse::<u16>().ok()) else {
            return ControlCmd::Ignore;
        };
        let Some(pid) = parts.next().and_then(|s| s.parse::<u32>().ok()) else {
            return ControlCmd::Ignore;
        };
        return ControlCmd::RegisterUi { port, pid };
    }
    if let Some(rest) = cmd.strip_prefix("ui-pos ") {
        let mut parts = rest.split_whitespace();
        let x = parts.next().and_then(parse_ui_fraction);
        let y = parts.next().and_then(parse_ui_fraction);
        return match (x, y) {
            (Some(x), Some(y)) => ControlCmd::UiPos { pos: [x, y] },
            _ => ControlCmd::Ignore,
        };
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

fn push_display_map(port: u16, body: &str) {
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let mut stream = match TcpStream::connect_timeout(&addr, Duration::from_millis(150)) {
        Ok(s) => s,
        Err(_) => return,
    };
    let _ = stream.set_write_timeout(Some(Duration::from_millis(150)));
    let _ = write!(stream, "{body}");
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamPrefs {
    pub scale: u32,
    pub fps: u32,
    pub codec: String,
    pub bitrate_mbps: u32,
    #[serde(default)]
    pub hotkey_shift: bool,
    #[serde(default)]
    pub game_mouse: bool,
    #[serde(default = "default_system_keys")]
    pub system_keys: String,
    #[serde(default)]
    pub audio_on_host: bool,
    #[serde(default = "default_audio_channels")]
    pub audio_channels: String,
    #[serde(default)]
    pub yuv444: bool,
    /// Mouse-assisted controls: the draggable Eclipse logo + menu in streams.
    #[serde(default = "default_stream_ui")]
    pub stream_ui: bool,
    /// Logo position as per-mille (0..=1000) fractions of the free space.
    #[serde(default = "default_ui_pos")]
    pub ui_pos: [u16; 2],
}

const FPS_CHOICES: [u32; 5] = [30, 60, 90, 120, 144];
const BITRATE_CHOICES_MBPS: [u32; 5] = [10, 20, 50, 100, 150];
const SYSTEM_KEYS: [&str; 3] = ["never", "fullscreen", "always"];
const AUDIO_CHANNELS: [&str; 3] = ["stereo", "5.1-surround", "7.1-surround"];

fn default_system_keys() -> String {
    "fullscreen".into()
}

fn default_audio_channels() -> String {
    "stereo".into()
}

fn default_stream_ui() -> bool {
    true
}

fn default_ui_pos() -> [u16; 2] {
    [970, 30]
}

/// `ECLIPSE_UI_POS` / `ui-pos` wire format: "0.970,0.030" style fractions.
fn ui_pos_env(pos: [u16; 2]) -> String {
    format!("{:.3},{:.3}", f32::from(pos[0]) / 1000.0, f32::from(pos[1]) / 1000.0)
}

fn parse_ui_fraction(text: &str) -> Option<u16> {
    let value = text.trim().parse::<f32>().ok()?;
    value
        .is_finite()
        .then(|| (value.clamp(0.0, 1.0) * 1000.0).round() as u16)
}

impl Default for StreamPrefs {
    fn default() -> Self {
        Self {
            scale: 1,
            fps: STREAM_FPS,
            codec: DEFAULT_CODEC.into(),
            bitrate_mbps: DEFAULT_BITRATE_MBPS,
            hotkey_shift: false,
            game_mouse: false,
            system_keys: default_system_keys(),
            audio_on_host: false,
            audio_channels: default_audio_channels(),
            yuv444: false,
            stream_ui: default_stream_ui(),
            ui_pos: default_ui_pos(),
        }
    }
}

fn validate_prefs(prefs: StreamPrefs) -> Result<StreamPrefs, String> {
    if !matches!(prefs.scale, 0 | 1 | 2 | 4) {
        return Err("Stream scale must be Quarter, Half, or Native.".into());
    }
    if !FPS_CHOICES.contains(&prefs.fps) {
        return Err("FPS must be 30, 60, 90, 120, or 144.".into());
    }
    if !matches!(prefs.codec.as_str(), "H.264" | "HEVC" | "AV1") {
        return Err("Codec must be H.264, HEVC, or AV1.".into());
    }
    if !BITRATE_CHOICES_MBPS.contains(&prefs.bitrate_mbps) {
        return Err("Max bandwidth must be 10, 20, 50, 100, or 150 Mb/s.".into());
    }
    if !SYSTEM_KEYS.contains(&prefs.system_keys.as_str()) {
        return Err("System keys must be Off, Fullscreen, or Always.".into());
    }
    if !AUDIO_CHANNELS.contains(&prefs.audio_channels.as_str()) {
        return Err("Audio must be Stereo, 5.1, or 7.1.".into());
    }
    if prefs.ui_pos.iter().any(|v| *v > 1000) {
        return Err("Stream controls position is out of range.".into());
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
    pub hotkey_shift: bool,
    pub game_mouse: bool,
    pub system_keys: String,
    pub audio_on_host: bool,
    pub audio_channels: String,
    pub yuv444: bool,
    pub stream_ui: bool,
    pub ui_pos: [u16; 2],
    pub displays: Vec<StreamWindowStatus>,
    pub scale_options: Vec<ScaleOption>,
}

struct StreamChild {
    child: Child,
    display_index: usize,
    /// This window should play stream audio (exactly one per session).
    audio: bool,
    /// Moonlight has been told `audio` (ECLIPSE_AUDIO at spawn, or `audio 1`).
    audio_sent: bool,
    ui_port: Option<u16>,
}

struct SessionInner {
    pair: Mutex<Option<PairProc>>,
    windows: Mutex<Vec<StreamChild>>,
    connected_ip: Mutex<Option<String>>,
    host: Mutex<HostDisplayCache>,
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
            host: Mutex::new(HostDisplayCache::default()),
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

    pub fn connect(
        &self,
        peer_ip: &str,
        trust_new_cert: bool,
        known_displays: Vec<DisplayInfo>,
    ) -> Result<(), String> {
        self.inner.connect(peer_ip, trust_new_cert, known_displays)
    }
}

impl SessionInner {
    fn lock_host(&self) -> std::sync::MutexGuard<'_, HostDisplayCache> {
        self.host.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn require_connected_ip(&self) -> Result<String, String> {
        self.connected_ip
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
            .ok_or_else(|| "Not connected.".to_string())
    }

    fn open_display_indices(&self) -> Vec<usize> {
        self.windows
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .map(|w| w.display_index)
            .collect()
    }

    fn host_display_count(&self) -> usize {
        self.lock_host().len().max(1)
    }

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
        // The audio window closed: move audio to a remaining window.
        if !windows.is_empty() && !windows.iter().any(|w| w.audio) {
            windows[0].audio = true;
            windows[0].audio_sent = false;
        }
        let promote: Vec<u16> = windows
            .iter_mut()
            .filter(|w| w.audio && !w.audio_sent)
            .filter_map(|w| {
                let port = w.ui_port?;
                w.audio_sent = true;
                Some(port)
            })
            .collect();
        if !promote.is_empty() {
            drop(windows);
            for port in promote {
                push_display_map(port, "audio 1");
            }
            windows = self.windows.lock().unwrap_or_else(|e| e.into_inner());
        }
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
        let host = self.lock_host();
        let prefs = self.prefs.lock().unwrap_or_else(|e| e.into_inner()).clone();
        let scale = prefs.scale;
        let open_indices: Vec<usize> = windows.iter().map(|w| w.display_index).collect();
        drop(windows);

        let mut displays = Vec::new();
        let mut width = DEFAULT_WIDTH;
        let mut height = DEFAULT_HEIGHT;
        let mut options = Vec::new();
        for (i, host_d) in host.displays().iter().enumerate() {
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
            hotkey_shift: prefs.hotkey_shift,
            game_mouse: prefs.game_mouse,
            system_keys: prefs.system_keys,
            audio_on_host: prefs.audio_on_host,
            audio_channels: prefs.audio_channels,
            yuv444: prefs.yuv444,
            stream_ui: prefs.stream_ui,
            ui_pos: prefs.ui_pos,
            displays,
            scale_options: options,
        }
    }

    fn set_prefs(&self, prefs: StreamPrefs) -> Result<(), String> {
        let prefs = validate_prefs(prefs)?;
        let old = self.prefs.lock().unwrap_or_else(|e| e.into_inner()).clone();
        // Launch-arg prefs: open windows respawn so the change takes effect.
        let stream_changed = old.scale != prefs.scale
            || old.fps != prefs.fps
            || old.codec != prefs.codec
            || old.bitrate_mbps != prefs.bitrate_mbps
            || old.game_mouse != prefs.game_mouse
            || old.system_keys != prefs.system_keys
            || old.audio_on_host != prefs.audio_on_host
            || old.audio_channels != prefs.audio_channels
            || old.yuv444 != prefs.yuv444;
        let hotkey_changed = old.hotkey_shift != prefs.hotkey_shift;
        let stream_ui_changed = old.stream_ui != prefs.stream_ui;
        *self.prefs.lock().unwrap_or_else(|e| e.into_inner()) = prefs.clone();
        save_prefs(&prefs);
        self.lock_host().set_hotkey_shift(prefs.hotkey_shift);
        if hotkey_changed {
            self.broadcast_hotkey_shift(prefs.hotkey_shift);
            let _ = self.broadcast_display_map();
        }
        if stream_ui_changed {
            self.broadcast(if prefs.stream_ui { "stream-ui 1" } else { "stream-ui 0" });
        }
        if !stream_changed {
            return Ok(());
        }
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
        let ip = self.require_connected_ip()?;
        self.refresh_host_displays(&ip);
        let mut count = self.host_display_count();
        if count > MAX_DISPLAY_WINDOWS {
            eprintln!(
                "Eclipse: opening first {MAX_DISPLAY_WINDOWS} of {count} host displays (GPU encode limit)"
            );
            count = MAX_DISPLAY_WINDOWS;
        }
        let open = self.open_display_indices();
        for index in 0..count {
            if !open.contains(&index) {
                let audio = !self.has_audio_window();
                self.spawn_window(&ip, index, audio)?;
            }
        }
        Ok(())
    }

    fn switch_display(&self, index: usize, pid: Option<u32>) -> Result<(), String> {
        let ip = self.require_connected_ip()?;
        self.refresh_host_displays(&ip);
        let count = self.host_display_count();
        if index >= count {
            return Ok(());
        }
        let retarget = {
            let mut windows = self.windows.lock().unwrap_or_else(|e| e.into_inner());
            let pos = if let Some(pid) = pid {
                windows
                    .iter()
                    .position(|w| w.child.id() == pid)
                    .or_else(|| windows.len().checked_sub(1))
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

    fn connect(
        &self,
        peer_ip: &str,
        trust_new_cert: bool,
        known_displays: Vec<DisplayInfo>,
    ) -> Result<(), String> {
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
        self.seed_host_displays(ip, known_displays);

        pairing::ensure_gamestream_http(ip).map_err(|err| err.into_message())?;
        pairing::ensure_host_tls(ip, trust_new_cert).map_err(|err| err.into_message())?;
        // `moonlight list` waits up to 30s when the host is missing from Moonlight's
        // cache. `pair` already succeeds immediately if the host is paired.
        self.pair_host(ip)?;

        pairing::ensure_host_video(ip).map_err(|err| err.into_message())?;

        self.refresh_host_displays(ip);
        let host = self.lock_host();
        let native = host.first().cloned();
        let start_index = host.focused_index();
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

    fn seed_host_displays(&self, ip: &str, known: Vec<DisplayInfo>) {
        let seed = if known.is_empty() {
            crate::discovery::list_peers()
                .peers
                .into_iter()
                .find(|p| p.ip == ip)
                .map(|p| p.displays)
                .unwrap_or_default()
        } else {
            known
        };
        let shift = self
            .prefs
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .hotkey_shift;
        let mut host = self.lock_host();
        host.clear();
        host.set_hotkey_shift(shift);
        host.seed(seed);
    }

    fn refresh_host_displays(&self, ip: &str) {
        let fetched = pairing::fetch_displays(ip);
        self.lock_host().merge_fetch(fetched);
    }

    fn stream_size_for(&self, display_index: usize) -> (u32, u32) {
        let host = self.lock_host();
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
        let (display_map, mut env) = self.moonlight_display_env(display_index);
        env.push(("ECLIPSE_AUDIO", if audio { "1".into() } else { "0".into() }));
        let args = stream_args(ip, width, height, Some(display_index), &prefs, &display_map);
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
                audio_sent: true,
                ui_port: None,
            });
        Ok(())
    }

    fn has_audio_window(&self) -> bool {
        self.windows
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .any(|w| w.audio)
    }

    fn moonlight_display_env(
        &self,
        display_index: usize,
    ) -> (String, Vec<(&'static str, String)>) {
        let port = *self.control_port.lock().unwrap_or_else(|e| e.into_inner());
        let host = self.lock_host();
        let display_name = host.name(display_index).map(str::to_string);
        let display_map = host.map().to_string();
        drop(host);
        let mut env = Vec::new();
        if let Some(port) = port {
            env.push(("ECLIPSE_CONTROL", format!("127.0.0.1:{port}")));
        }
        if let Some(name) = display_name {
            env.push(("ECLIPSE_DISPLAY_NAME", name));
        }
        if !display_map.is_empty() {
            env.push(("ECLIPSE_DISPLAY_MAP", display_map.clone()));
        }
        let prefs = self.prefs.lock().unwrap_or_else(|e| e.into_inner()).clone();
        env.push((
            "ECLIPSE_HOTKEY_SHIFT",
            if prefs.hotkey_shift { "1".into() } else { "0".into() },
        ));
        env.push((
            "ECLIPSE_STREAM_UI",
            if prefs.stream_ui { "1".into() } else { "0".into() },
        ));
        env.push(("ECLIPSE_UI_POS", ui_pos_env(prefs.ui_pos)));
        (display_map, env)
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

    fn register_ui_port(&self, pid: u32, port: u16) {
        if port == 0 {
            return;
        }
        let mut windows = self.windows.lock().unwrap_or_else(|e| e.into_inner());
        let mut promote = false;
        if let Some(slot) = windows.iter_mut().find(|w| w.child.id() == pid) {
            slot.ui_port = Some(port);
            if slot.audio && !slot.audio_sent {
                slot.audio_sent = true;
                promote = true;
            }
        }
        drop(windows);
        if promote {
            push_display_map(port, "audio 1");
        }
    }

    fn broadcast(&self, body: &str) {
        let ports: Vec<u16> = self
            .windows
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .filter_map(|w| w.ui_port)
            .collect();
        for port in ports {
            push_display_map(port, body);
        }
    }

    /// Moonlight reports the logo position after a drag; new windows reuse it.
    fn set_ui_pos(&self, pos: [u16; 2]) {
        let mut prefs = self.prefs.lock().unwrap_or_else(|e| e.into_inner());
        if prefs.ui_pos != pos {
            prefs.ui_pos = pos;
            save_prefs(&prefs);
        }
    }

    fn broadcast_hotkey_shift(&self, shift: bool) {
        let body = if shift { "hotkey-shift 1" } else { "hotkey-shift 0" };
        let ports: Vec<u16> = self
            .windows
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .filter_map(|w| w.ui_port)
            .collect();
        for port in ports {
            push_display_map(port, body);
        }
    }

    fn broadcast_display_map(&self) -> String {
        let body = self.lock_host().map().to_string();
        let ports: Vec<u16> = self
            .windows
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .filter_map(|w| w.ui_port)
            .collect();
        if !body.is_empty() {
            for port in ports {
                push_display_map(port, &body);
            }
        }
        body
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
            let reply = match cmd {
                ControlCmd::DisconnectAll => {
                    inner.stop();
                    "ok".to_string()
                }
                ControlCmd::OpenAll => {
                    if inner.open_all_displays().is_ok() {
                        "ok".into()
                    } else {
                        "err".into()
                    }
                }
                ControlCmd::ShowDisplays => {
                    let body = inner.broadcast_display_map();
                    format!("ok\n{body}")
                }
                ControlCmd::RegisterUi { port, pid } => {
                    inner.register_ui_port(pid, port);
                    "ok".into()
                }
                ControlCmd::SwitchDisplay { index, pid } => {
                    if inner.switch_display(index, pid).is_ok() {
                        "ok".into()
                    } else {
                        "err".into()
                    }
                }
                ControlCmd::UiPos { pos } => {
                    inner.set_ui_pos(pos);
                    "ok".into()
                }
                ControlCmd::Ignore => "ok".into(),
            };
            let _ = write!(stream, "{reply}");
            if !reply.ends_with('\n') {
                let _ = writeln!(stream);
            }
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
        .filter(|n| FPS_CHOICES.contains(n))
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
        .filter(|n| BITRATE_CHOICES_MBPS.contains(n))
    {
        prefs.bitrate_mbps = n;
    }
    if let Some(shift) = value.get("hotkeyShift").and_then(|d| d.as_bool()) {
        prefs.hotkey_shift = shift;
    }
    if let Some(game) = value.get("gameMouse").and_then(|d| d.as_bool()) {
        prefs.game_mouse = game;
    }
    if let Some(keys) = value
        .get("systemKeys")
        .and_then(|d| d.as_str())
        .filter(|k| SYSTEM_KEYS.contains(k))
    {
        prefs.system_keys = keys.to_string();
    }
    if let Some(host) = value.get("audioOnHost").and_then(|d| d.as_bool()) {
        prefs.audio_on_host = host;
    }
    if let Some(yuv444) = value.get("yuv444").and_then(|d| d.as_bool()) {
        prefs.yuv444 = yuv444;
    }
    if let Some(on) = value.get("streamUi").and_then(|d| d.as_bool()) {
        prefs.stream_ui = on;
    }
    if let Some(pos) = value.get("uiPos").and_then(|d| d.as_array()) {
        let coords: Vec<u16> = pos
            .iter()
            .filter_map(|v| v.as_u64())
            .filter(|v| *v <= 1000)
            .map(|v| v as u16)
            .collect();
        if let [x, y] = coords[..] {
            prefs.ui_pos = [x, y];
        }
    }
    if let Some(channels) = value
        .get("audioChannels")
        .and_then(|d| d.as_str())
        .filter(|c| AUDIO_CHANNELS.contains(c))
    {
        prefs.audio_channels = channels.to_string();
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
        "hotkeyShift": prefs.hotkey_shift,
        "gameMouse": prefs.game_mouse,
        "systemKeys": prefs.system_keys,
        "audioOnHost": prefs.audio_on_host,
        "audioChannels": prefs.audio_channels,
        "yuv444": prefs.yuv444,
        "streamUi": prefs.stream_ui,
        "uiPos": prefs.ui_pos,
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
                &StreamPrefs::default(),
                "0;Ctrl+Alt+1;DP-1|1;Ctrl+Alt+2;HDMI",
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
                "--no-yuv444",
                "--no-performance-overlay",
                "--packet-size",
                "1024",
                "--absolute-mouse",
                "--display-mode",
                "windowed",
                "--capture-system-keys",
                "fullscreen",
                "--audio-config",
                "stereo",
                "--no-audio-on-host",
                "--no-mute-on-focus-loss",
                "--no-mouse-buttons-swap",
                "--no-reverse-scroll-direction",
                "--no-touchscreen-trackpad",
                "--host-display",
                "0",
                "--eclipse-display-map",
                "0;Ctrl+Alt+1;DP-1|1;Ctrl+Alt+2;HDMI",
                "100.64.0.1",
                "Desktop"
            ]
        );
    }

    #[test]
    fn game_mouse_streams_relative_motion() {
        let defaults = StreamPrefs::default();
        let desktop = stream_args("100.64.0.1", 1920, 1080, None, &defaults, "");
        assert!(desktop.iter().any(|a| a == "--absolute-mouse"));
        assert!(!desktop.iter().any(|a| a == "--no-absolute-mouse"));

        let prefs = StreamPrefs {
            game_mouse: true,
            ..StreamPrefs::default()
        };
        let game = stream_args("100.64.0.1", 1920, 1080, None, &prefs, "");
        assert!(game.iter().any(|a| a == "--no-absolute-mouse"));
        assert!(!game.iter().any(|a| a == "--absolute-mouse"));
    }

    #[test]
    fn stream_prefs_pin_keys_and_audio() {
        let prefs = StreamPrefs {
            system_keys: "always".into(),
            audio_on_host: true,
            audio_channels: "5.1-surround".into(),
            ..StreamPrefs::default()
        };
        let args = stream_args("100.64.0.1", 1920, 1080, None, &prefs, "");
        let pair = |flag: &str| {
            args.iter()
                .position(|a| a == flag)
                .and_then(|i| args.get(i + 1))
                .cloned()
        };
        assert_eq!(pair("--capture-system-keys").as_deref(), Some("always"));
        assert_eq!(pair("--audio-config").as_deref(), Some("5.1-surround"));
        assert!(args.iter().any(|a| a == "--audio-on-host"));
        assert!(!args.iter().any(|a| a == "--mute-on-focus-loss"));
    }

    #[test]
    fn invalid_keys_or_audio_prefs_are_rejected() {
        let keys = StreamPrefs {
            system_keys: "sometimes".into(),
            ..StreamPrefs::default()
        };
        assert!(validate_prefs(keys).is_err());
        let audio = StreamPrefs {
            audio_channels: "mono".into(),
            ..StreamPrefs::default()
        };
        assert!(validate_prefs(audio).is_err());
        assert!(validate_prefs(StreamPrefs::default()).is_ok());
    }

    #[test]
    fn high_refresh_and_bitrate_tiers_validate() {
        for fps in FPS_CHOICES {
            for bitrate_mbps in BITRATE_CHOICES_MBPS {
                let prefs = StreamPrefs {
                    fps,
                    bitrate_mbps,
                    ..StreamPrefs::default()
                };
                assert!(validate_prefs(prefs).is_ok(), "{fps} fps / {bitrate_mbps} Mb/s");
            }
        }
        let odd_fps = StreamPrefs {
            fps: 75,
            ..StreamPrefs::default()
        };
        assert!(validate_prefs(odd_fps).is_err());
        let odd_bitrate = StreamPrefs {
            bitrate_mbps: 30,
            ..StreamPrefs::default()
        };
        assert!(validate_prefs(odd_bitrate).is_err());

        let prefs = StreamPrefs {
            fps: 120,
            bitrate_mbps: 150,
            ..StreamPrefs::default()
        };
        let args = stream_args("100.64.0.1", 1920, 1080, None, &prefs, "");
        assert!(args.windows(2).any(|w| w[0] == "--fps" && w[1] == "120"));
        assert!(args.windows(2).any(|w| w[0] == "--bitrate" && w[1] == "150000"));
    }

    #[test]
    fn stream_ui_env_and_position_round_trip() {
        assert_eq!(ui_pos_env(default_ui_pos()), "0.970,0.030");
        assert_eq!(ui_pos_env([0, 1000]), "0.000,1.000");
        let bad = StreamPrefs {
            ui_pos: [1001, 0],
            ..StreamPrefs::default()
        };
        assert!(validate_prefs(bad).is_err());
        assert!(StreamPrefs::default().stream_ui);
    }

    #[test]
    fn yuv444_flag_is_pinned_both_ways() {
        let off = stream_args("100.64.0.1", 1920, 1080, None, &StreamPrefs::default(), "");
        assert!(off.iter().any(|a| a == "--no-yuv444"));
        let prefs = StreamPrefs {
            yuv444: true,
            ..StreamPrefs::default()
        };
        let on = stream_args("100.64.0.1", 1920, 1080, None, &prefs, "");
        assert!(on.iter().any(|a| a == "--yuv444"));
        assert!(!on.iter().any(|a| a == "--no-yuv444"));
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
        let err = session.connect("  ", false, Vec::new()).unwrap_err();
        assert_eq!(err, "No peer IP.");
    }

    #[test]
    fn connect_rejects_rfc1918() {
        let session = SessionProcess::new();
        let err = session.connect("192.168.1.1", false, Vec::new()).unwrap_err();
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
        assert_eq!(parse_control_line("show-displays"), ControlCmd::ShowDisplays);
        assert_eq!(
            parse_control_line("register-ui 51234 4242"),
            ControlCmd::RegisterUi {
                port: 51234,
                pid: 4242,
            }
        );
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
        assert_eq!(
            parse_control_line("ui-pos 0.250 0.900"),
            ControlCmd::UiPos { pos: [250, 900] }
        );
        assert_eq!(
            parse_control_line("ui-pos 1.7 -2"),
            ControlCmd::UiPos { pos: [1000, 0] }
        );
        assert_eq!(parse_control_line("ui-pos 0.5"), ControlCmd::Ignore);
        assert_eq!(parse_control_line("ui-pos nan 0.1"), ControlCmd::Ignore);
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
            audio_sent: true,
            ui_port: None,
        });
        session.inner.windows.lock().unwrap().push(StreamChild {
            child: live,
            display_index: 1,
            audio: false,
            audio_sent: true,
            ui_port: None,
        });
        session.inner.reap_windows();
        assert!(session.inner.connected_ip.lock().unwrap().is_some());
        assert_eq!(session.inner.windows.lock().unwrap().len(), 1);
        {
            // Audio moves to the surviving window; delivery waits for its UI port.
            let windows = session.inner.windows.lock().unwrap();
            assert!(windows[0].audio);
            assert!(!windows[0].audio_sent);
        }
        let pid = session.inner.windows.lock().unwrap()[0].child.id();
        // Nothing listens on this port; the push fails fast but marks delivery.
        let unused = TcpListener::bind("127.0.0.1:0")
            .and_then(|l| l.local_addr())
            .map(|a| a.port())
            .expect("port");
        session.inner.register_ui_port(pid, unused);
        assert!(session.inner.windows.lock().unwrap()[0].audio_sent);
        {
            let mut windows = session.inner.windows.lock().unwrap();
            kill_child(windows.pop().map(|w| w.child));
        }
        session.inner.reap_windows();
        assert!(session.inner.connected_ip.lock().unwrap().is_none());
    }
}
