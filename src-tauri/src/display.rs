//! Physical display inventory and integer-scale stream sizes.
//!
//! Source of truth is the current pixel mode of each output (Hyprland `width`/`height`,
//! DRM `mode`, Windows `DEVMODE`), not CSS pixels or a hardcoded 1080p stream.

use serde::{Deserialize, Serialize};

#[cfg(target_os = "linux")]
use std::fs;

/// One connected monitor, in physical pixels of the current mode.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DisplayInfo {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub focused: bool,
}

/// Even integer divisors of a host mode that stay large enough to encode.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScaleOption {
    pub denominator: u32,
    pub width: u32,
    pub height: u32,
}

/// Physical current-mode inventory for this machine.
pub fn local_displays() -> Vec<DisplayInfo> {
    os_displays()
}

/// Focused (or first) local display size, for callers that still want a pair.
pub fn focused_size(displays: &[DisplayInfo]) -> (Option<u32>, Option<u32>) {
    let chosen = displays
        .iter()
        .find(|d| d.focused)
        .or_else(|| displays.first());
    match chosen {
        Some(d) => (Some(d.width), Some(d.height)),
        None => (None, None),
    }
}

/// Stream presets: Native (1), Half (2), Quarter (4). Edges are rounded down to even.
pub fn scale_options(width: u32, height: u32) -> Vec<ScaleOption> {
    [1u32, 2, 4]
        .into_iter()
        .filter_map(|n| {
            if width == 0 || height == 0 {
                return Some(ScaleOption {
                    denominator: n,
                    width: 0,
                    height: 0,
                });
            }
            let w = (width / n) & !1;
            let h = (height / n) & !1;
            if w < 2 || h < 2 {
                return None;
            }
            Some(ScaleOption {
                denominator: n,
                width: w,
                height: h,
            })
        })
        .collect()
}

/// Smallest Native / Half / Quarter denominator whose size fits in `max_w`×`max_h` (and 1920×1080).
pub fn default_denominator(width: u32, height: u32, max_w: u32, max_h: u32) -> u32 {
    let options = scale_options(width, height);
    if options.is_empty() {
        return 1;
    }
    let cap_w = max_w.min(1920).max(1);
    let cap_h = max_h.min(1080).max(1);
    if let Some(fit) = options
        .iter()
        .find(|o| o.width <= cap_w && o.height <= cap_h)
    {
        return fit.denominator;
    }
    if options.iter().any(|o| o.denominator == 2) {
        return 2;
    }
    options[0].denominator
}

pub fn apply_scale(width: u32, height: u32, denominator: u32) -> (u32, u32) {
    let options = scale_options(width, height);
    if let Some(opt) = options.iter().find(|o| o.denominator == denominator) {
        return (opt.width, opt.height);
    }
    options
        .first()
        .map(|o| (o.width, o.height))
        .unwrap_or((width.max(1), height.max(1)))
}

pub fn format_displays(displays: &[DisplayInfo]) -> String {
    if displays.is_empty() {
        return "—".into();
    }
    displays
        .iter()
        .map(|d| {
            if d.name.is_empty() {
                format!("{}×{}", d.width, d.height)
            } else {
                format!("{} {}×{}", d.name, d.width, d.height)
            }
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn os_displays() -> Vec<DisplayInfo> {
    #[cfg(target_os = "linux")]
    {
        hyprctl_displays().or_else(drm_displays).unwrap_or_default()
    }
    #[cfg(windows)]
    {
        windows_displays()
    }
    #[cfg(not(any(target_os = "linux", windows)))]
    {
        Vec::new()
    }
}

#[cfg(target_os = "linux")]
fn hyprctl_displays() -> Option<Vec<DisplayInfo>> {
    let out = std::process::Command::new("hyprctl")
        .args(["monitors", "-j"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    parse_hyprctl_monitors(&String::from_utf8_lossy(&out.stdout))
}

pub fn parse_hyprctl_monitors(json: &str) -> Option<Vec<DisplayInfo>> {
    let monitors: Vec<serde_json::Value> = serde_json::from_str(json).ok()?;
    let mut out = Vec::new();
    for m in monitors {
        let w = m.get("width")?.as_u64()? as u32;
        let h = m.get("height")?.as_u64()? as u32;
        if w == 0 || h == 0 {
            continue;
        }
        let name = m
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let focused = m.get("focused").and_then(|v| v.as_bool()).unwrap_or(false);
        out.push(DisplayInfo {
            name,
            width: w,
            height: h,
            focused,
        });
    }
    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

#[cfg(target_os = "linux")]
fn drm_displays() -> Option<Vec<DisplayInfo>> {
    let drm = fs::read_dir("/sys/class/drm").ok()?;
    let mut out = Vec::new();
    for entry in drm {
        let path = entry.ok()?.path();
        let name = path.file_name()?.to_string_lossy().into_owned();
        if !name.contains('-') {
            continue;
        }
        let status = fs::read_to_string(path.join("status")).unwrap_or_default();
        if status.trim() != "connected" {
            continue;
        }
        let current = fs::read_to_string(path.join("mode")).unwrap_or_default();
        let listed = fs::read_to_string(path.join("modes")).ok();
        let mode = current
            .lines()
            .next()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .or_else(|| {
                listed.as_ref().and_then(|m| {
                    m.lines()
                        .next()
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .map(str::to_string)
                })
            });
        let Some(mode) = mode else {
            continue;
        };
        let Some((w, h)) = parse_mode_line(&mode) else {
            continue;
        };
        let pretty = name
            .split_once('-')
            .map(|(_, rest)| rest.to_string())
            .unwrap_or(name);
        out.push(DisplayInfo {
            name: pretty,
            width: w,
            height: h,
            focused: false,
        });
    }
    if out.is_empty() {
        None
    } else {
        if let Some(first) = out.first_mut() {
            first.focused = true;
        }
        Some(out)
    }
}

pub fn parse_mode_line(line: &str) -> Option<(u32, u32)> {
    let (w_s, h_s) = line.trim().split_once('x')?;
    let w: u32 = w_s.parse().ok()?;
    let h: u32 = h_s
        .split(|c: char| !c.is_ascii_digit())
        .next()?
        .parse()
        .ok()?;
    if w == 0 || h == 0 {
        return None;
    }
    Some((w, h))
}

#[cfg(windows)]
fn windows_displays() -> Vec<DisplayInfo> {
    windows_enum::enumerate()
}

#[cfg(windows)]
mod windows_enum {
    use super::DisplayInfo;
    use std::mem::{size_of, zeroed};

    const CCHDEVICENAME: usize = 32;
    const MONITORINFOF_PRIMARY: u32 = 1;
    #[repr(C)]
    struct Rect {
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    }

    #[repr(C)]
    struct MonitorInfoExW {
        cb_size: u32,
        rc_monitor: Rect,
        rc_work: Rect,
        dw_flags: u32,
        sz_device: [u16; CCHDEVICENAME],
    }

    type Hmonitor = *mut std::ffi::c_void;
    type Hdc = *mut std::ffi::c_void;
    type Lparam = isize;

    extern "system" {
        fn EnumDisplayMonitors(
            hdc: Hdc,
            lprc: *const Rect,
            lpfn: unsafe extern "system" fn(Hmonitor, Hdc, *mut Rect, Lparam) -> i32,
            dw_data: Lparam,
        ) -> i32;
        fn GetMonitorInfoW(monitor: Hmonitor, info: *mut MonitorInfoExW) -> i32;
    }

    struct Bag(Vec<DisplayInfo>);

    pub fn enumerate() -> Vec<DisplayInfo> {
        let mut bag = Bag(Vec::new());
        unsafe {
            EnumDisplayMonitors(
                std::ptr::null_mut(),
                std::ptr::null(),
                monitor_cb,
                &mut bag as *mut Bag as Lparam,
            );
        }
        bag.0
    }

    unsafe extern "system" fn monitor_cb(
        monitor: Hmonitor,
        _hdc: Hdc,
        _rc: *mut Rect,
        data: Lparam,
    ) -> i32 {
        let bag = &mut *(data as *mut Bag);
        let mut info: MonitorInfoExW = zeroed();
        info.cb_size = size_of::<MonitorInfoExW>() as u32;
        if GetMonitorInfoW(monitor, &mut info) == 0 {
            return 1;
        }
        let name = String::from_utf16_lossy(&info.sz_device)
            .trim_end_matches('\0')
            .to_string();
        let width = (info.rc_monitor.right - info.rc_monitor.left).max(0) as u32;
        let height = (info.rc_monitor.bottom - info.rc_monitor.top).max(0) as u32;
        if width == 0 || height == 0 {
            return 1;
        }
        bag.0.push(DisplayInfo {
            name,
            width,
            height,
            focused: info.dw_flags & MONITORINFOF_PRIMARY != 0,
        });
        1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hyprctl_lists_every_monitor_and_marks_focused() {
        let json = r#"[{"id":0,"name":"HDMI-A-1","width":1920,"height":1080,"focused":false},{"id":1,"name":"eDP-1","width":2880,"height":1920,"focused":true}]"#;
        let displays = parse_hyprctl_monitors(json).expect("monitors");
        assert_eq!(displays.len(), 2);
        assert_eq!(displays[0].name, "HDMI-A-1");
        assert_eq!(displays[0].width, 1920);
        assert!(!displays[0].focused);
        assert_eq!(displays[1].name, "eDP-1");
        assert_eq!(displays[1].width, 2880);
        assert!(displays[1].focused);
        assert_eq!(focused_size(&displays), (Some(2880), Some(1920)));
    }

    #[test]
    fn drm_mode_line_strips_refresh_suffix() {
        assert_eq!(parse_mode_line("2880x1920"), Some((2880, 1920)));
        assert_eq!(parse_mode_line("1920x1080i"), Some((1920, 1080)));
        assert_eq!(parse_mode_line("3840x2160\n"), Some((3840, 2160)));
        assert_eq!(parse_mode_line(""), None);
    }

    #[test]
    fn scale_3840x2160() {
        let opts = scale_options(3840, 2160);
        assert_eq!(
            opts.iter().map(|o| (o.denominator, o.width, o.height)).collect::<Vec<_>>(),
            vec![(1, 3840, 2160), (2, 1920, 1080), (4, 960, 540)]
        );
        assert_eq!(default_denominator(3840, 2160, 1920, 1080), 2);
    }

    #[test]
    fn scale_2880x1920() {
        let opts = scale_options(2880, 1920);
        assert_eq!(
            opts.iter().map(|o| (o.denominator, o.width, o.height)).collect::<Vec<_>>(),
            vec![(1, 2880, 1920), (2, 1440, 960), (4, 720, 480)]
        );
        assert_eq!(default_denominator(2880, 1920, 1920, 1080), 2);
        assert_eq!(apply_scale(2880, 1920, 2), (1440, 960));
        assert_eq!(apply_scale(2880, 1920, 4), (720, 480));
    }

    #[test]
    fn scale_2560x1440() {
        let opts = scale_options(2560, 1440);
        assert_eq!(
            opts.iter().map(|o| (o.denominator, o.width, o.height)).collect::<Vec<_>>(),
            vec![(1, 2560, 1440), (2, 1280, 720), (4, 640, 360)]
        );
        assert!(!opts.iter().any(|o| o.denominator == 3));
    }

    #[test]
    fn scale_3440x1440() {
        let opts = scale_options(3440, 1440);
        assert_eq!(opts[0], ScaleOption { denominator: 1, width: 3440, height: 1440 });
        assert_eq!(opts[1], ScaleOption { denominator: 2, width: 1720, height: 720 });
        assert_eq!(opts[2], ScaleOption { denominator: 4, width: 860, height: 360 });
    }

    #[test]
    fn format_joins_named_modes() {
        let text = format_displays(&[
            DisplayInfo { name: "eDP-1".into(), width: 2880, height: 1920, focused: true },
            DisplayInfo { name: "HDMI-A-1".into(), width: 1920, height: 1080, focused: false },
        ]);
        assert_eq!(text, "eDP-1 2880×1920, HDMI-A-1 1920×1080");
    }
}
