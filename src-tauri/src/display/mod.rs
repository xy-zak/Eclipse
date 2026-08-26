//! Physical display inventory, stream scale, and the Moonlight host-display map.
//!
//! Local inventory is the current pixel mode of each output (Hyprland `width`/`height`,
//! DRM `mode`, Windows `MONITORINFO`). Remote inventory is Sunshine `GET /api/displays`.
//! The in-stream overlay uses a compact map string (`index;shortcut;name|...`).

mod inventory;
mod map;
mod scale;

pub use map::{parse_host_identity, HostDisplayCache, HostInventory};
pub use scale::{apply_scale, default_denominator, scale_options, ScaleOption};

use serde::{Deserialize, Serialize};

/// One connected monitor, in physical pixels of the current mode.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DisplayInfo {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub focused: bool,
}

/// Physical current-mode inventory for this machine.
pub fn local_displays() -> Vec<DisplayInfo> {
    inventory::os_displays()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn focused_prefers_marked_output() {
        let displays = [
            DisplayInfo {
                name: "HDMI-A-1".into(),
                width: 1920,
                height: 1080,
                focused: false,
            },
            DisplayInfo {
                name: "eDP-1".into(),
                width: 2880,
                height: 1920,
                focused: true,
            },
        ];
        assert_eq!(focused_size(&displays), (Some(2880), Some(1920)));
    }
}
