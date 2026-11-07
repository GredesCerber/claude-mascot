pub mod activities;
pub mod engine;
#[cfg(windows)]
pub mod media;
#[cfg(windows)]
pub mod platform;
pub mod render;
pub mod settings;
pub mod social;
pub mod timing;
#[cfg(windows)]
pub mod update;
#[cfg(windows)]
pub mod update_apply;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec2 {
    pub x: f64,
    pub y: f64,
}
impl Vec2 {
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}
impl Rect {
    pub fn new(x: f64, y: f64, w: f64, h: f64) -> Self {
        Self { x, y, w, h }
    }
    pub fn right(self) -> f64 {
        self.x + self.w
    }
    pub fn bottom(self) -> f64 {
        self.y + self.h
    }
    pub fn contains(self, p: Vec2) -> bool {
        p.x >= self.x && p.x < self.right() && p.y >= self.y && p.y < self.bottom()
    }
    pub fn is_valid(self) -> bool {
        self.x.is_finite()
            && self.y.is_finite()
            && self.w.is_finite()
            && self.h.is_finite()
            && self.w > 0.0
            && self.h > 0.0
            && self.right().is_finite()
            && self.bottom().is_finite()
    }
}

#[derive(Clone, Debug)]
pub struct WindowInfo {
    pub id: usize,
    pub rect: Rect,
    pub title: String,
    pub active: bool,
    pub fullscreen: bool,
}

#[derive(Clone, Debug)]
pub struct World {
    /// Visible foreign windows, bottom to top in stacking order.
    pub windows: Vec<WindowInfo>,
    pub work_areas: Vec<Rect>,
    pub cursor: Vec2,
    pub idle_seconds: f64,
    /// Populated only by a media provider, never inferred as actual playback from a title.
    pub video_window: Option<usize>,
}
impl Default for World {
    fn default() -> Self {
        Self {
            windows: vec![],
            work_areas: vec![Rect::new(0.0, 0.0, 1920.0, 1040.0)],
            cursor: Vec2::default(),
            idle_seconds: 0.0,
            video_window: None,
        }
    }
}
impl World {
    pub fn area_at(&self, p: Vec2) -> Rect {
        self.work_areas
            .iter()
            .copied()
            .filter(|a| a.is_valid())
            .find(|a| a.contains(p))
            .or_else(|| {
                self.work_areas
                    .iter()
                    .copied()
                    .filter(|a| a.is_valid())
                    .min_by(|a, b| {
                        let distance = |r: Rect| {
                            let dx = p.x - p.x.clamp(r.x, r.right());
                            let dy = p.y - p.y.clamp(r.y, r.bottom());
                            dx * dx + dy * dy
                        };
                        distance(*a).total_cmp(&distance(*b))
                    })
            })
            .unwrap_or(Rect::new(0.0, 0.0, 1920.0, 1040.0))
    }
    pub fn fullscreen_active(&self) -> bool {
        self.windows.iter().any(|w| w.active && w.fullscreen)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn nearest_monitor_ignores_invalid_geometry() {
        let w = World {
            work_areas: vec![
                Rect::new(0.0, 0.0, -10.0, 10.0),
                Rect::new(f64::NAN, 0.0, 10.0, 10.0),
                Rect::new(-1920.0, 0.0, 1920.0, 1040.0),
            ],
            ..World::default()
        };
        assert_eq!(w.area_at(Vec2::new(-100.0, 200.0)).x, -1920.0);
        assert_eq!(w.area_at(Vec2::new(100.0, 200.0)).x, -1920.0);
    }
}
