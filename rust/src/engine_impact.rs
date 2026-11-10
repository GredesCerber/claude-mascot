//! Expressive knock-back after a genuinely strong impact.
//!
//! A hard body collision, a fast throw into a screen wall, or a hard thrown
//! floor landing hands the body to a short tumble: bounded angular speed that
//! follows the impact direction, a finite bounce budget, a brief dizzy
//! recovery on the ground and an eased return upright. Everything advances on
//! the fixed physics step; the renderer only reads `Pet::tilt`.
use super::{valid_rect, Pet, State, World};
use crate::dialogue::SpeechEvent;
use crate::Vec2;
use std::f64::consts::{PI, TAU};

/// Upper bound of angular speed, radians per second (about 800 degrees/s).
pub(crate) const MAX_SPIN: f64 = 14.0;
const MIN_SPIN: f64 = 5.0;
/// Light air drag on rotation so long flights visibly slow their spin.
const AIR_SPIN_DAMPING: f64 = 0.35;
/// Rate at which a landed body rotates back upright, radians per second.
const UPRIGHT_RATE: f64 = 13.0;
/// A body cannot be re-launched (and receive another lift) inside this window.
const IMPACT_COOLDOWN: f64 = 0.45;
/// Landings slower than this end the tumble instead of bouncing again.
const TUMBLE_BOUNCE_MIN: f64 = 260.0;
const TUMBLE_BOUNCE_RESTITUTION: f64 = 0.45;
/// A thrown body hitting a screen wall faster than this starts tumbling.
pub(super) const WALL_TUMBLE_SPEED: f64 = 900.0;
/// Upward pop given to a grounded body by a strong side hit.
const MAX_LIFT: f64 = 560.0;
/// Speed of the friendly rough-play shove at 100% scale.
const PLAY_SHOVE: Vec2 = Vec2 {
    x: 240.0,
    y: -420.0,
};
const PLAY_SPIN: f64 = 11.0;

/// Direction (sign of x) and strength (velocity change, px/s) of a knock.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Knock {
    pub direction: f64,
    pub strength: f64,
}

impl Pet {
    /// True while a knock-back owns the body in the air.
    pub fn is_tumbling(&self) -> bool {
        matches!(self.state, State::Thrown | State::Fall)
            && (self.spin != 0.0 || self.dizzy_next || self.impact_bounces > 0)
    }

    /// Remaining floor bounces of the current tumble.
    pub fn tumble_bounces_left(&self) -> u8 {
        self.impact_bounces
    }

    /// Current angular speed of the tumble in radians per second.
    pub fn tumble_spin(&self) -> f64 {
        self.spin
    }

    /// Explicitly knock this pet away. Returns false when the pet is held,
    /// was just knocked, or the input is not finite.
    /// `knock.strength` is the horizontal velocity change in px/s.
    pub fn knock_back(&mut self, knock: Knock) -> bool {
        let grounded = !self.is_airborne();
        if !self.start_tumble(knock, grounded, false) {
            return false;
        }
        let direction = if knock.direction < 0.0 { -1.0 } else { 1.0 };
        self.vel.x = (self.vel.x + direction * knock.strength).clamp(-2_200.0, 2_200.0);
        true
    }

    /// `lift` gives a grounded body a bounded upward pop so a side hit
    /// becomes a visible flight instead of a floor slide.
    pub(crate) fn start_tumble(&mut self, knock: Knock, lift: bool, play: bool) -> bool {
        if self.dragging
            || self.impact_cooldown > 0.0
            || !knock.direction.is_finite()
            || !knock.strength.is_finite()
            || knock.strength <= 0.0
        {
            return false;
        }
        let direction = if knock.direction < 0.0 { -1.0 } else { 1.0 };
        let k = ((knock.strength - 250.0) / 900.0).clamp(0.0, 1.0);
        let spin = direction * (MIN_SPIN + (MAX_SPIN - MIN_SPIN) * k);
        // A second hit in the same direction never slows an existing spin;
        // an opposite hit replaces it. Either way the result stays bounded.
        let keep_current =
            self.spin != 0.0 && self.spin.signum() == spin.signum() && self.spin.abs() > spin.abs();
        let next_spin = if keep_current { self.spin } else { spin };
        self.spin = next_spin.clamp(-MAX_SPIN, MAX_SPIN);
        self.impact_bounces = if !play && knock.strength >= 800.0 {
            2
        } else {
            1
        };
        self.dizzy_next = true;
        self.dizzy_time = if play { 0.7 } else { 1.0 + 0.8 * k };
        self.play_tumble = play;
        self.impact_cooldown = IMPACT_COOLDOWN;
        self.release_ownership_for_impact();
        if lift {
            let pop = (180.0 + knock.strength * 0.3).min(MAX_LIFT);
            self.vel.y = self.vel.y.min(-pop);
        }
        self.squash = self.squash.max(0.35);
        self.state = State::Thrown;
        self.state_time = 0.0;
        self.state_duration = 0.0;
        if play {
            self.speak(SpeechEvent::PlayTumble, 1.4);
        } else {
            if self.satisfaction.is_finite() {
                self.satisfaction = (self.satisfaction - 3.0).max(0.0);
            }
            self.speak(SpeechEvent::Knocked, 1.4);
        }
        true
    }

    /// Friendly shove from rough play: a small hop with roughly one roll.
    pub(crate) fn play_knock(&mut self, direction: f64) -> bool {
        if self.dragging || self.is_airborne() || !direction.is_finite() {
            return false;
        }
        let direction = if direction < 0.0 { -1.0 } else { 1.0 };
        self.impact_cooldown = 0.0;
        if !self.start_tumble(
            Knock {
                direction,
                strength: 420.0,
            },
            false,
            true,
        ) {
            return false;
        }
        let scale = if self.display_scale.is_finite() {
            self.display_scale.clamp(0.5, 2.0).sqrt()
        } else {
            1.0
        };
        self.vel = Vec2::new(direction * PLAY_SHOVE.x * scale, PLAY_SHOVE.y * scale);
        self.spin = direction * PLAY_SPIN;
        true
    }

    /// The impact takes over: drop every stale plan that could steer the body.
    fn release_ownership_for_impact(&mut self) {
        if self.state == State::Hold {
            self.grab_cooldown = self.grab_cooldown.max(1.0);
        }
        self.social = None;
        self.social_motion = None;
        self.activity = None;
        self.pending_activity = None;
        if self.pending_ground == Some(State::Activity) {
            self.pending_ground = None;
        }
        self.forced_sleep = false;
        self.forced_sleep_armed = false;
        self.auto_pursuit = false;
        self.auto_pursuit_time = 0.0;
        self.pursuit_stationary = 0.0;
        self.behavior_velocity = None;
        self.behavior_velocity_ttl = 0.0;
        self.exploration_route = None;
        self.target = None;
        self.peek_goal = None;
        self.hold_offset = Vec2::default();
        self.surface = None;
        self.surface_id = None;
        self.surface_prev = None;
    }

    /// Advance rotation once per physics step, independent of render rate.
    pub(super) fn advance_tumble(&mut self, dt: f64) {
        self.impact_cooldown = (self.impact_cooldown - dt).max(0.0);
        if self.dragging || !self.tilt.is_finite() || !self.spin.is_finite() {
            self.reset_tumble();
            return;
        }
        match self.state {
            State::Thrown | State::Fall | State::Jump => {
                if self.spin != 0.0 {
                    self.spin =
                        self.spin.clamp(-MAX_SPIN, MAX_SPIN) * (-AIR_SPIN_DAMPING * dt).exp();
                    self.tilt = wrap_angle(self.tilt + self.spin * dt);
                } else if self.tilt != 0.0 {
                    self.tilt = ease_upright(self.tilt, UPRIGHT_RATE * dt);
                }
            }
            State::Dizzy => {
                self.spin = 0.0;
                self.tilt = ease_upright(self.tilt, UPRIGHT_RATE * dt);
            }
            _ => self.reset_tumble(),
        }
        // Physics rotates every step; the drawn pose only samples it when the
        // shared 12 Hz pose clock advances, like every other sprite pose.
        let tick = if self.time.is_finite() {
            (self.time.max(0.0) * 12.0 + 1e-8).floor() as i64
        } else {
            0
        };
        if tick != self.visual_tilt_tick {
            self.visual_tilt_tick = tick;
            self.visual_tilt = self.tilt;
        }
    }

    /// Rotation the renderer should draw: `tilt` held for one pose frame.
    pub fn visual_tilt(&self) -> f64 {
        self.visual_tilt
    }

    pub(super) fn reset_tumble(&mut self) {
        self.tilt = 0.0;
        self.visual_tilt = 0.0;
        self.spin = 0.0;
        self.impact_bounces = 0;
        self.dizzy_next = false;
        self.play_tumble = false;
    }

    /// Manual commands keep the physical flight but cancel the dizzy finale.
    pub(super) fn interrupt_tumble(&mut self) {
        self.impact_bounces = 0;
        self.dizzy_next = false;
        self.play_tumble = false;
    }

    /// Called when the feet reach a support during a fall. Returns true when
    /// the tumble consumed the landing (by bouncing or entering dizziness).
    pub(super) fn tumble_landing(&mut self, impact: f64) -> bool {
        let tumbling = self.spin != 0.0 || self.dizzy_next;
        if tumbling && self.impact_bounces > 0 && impact > TUMBLE_BOUNCE_MIN {
            self.impact_bounces -= 1;
            self.vel.y = -impact * TUMBLE_BOUNCE_RESTITUTION;
            self.vel.x *= 0.7;
            self.spin *= 0.6;
            self.squash = 0.5;
            self.surface = None;
            self.surface_id = None;
            self.surface_prev = None;
            self.set_state(State::Thrown, 0.0);
            return true;
        }
        if self.dizzy_next {
            self.vel.y = 0.0;
            self.vel.x *= 0.6;
            self.squash = 0.6;
            self.enter_dizzy();
            return true;
        }
        false
    }

    pub(super) fn enter_dizzy(&mut self) {
        self.dizzy_next = false;
        self.spin = 0.0;
        self.impact_bounces = 0;
        let duration = if self.dizzy_time.is_finite() {
            self.dizzy_time.clamp(0.5, 2.0)
        } else {
            1.2
        };
        self.set_state(State::Dizzy, duration);
        if !self.play_tumble {
            self.speak(SpeechEvent::Dizzy, 1.6);
        }
    }

    pub(super) fn finish_dizzy(&mut self) {
        let play = self.play_tumble;
        self.play_tumble = false;
        self.tilt = 0.0;
        self.visual_tilt = 0.0;
        if play {
            self.satisfaction = (self.satisfaction + 2.0).min(100.0);
            self.speak(SpeechEvent::PlayLaugh, 1.6);
        } else {
            self.speak(SpeechEvent::Recovered, 1.6);
        }
    }

    /// Keep a tumbling body below the top of its work area so it never spins
    /// out of view; a ceiling hit returns a fraction of the vertical speed.
    pub(super) fn screen_ceiling(&mut self, old: Vec2, world: &World) {
        let area = world.area_at(Vec2::new(self.pos.x, old.y));
        if !valid_rect(area) {
            return;
        }
        // The rotated sprite is taller than its ordinary upright contact box.
        let scale = self.display_scale.clamp(0.5, 2.0);
        let clearance = if self.spin != 0.0 || self.dizzy_next {
            super::TUMBLE_HEADROOM * scale
        } else {
            self.head_clearance()
        };
        let top = area.y + clearance.min(area.h);
        // An adjoining upper display is an open passage, not a ceiling.
        // The whole body must fit horizontally through that passage.
        let half = super::BODY_HALF_WIDTH * scale;
        if world.work_areas.iter().any(|other| {
            valid_rect(*other)
                && other.y < area.y
                && other.bottom() >= area.y - 1.0
                && self.pos.x - half >= other.x
                && self.pos.x + half <= other.right()
                && self.pos.y - clearance >= other.y
        }) {
            return;
        }
        if self.pos.y < top && self.vel.y < 0.0 {
            let impact = -self.vel.y;
            self.pos.y = top;
            self.vel.y = impact * 0.4;
            self.squash = self.squash.max(0.45);
            if self.state == State::Thrown && impact >= 500.0 && self.impact_cooldown <= 0.0 {
                self.speak(SpeechEvent::Knocked, 1.4);
                self.impact_cooldown = IMPACT_COOLDOWN;
            }
        }
    }
}

fn wrap_angle(angle: f64) -> f64 {
    if !angle.is_finite() {
        return 0.0;
    }
    let wrapped = (angle + PI).rem_euclid(TAU) - PI;
    if wrapped <= -PI {
        wrapped + TAU
    } else {
        wrapped
    }
}

fn ease_upright(angle: f64, step: f64) -> f64 {
    let angle = wrap_angle(angle);
    if angle.abs() <= step {
        0.0
    } else {
        angle - angle.signum() * step
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn angles_wrap_and_ease_by_the_shortest_path() {
        assert!((wrap_angle(3.0 * PI) - PI).abs() < 1e-9);
        assert!((wrap_angle(-0.5) + 0.5).abs() < 1e-12);
        assert_eq!(wrap_angle(f64::NAN), 0.0);
        assert_eq!(ease_upright(0.3, 0.5), 0.0);
        assert!(ease_upright(PI - 0.1, 0.2) < PI - 0.1);
        assert!(ease_upright(-(PI - 0.1), 0.2) > -(PI - 0.1));
    }
}
