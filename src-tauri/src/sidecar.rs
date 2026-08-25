//! Data-plane sidecars. Sunshine and Moonlight are opaque binaries; this module
//! is the only place that knows their names, asset env, and health port.
//! Packaged installs attach to a boot-time Sunshine service; `tauri dev` still
//! spawns a child and kills only that child on exit.

use std::fs;
use std::net::{TcpStream, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Compile-time Sunshine assets path used by the Windows sidecar build
/// (`-DSUNSHINE_ASSETS_DIR=share/sunshine`). Sunshine concatenates this with
/// `/apps.json` in C++; it does not read `SUNSHINE_ASSETS_DIR` from the environment.
const SUNSHINE_SHARE_REL: &str = "share/sunshine";

/// Windows Moonlight Qt/SDL/FFmpeg files produced by `scripts/build-sidecars.ps1`.
const MOONLIGHT_RUNTIME_REL: &str = "resources/moonlight-runtime";

/// Contract for a vendored GameStream engine. Engine upgrades should only
/// require changing these constants (plus the build script), not the UI.
#[derive(Clone, Copy, Debug)]
pub struct Engine {
    pub name: &'static str,
    asset_env: Option<&'static str>,
    assets_rel: Option<&'static str>,
    health: Option<Health>,
}

#[derive(Clone, Copy, Debug)]
pub struct Health {
    pub host: &'static str,
    pub port: u16,
}

impl Health {
    pub fn url(self) -> String {
        format!("https://{}:{}", self.host, self.port)
    }

    /// TCP connect to `host:self.port`. Used for peers; local smoke uses `127.0.0.1`.
    pub fn probe(self, host: &str, timeout: Duration) -> bool {
        probe_tcp(host, self.port, timeout)
    }
}

pub fn probe_tcp(host: &str, port: u16, timeout: Duration) -> bool {
    probe_tcp_ms(host, port, timeout).is_some()
}

/// Round-trip time of a TCP connect to `host:port`, in milliseconds.
pub fn probe_tcp_ms(host: &str, port: u16, timeout: Duration) -> Option<u32> {
    let addr = format!("{host}:{port}");
    let sock = addr.to_socket_addrs().ok()?.next()?;
    let start = Instant::now();
    TcpStream::connect_timeout(&sock, timeout).ok()?;
    Some(start.elapsed().as_millis() as u32)
}

pub const SUNSHINE: Engine = Engine {
    name: "sunshine",
    asset_env: Some("SUNSHINE_ASSETS_DIR"),
    assets_rel: Some("resources/sunshine"),
    health: Some(Health {
        host: "127.0.0.1",
        port: 47990,
    }),
};

pub const MOONLIGHT: Engine = Engine {
    name: "moonlight",
    asset_env: None,
    assets_rel: None,
    health: None,
};

/// Seeded Sunshine Web UI credentials when the host has none yet.
/// Tailscale is the security boundary; these are not a secret.
pub const SUNSHINE_UI_USER: &str = "eclipse";
pub const SUNSHINE_UI_PASS: &str = "eclipse";

/// Packaged Linux host service config (`ConfigurationDirectory=eclipse`).
const LINUX_SYSTEM_SUNSHINE_DIR: &str = "/etc/eclipse/sunshine";

/// Windows SCM name hardcoded in `vendor/sunshine/tools/sunshinesvc.cpp`.
#[cfg(windows)]
const WINDOWS_HOST_SERVICE: &str = "SunshineService";

/// systemd unit installed by Linux packages.
#[cfg(not(windows))]
const LINUX_HOST_SERVICE: &str = "eclipse-host.service";

const HOST_HEALTH_TIMEOUT: Duration = Duration::from_millis(400);

/// Sunshine `credentials_file` default (`file_state`): `sunshine_state.json` under appdata.
pub fn sunshine_state_path() -> PathBuf {
    #[cfg(windows)]
    {
        if let Some(bin) = SUNSHINE.resolve() {
            if let Some(dir) = bin.parent() {
                return dir.join("config").join("sunshine_state.json");
            }
        }
        PathBuf::from("config").join("sunshine_state.json")
    }
    #[cfg(not(windows))]
    {
        linux_sunshine_state_path(
            Path::new(LINUX_SYSTEM_SUNSHINE_DIR),
            std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from),
            std::env::var_os("HOME").map(PathBuf::from),
        )
    }
}

fn linux_sunshine_state_path(
    system_dir: &Path,
    xdg_config: Option<PathBuf>,
    home: Option<PathBuf>,
) -> PathBuf {
    if system_dir.is_dir() {
        return system_dir.join("sunshine_state.json");
    }
    let base = xdg_config
        .or_else(|| home.map(|h| h.join(".config")))
        .unwrap_or_else(|| PathBuf::from(".config"));
    base.join("sunshine").join("sunshine_state.json")
}

pub fn sunshine_creds_present(path: &Path) -> bool {
    let Ok(text) = std::fs::read_to_string(path) else {
        return false;
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else {
        return false;
    };
    let nonempty = |key: &str| {
        value
            .get(key)
            .and_then(|v| v.as_str())
            .is_some_and(|s| !s.is_empty())
    };
    nonempty("username") && nonempty("password") && nonempty("salt")
}

fn ensure_ui_credentials() {
    let path = sunshine_state_path();
    if sunshine_creds_present(&path) {
        return;
    }
    let args = [
        "--creds".to_string(),
        SUNSHINE_UI_USER.to_string(),
        SUNSHINE_UI_PASS.to_string(),
    ];
    match SUNSHINE.run_args(&args) {
        Ok(out) if out.status.success() => {
            eprintln!(
                "Eclipse: seeded Sunshine Web UI credentials ({SUNSHINE_UI_USER}/{SUNSHINE_UI_PASS})."
            );
        }
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr);
            eprintln!(
                "Eclipse: sunshine --creds exited {} ({})",
                out.status,
                stderr.trim()
            );
        }
        Err(err) => {
            eprintln!(
                "Eclipse: sunshine --creds failed ({err}); Web UI welcome may still be required."
            );
        }
    }
}

impl Engine {
    pub fn health(self) -> Option<Health> {
        self.health
    }

    pub fn triple_filename(self) -> String {
        if cfg!(windows) {
            format!("{}-{}.exe", self.name, env!("HOST_TRIPLE"))
        } else {
            format!("{}-{}", self.name, env!("HOST_TRIPLE"))
        }
    }

    pub fn source_path(self) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("binaries")
            .join(self.triple_filename())
    }

    pub fn assets_dir(self) -> Option<PathBuf> {
        self.assets_rel?;
        sunshine_asset_candidates(current_exe_dir().as_deref())
            .into_iter()
            .find(|p| p.join("apps.json").is_file())
    }

    /// Dev builds copy `externalBin` next to the exe without a triple suffix.
    pub fn resolve(self) -> Option<PathBuf> {
        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                let beside = if cfg!(windows) {
                    dir.join(format!("{}.exe", self.name))
                } else {
                    dir.join(self.name)
                };
                if beside.is_file() {
                    return Some(beside);
                }
            }
        }
        let vendored = self.source_path();
        vendored.is_file().then_some(vendored)
    }

    pub fn is_present(self) -> bool {
        self.resolve().is_some()
    }

    pub fn probe_health(self, timeout: Duration) -> bool {
        let Some(health) = self.health else {
            return false;
        };
        health.probe(health.host, timeout)
    }

    pub fn probe_at(self, host: &str, timeout: Duration) -> bool {
        let Some(health) = self.health else {
            return false;
        };
        health.probe(host, timeout)
    }

    fn command(self) -> Result<Command, std::io::Error> {
        let bin = self.resolve().ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("{} sidecar not built", self.name),
            )
        })?;
        let mut cmd = Command::new(&bin);
        if let Some(parent) = bin.parent() {
            let mut cwd = parent.to_path_buf();
            if self.assets_rel.is_some() {
                if let Some((run_cwd, staged)) = prepare_sunshine_share(parent) {
                    if let Some(var) = self.asset_env {
                        cmd.env(var, &staged);
                    }
                    cwd = run_cwd;
                } else {
                    eprintln!("Eclipse: Sunshine assets not found; host UI may fail");
                }
            }
            if cfg!(windows) && self.name == MOONLIGHT.name {
                prepare_moonlight_runtime(parent, &mut cmd);
            }
            cmd.current_dir(cwd);
        }
        Ok(cmd)
    }

    pub(crate) fn spawn_args(self, args: &[String]) -> Result<Child, std::io::Error> {
        self.spawn_args_env(args, &[])
    }

    pub(crate) fn spawn_args_env(
        self,
        args: &[String],
        env: &[(&str, String)],
    ) -> Result<Child, std::io::Error> {
        let mut cmd = self.command()?;
        cmd.args(args);
        for (key, value) in env {
            cmd.env(key, value);
        }
        cmd.spawn()
    }

    pub(crate) fn run_args(self, args: &[String]) -> Result<Output, std::io::Error> {
        let mut cmd = self.command()?;
        cmd.args(args);
        cmd.output()
    }

    fn spawn(self) -> Result<Child, std::io::Error> {
        self.spawn_args(&[])
    }
}

/// Recursively copy `src` into `dst`, creating directories as needed.
fn copy_dir_all(src: &Path, dst: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir_all(&from, &to)?;
        } else {
            fs::copy(&from, &to)?;
        }
    }
    Ok(())
}

fn paths_same(a: &Path, b: &Path) -> bool {
    match (fs::canonicalize(a), fs::canonicalize(b)) {
        (Ok(left), Ok(right)) => left == right,
        _ => false,
    }
}

fn current_exe_dir() -> Option<PathBuf> {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))
}

fn linux_runtime_data_dir() -> PathBuf {
    if let Some(xdg) = std::env::var_os("XDG_DATA_HOME") {
        return PathBuf::from(xdg).join("eclipse");
    }
    if let Some(home) = std::env::var_os("HOME") {
        return PathBuf::from(home).join(".local/share/eclipse");
    }
    std::env::temp_dir().join("eclipse")
}

/// Locations that may hold Sunshine `apps.json` in dev and packaged installs.
fn sunshine_asset_candidates(exe_dir: Option<&Path>) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Some(dir) = exe_dir {
        out.push(dir.join(SUNSHINE_SHARE_REL));
        out.push(dir.join("resources").join("sunshine"));
        out.push(dir.join("share").join("sunshine"));
        if let Some(parent) = dir.parent() {
            out.push(parent.join("lib").join("Eclipse").join(SUNSHINE_SHARE_REL));
            out.push(
                parent
                    .join("lib")
                    .join("Eclipse")
                    .join("resources")
                    .join("sunshine"),
            );
        }
    }
    out.push(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("resources")
            .join("sunshine"),
    );
    out
}

fn prepare_sunshine_share(bin_dir: &Path) -> Option<(PathBuf, PathBuf)> {
    let dest = bin_dir.join(SUNSHINE_SHARE_REL);
    if dest.join("apps.json").is_file() {
        return Some((bin_dir.to_path_buf(), dest));
    }
    if let Some(parent) = bin_dir.parent() {
        let lib = parent.join("lib").join("Eclipse");
        let share = lib.join(SUNSHINE_SHARE_REL);
        if share.join("apps.json").is_file() {
            return Some((lib, share));
        }
    }
    let assets = sunshine_asset_candidates(Some(bin_dir))
        .into_iter()
        .find(|p| p.join("apps.json").is_file())?;
    match stage_sunshine_share(bin_dir, &assets) {
        Ok(staged) => Some((bin_dir.to_path_buf(), staged)),
        Err(err) => {
            if cfg!(windows) {
                eprintln!("Eclipse: could not stage Sunshine assets ({err})");
                return None;
            }
            let data = linux_runtime_data_dir();
            match stage_sunshine_share(&data, &assets) {
                Ok(staged) => Some((data, staged)),
                Err(err) => {
                    eprintln!("Eclipse: could not stage Sunshine assets ({err})");
                    None
                }
            }
        }
    }
}

fn moonlight_runtime_candidates(bin_dir: &Path) -> Vec<PathBuf> {
    vec![
        bin_dir.to_path_buf(),
        bin_dir.join("moonlight-runtime"),
        bin_dir.join("resources").join("moonlight-runtime"),
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(MOONLIGHT_RUNTIME_REL),
    ]
}

fn prepend_env_path(cmd: &mut Command, dir: &Path) {
    let mut paths = vec![dir.to_path_buf()];
    if let Some(existing) = std::env::var_os("PATH") {
        paths.extend(std::env::split_paths(&existing));
    }
    if let Ok(joined) = std::env::join_paths(paths) {
        cmd.env("PATH", joined);
    }
}

fn prepare_moonlight_runtime(bin_dir: &Path, cmd: &mut Command) {
    if moonlight_runtime_ready(bin_dir) {
        return;
    }
    for runtime in moonlight_runtime_candidates(bin_dir) {
        if !runtime.is_dir() {
            continue;
        }
        if paths_same(&runtime, bin_dir) {
            if moonlight_runtime_ready(bin_dir) {
                return;
            }
            continue;
        }
        if stage_moonlight_runtime(bin_dir, &runtime).is_ok() && moonlight_runtime_ready(bin_dir) {
            return;
        }
        if runtime.join("Qt6Core.dll").is_file() {
            prepend_env_path(cmd, &runtime);
            return;
        }
    }
}

/// Copy vendored Sunshine web/shader assets into `{bin_dir}/share/sunshine`.
///
/// Windows Sunshine is built with a cwd-relative assets path. Setting
/// `SUNSHINE_ASSETS_DIR` in the environment does not override that path.
fn stage_sunshine_share(bin_dir: &Path, assets: &Path) -> std::io::Result<PathBuf> {
    if !assets.is_dir() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Sunshine assets directory is missing",
        ));
    }
    let dest = bin_dir.join(SUNSHINE_SHARE_REL);
    if paths_same(assets, &dest) {
        return Ok(dest);
    }
    copy_dir_all(assets, &dest)?;
    Ok(dest)
}

fn moonlight_runtime_ready(bin_dir: &Path) -> bool {
    bin_dir.join("Qt6Core.dll").is_file()
        && bin_dir.join("platforms").join("qwindows.dll").is_file()
}

/// Copy Windows Moonlight Qt/SDL/FFmpeg runtime files beside the sidecar binary.
///
/// `moonlight.exe` is dynamically linked. Without these files it exits immediately
/// (`STATUS_DLL_NOT_FOUND`) and Connect reports a PIN timeout.
fn stage_moonlight_runtime(bin_dir: &Path, runtime: &Path) -> std::io::Result<()> {
    if !runtime.is_dir() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Moonlight runtime directory is missing",
        ));
    }
    if paths_same(runtime, bin_dir) || moonlight_runtime_ready(bin_dir) {
        return Ok(());
    }
    copy_dir_all(runtime, bin_dir)
}

fn child_alive(child: Option<&mut Child>) -> bool {
    match child {
        Some(c) => matches!(c.try_wait(), Ok(None)),
        None => false,
    }
}

/// Holds the host engine child. Moonlight is launched per session (see `session`).
pub struct HostProcess {
    child: Mutex<Option<Child>>,
}

impl HostProcess {
    pub fn new() -> Self {
        Self {
            child: Mutex::new(None),
        }
    }

    pub fn start_sunshine(&self) {
        let mut slot = self.child.lock().unwrap_or_else(|e| e.into_inner());
        if child_alive(slot.as_mut()) {
            return;
        }
        *slot = None;
        if host_service_installed() {
            if sunshine_already_hosted() {
                eprintln!(
                    "Eclipse: Sunshine already running (health {}); attaching, not spawning.",
                    SUNSHINE.health().map(Health::url).unwrap_or_default()
                );
            } else {
                eprintln!(
                    "Eclipse: host service is installed but stopped; not spawning a competing child."
                );
            }
            return;
        }
        if sunshine_already_hosted() {
            eprintln!(
                "Eclipse: Sunshine already running (health {}); attaching, not spawning.",
                SUNSHINE.health().map(Health::url).unwrap_or_default()
            );
            return;
        }
        ensure_ui_credentials();
        match SUNSHINE.spawn() {
            Ok(child) => {
                eprintln!(
                    "Eclipse: Sunshine sidecar started (health {}).",
                    SUNSHINE.health().map(Health::url).unwrap_or_default()
                );
                *slot = Some(child);
            }
            Err(err) => {
                eprintln!("Eclipse: Sunshine sidecar failed to start ({err}); UI will still open.");
            }
        }
    }

    /// Tray/package control: start or stop the OS host service when installed,
    /// otherwise the GUI-owned Sunshine child (source/`tauri dev`).
    pub fn set_host_running(&self, on: bool) -> Result<(), String> {
        if host_service_installed() {
            if on {
                self.stop_sunshine();
                start_host_service()?;
                if wait_for(
                    || SUNSHINE.probe_health(HOST_HEALTH_TIMEOUT),
                    Duration::from_secs(8),
                ) {
                    return Ok(());
                }
                return Err(
                    "host service started but Sunshine is not listening on 127.0.0.1:47990".into(),
                );
            }
            stop_host_service()?;
            self.stop_sunshine();
            return Ok(());
        }
        if on {
            self.start_sunshine();
            if self.sunshine_running()
                || wait_for(
                    || SUNSHINE.probe_health(HOST_HEALTH_TIMEOUT),
                    Duration::from_secs(8),
                )
            {
                return Ok(());
            }
            return Err("Sunshine sidecar did not start".into());
        }
        self.stop_sunshine();
        Ok(())
    }

    pub fn stop_sunshine(&self) {
        let mut slot = self.child.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(mut child) = slot.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }

    pub fn sunshine_running(&self) -> bool {
        let mut slot = self.child.lock().unwrap_or_else(|e| e.into_inner());
        let alive = child_alive(slot.as_mut());
        if !alive {
            *slot = None;
        }
        alive || SUNSHINE.probe_health(HOST_HEALTH_TIMEOUT)
    }
}

fn wait_for(pred: impl Fn() -> bool, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    loop {
        if pred() {
            return true;
        }
        if Instant::now() >= deadline {
            return pred();
        }
        std::thread::sleep(Duration::from_millis(150));
    }
}

fn sunshine_already_hosted() -> bool {
    SUNSHINE.probe_health(HOST_HEALTH_TIMEOUT) || host_service_active()
}

pub fn host_service_installed() -> bool {
    #[cfg(windows)]
    {
        Command::new("sc")
            .args(["query", WINDOWS_HOST_SERVICE])
            .status()
            .map(|status| status.success())
            .unwrap_or(false)
    }
    #[cfg(not(windows))]
    {
        linux_host_unit_installed(&[
            Path::new("/usr/lib/systemd/system/eclipse-host.service"),
            Path::new("/etc/systemd/system/eclipse-host.service"),
        ])
    }
}

#[cfg_attr(windows, allow(dead_code))]
fn linux_host_unit_installed(candidates: &[&Path]) -> bool {
    candidates.iter().any(|p| p.is_file())
}

#[cfg(not(windows))]
const LINUX_HOSTCTL: &str = "/usr/lib/eclipse/eclipse-hostctl";

fn start_host_service() -> Result<(), String> {
    control_host_service("start")
}

fn stop_host_service() -> Result<(), String> {
    control_host_service("stop")
}

fn control_host_service(action: &str) -> Result<(), String> {
    #[cfg(windows)]
    {
        let status = Command::new("net")
            .args([action, WINDOWS_HOST_SERVICE])
            .status()
            .map_err(|err| format!("net {action} {WINDOWS_HOST_SERVICE}: {err}"))?;
        if status.success() {
            return Ok(());
        }
        // `net start` when already running / `net stop` when already stopped.
        if action == "start" && host_service_active() {
            return Ok(());
        }
        if action == "stop" && !host_service_active() {
            return Ok(());
        }
        Err(format!(
            "net {action} {WINDOWS_HOST_SERVICE} failed (is the service ACL missing?)"
        ))
    }
    #[cfg(not(windows))]
    {
        let status = Command::new("systemctl")
            .args(["--no-ask-password", action, LINUX_HOST_SERVICE])
            .status()
            .map_err(|err| format!("systemctl {action}: {err}"))?;
        if status.success() {
            return Ok(());
        }
        if Path::new(LINUX_HOSTCTL).is_file() {
            let status = Command::new("pkexec")
                .args([LINUX_HOSTCTL, action])
                .status()
                .map_err(|err| format!("pkexec eclipse-hostctl {action}: {err}"))?;
            if status.success() {
                return Ok(());
            }
        }
        if action == "start" && host_service_active() {
            return Ok(());
        }
        if action == "stop" && !host_service_active() {
            return Ok(());
        }
        Err(format!(
            "could not {action} {LINUX_HOST_SERVICE} (install polkit helper or run as root)"
        ))
    }
}

fn host_service_active() -> bool {
    #[cfg(windows)]
    {
        let Ok(out) = Command::new("sc")
            .args(["query", WINDOWS_HOST_SERVICE])
            .output()
        else {
            return false;
        };
        String::from_utf8_lossy(&out.stdout).contains("RUNNING")
    }
    #[cfg(not(windows))]
    {
        Command::new("systemctl")
            .args(["is-active", "--quiet", LINUX_HOST_SERVICE])
            .status()
            .map(|status| status.success())
            .unwrap_or(false)
    }
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SidecarStatus {
    pub sunshine_present: bool,
    pub moonlight_present: bool,
    pub sunshine_running: bool,
    pub host_triple: String,
}

impl SidecarStatus {
    pub fn snapshot(host: &HostProcess) -> Self {
        Self {
            sunshine_present: SUNSHINE.is_present(),
            moonlight_present: MOONLIGHT.is_present(),
            sunshine_running: host.sunshine_running(),
            host_triple: env!("HOST_TRIPLE").to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn engine_names_are_stable_contracts() {
        assert_eq!(SUNSHINE.name, "sunshine");
        assert_eq!(MOONLIGHT.name, "moonlight");
    }

    #[test]
    fn sunshine_health_is_local_web_ui() {
        let health = SUNSHINE.health().expect("sunshine exposes a health port");
        assert_eq!(health.port, 47990);
        assert_eq!(health.url(), "https://127.0.0.1:47990");
        assert!(MOONLIGHT.health().is_none());
    }

    #[test]
    fn probe_tcp_accepts_bound_listener() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let handle = std::thread::spawn(move || {
            let _ = listener.accept();
        });
        assert!(probe_tcp("127.0.0.1", port, Duration::from_secs(1)));
        let _ = handle.join();
    }

    #[test]
    fn probe_tcp_fails_on_closed_port() {
        assert!(!probe_tcp("127.0.0.1", 1, Duration::from_millis(200)));
    }

    #[test]
    fn triple_filenames_include_engine_and_host() {
        let sun = SUNSHINE.triple_filename();
        let moon = MOONLIGHT.triple_filename();
        assert!(sun.contains("sunshine"), "{sun}");
        assert!(moon.contains("moonlight"), "{moon}");
        assert!(sun.contains(env!("HOST_TRIPLE")), "{sun}");
        assert_ne!(sun, moon);
    }

    #[test]
    fn sunshine_creds_present_requires_username_password_salt() {
        let dir = std::env::temp_dir().join(format!(
            "eclipse-creds-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("sunshine_state.json");
        std::fs::write(&path, r#"{"unique_id":"x"}"#).unwrap();
        assert!(!sunshine_creds_present(&path));
        std::fs::write(
            &path,
            r#"{"username":"eclipse","password":"hash","salt":"abc"}"#,
        )
        .unwrap();
        assert!(sunshine_creds_present(&path));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn linux_sunshine_state_path_prefers_system_dir_when_present() {
        let root = unique_temp("eclipse-sys-sunshine");
        let system = root.join("eclipse").join("sunshine");
        std::fs::create_dir_all(&system).unwrap();
        let path =
            linux_sunshine_state_path(&system, Some(root.join("xdg")), Some(root.join("home")));
        assert_eq!(path, system.join("sunshine_state.json"));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn linux_sunshine_state_path_uses_xdg_when_system_dir_missing() {
        let root = unique_temp("eclipse-xdg-sunshine");
        let missing = root.join("missing-system");
        let path =
            linux_sunshine_state_path(&missing, Some(root.join("xdg")), Some(root.join("home")));
        assert_eq!(
            path,
            root.join("xdg")
                .join("sunshine")
                .join("sunshine_state.json")
        );
        assert_eq!(LINUX_SYSTEM_SUNSHINE_DIR, "/etc/eclipse/sunshine");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn linux_host_unit_installed_checks_candidate_paths() {
        let root = unique_temp("eclipse-unit");
        std::fs::create_dir_all(&root).unwrap();
        let unit = root.join("eclipse-host.service");
        assert!(!linux_host_unit_installed(&[&unit]));
        std::fs::write(&unit, "[Unit]\n").unwrap();
        assert!(linux_host_unit_installed(&[&unit]));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn stop_sunshine_is_a_no_op_without_a_child() {
        let host = HostProcess::new();
        host.stop_sunshine();
        // No child and (usually) nothing on :47990 in unit tests.
        let _ = host.sunshine_running();
    }

    #[test]
    fn sunshine_running_follows_health_probe_without_a_child() {
        let _listener = std::net::TcpListener::bind("127.0.0.1:47990");
        let host = HostProcess::new();
        if SUNSHINE.probe_health(HOST_HEALTH_TIMEOUT) {
            assert!(host.sunshine_running());
        }
        host.stop_sunshine();
        if SUNSHINE.probe_health(HOST_HEALTH_TIMEOUT) {
            assert!(
                host.sunshine_running(),
                "stop_sunshine must not take down a host we did not spawn"
            );
        }
    }

    fn unique_temp(prefix: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "{prefix}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ))
    }

    #[test]
    fn sunshine_asset_candidates_include_packaged_lib_dir() {
        let bin = PathBuf::from("/usr/bin");
        let c = sunshine_asset_candidates(Some(&bin));
        assert!(c.contains(&PathBuf::from("/usr/lib/Eclipse/share/sunshine")));
        assert!(c.contains(&PathBuf::from("/usr/bin/share/sunshine")));
    }

    #[test]
    fn prepare_sunshine_share_uses_usr_lib_eclipse_without_copy() {
        let root = unique_temp("eclipse-pkg-layout");
        let usr = root.join("usr");
        let bin_dir = usr.join("bin");
        let share = usr
            .join("lib")
            .join("Eclipse")
            .join("share")
            .join("sunshine");
        std::fs::create_dir_all(&bin_dir).unwrap();
        std::fs::create_dir_all(&share).unwrap();
        std::fs::write(share.join("apps.json"), r#"{"apps":[]}"#).unwrap();
        let (cwd, staged) = prepare_sunshine_share(&bin_dir).expect("packaged assets");
        assert_eq!(cwd, usr.join("lib").join("Eclipse"));
        assert_eq!(staged, share);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn copy_dir_all_copies_nested_files() {
        let root = unique_temp("eclipse-copy");
        let src = root.join("src");
        let dst = root.join("dst");
        std::fs::create_dir_all(src.join("web")).unwrap();
        std::fs::write(src.join("apps.json"), "{}").unwrap();
        std::fs::write(src.join("web").join("index.html"), "<html>").unwrap();
        copy_dir_all(&src, &dst).unwrap();
        assert_eq!(
            std::fs::read_to_string(dst.join("apps.json")).unwrap(),
            "{}"
        );
        assert_eq!(
            std::fs::read_to_string(dst.join("web").join("index.html")).unwrap(),
            "<html>"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn stage_sunshine_share_creates_compile_time_layout() {
        let root = unique_temp("eclipse-stage");
        let assets = root.join("resources").join("sunshine");
        let bin_dir = root.join("debug");
        std::fs::create_dir_all(&assets).unwrap();
        std::fs::create_dir_all(&bin_dir).unwrap();
        std::fs::write(assets.join("apps.json"), r#"{"apps":[]}"#).unwrap();
        let dest = stage_sunshine_share(&bin_dir, &assets).unwrap();
        assert_eq!(dest, bin_dir.join("share").join("sunshine"));
        assert_eq!(
            std::fs::read_to_string(dest.join("apps.json")).unwrap(),
            r#"{"apps":[]}"#
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn stage_sunshine_share_errors_when_assets_missing() {
        let root = unique_temp("eclipse-stage-missing");
        std::fs::create_dir_all(&root).unwrap();
        let err = stage_sunshine_share(&root, &root.join("missing")).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::NotFound);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn stage_sunshine_share_skips_copy_when_already_in_place() {
        let root = unique_temp("eclipse-stage-same");
        let dest = root.join("share").join("sunshine");
        std::fs::create_dir_all(&dest).unwrap();
        std::fs::write(dest.join("apps.json"), "keep").unwrap();
        let staged = stage_sunshine_share(root.as_path(), &dest).unwrap();
        assert_eq!(staged, dest);
        assert_eq!(
            std::fs::read_to_string(dest.join("apps.json")).unwrap(),
            "keep"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn stage_moonlight_runtime_copies_qt_loader_files() {
        let root = unique_temp("eclipse-moon-runtime");
        let runtime = root.join("runtime");
        let bin_dir = root.join("debug");
        std::fs::create_dir_all(runtime.join("platforms")).unwrap();
        std::fs::create_dir_all(&bin_dir).unwrap();
        std::fs::write(runtime.join("Qt6Core.dll"), "qt").unwrap();
        std::fs::write(runtime.join("platforms").join("qwindows.dll"), "plugin").unwrap();
        stage_moonlight_runtime(&bin_dir, &runtime).unwrap();
        assert!(moonlight_runtime_ready(&bin_dir));
        assert_eq!(
            std::fs::read_to_string(bin_dir.join("Qt6Core.dll")).unwrap(),
            "qt"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn stage_moonlight_runtime_skips_when_already_ready() {
        let root = unique_temp("eclipse-moon-ready");
        let runtime = root.join("runtime");
        let bin_dir = root.join("debug");
        std::fs::create_dir_all(runtime.join("platforms")).unwrap();
        std::fs::create_dir_all(bin_dir.join("platforms")).unwrap();
        std::fs::write(runtime.join("Qt6Core.dll"), "new").unwrap();
        std::fs::write(runtime.join("platforms").join("qwindows.dll"), "new").unwrap();
        std::fs::write(bin_dir.join("Qt6Core.dll"), "old").unwrap();
        std::fs::write(bin_dir.join("platforms").join("qwindows.dll"), "old").unwrap();
        stage_moonlight_runtime(&bin_dir, &runtime).unwrap();
        assert_eq!(
            std::fs::read_to_string(bin_dir.join("Qt6Core.dll")).unwrap(),
            "old"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn stage_moonlight_runtime_errors_when_runtime_missing() {
        let root = unique_temp("eclipse-moon-missing");
        std::fs::create_dir_all(&root).unwrap();
        let err = stage_moonlight_runtime(&root, &root.join("missing")).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::NotFound);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn source_paths_live_under_binaries() {
        let path = SUNSHINE.source_path();
        assert!(path.ends_with(SUNSHINE.triple_filename()));
        assert_eq!(
            path.parent().and_then(|p| p.file_name()).unwrap(),
            "binaries"
        );
    }

    #[test]
    fn sunshine_smoke_binds_health_port() {
        if std::env::var_os("ECLIPSE_SMOKE").is_none() {
            return;
        }
        let timeout = Duration::from_secs(2);
        if SUNSHINE.probe_health(timeout) {
            panic!("port 47990 already in use; stop Eclipse/Sunshine and retry");
        }
        let Some(_) = SUNSHINE.resolve() else {
            panic!("sunshine sidecar missing; run ./scripts/build-sidecars.sh");
        };
        let host = HostProcess::new();
        host.start_sunshine();
        let deadline = std::time::Instant::now() + Duration::from_secs(20);
        let mut up = false;
        while std::time::Instant::now() < deadline {
            if SUNSHINE.probe_health(timeout) {
                up = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(200));
        }
        host.stop_sunshine();
        assert!(up, "Sunshine did not accept TCP on 47990 after spawn");
    }
}
