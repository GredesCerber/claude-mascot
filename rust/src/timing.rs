//! Frame pacing and fixed-step timing helpers.
use crate::Vec2;
use std::collections::VecDeque;
use std::time::{Duration, Instant};

pub const MIN_FPS: u32 = 30;
pub const MAX_FPS: u32 = 240;
pub const DEFAULT_FPS: u32 = 60;
pub const SIMULATION_HZ: u32 = 240;
pub const MAX_CATCH_UP: Duration = Duration::from_millis(100);
pub const DRAG_SAMPLE_WINDOW: Duration = Duration::from_millis(120);

pub fn valid_fps(fps: u32) -> bool {
    (MIN_FPS..=MAX_FPS).contains(&fps)
}

pub fn frame_interval(fps: u32) -> Duration {
    Duration::from_secs_f64(1.0 / fps.clamp(MIN_FPS, MAX_FPS) as f64)
}

/// Maintains a cadence against absolute deadlines, avoiding accumulated sleep drift.
#[derive(Clone, Debug)]
pub struct FramePacer {
    deadline: Instant,
    interval: Duration,
}

impl FramePacer {
    pub fn new(now: Instant, fps: u32) -> Self {
        let interval = frame_interval(fps);
        Self {
            deadline: now,
            interval,
        }
    }

    pub fn set_fps(&mut self, now: Instant, fps: u32) {
        self.interval = frame_interval(fps);
        self.deadline = now;
    }

    pub fn due(&self, now: Instant) -> bool {
        now >= self.deadline
    }

    pub fn time_until_due(&self, now: Instant) -> Duration {
        self.deadline.saturating_duration_since(now)
    }

    /// Call after a frame. Missed deadlines are skipped instead of triggering bursts.
    pub fn advance(&mut self, now: Instant) {
        self.deadline += self.interval;
        if self.deadline <= now {
            self.deadline = now + self.interval;
        }
    }
}

/// Estimates pointer velocity over a short rolling window, avoiding spikes when
/// Windows delivers multiple pointer events with the same timestamp.
#[derive(Clone, Debug, Default)]
pub struct DragVelocity {
    samples: VecDeque<(Instant, Vec2)>,
}

impl DragVelocity {
    pub fn begin(&mut self, at: Instant, position: Vec2) {
        self.samples.clear();
        self.samples.push_back((at, position));
    }

    pub fn push(&mut self, at: Instant, position: Vec2) -> Vec2 {
        self.samples.push_back((at, position));
        while self.samples.len() > 2 && at.duration_since(self.samples[0].0) > DRAG_SAMPLE_WINDOW {
            self.samples.pop_front();
        }
        let (Some((first_at, first)), Some((last_at, last))) =
            (self.samples.front(), self.samples.back())
        else {
            return Vec2::default();
        };
        let dt = last_at.duration_since(*first_at).as_secs_f64();
        if dt <= 0.0 {
            return Vec2::default();
        }
        Vec2::new(
            ((last.x - first.x) / dt).clamp(-1800.0, 1800.0),
            ((last.y - first.y) / dt).clamp(-1800.0, 1800.0),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_requested_range_and_calculates_interval() {
        assert!(!valid_fps(29));
        assert!(valid_fps(30));
        assert!(valid_fps(240));
        assert!(!valid_fps(241));
        assert_eq!(frame_interval(120), Duration::from_nanos(8_333_333));
    }

    #[test]
    fn pacer_uses_deadlines_and_skips_a_late_frame() {
        let start = Instant::now();
        let mut pacer = FramePacer::new(start, 60);
        assert!(pacer.due(start));
        pacer.advance(start);
        assert!(!pacer.due(start));
        assert_eq!(pacer.time_until_due(start), frame_interval(60));
        let late = start + Duration::from_millis(100);
        assert!(pacer.due(late));
        pacer.advance(late);
        assert_eq!(pacer.time_until_due(late), frame_interval(60));
    }

    #[test]
    fn drag_velocity_uses_rolling_samples_and_ignores_duplicate_timestamps() {
        let t = Instant::now();
        let mut drag = DragVelocity::default();
        drag.begin(t, Vec2::new(0.0, 0.0));
        assert_eq!(drag.push(t, Vec2::new(10.0, 0.0)), Vec2::default());
        assert_eq!(
            drag.push(t + Duration::from_millis(50), Vec2::new(60.0, 0.0)),
            Vec2::new(1200.0, 0.0)
        );
        assert_eq!(
            drag.push(t + Duration::from_millis(150), Vec2::new(160.0, 0.0)),
            Vec2::new(1000.0, 0.0)
        );
    }

    #[test]
    fn stationary_release_does_not_reuse_old_throw_velocity() {
        let t = Instant::now();
        let mut drag = DragVelocity::default();
        drag.begin(t, Vec2::default());
        assert!(
            drag.push(t + Duration::from_millis(50), Vec2::new(60.0, 0.0))
                .x
                > 0.0
        );
        assert_eq!(
            drag.push(t + Duration::from_millis(250), Vec2::new(60.0, 0.0)),
            Vec2::default()
        );
    }

    #[test]
    fn release_position_contributes_to_throw_velocity() {
        let t = Instant::now();
        let mut drag = DragVelocity::default();
        drag.begin(t, Vec2::default());
        drag.push(t + Duration::from_millis(50), Vec2::new(10.0, 0.0));
        let release = drag.push(t + Duration::from_millis(100), Vec2::new(80.0, -40.0));
        assert_eq!(release, Vec2::new(800.0, -400.0));
    }
}
