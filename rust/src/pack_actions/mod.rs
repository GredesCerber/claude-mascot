//! Authored actions for the original 75 illustrations. IDs stay catalog-owned.
use crate::{
    render::{Canvas, FEET_X, FEET_Y, UNIT},
    showcase::Pose,
    Vec2,
};
mod advanced;
mod simple;
#[derive(Clone, Copy, Debug)]
pub(crate) struct Frame {
    pub body: Pose,
    pub gaze: Vec2,
    /// Logical-pixel translation applied only to imported primary props.
    pub primary_shift: Vec2,
    /// Replacement scenes draw their own props; the common renderer owns Claude.
    pub replace_props: bool,
    /// Suppress the legacy per-category generic sparkle/steam layer.
    pub replace_effects: bool,
}
impl Frame {
    pub fn neutral() -> Self {
        Self {
            body: Pose {
                body_x: 0.0,
                bob: 0.0,
                hands: [Vec2::new(-8.0, -8.0), Vec2::new(6.0, -8.0)],
                lifted_pair: None,
            },
            gaze: Vec2::default(),
            primary_shift: Vec2::default(),
            replace_props: true,
            replace_effects: true,
        }
    }
}
pub(crate) fn frame(id: usize, t: f64, duration: f64) -> Option<Frame> {
    let t = crate::render::pose_time(t);
    simple::frame(id, t, duration).or_else(|| advanced::frame(id, t, duration))
}
pub(crate) fn draw(c: &mut Canvas, id: usize, t: f64, facing: f64, duration: f64) {
    let t = crate::render::pose_time(t);
    if t < 0.0 || t >= duration {
        return;
    }
    if simple::frame(id, t, duration).is_some() {
        simple::draw(c, id, t, facing, duration);
    } else {
        advanced::draw(c, id, t, facing, duration);
    }
}
/// Mirror whole logical cells around the common feet anchor.
// The pixel primitive carries explicit mirror, geometry, RGB and opacity.
#[allow(clippy::too_many_arguments)]
pub(crate) fn cell(
    c: &mut Canvas,
    facing: f64,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    rgb: u32,
    alpha: u32,
) {
    let x = if facing < 0.0 { -x - w } else { x };
    c.rect(
        FEET_X + x * UNIT,
        FEET_Y + y * UNIT,
        w * UNIT,
        h * UNIT,
        (alpha.min(255) << 24) | (rgb & 0xFFFFFF),
    );
}
pub(crate) fn alpha(t: f64, duration: f64) -> u32 {
    if t <= 0.0 || t >= duration {
        return 0;
    }
    (((t / 0.25).min((duration - t) / 0.3).clamp(0.0, 1.0) * 3.0).ceil() as u32) * 85
}
pub(crate) fn stage(t: f64, duration: f64, count: usize) -> usize {
    ((t / duration * count as f64).floor() as usize).min(count.saturating_sub(1))
}

/// Only foreground props (glasses, a cup at the mouth, a tool in front of a hand).
pub(crate) fn draw_foreground(c: &mut Canvas, id: usize, t: f64, facing: f64, duration: f64) {
    let t = crate::render::pose_time(t);
    if t < 0.0 || t >= duration {
        return;
    }
    if simple::frame(id, t, duration).is_some() {
        simple::draw_foreground(c, id, t, facing, duration);
    } else {
        advanced::draw_foreground(c, id, t, facing, duration);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn original_catalog_has_exactly_one_authored_owner_per_scene() {
        for spec in &crate::activities::CATALOG[..75] {
            let a = simple::frame(spec.id.index(), 0.0, spec.duration);
            let b = advanced::frame(spec.id.index(), 0.0, spec.duration);
            assert_ne!(
                a.is_some(),
                b.is_some(),
                "missing or duplicated scene {}",
                spec.slug
            );
            let f = frame(spec.id.index(), 0.0, spec.duration).unwrap();
            assert_eq!(f.body.body_x, 0.0, "non-neutral start {}", spec.slug);
            assert_eq!(f.body.bob, 0.0, "non-neutral start {}", spec.slug);
        }
    }
    #[test]
    fn all_authored_prop_layers_close_and_have_bounded_discrete_poses() {
        for spec in &crate::activities::CATALOG[..75] {
            for facing in [-1.0, 1.0] {
                for time in [0.0, spec.duration + 0.1] {
                    let mut c = Canvas::new(280, 240);
                    draw(&mut c, spec.id.index(), time, facing, spec.duration);
                    draw_foreground(&mut c, spec.id.index(), time, facing, spec.duration);
                    assert!(
                        c.pixels.iter().all(|p| *p == 0),
                        "props outside lifetime {}",
                        spec.slug
                    );
                }
            }
            for sample in 0..=(spec.duration * 12.0) as usize {
                let f = frame(spec.id.index(), sample as f64 / 12.0, spec.duration).unwrap();
                for value in [
                    f.body.body_x,
                    f.body.bob,
                    f.primary_shift.x,
                    f.primary_shift.y,
                ] {
                    assert!(
                        value.is_finite() && value.abs() <= 16.0,
                        "unbounded rig {}",
                        spec.slug
                    );
                    assert_eq!(value, value.round(), "subpixel rig {}", spec.slug);
                }
                for hand in f.body.hands {
                    assert!(hand.x.is_finite() && hand.y.is_finite());
                    assert!((-25.0..=25.0).contains(&hand.x) && (-40.0..=0.0).contains(&hand.y));
                }
            }
        }
    }
}
