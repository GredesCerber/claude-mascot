//! Placement of a company in a work area. Positions are body feet anchors.
use crate::{Rect, Vec2};
/// Arrange up to eight pets without body contacts when the work area permits.
pub fn positions(area: Rect, scale: f64, count: usize) -> Vec<Vec2> {
    let count = count.min(8);
    if count == 0 || !area.is_valid() || !scale.is_finite() || scale <= 0.0 {
        return Vec::new();
    }
    let width = 80.0 * scale;
    let height = 60.0 * scale;
    let columns = ((area.w / width).floor() as usize).clamp(1, count);
    let rows = count.div_ceil(columns);
    let x_step = width.min(area.w / columns as f64);
    let y_step = height.min(area.h / rows as f64);
    (0..count)
        .map(|index| {
            let row = index / columns;
            let in_row = (count - row * columns).min(columns);
            let x = area.x
                + area.w / 2.0
                + ((index % columns) as f64 - (in_row - 1) as f64 / 2.0) * x_step;
            let y = area.y + y_step * (row + 1) as f64;
            Vec2::new(x, y)
        })
        .collect()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn all_supported_scales_and_counts_fit_without_contacts() {
        for area in [
            Rect::new(0.0, 0.0, 1920.0, 1080.0),
            Rect::new(-1300.0, -200.0, 130.0, 1000.0),
        ] {
            for scale in [0.75, 1.0, 1.5] {
                for count in 1..=8 {
                    let positions = positions(area, scale, count);
                    assert_eq!(positions.len(), count);
                    for (i, p) in positions.iter().enumerate() {
                        assert!(p.x - 38.0 * scale >= area.x && p.x + 38.0 * scale <= area.right());
                        assert!(p.y - 55.0 * scale >= area.y && p.y <= area.bottom());
                        for q in &positions[..i] {
                            assert!(
                                (p.x - q.x).abs() >= 76.0 * scale
                                    || (p.y - q.y).abs() >= 55.0 * scale
                            );
                        }
                    }
                }
            }
        }
    }
    #[test]
    fn tiny_work_area_is_bounded_and_never_panics() {
        let area = Rect::new(-50.0, -50.0, 20.0, 20.0);
        for p in positions(area, 1.5, 8) {
            assert!(
                area.contains(p) || (p.y == area.bottom() && p.x >= area.x && p.x <= area.right())
            );
        }
        assert!(positions(area, f64::NAN, 2).is_empty());
    }
}
