//! One body palette shared by every procedural Claude pose.
pub const DEFAULT_BODY_COLOR: u32 = 0xD97757;
pub const PRESETS: &[(u16, &str, u32)] = &[
    (180, "Claude", DEFAULT_BODY_COLOR),
    (181, "Коралловый", 0xE76F7B),
    (182, "Золотой", 0xD7AA48),
    (183, "Мятный", 0x63B69A),
    (184, "Небесный", 0x609BD5),
    (185, "Лавандовый", 0xA08BCD),
    (186, "Розовый", 0xCF80AD),
    (187, "Графитовый", 0x565D69),
];

pub fn distinct_color(index: usize) -> u32 {
    PRESETS[index % PRESETS.len()].2
}

/// Prefer an unused preset when adding a friend to an existing company.
pub fn unused_color(colors: impl IntoIterator<Item = u32>, start: usize) -> u32 {
    let used: Vec<_> = colors.into_iter().take(8).map(|c| c & 0xFFFFFF).collect();
    (0..PRESETS.len())
        .map(|offset| distinct_color(start.wrapping_add(offset)))
        .find(|color| !used.contains(color))
        .unwrap_or_else(|| distinct_color(start))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Palette {
    pub body: u32,
    pub shadow: u32,
    pub highlight: u32,
    pub eyes: u32,
}
impl Palette {
    pub fn from_rgb(rgb: u32) -> Self {
        let rgb = rgb & 0xFFFFFF;
        if rgb == DEFAULT_BODY_COLOR {
            return Self {
                body: 0xFFD97757,
                shadow: 0xFFB85C3E,
                highlight: 0xFFE8957A,
                eyes: 0xFF1B1B1B,
            };
        }
        let tint = |light: bool| {
            let channel = |shift: u32| {
                let value = (rgb >> shift) & 255u32;
                if light {
                    value + (255 - value) * 22 / 100
                } else {
                    value * 72 / 100
                }
            };
            0xFF000000 | (channel(16) << 16) | (channel(8) << 8) | channel(0)
        };
        let luminance =
            (((rgb >> 16) & 255) * 299 + ((rgb >> 8) & 255) * 587 + (rgb & 255) * 114) / 1000;
        Self {
            body: 0xFF000000 | rgb,
            shadow: tint(false),
            highlight: tint(true),
            eyes: if luminance < 72 {
                0xFFF3EDE2
            } else {
                0xFF1B1B1B
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn original_palette_is_exact_and_dark_colors_keep_visible_eyes() {
        assert_eq!(Palette::from_rgb(DEFAULT_BODY_COLOR).shadow, 0xFFB85C3E);
        assert_eq!(Palette::from_rgb(0).eyes, 0xFFF3EDE2);
        let blue = Palette::from_rgb(0x609BD5);
        assert_eq!(blue.body, 0xFF609BD5);
        assert!(blue.shadow & 0xFFFFFF < blue.body & 0xFFFFFF);
        assert!(blue.highlight & 0xFFFFFF > blue.body & 0xFFFFFF);
    }
}

#[cfg(test)]
mod unused_color_tests {
    use super::*;
    #[test]
    fn adding_after_removal_or_custom_recoloring_uses_a_free_preset() {
        for (missing, preset) in PRESETS.iter().enumerate() {
            let used: Vec<_> = PRESETS
                .iter()
                .enumerate()
                .filter(|(i, _)| *i != missing)
                .map(|(_, p)| p.2)
                .collect();
            for start in 0..PRESETS.len() {
                assert_eq!(unused_color(used.clone(), start), preset.2);
            }
        }
        assert_eq!(
            unused_color([0x123456, distinct_color(2)], 2),
            distinct_color(3)
        );
    }
}
