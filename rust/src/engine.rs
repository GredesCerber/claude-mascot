//! Deterministic, platform-independent mascot simulation.
//!
//! Coordinates are desktop physical pixels and `pos` is the point between the
//! character's feet. The engine deliberately has no windowing or rendering
//! dependencies; callers provide a snapshot of the desktop once per frame.

use std::collections::VecDeque;

use serde::{Deserialize, Serialize};

use crate::{Rect, Vec2, WindowInfo, World};

const BODY_HALF_WIDTH: f64 = 24.0;
const GRAVITY: f64 = 2_200.0;
const WALK_SPEED: f64 = 55.0;
const RUN_SPEED: f64 = 150.0;
const IDLE_SLEEP: f64 = 60.0;
const SHAKE_WINDOW: f64 = 1.5;
const SHAKE_HITS: usize = 7;
const MAX_COORD: f64 = 10_000_000.0;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Skin {
    #[default]
    Claude,
    Tagilla,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Idle,
    Walk,
    Run,
    Sit,
    Sleep,
    Happy,
    Fall,
    Jump,
    Thrown,
    Land,
    Dizzy,
    Climb,
    Ledge,
    Chase,
    Hold,
    Peek,
    Smash,
    Hunt,
    Flee,
    Block,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    Wander,
    Sit,
    Sleep,
    Climb,
    Chase,
    Smash,
    Rage,
    ToggleRage,
}

/// A simulation-only attack impact for the host to render as an effect.
#[derive(Clone, Debug, PartialEq)]
pub enum Attack {
    Window { window_id: usize, point: Vec2 },
    Cursor { point: Vec2 },
    Pet { pet_id: u64, point: Vec2 },
}

#[derive(Clone, Debug)]
pub struct Pet {
    pub id: u64,
    pub skin: Skin,
    pub pos: Vec2,
    pub vel: Vec2,
    pub state: State,
    pub facing: f64,
    pub time: f64,
    pub state_time: f64,
    pub surface: Option<usize>,
    pub bubble: Option<String>,
    pub bubble_time: f64,
    pub hearts: f64,
    pub rage: bool,
    pub rage_forever: bool,
    pub dragging: bool,
    release_pending: bool,
    pub chase: bool,
    pub squash: f64,
    /// Target of the current or most recent swing. `attack_serial` identifies impacts.
    pub attack: Option<Attack>,
    pub attack_serial: u64,
    pub(crate) rng: u64,
    pub(crate) idle_for: f64,
    pub(crate) forced_sleep: bool,
    pub(crate) forced_sleep_max: f64,
    pub(crate) forced_sleep_armed: bool,
    pub(crate) pending_ground: Option<State>,
    pub(crate) rage_timer: f64,
    pub(crate) state_duration: f64,
    pub(crate) prior_windows: Vec<(usize, Rect)>,
    pub(crate) surface_prev: Option<Rect>,
    pub(crate) surface_id: Option<usize>,
    pub(crate) shake_samples: VecDeque<(f64, Vec2)>,
    pub(crate) hold_offset: Vec2,
    pub(crate) last_cursor: Vec2,
    pub(crate) initialized: bool,
    pub(crate) frame: u64,
    pub(crate) target: Option<u64>,
    pub(crate) cooldown: f64,
    pub(crate) peek_goal: Option<(usize, Vec2)>,
    pub(crate) peek_route_age: f64,
    pub(crate) smash_goal: Option<(usize, Vec2)>,
    pub(crate) smash_impacted: bool,
}

impl Pet {
    pub fn new(id: u64, skin: Skin, pos: Vec2) -> Self {
        let pos = if valid_vec(pos) {
            pos
        } else {
            Vec2::new(0.0, 0.0)
        };
        Self {
            id,
            skin,
            pos,
            vel: Vec2::default(),
            state: State::Fall,
            facing: 1.0,
            time: 0.0,
            state_time: 0.0,
            surface: None,
            bubble: None,
            bubble_time: 0.0,
            hearts: 0.0,
            rage: false,
            rage_forever: false,
            dragging: false,
            release_pending: false,
            chase: false,
            squash: 0.0,
            attack: None,
            attack_serial: 0,
            rng: id.wrapping_mul(0x9e3779b97f4a7c15).max(1),
            idle_for: 0.0,
            forced_sleep: false,
            forced_sleep_max: 0.0,
            forced_sleep_armed: false,
            pending_ground: None,
            rage_timer: 30.0 + (id % 61) as f64,
            state_duration: 0.0,
            prior_windows: Vec::new(),
            surface_prev: None,
            surface_id: None,
            shake_samples: VecDeque::new(),
            hold_offset: Vec2::default(),
            last_cursor: Vec2::default(),
            initialized: false,
            frame: 0,
            target: None,
            cooldown: 0.0,
            peek_goal: None,
            peek_route_age: 0.0,
            smash_goal: None,
            smash_impacted: false,
        }
    }

    /// Advance by a bounded frame step. Large delays are clamped to 50 ms so
    /// dropped frames cannot tunnel through a platform or produce huge forces.
    pub fn update(&mut self, dt: f64, world: &World) {
        let dt = if dt.is_finite() {
            dt.clamp(0.0, 0.05)
        } else {
            0.0
        };
        self.time += dt;
        self.state_time += dt;
        self.frame = self.frame.wrapping_add(1);
        self.cooldown = (self.cooldown - dt).max(0.0);
        self.bubble_time = (self.bubble_time - dt).max(0.0);
        if self.bubble_time == 0.0 {
            self.bubble = None;
        }
        self.hearts = (self.hearts - dt).max(0.0);
        self.squash = (self.squash - dt * 2.0).max(0.0);
        if !valid_vec(self.pos) || !valid_vec(self.vel) {
            self.recover(world);
            return;
        }
        self.prune_shake();

        if !self.initialized {
            self.last_cursor = world.cursor;
            self.initialized = true;
        }

        self.update_window_motion(world);
        self.update_idle_sleep(world);
        self.update_rage(dt);
        if self.skin == Skin::Tagilla
            && self.state == State::Idle
            && self.target.is_none()
            && !self.chase
            && !self.dragging
            && self.random_chance(dt * 0.02)
        {
            self.plan_window_smash(world);
        }

        if self.dragging {
            self.last_cursor = world.cursor;
            self.snapshot_windows(world);
            return;
        }
        if self.release_pending {
            self.release_pending = false;
            let area = world.area_at(self.pos);
            if valid_rect(area) {
                let inset = BODY_HALF_WIDTH.min(area.w * 0.5);
                self.pos.x = self.pos.x.clamp(area.x + inset, area.right() - inset);
                if self.pos.y > area.bottom() {
                    self.pos.y = area.bottom();
                    self.vel.y = 0.0;
                    self.vel.x *= 0.7;
                    self.surface = None;
                    self.surface_id = None;
                    self.surface_prev = None;
                    self.set_state(State::Land, 0.55);
                } else if self.pos.y < area.y {
                    // Start a dragged pet at the top edge, then let gravity
                    // handle it as an ordinary fall after release.
                    self.pos.y = area.y;
                } else {
                    // A point in a monitor gap is moved to the nearest usable
                    // vertical span rather than left unsupported indefinitely.
                    self.pos.y = self.pos.y.clamp(area.y, area.bottom());
                }
            }
        }
        if self.state == State::Sleep && self.forced_sleep {
            if world.idle_seconds > self.forced_sleep_max + 1.0 {
                self.forced_sleep_armed = true;
            }
            self.forced_sleep_max = self.forced_sleep_max.max(world.idle_seconds);
            if self.forced_sleep_armed && world.idle_seconds < self.forced_sleep_max - 1.0 {
                self.forced_sleep = false;
                self.forced_sleep_armed = false;
                self.say("доброе утро!", 2.0);
                self.set_state(State::Idle, 0.0);
            }
        } else if self.state == State::Sleep && world.idle_seconds < IDLE_SLEEP {
            self.say("доброе утро!", 2.0);
            self.set_state(State::Idle, 0.0);
        }
        if self.state == State::Sleep {
            // Sleeping pets still stand on the desktop floor. The early
            // return below skips update_motion, so keep floor support in sync
            // when the taskbar or monitor work area changes.
            if let Some(i) = self.current_surface_index(world) {
                self.follow_surface(i, world);
            } else {
                self.surface = None;
                self.surface_id = None;
                self.surface_prev = None;
                self.pos.y = self.floor_y(world);
                self.clamp_to_area(world);
            }
            self.snapshot_windows(world);
            return;
        }

        if self.state == State::Hold {
            self.track_hold(world, dt);
            self.snapshot_windows(world);
            return;
        }
        if self.state == State::Peek {
            if world.video_window.is_none() || self.state_time > 8.0 {
                self.set_state(State::Idle, 0.0);
            }
            self.snapshot_windows(world);
            return;
        }

        // Only a confirmed media session enables peeking; a title match alone
        // never creates a video target in the engine.
        if let Some(video_id) = world.video_window {
            if matches!(self.state, State::Idle | State::Walk | State::Sit)
                && world.windows.iter().any(|w| w.id == video_id)
                && self.peek_goal.is_none()
                && self.random_chance(0.0005)
            {
                self.start_peek_route(video_id, world);
            }
        }

        self.update_motion(dt, world);
        self.advance_smash_route(world);
        self.advance_peek_route(dt, world);
        self.snapshot_windows(world);
        self.last_cursor = world.cursor;
    }

    pub fn command(&mut self, command: Command, world: &World) {
        if command != Command::Smash {
            self.smash_goal = None;
            if self.state != State::Smash {
                self.attack = None;
                self.smash_impacted = false;
            }
        }
        if command != Command::Sleep {
            self.forced_sleep = false;
            self.forced_sleep_armed = false;
        }
        match command {
            Command::Wander => {
                self.chase = false;
                if !self.has_support(world) {
                    self.surface = None;
                    self.surface_id = None;
                    self.surface_prev = None;
                    self.set_state(State::Fall, 0.0);
                } else {
                    self.set_state(State::Idle, 0.0);
                }
                self.say("пойду гулять", 1.6);
            }
            Command::Sit => {
                if !self.has_support(world) {
                    self.pending_ground = Some(State::Sit);
                    if !self.is_airborne() {
                        self.vel = Vec2::default();
                        self.set_state(State::Fall, 0.0);
                    }
                } else {
                    self.vel = Vec2::default();
                    self.set_state(State::Sit, 0.0);
                }
            }
            Command::Sleep => {
                self.forced_sleep = true;
                self.forced_sleep_max = world.idle_seconds.max(0.0);
                self.forced_sleep_armed = false;
                if !self.has_support(world) {
                    self.pending_ground = Some(State::Sleep);
                    if !self.is_airborne() {
                        self.vel = Vec2::default();
                        self.set_state(State::Fall, 0.0);
                    }
                } else {
                    self.vel = Vec2::default();
                    self.set_state(State::Sleep, 0.0);
                }
            }
            Command::Climb => {
                if let Some((i, w, dir)) = self.nearest_edge(world) {
                    let window_id = w.id;
                    let rect = w.rect;
                    self.surface = Some(i);
                    self.surface_id = Some(window_id);
                    self.surface_prev = Some(rect);
                    self.facing = dir;
                    self.pos.x = if dir < 0.0 {
                        rect.x - BODY_HALF_WIDTH
                    } else {
                        rect.right() + BODY_HALF_WIDTH
                    };
                    self.pos.y = self.pos.y.min(rect.bottom() - 10.0);
                    self.set_state(State::Climb, 0.0);
                }
            }
            Command::Chase => {
                self.chase = !self.chase;
                if !self.chase && self.state == State::Chase {
                    self.set_state(State::Idle, 0.0);
                }
            }
            Command::Smash => {
                if self.skin == Skin::Tagilla {
                    self.plan_window_smash(world);
                }
            }
            Command::Rage => {
                self.rage = true;
                self.rage_timer = 30.0;
                self.say("ГРАААА!", 2.0);
            }
            Command::ToggleRage => {
                self.rage_forever = !self.rage_forever;
                self.rage = self.rage_forever;
                self.rage_timer = if self.rage { f64::MAX } else { 30.0 };
            }
        }
    }

    /// Change appearance without carrying stale Tagilla/Claude-only behavior
    /// into the new skin. The current desktop position and valid surface stay.
    pub fn set_skin(&mut self, skin: Skin) {
        if self.skin == skin {
            return;
        }
        let was_airborne = matches!(self.state, State::Fall | State::Jump | State::Thrown);
        self.skin = skin;
        self.rage = false;
        self.rage_forever = false;
        self.rage_timer = 30.0;
        self.chase = false;
        self.peek_goal = None;
        self.dragging = false;
        self.vel = Vec2::default();
        self.bubble = None;
        self.bubble_time = 0.0;
        self.hearts = 0.0;
        self.forced_sleep = false;
        self.forced_sleep_armed = false;
        self.set_state(
            if was_airborne {
                State::Fall
            } else {
                State::Idle
            },
            0.0,
        );
    }

    /// The stable pet ID currently selected by this pet's hunt behavior.
    pub fn target_id(&self) -> Option<u64> {
        self.target
    }

    pub fn drag_to(&mut self, pos: Vec2) {
        if !valid_vec(pos) {
            return;
        }
        self.dragging = true;
        self.release_pending = false;
        self.forced_sleep = false;
        self.surface = None;
        self.surface_id = None;
        self.surface_prev = None;
        self.pos = pos;
        self.vel = Vec2::default();
        self.set_state(State::Thrown, 0.0);
    }

    pub fn release(&mut self, velocity: Vec2) {
        self.dragging = false;
        self.release_pending = true;
        self.vel = if valid_vec(velocity) {
            Vec2::new(
                velocity.x.clamp(-1800.0, 1800.0),
                velocity.y.clamp(-1800.0, 1800.0),
            )
        } else {
            Vec2::default()
        };
        self.surface = None;
        self.surface_id = None;
        self.surface_prev = None;
        self.set_state(
            if self.vel.y.abs() + self.vel.x.abs() > 80.0 {
                State::Thrown
            } else {
                State::Fall
            },
            0.0,
        );
    }

    pub fn pet(&mut self) {
        self.forced_sleep = false;
        self.hearts = 1.5;
        self.squash = 0.35;
        self.say("приятно :) ", 1.8);
        if matches!(self.state, State::Idle | State::Walk | State::Sit) {
            self.set_state(State::Happy, 1.1);
        }
    }

    pub fn jump(&mut self) {
        if self.surface.is_some()
            || matches!(
                self.state,
                State::Idle | State::Walk | State::Sit | State::Land | State::Ledge | State::Happy
            )
        {
            self.surface = None;
            self.surface_id = None;
            self.surface_prev = None;
            self.vel.y = -720.0;
            self.vel.x = self.facing * 100.0;
            self.set_state(State::Jump, 0.0);
        }
    }

    fn update_motion(&mut self, dt: f64, world: &World) {
        if self.state == State::Climb {
            self.update_climb(dt, world);
            return;
        }
        if self.state == State::Smash {
            if !self.smash_impacted && self.state_time >= 0.35 && self.attack.is_some() {
                self.attack_serial = self.attack_serial.wrapping_add(1);
                self.smash_impacted = true;
            }
            if self.state_time > self.state_duration.max(0.35) {
                self.set_state(State::Idle, 0.0);
            } else {
                self.vel = Vec2::default();
            }
            return;
        }
        if self.state == State::Block || self.state == State::Dizzy {
            if self.state_time > 1.5 {
                self.set_state(State::Idle, 0.0);
            }
            return;
        }
        if self.state == State::Ledge {
            if self.state_time > self.state_duration.max(0.35) {
                self.set_state(State::Idle, 0.0);
            }
            if let Some(i) = self.current_surface_index(world) {
                self.follow_surface(i, world);
            }
            return;
        }
        if self.state == State::Land {
            if self.state_time > self.state_duration.max(0.55) {
                let next = self.pending_ground.take().unwrap_or(State::Idle);
                self.set_state(next, 0.0);
            } else {
                // Preserve horizontal impact momentum and let ground friction
                // bring the pet to rest instead of snapping it to a stop.
                if let Some(i) = self.current_surface_index(world) {
                    self.follow_surface(i, world);
                } else {
                    self.pos.y = self.floor_y(world);
                }
                self.pos.x += self.vel.x * dt;
                self.vel.x *= (-5.0 * dt).exp();
                if self.vel.x.abs() < 2.0 {
                    self.vel.x = 0.0;
                }
                self.clamp_to_area(world);
                return;
            }
        }
        if self.state == State::Happy {
            if self.state_time > self.state_duration.max(0.55) {
                let next = self.pending_ground.take().unwrap_or(State::Idle);
                self.set_state(next, 0.0);
            } else {
                self.vel = Vec2::default();
                return;
            }
        }
        if self.state == State::Sit {
            if let Some(i) = self.surface {
                self.follow_surface(i, world);
            }
            return;
        }

        if self.state == State::Chase || self.chase {
            self.update_chase(dt, world);
            return;
        }

        if self.state == State::Fall || self.state == State::Jump || self.state == State::Thrown {
            // A drag can release the pet below the desktop, where the normal
            // floor-crossing test cannot catch it (old_y is already past it).
            // Recover to the nearest remaining monitor's floor before applying
            // gravity so it cannot fall forever after a monitor change either.
            let floor = self.floor_y(world);
            if self.pos.y > floor {
                self.pos.y = floor;
                self.vel.y = 0.0;
                self.surface = None;
                self.surface_id = None;
                self.surface_prev = None;
                self.clamp_to_area(world);
                self.set_state(State::Land, 0.55);
                return;
            }
            let old = self.pos;
            self.vel.y = (self.vel.y + GRAVITY * dt).clamp(-1400.0, 2200.0);
            self.pos.x += self.vel.x * dt;
            self.pos.y += self.vel.y * dt;
            self.clamp_to_area(world);
            if self.vel.y >= 0.0 {
                if let Some((ground_y, surface)) = self.find_landing(old.y, world) {
                    self.pos.y = ground_y;
                    self.surface = surface;
                    self.surface_id = surface.and_then(|i| world.windows.get(i).map(|w| w.id));
                    self.surface_prev = surface.and_then(|i| world.windows.get(i).map(|w| w.rect));
                    let impact = self.vel.y;
                    if impact > 1_300.0 && self.state != State::Jump {
                        self.vel.y = -impact * 0.35;
                        self.vel.x *= 0.6;
                        self.squash = 0.45;
                        self.surface = None;
                        self.surface_id = None;
                        self.surface_prev = None;
                        self.set_state(State::Thrown, 0.0);
                    } else {
                        self.vel.y = 0.0;
                        self.vel.x *= 0.7;
                        self.squash = 0.8;
                        self.set_state(State::Land, 0.55);
                    }
                }
            }
            self.facing = if self.vel.x < -1.0 {
                -1.0
            } else if self.vel.x > 1.0 {
                1.0
            } else {
                self.facing
            };
            return;
        }

        if let Some(i) = self.current_surface_index(world) {
            self.surface = Some(i);
            self.follow_surface(i, world);
        } else {
            let floor = self.floor_y(world);
            if self.pos.y < floor - 2.0
                && !matches!(self.state, State::Idle | State::Sit | State::Sleep)
            {
                self.set_state(State::Fall, 0.0);
                self.surface = None;
                self.vel.y = self.vel.y.max(0.0);
                return;
            }
            // Idle floor support is positional, not a one-time crossing test:
            // the taskbar can resize the work area while the pet is standing.
            self.pos.y = floor;
        }
        if self.state == State::Walk || self.state == State::Run {
            self.pos.x += self.facing * self.speed() * dt;
            if !self.platform_under(self.pos.x, world) {
                self.vel.x = self.facing * self.speed();
                self.vel.y = 20.0;
                self.surface = None;
                self.set_state(State::Fall, 0.0);
            } else {
                self.vel.x = 0.0;
                if self.random_chance(dt * 0.12) {
                    self.set_state(State::Idle, 0.0);
                }
            }
        } else if self.state == State::Idle
            && self.random_chance(dt * 0.16)
            && !self.try_explore_jump(world)
        {
            if self.random_chance(0.35) {
                self.set_state(State::Sit, 0.0);
            } else {
                self.facing *= -1.0;
                self.set_state(
                    if self.skin == Skin::Tagilla {
                        State::Run
                    } else {
                        State::Walk
                    },
                    0.0,
                );
            }
        }
        self.clamp_to_area(world);
    }

    fn find_landing(&self, old_y: f64, world: &World) -> Option<(f64, Option<usize>)> {
        let mut best: Option<(f64, Option<usize>)> = None;
        // Windows are ordered bottom-to-top. Iterate in reverse so the topmost
        // visible candidate wins at equal height.
        for i in (0..world.windows.len()).rev() {
            let w = &world.windows[i];
            if !valid_rect(w.rect) || w.rect.w < 40.0 || w.rect.h < 30.0 {
                continue;
            }
            let y = w.rect.y;
            if old_y <= y + 2.0
                && self.pos.y >= y
                && self.overlaps_x(w.rect)
                && !covered(world, Vec2::new(self.pos.x, y - 1.0), i)
                && best.is_none_or(|b| y < b.0)
            {
                best = Some((y, Some(i)));
            }
        }
        let floor = self.floor_y(world);
        if old_y <= floor && self.pos.y >= floor && best.is_none_or(|b| floor <= b.0) {
            best = Some((floor, None));
        }
        best
    }

    fn floor_y(&self, world: &World) -> f64 {
        // area_at also chooses the nearest valid monitor when a work-area
        // resize temporarily puts the pet just outside its previous bounds.
        let a = world.area_at(self.pos);
        if valid_rect(a) {
            a.bottom()
        } else {
            1040.0
        }
    }

    fn overlaps_x(&self, r: Rect) -> bool {
        self.pos.x + BODY_HALF_WIDTH > r.x && self.pos.x - BODY_HALF_WIDTH < r.right()
    }

    fn platform_under(&self, x: f64, world: &World) -> bool {
        if let Some(i) = self.current_surface_index(world) {
            return world.windows.get(i).is_some_and(|w| {
                valid_rect(w.rect)
                    && x + BODY_HALF_WIDTH > w.rect.x
                    && x - BODY_HALF_WIDTH < w.rect.right()
            });
        }
        x >= world.area_at(self.pos).x - BODY_HALF_WIDTH
            && x <= world.area_at(self.pos).right() + BODY_HALF_WIDTH
    }

    fn follow_surface(&mut self, index: usize, world: &World) {
        if let Some(w) = world.windows.get(index) {
            if valid_rect(w.rect)
                && self.pos.x >= w.rect.x - BODY_HALF_WIDTH
                && self.pos.x <= w.rect.right() + BODY_HALF_WIDTH
            {
                self.pos.y = w.rect.y;
                self.surface_prev = Some(w.rect);
            } else {
                self.surface = None;
                self.surface_prev = None;
                self.set_state(State::Fall, 0.0);
            }
        } else {
            self.surface = None;
            self.surface_prev = None;
            self.set_state(State::Fall, 0.0);
        }
    }

    fn update_window_motion(&mut self, world: &World) {
        if self.surface.is_none() {
            return;
        }
        let Some(i) = self.current_surface_index(world) else {
            self.surface = None;
            self.surface_id = None;
            self.surface_prev = None;
            self.vel = Vec2::new(0.0, 0.0);
            self.set_state(State::Fall, 0.0);
            return;
        };
        let Some(current) = world
            .windows
            .get(i)
            .filter(|w| valid_rect(w.rect))
            .map(|w| w.rect)
        else {
            self.surface = None;
            self.surface_prev = None;
            self.set_state(State::Fall, 0.0);
            return;
        };
        self.surface_id = Some(world.windows[i].id);
        if let Some(old) = self.surface_prev {
            let dx = current.x - old.x;
            let dy = current.y - old.y;
            if dx.abs() > 0.01 || dy.abs() > 0.01 {
                self.pos.x += dx;
                self.pos.y += dy;
                if (dx.abs() + dy.abs()) > 45.0 {
                    self.surface = None;
                    self.surface_prev = None;
                    self.vel = Vec2::new(dx * 8.0, -180.0 - dy.abs() * 2.0);
                    self.set_state(State::Thrown, 0.0);
                } else {
                    self.surface_prev = Some(current);
                }
            }
        } else {
            self.surface_prev = Some(current);
        }
    }

    fn update_climb(&mut self, dt: f64, world: &World) {
        let Some(i) = self.current_surface_index(world) else {
            self.set_state(State::Fall, 0.0);
            return;
        };
        let Some(w) = world.windows.get(i).filter(|w| valid_rect(w.rect)) else {
            self.surface = None;
            self.set_state(State::Fall, 0.0);
            return;
        };
        let side = if self.facing < 0.0 {
            w.rect.x
        } else {
            w.rect.right()
        };
        self.pos.x = side + self.facing * BODY_HALF_WIDTH;
        self.pos.y -= 65.0 * dt;
        if self.pos.y <= w.rect.y {
            self.pos.y = w.rect.y;
            self.surface = Some(i);
            self.set_state(State::Ledge, 0.45);
        }
        if self.pos.y > w.rect.bottom() {
            self.surface = None;
            self.set_state(State::Fall, 0.0);
        }
    }

    fn update_chase(&mut self, dt: f64, world: &World) {
        let dx = world.cursor.x - self.pos.x;
        if dx.abs() < 24.0
            && (world.cursor.y - self.pos.y).abs() < 70.0
            && self.skin == Skin::Claude
        {
            self.hold_offset = Vec2::new(self.pos.x - world.cursor.x, self.pos.y - world.cursor.y);
            self.set_state(State::Hold, 0.0);
            self.say("вишу!", 1.5);
            return;
        }
        if dx.abs() < 40.0
            && (world.cursor.y - self.pos.y).abs() < 90.0
            && self.skin == Skin::Tagilla
        {
            self.begin_smash(
                Attack::Cursor {
                    point: world.cursor,
                },
                0.7,
            );
            return;
        }
        self.facing = if dx < 0.0 { -1.0 } else { 1.0 };
        self.pos.x += self.facing * RUN_SPEED * self.skin_speed() * dt;
        self.set_state(State::Chase, 0.0);
    }

    fn track_hold(&mut self, world: &World, dt: f64) {
        let cursor = world.cursor;
        if valid_vec(cursor) {
            self.pos = Vec2::new(cursor.x + self.hold_offset.x, cursor.y + self.hold_offset.y);
        }
        let movement = distance(cursor, self.last_cursor);
        if movement > 35.0 {
            self.shake_samples.push_back((self.time, cursor));
        }
        self.last_cursor = cursor;
        self.prune_shake();
        let oscillations = direction_reversals(&self.shake_samples);
        if oscillations >= SHAKE_HITS {
            self.surface = None;
            self.vel = Vec2::new((cursor.x - self.pos.x) * 5.0, -350.0);
            self.set_state(State::Thrown, 0.0);
            self.say("аааа!", 1.5);
        } else if dt > 0.0 {
            self.state = State::Hold;
        }
    }

    fn update_idle_sleep(&mut self, world: &World) {
        if world.idle_seconds >= IDLE_SLEEP {
            self.idle_for = world.idle_seconds;
        } else {
            self.idle_for = 0.0;
        }
        if !self.forced_sleep
            && self.idle_for >= IDLE_SLEEP
            && world.video_window.is_none()
            && !self.dragging
            && self.has_support(world)
            && !matches!(
                self.state,
                State::Fall | State::Jump | State::Thrown | State::Smash | State::Block
            )
        {
            self.vel = Vec2::default();
            self.set_state(State::Sleep, 0.0);
        }
    }

    fn update_rage(&mut self, dt: f64) {
        if self.skin != Skin::Tagilla {
            return;
        }
        if self.rage_forever {
            self.rage = true;
            return;
        }
        self.rage_timer -= dt;
        if self.rage_timer <= 0.0 {
            self.rage = !self.rage;
            self.rage_timer = if self.rage {
                20.0
            } else {
                30.0 + self.random() * 90.0
            };
        }
    }

    fn current_surface_index(&self, world: &World) -> Option<usize> {
        let index = self.surface?;
        if let Some(id) = self.surface_id {
            world.windows.iter().position(|w| w.id == id)
        } else {
            world.windows.get(index).map(|_| index)
        }
    }

    fn is_airborne(&self) -> bool {
        matches!(self.state, State::Fall | State::Jump | State::Thrown)
    }

    fn has_support(&self, world: &World) -> bool {
        if let Some(i) = self.current_surface_index(world) {
            return world.windows.get(i).is_some_and(|w| {
                valid_rect(w.rect)
                    && (self.pos.y - w.rect.y).abs() <= 3.0
                    && self.overlaps_x(w.rect)
            });
        }
        (self.pos.y - self.floor_y(world)).abs() <= 3.0
    }

    fn nearest_edge<'a>(&self, world: &'a World) -> Option<(usize, &'a WindowInfo, f64)> {
        world
            .windows
            .iter()
            .enumerate()
            .filter(|(_, w)| valid_rect(w.rect) && w.rect.w >= 80.0 && w.rect.h >= 60.0)
            .min_by(|(_, a), (_, b)| {
                distance_sq(self.pos, Vec2::new(a.rect.x, a.rect.y))
                    .total_cmp(&distance_sq(self.pos, Vec2::new(b.rect.x, b.rect.y)))
            })
            .map(|(i, w)| {
                (
                    i,
                    w,
                    if self.pos.x < w.rect.x + w.rect.w * 0.5 {
                        -1.0
                    } else {
                        1.0
                    },
                )
            })
    }

    /// Occasionally take a reachable window-top jump while idle. Candidate
    /// reachability uses a fixed ballistic arc so it stays deterministic.
    fn try_explore_jump(&mut self, world: &World) -> bool {
        const LAUNCH: f64 = 1_350.0;
        let mut best: Option<(f64, f64, f64, usize)> = None;
        for (i, w) in world.windows.iter().enumerate() {
            if !valid_rect(w.rect) || w.rect.w < 80.0 || w.rect.y >= self.pos.y - 60.0 {
                continue;
            }
            let target_x =
                (self.pos.x).clamp(w.rect.x + BODY_HALF_WIDTH, w.rect.right() - BODY_HALF_WIDTH);
            let dy = w.rect.y - self.pos.y;
            let disc = LAUNCH * LAUNCH + 2.0 * GRAVITY * dy;
            if disc < 0.0 {
                continue;
            }
            let t = (LAUNCH + disc.sqrt()) / GRAVITY;
            let dx = target_x - self.pos.x;
            let vx = dx / t;
            if t <= 0.0
                || vx.abs() > 360.0
                || covered(world, Vec2::new(target_x, w.rect.y - 1.0), i)
            {
                continue;
            }
            let score = dx.abs() + dy.abs() * 0.7;
            if best.is_none_or(|b| score < b.0) {
                best = Some((score, vx, target_x, i));
            }
        }
        let Some((_, vx, target_x, index)) = best else {
            return false;
        };
        self.facing = if target_x < self.pos.x { -1.0 } else { 1.0 };
        self.vel = Vec2::new(vx, -LAUNCH);
        self.surface = None;
        self.surface_id = None;
        self.surface_prev = None;
        self.set_state(State::Jump, 0.0);
        self.target = Some(world.windows[index].id as u64);
        true
    }

    fn plan_window_smash(&mut self, world: &World) {
        let candidate = world
            .windows
            .iter()
            .enumerate()
            .filter(|(i, w)| {
                valid_rect(w.rect)
                    && w.rect.w >= 40.0
                    && w.rect.h >= 30.0
                    && !covered(
                        world,
                        Vec2::new(w.rect.x + w.rect.w * 0.5, w.rect.y + w.rect.h * 0.5),
                        *i,
                    )
            })
            .min_by(|(_, a), (_, b)| {
                distance_sq(
                    self.pos,
                    Vec2::new(a.rect.x + a.rect.w * 0.5, a.rect.y + a.rect.h * 0.5),
                )
                .total_cmp(&distance_sq(
                    self.pos,
                    Vec2::new(b.rect.x + b.rect.w * 0.5, b.rect.y + b.rect.h * 0.5),
                ))
            });
        let Some((index, window)) = candidate else {
            self.attack = None;
            self.smash_goal = None;
            if self.state != State::Smash {
                if self.has_support(world) {
                    self.vel = Vec2::default();
                    self.set_state(State::Idle, 0.0);
                } else {
                    self.set_state(State::Fall, 0.0);
                }
            }
            return;
        };

        let center = Vec2::new(
            window.rect.x + window.rect.w * 0.5,
            window.rect.y + window.rect.h * 0.5,
        );
        let already_on_window = self.surface_id == Some(window.id)
            || (self.surface == Some(index) && (self.pos.y - window.rect.y).abs() <= 3.0);
        if already_on_window {
            self.begin_smash(
                Attack::Window {
                    window_id: window.id,
                    point: self.pos,
                },
                1.0,
            );
        } else {
            self.attack = Some(Attack::Window {
                window_id: window.id,
                point: center,
            });
            self.smash_goal = Some((window.id, Vec2::new(center.x, window.rect.y)));
            self.smash_impacted = false;
            self.advance_smash_route(world);
        }
    }

    fn begin_smash(&mut self, attack: Attack, duration: f64) {
        self.attack = Some(attack);
        self.smash_goal = None;
        self.smash_impacted = false;
        self.state_time = 0.0;
        self.set_state(State::Smash, duration);
        self.say("БАХ!", duration.min(1.0));
    }

    fn advance_smash_route(&mut self, world: &World) {
        let Some((window_id, _)) = self.smash_goal else {
            return;
        };
        let Some(window) = world
            .windows
            .iter()
            .find(|w| w.id == window_id && valid_rect(w.rect))
        else {
            self.smash_goal = None;
            self.attack = None;
            if !self.is_airborne() {
                self.set_state(State::Idle, 0.0);
            }
            return;
        };
        let goal = Vec2::new(window.rect.x + window.rect.w * 0.5, window.rect.y);
        let center = Vec2::new(goal.x, window.rect.y + window.rect.h * 0.5);
        self.attack = Some(Attack::Window {
            window_id,
            point: center,
        });
        self.smash_goal = Some((window_id, goal));
        if self.state == State::Smash || self.is_airborne() {
            return;
        }
        let dx = goal.x - self.pos.x;
        let dy = goal.y - self.pos.y;
        if self.has_support(world) && dx.abs() <= 32.0 && dy.abs() <= 5.0 {
            self.pos = goal;
            self.begin_smash(
                Attack::Window {
                    window_id,
                    point: center,
                },
                1.0,
            );
            return;
        }

        if self.has_support(world) && dy < -5.0 {
            // Leave headroom over the continuous ballistic estimate: the
            // semi-implicit frame integrator loses a little jump height.
            const LAUNCH: f64 = 1_500.0;
            let disc = LAUNCH * LAUNCH + 2.0 * GRAVITY * dy;
            if disc >= 0.0 {
                let flight = (LAUNCH + disc.sqrt()) / GRAVITY;
                if flight > 0.0 && dx.abs() <= 360.0 * flight {
                    self.facing = if dx < 0.0 { -1.0 } else { 1.0 };
                    self.vel = Vec2::new(dx / flight, -LAUNCH);
                    self.surface = None;
                    self.surface_id = None;
                    self.surface_prev = None;
                    self.set_state(State::Jump, 0.0);
                    return;
                }
            }
        }

        if self.has_support(world) && dx.abs() > 32.0 {
            self.facing = if dx < 0.0 { -1.0 } else { 1.0 };
            self.set_state(State::Walk, 0.0);
        } else if self.has_support(world) {
            self.set_state(State::Idle, 0.0);
        }
    }

    fn start_peek_route(&mut self, video_id: usize, world: &World) {
        let Some((index, window)) = world
            .windows
            .iter()
            .enumerate()
            .find(|(_, w)| w.id == video_id)
        else {
            return;
        };
        if !valid_rect(window.rect) || window.rect.h < 100.0 {
            return;
        }
        let left = self.pos.x <= window.rect.x + window.rect.w * 0.5;
        let goal = Vec2::new(
            if left {
                window.rect.x - BODY_HALF_WIDTH * 0.7
            } else {
                window.rect.right() + BODY_HALF_WIDTH * 0.7
            },
            window.rect.y,
        );
        let dy = goal.y - self.pos.y;
        if dy > 2.0 || dy < -390.0 {
            return;
        }
        if dy.abs() <= 2.0 && self.has_support(world) {
            self.peek_goal = Some((video_id, goal));
            self.peek_route_age = 0.0;
            self.set_state(State::Walk, 0.0);
            return;
        }
        const PEEK_LAUNCH: f64 = 1_350.0;
        let disc = PEEK_LAUNCH * PEEK_LAUNCH + 2.0 * GRAVITY * dy;
        if disc < 0.0 {
            return;
        }
        let flight = (PEEK_LAUNCH + disc.sqrt()) / GRAVITY;
        let vx = (goal.x - self.pos.x) / flight;
        if !flight.is_finite() || flight <= 0.0 || vx.abs() > 360.0 {
            return;
        }
        self.peek_goal = Some((video_id, goal));
        self.peek_route_age = 0.0;
        self.facing = if vx < 0.0 { -1.0 } else { 1.0 };
        self.surface = None;
        self.surface_id = None;
        self.surface_prev = None;
        self.vel = Vec2::new(vx, -PEEK_LAUNCH);
        self.set_state(State::Jump, 0.0);
        let _ = index;
    }

    fn advance_peek_route(&mut self, dt: f64, world: &World) {
        let Some((window_id, goal)) = self.peek_goal else {
            return;
        };
        self.peek_route_age += dt;
        let Some(index) = world
            .windows
            .iter()
            .position(|w| w.id == window_id && valid_rect(w.rect))
        else {
            self.peek_goal = None;
            return;
        };
        if self.peek_route_age > 8.0 || world.video_window != Some(window_id) {
            self.peek_goal = None;
            return;
        }
        if self.surface_id == Some(window_id)
            && (self.pos.x - goal.x).abs() <= 22.0
            && (self.pos.y - goal.y).abs() <= 3.0
        {
            self.surface = Some(index);
            self.pos = goal;
            self.vel = Vec2::default();
            self.peek_goal = None;
            self.set_state(State::Peek, 8.0);
            self.say("что смотрим?", 2.0);
            return;
        }
        if !self.is_airborne() && self.surface_id == Some(window_id) {
            self.facing = if goal.x < self.pos.x { -1.0 } else { 1.0 };
            self.pos.x += self.facing * self.speed() * dt;
            self.set_state(State::Walk, 0.0);
        }
    }

    fn snapshot_windows(&mut self, world: &World) {
        self.prior_windows = window_rects(world);
    }
    fn set_state(&mut self, state: State, duration: f64) {
        if self.state != state {
            if self.state == State::Smash && state != State::Smash {
                self.attack = None;
                self.smash_goal = None;
                self.smash_impacted = false;
            }
            self.state = state;
            self.state_time = 0.0;
        }
        self.state_duration = duration;
    }
    fn say(&mut self, text: &str, duration: f64) {
        self.bubble = Some(text.to_owned());
        self.bubble_time = duration;
    }
    fn speed(&self) -> f64 {
        if self.state == State::Run
            || self.state == State::Chase
            || self.state == State::Hunt
            || self.state == State::Flee
        {
            RUN_SPEED * self.skin_speed()
        } else {
            WALK_SPEED * self.skin_speed()
        }
    }
    fn skin_speed(&self) -> f64 {
        if self.skin == Skin::Tagilla {
            1.35
        } else {
            1.0
        }
    }
    fn random(&mut self) -> f64 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 7;
        self.rng ^= self.rng << 17;
        (self.rng >> 11) as f64 / ((1u64 << 53) as f64)
    }
    fn random_chance(&mut self, chance: f64) -> bool {
        self.random() < chance.clamp(0.0, 1.0)
    }
    fn prune_shake(&mut self) {
        while self
            .shake_samples
            .front()
            .is_some_and(|(t, _)| self.time - *t > SHAKE_WINDOW)
        {
            self.shake_samples.pop_front();
        }
    }
    fn recover(&mut self, world: &World) {
        self.pos = if valid_vec(world.cursor) {
            world.cursor
        } else {
            Vec2::new(0.0, 0.0)
        };
        self.vel = Vec2::default();
        self.surface = None;
        self.surface_prev = None;
        self.dragging = false;
        self.set_state(State::Fall, 0.0);
    }
    fn clamp_to_area(&mut self, world: &World) {
        let a = world.area_at(self.pos);
        if valid_rect(a) {
            let inset = BODY_HALF_WIDTH.min(a.w * 0.5);
            self.pos.x = self.pos.x.clamp(a.x + inset, a.right() - inset);
        }
    }
}

/// Pairwise autonomous interaction pass. Call after each pet's normal update.
/// It mutates only pet simulation state; the real cursor and desktop windows
/// remain entirely outside the engine.
pub fn update_interactions(pets: &mut [Pet], dt: f64, world: &World) {
    let dt = if dt.is_finite() {
        dt.clamp(0.0, 0.05)
    } else {
        0.0
    };
    let mut hits: Vec<(usize, usize)> = Vec::new();
    for i in 0..pets.len() {
        if pets[i].skin != Skin::Tagilla
            || !pets[i].rage
            || pets[i].cooldown > 0.0
            || pets[i].dragging
        {
            continue;
        }
        let victim = (0..pets.len())
            .filter(|&j| i != j && pets[j].skin != Skin::Tagilla && !pets[j].dragging)
            .min_by(|&a, &b| {
                distance_sq(pets[i].pos, pets[a].pos)
                    .total_cmp(&distance_sq(pets[i].pos, pets[b].pos))
            });
        let Some(j) = victim else {
            continue;
        };
        let dx = pets[j].pos.x - pets[i].pos.x;
        pets[i].target = Some(pets[j].id);
        if dx.abs() < 48.0 && (pets[i].pos.y - pets[j].pos.y).abs() < 100.0 {
            if pets[i].random_chance(0.2) {
                pets[i].vel = Vec2::new(-dx.signum() * 260.0, -500.0);
                pets[i].surface = None;
                pets[i].set_state(State::Thrown, 0.0);
                pets[i].cooldown = 2.5;
                pets[j].set_state(State::Block, 1.0);
                pets[j].cooldown = 1.0;
            } else {
                pets[j].vel = Vec2::new(dx.signum() * 430.0, -650.0);
                pets[j].surface = None;
                pets[j].set_state(State::Thrown, 0.0);
                pets[j].say("АЙ!", 1.5);
                pets[i].set_state(State::Dizzy, 0.8);
                pets[i].cooldown = 1.8;
            }
        } else {
            pets[i].facing = if dx < 0.0 { -1.0 } else { 1.0 };
            pets[i].pos.x += pets[i].facing * RUN_SPEED * 1.35 * dt;
            pets[i].set_state(State::Hunt, 0.0);
            pets[j].facing = -pets[i].facing;
            pets[j].pos.x += pets[j].facing * RUN_SPEED * dt;
            let trapped = (0..pets.len())
                .any(|k| k != i && k != j && (pets[k].pos.x - pets[j].pos.x).abs() < 50.0);
            if trapped && pets[j].surface.is_some() {
                pets[j].jump();
            } else {
                pets[j].set_state(State::Flee, 0.0);
            }
        }
        hits.push((i, j));
    }
    // If several Tagillas selected one victim, retain only outcomes already
    // applied; a zero-dt update remains deterministic and cannot affect OS state.
    for (i, j) in hits {
        pets[i].target = Some(pets[j].id);
    }
    let _ = world;
}

fn valid_vec(p: Vec2) -> bool {
    p.x.is_finite() && p.y.is_finite() && p.x.abs() < MAX_COORD && p.y.abs() < MAX_COORD
}
fn valid_rect(r: Rect) -> bool {
    valid_vec(Vec2::new(r.x, r.y))
        && r.w.is_finite()
        && r.h.is_finite()
        && r.w > 0.0
        && r.h > 0.0
        && r.w < MAX_COORD
        && r.h < MAX_COORD
        && r.right().is_finite()
        && r.bottom().is_finite()
        && r.right().abs() < MAX_COORD
        && r.bottom().abs() < MAX_COORD
}
fn window_rects(world: &World) -> Vec<(usize, Rect)> {
    world.windows.iter().map(|w| (w.id, w.rect)).collect()
}
fn distance(a: Vec2, b: Vec2) -> f64 {
    ((a.x - b.x).powi(2) + (a.y - b.y).powi(2)).sqrt()
}
fn distance_sq(a: Vec2, b: Vec2) -> f64 {
    (a.x - b.x).powi(2) + (a.y - b.y).powi(2)
}
fn covered(world: &World, p: Vec2, index: usize) -> bool {
    world
        .windows
        .iter()
        .skip(index + 1)
        .any(|w| valid_rect(w.rect) && w.rect.contains(p))
}
fn direction_reversals(samples: &VecDeque<(f64, Vec2)>) -> usize {
    let mut last: Option<f64> = None;
    let mut reversals = 0;
    for (a, b) in samples.iter().zip(samples.iter().skip(1)) {
        let dx = b.1.x - a.1.x;
        if dx.abs() < 10.0 {
            continue;
        }
        let sign = dx.signum();
        if last.is_some_and(|p| p != sign) {
            reversals += 1;
        }
        last = Some(sign);
    }
    reversals
}
