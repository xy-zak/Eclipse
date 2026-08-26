//! Native / Half / Quarter stream sizes derived from a host mode.

use serde::Serialize;

/// Even integer divisors of a host mode that stay large enough to encode.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScaleOption {
    pub denominator: u32,
    pub width: u32,
    pub height: u32,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scale_3840x2160() {
        let opts = scale_options(3840, 2160);
        assert_eq!(
            opts.iter()
                .map(|o| (o.denominator, o.width, o.height))
                .collect::<Vec<_>>(),
            vec![(1, 3840, 2160), (2, 1920, 1080), (4, 960, 540)]
        );
        assert_eq!(default_denominator(3840, 2160, 1920, 1080), 2);
    }

    #[test]
    fn scale_2880x1920() {
        let opts = scale_options(2880, 1920);
        assert_eq!(
            opts.iter()
                .map(|o| (o.denominator, o.width, o.height))
                .collect::<Vec<_>>(),
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
            opts.iter()
                .map(|o| (o.denominator, o.width, o.height))
                .collect::<Vec<_>>(),
            vec![(1, 2560, 1440), (2, 1280, 720), (4, 640, 360)]
        );
        assert!(!opts.iter().any(|o| o.denominator == 3));
    }

    #[test]
    fn scale_3440x1440() {
        let opts = scale_options(3440, 1440);
        assert_eq!(
            opts[0],
            ScaleOption {
                denominator: 1,
                width: 3440,
                height: 1440
            }
        );
        assert_eq!(
            opts[1],
            ScaleOption {
                denominator: 2,
                width: 1720,
                height: 720
            }
        );
        assert_eq!(
            opts[2],
            ScaleOption {
                denominator: 4,
                width: 860,
                height: 360
            }
        );
    }
}
