//! Moonlight overlay map wire format and the session-owned host inventory cache.

use super::DisplayInfo;

const MAP_ROW_SEP: char = '|';
const MAP_COL_SEP: char = ';';

/// Ctrl+Alt+N (or Ctrl+Alt+Shift+N) for overlay rows 0..=8. Index 9+ has no digit shortcut.
fn display_shortcut(index: usize, shift: bool) -> String {
    let key = index + 1;
    if key > 9 {
        return String::new();
    }
    if shift {
        format!("Ctrl+Alt+Shift+{key}")
    } else {
        format!("Ctrl+Alt+{key}")
    }
}

/// Moonlight `--eclipse-display-map` / `ECLIPSE_DISPLAY_MAP` payload.
fn format_display_map(displays: &[DisplayInfo], shift: bool) -> String {
    let mut parts = Vec::with_capacity(displays.len());
    for (i, d) in displays.iter().enumerate() {
        let name = d
            .name
            .replace([MAP_ROW_SEP, MAP_COL_SEP, '\t', '\n', '\r'], " ")
            .trim()
            .to_string();
        parts.push(format!(
            "{i}{MAP_COL_SEP}{}{MAP_COL_SEP}{name}",
            display_shortcut(i, shift)
        ));
    }
    parts.join(&MAP_ROW_SEP.to_string())
}

/// Keep a known inventory when `/api/displays` fails, returns nothing, or
/// shrinks mid-stream (Windows DXGI often lists only the captured output).
fn keep_host_displays(cache: &mut Vec<DisplayInfo>, fetched: Option<Vec<DisplayInfo>>) {
    let Some(displays) = fetched else {
        return;
    };
    if displays.is_empty() {
        return;
    }
    if displays.len() >= cache.len() {
        *cache = displays;
    }
}

/// Session-owned host output list plus the last non-empty Moonlight map.
#[derive(Clone, Debug, Default)]
pub struct HostDisplayCache {
    displays: Vec<DisplayInfo>,
    map: String,
    hotkey_shift: bool,
}

impl HostDisplayCache {
    pub fn displays(&self) -> &[DisplayInfo] {
        &self.displays
    }

    pub fn len(&self) -> usize {
        self.displays.len()
    }

    pub fn map(&self) -> &str {
        &self.map
    }

    pub fn set_hotkey_shift(&mut self, shift: bool) {
        if self.hotkey_shift == shift {
            return;
        }
        self.hotkey_shift = shift;
        self.sync_map();
    }

    pub fn first(&self) -> Option<&DisplayInfo> {
        self.displays.first()
    }

    pub fn get(&self, index: usize) -> Option<&DisplayInfo> {
        self.displays.get(index)
    }

    pub fn name(&self, index: usize) -> Option<&str> {
        self.get(index)
            .map(|d| d.name.as_str())
            .filter(|n| !n.is_empty())
    }

    pub fn focused_index(&self) -> usize {
        self.displays.iter().position(|d| d.focused).unwrap_or(0)
    }

    pub fn clear(&mut self) {
        self.displays.clear();
        self.map.clear();
    }

    pub fn seed(&mut self, known: Vec<DisplayInfo>) {
        if known.is_empty() {
            return;
        }
        self.displays = known;
        self.sync_map();
    }

    pub fn merge_fetch(&mut self, fetched: Option<Vec<DisplayInfo>>) {
        keep_host_displays(&mut self.displays, fetched);
        self.sync_map();
    }

    fn sync_map(&mut self) {
        let next = format_display_map(&self.displays, self.hotkey_shift);
        if !next.is_empty() {
            self.map = next;
        }
    }
}

/// OS / window-manager fields plus displays from Sunshine `GET /api/displays`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HostInventory {
    pub os: String,
    pub window_manager: String,
    pub displays: Vec<DisplayInfo>,
}

fn json_string(value: &serde_json::Value, key: &str) -> String {
    value
        .get(key)
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string()
}

/// Sunshine `GET /api/displays` body, including optional host identity.
pub fn parse_host_identity(value: &serde_json::Value) -> HostInventory {
    HostInventory {
        os: json_string(value, "os"),
        window_manager: json_string(value, "windowManager"),
        displays: parse_host_inventory(value),
    }
}

/// Sunshine `GET /api/displays` body. Zero-size rows are skipped.
pub fn parse_host_inventory(value: &serde_json::Value) -> Vec<DisplayInfo> {
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
        out.push(DisplayInfo {
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

fn format_displays(displays: &[DisplayInfo]) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_joins_named_modes() {
        let text = format_displays(&[
            DisplayInfo {
                name: "eDP-1".into(),
                width: 2880,
                height: 1920,
                focused: true,
            },
            DisplayInfo {
                name: "HDMI-A-1".into(),
                width: 1920,
                height: 1080,
                focused: false,
            },
        ]);
        assert_eq!(text, "eDP-1 2880×1920, HDMI-A-1 1920×1080");
    }

    #[test]
    fn display_map_lists_shortcuts() {
        let displays = [
            DisplayInfo {
                name: "eDP-1".into(),
                width: 1920,
                height: 1080,
                focused: true,
            },
            DisplayInfo {
                name: "HDMI-A-1".into(),
                width: 2560,
                height: 1440,
                focused: false,
            },
        ];
        assert_eq!(
            format_display_map(&displays, false),
            r"0;Ctrl+Alt+1;eDP-1|1;Ctrl+Alt+2;HDMI-A-1"
        );
        assert_eq!(
            format_display_map(&displays, true),
            r"0;Ctrl+Alt+Shift+1;eDP-1|1;Ctrl+Alt+Shift+2;HDMI-A-1"
        );
        assert_eq!(display_shortcut(0, false), "Ctrl+Alt+1");
        assert_eq!(display_shortcut(8, false), "Ctrl+Alt+9");
        assert_eq!(display_shortcut(0, true), "Ctrl+Alt+Shift+1");
        assert_eq!(display_shortcut(9, false), "");
    }

    #[test]
    fn host_cache_keeps_inventory_when_fetch_shrinks() {
        let mut cache = HostDisplayCache::default();
        cache.seed(vec![DisplayInfo {
            name: r"\\.\DISPLAY2".into(),
            width: 1920,
            height: 1080,
            focused: true,
        }]);
        cache.merge_fetch(None);
        cache.merge_fetch(Some(Vec::new()));
        assert_eq!(cache.len(), 1);
        cache.merge_fetch(Some(vec![DisplayInfo {
            name: r"\\.\DISPLAY2".into(),
            width: 1920,
            height: 1080,
            focused: true,
        }]));
        assert_eq!(cache.len(), 1, "must not shrink a known inventory");
        cache.merge_fetch(Some(vec![
            DisplayInfo {
                name: r"\\.\DISPLAY2".into(),
                width: 1920,
                height: 1080,
                focused: true,
            },
            DisplayInfo {
                name: r"\\.\DISPLAY1".into(),
                width: 1920,
                height: 1080,
                focused: false,
            },
        ]));
        assert_eq!(cache.len(), 2);
        assert_eq!(cache.get(1).map(|d| d.name.as_str()), Some(r"\\.\DISPLAY1"));
        assert!(cache.map().contains(r"\\.\DISPLAY1"));
        cache.set_hotkey_shift(true);
        assert!(cache.map().contains("Ctrl+Alt+Shift+1"));
        cache.set_hotkey_shift(false);
        assert!(cache.map().contains("Ctrl+Alt+1"));
        assert!(!cache.map().contains("Ctrl+Alt+Shift+"));
    }

    #[test]
    fn parse_host_inventory_skips_zero_size() {
        let value = serde_json::json!({
            "status": true,
            "displays": [
                {"index": 0, "name": "eDP-1", "width": 2880, "height": 1920, "focused": true},
                {"index": 1, "name": "HDMI-A-1", "width": 0, "height": 0, "focused": false},
                {"index": 2, "name": "DP-1", "width": 1920, "height": 1080, "focused": false}
            ]
        });
        let displays = parse_host_inventory(&value);
        assert_eq!(displays.len(), 2);
        assert_eq!(displays[0].name, "eDP-1");
        assert_eq!(displays[0].width, 2880);
        assert!(displays[0].focused);
        assert_eq!(displays[1].name, "DP-1");
        assert!(!displays[1].focused);
    }

    #[test]
    fn parse_host_identity_reads_os_fields() {
        let value = serde_json::json!({
            "status": true,
            "os": "Linux Arch",
            "windowManager": "Wayland",
            "displays": [
                {"index": 0, "name": "eDP-1", "width": 2880, "height": 1920, "focused": true}
            ]
        });
        let host = parse_host_identity(&value);
        assert_eq!(host.os, "Linux Arch");
        assert_eq!(host.window_manager, "Wayland");
        assert_eq!(host.displays.len(), 1);
    }
}
