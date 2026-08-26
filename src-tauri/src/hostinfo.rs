//! Local OS pretty-name and window-manager / session type for the dashboard.

#[cfg(target_os = "linux")]
use std::fs;

/// Distro/version for this machine (`Linux Arch`, `Windows 11 24H2`).
pub fn local_os() -> String {
    #[cfg(target_os = "linux")]
    {
        linux_from_os_release(&read_os_release()).unwrap_or_else(|| "Linux".into())
    }
    #[cfg(windows)]
    {
        windows_os_detail()
    }
    #[cfg(not(any(target_os = "linux", windows)))]
    {
        let os = std::env::consts::OS;
        let mut chars = os.chars();
        match chars.next() {
            Some(first) => format!("{}{}", first.to_uppercase(), chars.as_str()),
            None => String::new(),
        }
    }
}

/// Session compositor / protocol (`Wayland`, `Hyprland (Wayland)`, `X11`,
/// `Desktop Window Manager`). Empty when nothing can be detected.
pub fn local_window_manager() -> String {
    #[cfg(windows)]
    {
        return "Desktop Window Manager".into();
    }
    #[cfg(not(windows))]
    {
        let from_env = describe_window_manager(
            std::env::var("XDG_SESSION_TYPE").ok().as_deref(),
            std::env::var("XDG_CURRENT_DESKTOP").ok().as_deref(),
            std::env::var("HYPRLAND_INSTANCE_SIGNATURE").ok().as_deref(),
            std::env::var("WAYLAND_DISPLAY").ok().as_deref(),
            std::env::var("DISPLAY").ok().as_deref(),
        );
        if !from_env.is_empty() {
            return from_env;
        }
        #[cfg(target_os = "linux")]
        {
            return wayland_from_runtime_dir().unwrap_or_default();
        }
        #[cfg(not(target_os = "linux"))]
        {
            String::new()
        }
    }
}

#[cfg(target_os = "linux")]
fn read_os_release() -> String {
    fs::read_to_string("/etc/os-release").unwrap_or_default()
}

fn os_release_field(body: &str, key: &str) -> String {
    let prefix = format!("{key}=");
    for raw in body.lines() {
        let line = raw.trim_end_matches('\r');
        let Some(value) = line.strip_prefix(&prefix) else {
            continue;
        };
        let value = value.trim();
        if value.len() >= 2 && value.starts_with('"') && value.ends_with('"') {
            return value[1..value.len() - 1].to_string();
        }
        return value.to_string();
    }
    String::new()
}

fn distro_short(name: &str) -> String {
    let mut s = name.trim();
    s = s.strip_suffix(" GNU/Linux").unwrap_or(s);
    s = s.strip_suffix(" Linux").unwrap_or(s);
    s = s.strip_prefix("Linux ").unwrap_or(s);
    s.trim().to_string()
}

fn title_id(id: &str) -> String {
    let id = id.trim();
    let mut chars = id.chars();
    match chars.next() {
        Some(first) => format!("{}{}", first.to_uppercase(), chars.as_str()),
        None => String::new(),
    }
}

pub(crate) fn linux_from_os_release(body: &str) -> Option<String> {
    let name = os_release_field(body, "NAME");
    let id = os_release_field(body, "ID");
    let mut distro = distro_short(&name);
    if distro.is_empty() {
        distro = title_id(&id);
    }
    if distro.is_empty() {
        return None;
    }
    let mut version = os_release_field(body, "VERSION_ID");
    if version.is_empty() {
        version = os_release_field(body, "VERSION");
        if let Some((first, _)) = version.split_once(' ') {
            version = first.to_string();
        }
    }
    if version.is_empty() || version.eq_ignore_ascii_case("rolling") {
        Some(format!("Linux {distro}"))
    } else {
        Some(format!("Linux {distro} {version}"))
    }
}

fn first_desktop(current_desktop: Option<&str>) -> String {
    current_desktop
        .unwrap_or("")
        .split(':')
        .next()
        .unwrap_or("")
        .trim()
        .to_string()
}

pub(crate) fn describe_window_manager(
    session_type: Option<&str>,
    current_desktop: Option<&str>,
    hyprland_signature: Option<&str>,
    wayland_display: Option<&str>,
    display: Option<&str>,
) -> String {
    let desktop = first_desktop(current_desktop);
    let hypr = hyprland_signature.is_some_and(|s| !s.is_empty())
        || desktop.eq_ignore_ascii_case("hyprland");
    if hypr {
        return "Hyprland (Wayland)".into();
    }

    let session = session_type
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_ascii_lowercase);
    let protocol = session.or_else(|| {
        if wayland_display.is_some_and(|s| !s.is_empty()) {
            Some("wayland".into())
        } else if display.is_some_and(|s| !s.is_empty()) {
            Some("x11".into())
        } else {
            None
        }
    });

    match protocol.as_deref() {
        Some("wayland") => {
            if !desktop.is_empty() && !desktop.eq_ignore_ascii_case("wayland") {
                format!("{desktop} (Wayland)")
            } else {
                "Wayland".into()
            }
        }
        Some("x11") => {
            if !desktop.is_empty() && !desktop.eq_ignore_ascii_case("x11") {
                format!("{desktop} (X11)")
            } else {
                "X11".into()
            }
        }
        _ => desktop,
    }
}

#[cfg(target_os = "linux")]
fn wayland_from_runtime_dir() -> Option<String> {
    let root = fs::read_dir("/run/user").ok()?;
    for entry in root.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let hypr = path.join("hypr").is_dir();
        let wayland = fs::read_dir(&path).ok().is_some_and(|dir| {
            dir.flatten().any(|f| {
                f.file_name()
                    .to_string_lossy()
                    .starts_with("wayland-")
            })
        });
        if hypr {
            return Some("Hyprland (Wayland)".into());
        }
        if wayland {
            return Some("Wayland".into());
        }
    }
    None
}

#[cfg(any(windows, test))]
pub(crate) fn format_windows_os(build: u32, display_version: &str) -> String {
    let family = if build >= 22000 {
        "Windows 11"
    } else if build >= 10240 {
        "Windows 10"
    } else {
        "Windows"
    };
    let display_version = display_version.trim();
    if display_version.is_empty() {
        family.into()
    } else {
        format!("{family} {display_version}")
    }
}

#[cfg(windows)]
fn windows_os_detail() -> String {
    match windows_nt_current_version() {
        Some((build, display_version)) => format_windows_os(build, &display_version),
        None => "Windows".into(),
    }
}

#[cfg(windows)]
fn windows_nt_current_version() -> Option<(u32, String)> {
    type Hkey = *mut std::ffi::c_void;
    const HKEY_LOCAL_MACHINE: Hkey = 0x8000_0002u32 as usize as Hkey;
    const KEY_READ: u32 = 0x20019;
    const REG_SZ: u32 = 1;
    const REG_EXPAND_SZ: u32 = 2;

    #[link(name = "advapi32")]
    extern "system" {
        fn RegOpenKeyExW(
            hkey: Hkey,
            sub_key: *const u16,
            options: u32,
            sam: u32,
            result: *mut Hkey,
        ) -> i32;
        fn RegQueryValueExW(
            hkey: Hkey,
            name: *const u16,
            reserved: *mut u32,
            ty: *mut u32,
            data: *mut u8,
            size: *mut u32,
        ) -> i32;
        fn RegCloseKey(hkey: Hkey) -> i32;
    }

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn query_sz(key: Hkey, name: &str) -> Option<String> {
        let name_w = wide(name);
        let mut ty = 0u32;
        let mut size = 0u32;
        unsafe {
            let _ = RegQueryValueExW(
                key,
                name_w.as_ptr(),
                std::ptr::null_mut(),
                &mut ty,
                std::ptr::null_mut(),
                &mut size,
            );
        }
        if size == 0 {
            return None;
        }
        let mut buf = vec![0u8; size as usize];
        unsafe {
            if RegQueryValueExW(
                key,
                name_w.as_ptr(),
                std::ptr::null_mut(),
                &mut ty,
                buf.as_mut_ptr(),
                &mut size,
            ) != 0
            {
                return None;
            }
        }
        if ty != REG_SZ && ty != REG_EXPAND_SZ {
            return None;
        }
        let u16s: Vec<u16> = buf
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        let s = String::from_utf16_lossy(&u16s)
            .trim_end_matches('\0')
            .trim()
            .to_string();
        if s.is_empty() { None } else { Some(s) }
    }

    let sub = wide(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion");
    let mut key: Hkey = std::ptr::null_mut();
    unsafe {
        if RegOpenKeyExW(
            HKEY_LOCAL_MACHINE,
            sub.as_ptr(),
            0,
            KEY_READ,
            &mut key,
        ) != 0
        {
            return None;
        }
    }
    let build = query_sz(key, "CurrentBuildNumber")
        .or_else(|| query_sz(key, "CurrentBuild"))
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let display_version = query_sz(key, "DisplayVersion").unwrap_or_default();
    unsafe {
        let _ = RegCloseKey(key);
    }
    if build == 0 && display_version.is_empty() {
        None
    } else {
        Some((build, display_version))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arch_is_linux_arch() {
        let body = r#"NAME="Arch Linux"
PRETTY_NAME="Arch Linux"
ID=arch
BUILD_ID=rolling
"#;
        assert_eq!(linux_from_os_release(body).as_deref(), Some("Linux Arch"));
    }

    #[test]
    fn ubuntu_includes_version() {
        let body = "NAME=\"Ubuntu\"\nVERSION_ID=\"24.04\"\nID=ubuntu\n";
        assert_eq!(
            linux_from_os_release(body).as_deref(),
            Some("Linux Ubuntu 24.04")
        );
    }

    #[test]
    fn debian_gnu_linux_strips_suffix() {
        let body = "NAME=\"Debian GNU/Linux\"\nVERSION_ID=\"12\"\nID=debian\n";
        assert_eq!(
            linux_from_os_release(body).as_deref(),
            Some("Linux Debian 12")
        );
    }

    #[test]
    fn fedora_linux_strips_suffix() {
        let body = "NAME=\"Fedora Linux\"\nVERSION_ID=\"41\"\nID=fedora\n";
        assert_eq!(
            linux_from_os_release(body).as_deref(),
            Some("Linux Fedora 41")
        );
    }

    #[test]
    fn windows_11_uses_build_and_display_version() {
        assert_eq!(format_windows_os(26100, "24H2"), "Windows 11 24H2");
        assert_eq!(format_windows_os(19045, "22H2"), "Windows 10 22H2");
        assert_eq!(format_windows_os(26100, ""), "Windows 11");
    }

    #[test]
    fn wayland_session_is_wayland() {
        assert_eq!(
            describe_window_manager(Some("wayland"), None, None, None, None),
            "Wayland"
        );
        assert_eq!(
            describe_window_manager(Some("x11"), None, None, None, Some(":0")),
            "X11"
        );
        assert_eq!(
            describe_window_manager(
                Some("wayland"),
                Some("Hyprland"),
                Some("sig"),
                Some("wayland-1"),
                None
            ),
            "Hyprland (Wayland)"
        );
        assert_eq!(
            describe_window_manager(Some("wayland"), Some("GNOME"), None, None, None),
            "GNOME (Wayland)"
        );
    }
}
