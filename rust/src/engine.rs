//! Deterministic, platform-independent mascot simulation.
//!
//! Coordinates are desktop physical pixels and `pos` is the point between the
//! character's feet. The engine deliberately has no windowing or rendering
//! dependencies; callers provide a snapshot of the desktop once per frame.

use std::collections::VecDeque;

use serde::{de, Deserialize, Deserializer, Serialize};

use crate::social::SocialMotion;
use crate::{Rect, Vec2, WindowInfo, World};

const BODY_HALF_WIDTH: f64 = 24.0;
const GRAVITY: f64 = 2_200.0;
const WALK_SPEED: f64 = 55.0;
const RUN_SPEED: f64 = 150.0;
const IDLE_SLEEP: f64 = 60.0;
const SHAKE_WINDOW: f64 = 1.5;
const SHAKE_HITS: usize = 7;
const MAX_COORD: f64 = 10_000_000.0;
const WALL_BOUNCE_MIN_SPEED: f64 = 280.0;
const WALL_RESTITUTION: f64 = 0.62;
pub const CURSOR_HOLD_DY: f64 = 70.0;

#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Skin {
    #[default]
    Claude,
}

impl<'de> Deserialize<'de> for Skin {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let skin = String::deserialize(deserializer)?;
        match skin.as_str() {
            "Claude" | "Tagilla" => Ok(Self::Claude),
            _ => Err(de::Error::unknown_variant(&skin, &["Claude", "Tagilla"])),
        }
    }
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
    Climb,
    Ledge,
    Chase,
    Hold,
    Peek,
    Dance,
    Wave,
    Laptop,
    Stretch,
    LookAround,
    Social,
    Activity,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    Wander,
    Sit,
    Sleep,
    Climb,
    Chase,
    Dance,
    Wave,
    Laptop,
    Stretch,
    LookAround,
    Activity(crate::activities::ActivityId),
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
    pub dragging: bool,
    release_pending: bool,
    pub chase: bool,
    pub squash: f64,
    /// A small, transient mood signal for animation and gentle feedback.
    pub satisfaction: f64,
    /// Current coordinated interaction, when one is active.
    pub social: Option<crate::social::SocialActivity>,
    /// Movement contract issued by the social coordinator.
    pub social_motion: Option<SocialMotion>,
    /// Current render scale, mirrored into vertical social spacing.
    pub display_scale: f64,
    /// Normalized gaze direction in world axes, smoothed toward the cursor.
    pub gaze: Vec2,
    /// Short lived point of interest for the renderer (for example, a newly opened file manager).
    pub attention_target: Option<Vec2>,
    /// Enables gentle reactions to visible desktop activity.
    pub auto_reactions: bool,
    pub activity: Option<crate::activities::ActivityId>,
    pub auto_activities: bool,
    pub(crate) auto_activity_cooldown: f64,
    pub(crate) reaction_scan: f64,
    pub(crate) reaction_cooldown: f64,
    pub(crate) cursor_dwell: f64,
    pub(crate) cursor_interest: bool,
    pub(crate) cursor_motion_recent: f64,
    pub(crate) auto_pursuit_time: f64,
    pub(crate) pursuit_stationary: f64,
    pub(crate) auto_pursuit: bool,
    pub(crate) attention_time: f64,
    pub(crate) prior_reaction_windows: Vec<(usize, bool, bool, u64)>,
    pub(crate) current_reaction_windows: Vec<(usize, bool, bool, u64)>,
    pub(crate) reaction_baselined: bool,
    pub(crate) last_activity: Option<crate::activities::ActivityId>,
    pub(crate) pending_activity: Option<crate::activities::ActivityId>,
    pub(crate) rng: u64,
    pub(crate) idle_for: f64,
    pub(crate) forced_sleep: bool,
    pub(crate) forced_sleep_max: f64,
    pub(crate) forced_sleep_armed: bool,
    pub(crate) pending_ground: Option<State>,
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
    pub(crate) grab_cooldown: f64,
    pub(crate) explore_cooldown: f64,
    pub(crate) peek_goal: Option<(usize, Vec2)>,
    pub(crate) peek_route_age: f64,
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
            dragging: false,
            release_pending: false,
            chase: false,
            squash: 0.0,
            satisfaction: 75.0,
            social: None,
            social_motion: None,
            display_scale: 1.0,
            gaze: Vec2::default(),
            attention_target: None,
            auto_reactions: true,
            activity: None,
            auto_activities: true,
            auto_activity_cooldown: 12.0,
            reaction_scan: 0.0,
            reaction_cooldown: 0.0,
            cursor_dwell: 0.0,
            cursor_interest: false,
            cursor_motion_recent: 0.0,
            auto_pursuit_time: 0.0,
            pursuit_stationary: 0.0,
            auto_pursuit: false,
            attention_time: 0.0,
            prior_reaction_windows: Vec::with_capacity(256),
            current_reaction_windows: Vec::with_capacity(256),
            reaction_baselined: false,
            last_activity: None,
            pending_activity: None,
            rng: id.wrapping_mul(0x9e3779b97f4a7c15).max(1),
            idle_for: 0.0,
            forced_sleep: false,
            forced_sleep_max: 0.0,
            forced_sleep_armed: false,
            pending_ground: None,
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
            grab_cooldown: 0.0,
            explore_cooldown: 0.0,
            peek_goal: None,
            peek_route_age: 0.0,
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
        self.grab_cooldown = (self.grab_cooldown - dt).max(0.0);
        self.explore_cooldown = (self.explore_cooldown - dt).max(0.0);
        self.auto_activity_cooldown = (self.auto_activity_cooldown - dt).max(0.0);
        self.reaction_scan = (self.reaction_scan - dt).max(0.0);
        self.reaction_cooldown = (self.reaction_cooldown - dt).max(0.0);
        self.attention_time = (self.attention_time - dt).max(0.0);
        if self.attention_time == 0.0 {
            self.attention_target = None;
        }
        self.bubble_time = (self.bubble_time - dt).max(0.0);
        if self.bubble_time == 0.0 {
            self.bubble = None;
        }
        self.hearts = (self.hearts - dt).max(0.0);
        self.squash = (self.squash - dt * 2.0).max(0.0);
        if !self.satisfaction.is_finite() {
            self.satisfaction = 75.0;
        } else if self.satisfaction < 75.0 {
            self.satisfaction = (self.satisfaction + dt).clamp(0.0, 75.0);
        } else {
            self.satisfaction = self.satisfaction.clamp(0.0, 100.0);
        }
        self.update_gaze(dt, world);
        if !valid_vec(self.pos) || !valid_vec(self.vel) {
            self.recover(world);
            return;
        }
        self.prune_shake();

        if self.update_social_motion(dt, world) {
            self.snapshot_windows(world);
            self.last_cursor = world.cursor;
            return;
        }

        if self.social.is_some() && self.state == State::Social {
            if self.dragging {
                return;
            }
            if let Some(i) = self.current_surface_index(world) {
                self.follow_surface(i, world);
            } else {
                let floor = self.floor_y(world);
                if self.surface_id.is_some() || (self.pos.y - floor).abs() > 3.0 {
                    self.social = None;
                    self.surface = None;
                    self.surface_id = None;
                    self.surface_prev = None;
                    self.vel.y = self.vel.y.max(0.0);
                    self.set_state(State::Fall, 0.0);
                }
            }
            self.snapshot_windows(world);
            return;
        }

        if !self.initialized {
            self.last_cursor = world.cursor;
            self.initialized = true;
            self.prior_reaction_windows.clear();
            self.prior_reaction_windows.extend(
                world
                    .windows
                    .iter()
                    .filter(|w| !w.is_desktop_icon)
                    .take(256)
                    .map(|w| {
                        (
                            w.id,
                            w.is_file_manager,
                            w.active,
                            if w.is_file_manager {
                                title_hash(&w.title)
                            } else {
                                0
                            },
                        )
                    }),
            );
            self.reaction_baselined = true;
        }

        self.update_reactions(dt, world);

        self.update_window_motion(world);
        self.update_idle_sleep(world);
        self.update_idle_animation(dt, world);

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
            if self.chase {
                self.track_hold(world, dt);
            } else {
                self.cooldown = self.cooldown.max(1.0);
                self.hold_offset = Vec2::default();
                self.surface = None;
                self.surface_id = None;
                self.surface_prev = None;
                self.vel = Vec2::default();
                self.set_state(State::Fall, 0.0);
            }
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
        self.advance_peek_route(dt, world);
        self.snapshot_windows(world);
        self.last_cursor = world.cursor;
    }

    pub fn command(&mut self, command: Command, world: &World) {
        self.social_motion = None;
        self.cancel_auto_pursuit();
        self.cursor_dwell = 0.0;
        self.cursor_interest = false;
        self.attention_target = None;
        self.attention_time = 0.0;
        self.social = None;
        self.activity = None;
        self.pending_activity = None;
        self.pending_ground = None;
        if matches!(
            self.state,
            State::Laptop | State::Stretch | State::LookAround | State::Social | State::Activity
        ) {
            self.set_state(
                if self.has_support(world) {
                    State::Idle
                } else {
                    State::Fall
                },
                0.0,
            );
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
                    if w.is_desktop_icon {
                        let edge_x = if dir < 0.0 { w.rect.x } else { w.rect.right() };
                        if distance(self.pos, Vec2::new(edge_x, w.rect.y)) > 60.0 {
                            let _ = self.start_icon_jump(i, world);
                            return;
                        }
                    }
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
            Command::Dance => self.command_pose(State::Dance, 3.0, world),
            Command::Wave => self.command_pose(State::Wave, 2.0, world),
            Command::Laptop => self.command_pose(State::Laptop, 10.0, world),
            Command::Stretch => self.command_pose(State::Stretch, 2.5, world),
            Command::LookAround => self.command_pose(State::LookAround, 2.0, world),
            Command::Activity(id) => {
                self.chase = false;
                self.peek_goal = None;
                self.target = None;
                if self.has_support(world) {
                    self.vel = Vec2::default();
                    self.activity = Some(id);
                    self.set_state(State::Activity, id.spec().duration);
                    self.say(id.spec().phrase, id.spec().duration.min(3.0));
                } else {
                    self.pending_activity = Some(id);
                    self.pending_ground = Some(State::Activity);
                    self.chase = false;
                    self.peek_goal = None;
                    self.target = None;
                    self.hold_offset = Vec2::default();
                    if !self.is_airborne() {
                        self.surface = None;
                        self.surface_id = None;
                        self.surface_prev = None;
                        self.vel.x = 0.0;
                        self.set_state(State::Fall, 0.0);
                    }
                }
            }
        }
        match command {
            Command::Laptop => self.say("сейчас допечатаю", 2.0),
            Command::Stretch => self.say("разомнусь немного", 2.0),
            Command::LookAround => self.say("что тут интересного?", 2.0),
            _ => {}
        }
    }

    /// Window ID targeted by the current autonomous window hop, if any.
    pub fn target_id(&self) -> Option<u64> {
        self.target
    }

    pub fn drag_to(&mut self, pos: Vec2) {
        if !valid_vec(pos) {
            return;
        }
        self.dragging = true;
        self.cancel_auto_pursuit();
        self.attention_target = None;
        self.attention_time = 0.0;
        self.cursor_dwell = 0.0;
        self.social_motion = None;
        self.social = None;
        self.activity = None;
        self.pending_activity = None;
        self.pending_ground = None;
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
        self.cancel_auto_pursuit();
        self.attention_target = None;
        self.attention_time = 0.0;
        self.social_motion = None;
        self.social = None;
        self.activity = None;
        self.pending_activity = None;
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
        let speed = self.vel.x.hypot(self.vel.y);
        if speed > 400.0 {
            self.satisfaction =
                (self.satisfaction - (speed / 2500.0 * 18.0).clamp(6.0, 18.0)).max(0.0);
            if self.satisfaction < 30.0 {
                self.say("эй, аккуратнее!", 1.8);
            }
        }
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
        self.cancel_auto_pursuit();
        self.attention_target = None;
        self.attention_time = 0.0;
        self.social_motion = None;
        self.social = None;
        self.activity = None;
        self.pending_activity = None;
        self.pending_ground = None;
        self.forced_sleep = false;
        self.satisfaction = (self.satisfaction + 12.0).min(100.0);
        self.hearts = 1.5;
        self.squash = 0.35;
        self.say("приятно :) ", 1.8);
        if matches!(
            self.state,
            State::Idle | State::Walk | State::Sit | State::Social | State::Activity
        ) {
            self.set_state(State::Happy, 1.1);
        }
    }

    pub fn jump(&mut self) {
        self.cancel_auto_pursuit();
        self.attention_target = None;
        self.attention_time = 0.0;
        self.social_motion = None;
        self.social = None;
        self.activity = None;
        self.pending_activity = None;
        if self.pending_ground == Some(State::Activity) {
            self.pending_ground = None;
        }
        if self.surface.is_some()
            || matches!(
                self.state,
                State::Idle
                    | State::Walk
                    | State::Run
                    | State::Sit
                    | State::Sleep
                    | State::Land
                    | State::Ledge
                    | State::Happy
                    | State::Dance
                    | State::Wave
                    | State::Laptop
                    | State::Stretch
                    | State::LookAround
                    | State::Activity
                    | State::Social
                    | State::Hold
                    | State::Chase
            )
        {
            self.pending_ground = None;
            self.forced_sleep = false;
            self.forced_sleep_armed = false;
            if self.state == State::Hold {
                self.grab_cooldown = 1.0;
            }
            self.chase = false;
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
        if matches!(
            self.state,
            State::Dance
                | State::Wave
                | State::Laptop
                | State::Stretch
                | State::LookAround
                | State::Activity
        ) {
            if self.state_time >= self.state_duration.max(0.1) {
                if self.state == State::Activity {
                    self.activity = None;
                }
                self.set_state(State::Idle, 0.0);
            } else if let Some(i) = self.current_surface_index(world) {
                self.follow_surface(i, world);
            } else {
                let floor = self.floor_y(world);
                if self.pos.y < floor - 2.0 {
                    self.surface = None;
                    self.set_state(State::Fall, 0.0);
                } else {
                    self.pos.y = floor;
                }
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
                if next == State::Activity {
                    self.activity = self.pending_activity.take();
                    if let Some(id) = self.activity {
                        self.say(id.spec().phrase, id.spec().duration.min(3.0));
                    }
                    self.set_state(next, self.activity.map_or(0.0, |id| id.spec().duration));
                } else {
                    self.set_state(next, pose_duration(next));
                }
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
                // A horizontal throw may land before it reaches the wall.
                // Keep the same rebound while its landing momentum slides.
                self.bounce_from_screen_wall(world);
                return;
            }
        }
        if self.state == State::Happy {
            if self.state_time > self.state_duration.max(0.55) {
                let next = self.pending_ground.take().unwrap_or(State::Idle);
                self.set_state(next, pose_duration(next));
            } else {
                self.vel = Vec2::default();
                return;
            }
        }
        if self.state == State::Sit {
            if let Some(i) = self.surface {
                self.follow_surface(i, world);
            }
            if self.state_duration > 0.0 && self.state_time >= self.state_duration {
                self.set_state(State::Idle, 0.0);
            }
            return;
        }

        if self.state == State::Chase || (self.chase && !self.is_airborne()) {
            self.update_chase(dt, world);
            return;
        }

        if self.state == State::Fall || self.state == State::Jump || self.state == State::Thrown {
            if self.state == State::Jump && self.chase && self.skin == Skin::Claude {
                self.try_catch_cursor(world);
                if self.state == State::Hold {
                    return;
                }
            }
            // A drag can release the pet below the desktop, where the normal
            // floor-crossing test cannot catch it (old_y is already past it).
            // Recover to the nearest remaining monitor's floor before applying
            // gravity so it cannot fall forever after a monitor change either.
            let floor = self.floor_y(world);
            if self.pos.y > floor {
                if matches!(self.social_motion, Some(SocialMotion::Leap { .. })) {
                    self.social_motion = None;
                    self.social = None;
                }
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
            if self.state == State::Thrown {
                self.bounce_from_screen_wall(world);
            } else {
                self.clamp_to_area(world);
            }
            if let Some(SocialMotion::Leap { target, owner_id }) = self.social_motion {
                if self.vel.y >= 0.0
                    && old.y <= target.y
                    && self.pos.y >= target.y
                    && (self.pos.x - target.x).abs() <= BODY_HALF_WIDTH + 8.0
                    && (self.pos.y - target.y).abs() <= 80.0
                {
                    self.pos = target;
                    self.vel = Vec2::default();
                    self.social_motion = Some(SocialMotion::Perch { target, owner_id });
                    self.state = State::Social;
                    self.state_time = 0.0;
                    return;
                }
            }
            if self.vel.y >= 0.0 {
                if let Some((ground_y, surface)) = self.find_landing(old.y, world) {
                    if matches!(self.social_motion, Some(SocialMotion::Leap { .. })) {
                        self.social_motion = None;
                        self.social = None;
                    }
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
            let cue_speed = match self.social_motion {
                Some(SocialMotion::Run { speed, .. }) if speed.is_finite() => {
                    speed.clamp(0.0, 280.0)
                }
                _ => self.speed(),
            };
            self.pos.x += self.facing * cue_speed * dt;
            if !self.platform_under(self.pos.x, world) {
                self.vel.x = self.facing * self.speed();
                self.vel.y = 20.0;
                self.surface = None;
                self.set_state(State::Fall, 0.0);
            } else {
                self.vel.x = 0.0;
                if self.social_motion.is_none() && self.random_chance(dt * 0.12) {
                    self.set_state(State::Idle, 0.0);
                }
            }
        } else if self.state == State::Idle
            && self.random_chance(dt * 0.16)
            && !self.try_explore_jump(world)
        {
            let choice = self.random();
            if choice < 0.12 && self.satisfaction >= 30.0 && self.auto_activities {
                self.set_state(State::Dance, 2.6);
            } else if choice < 0.22 && self.satisfaction >= 30.0 && self.auto_activities {
                self.set_state(State::Wave, 1.8);
            } else if choice < 0.42 {
                let sit_duration = 4.0 + self.random() * 5.0;
                self.set_state(State::Sit, sit_duration);
            } else {
                self.facing *= -1.0;
                if choice > 0.86 {
                    self.vel = Vec2::new(self.facing * 90.0, -620.0);
                    self.squash = 0.25;
                    self.set_state(State::Jump, 0.0);
                    return;
                }
                self.set_state(
                    if choice > 0.67 {
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
            let icon = w.is_desktop_icon && w.rect.w >= 10.0 && w.rect.h >= 10.0;
            if !valid_rect(w.rect) || (!icon && (w.rect.w < 40.0 || w.rect.h < 30.0)) {
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
        if self.auto_pursuit {
            self.auto_pursuit_time = (self.auto_pursuit_time - dt).max(0.0);
            let area = world.area_at(self.pos);
            if distance(world.cursor, self.last_cursor) <= 2.5 {
                self.pursuit_stationary += dt;
            } else {
                self.pursuit_stationary = 0.0;
            }
            let reachable = valid_rect(area)
                && world.cursor.y >= area.y
                && world.cursor.y <= area.bottom()
                && world.cursor.x >= area.x
                && world.cursor.x <= area.right();
            if self.auto_pursuit_time <= 0.0
                || self.pursuit_stationary >= 0.55
                || !reachable
                || distance(world.cursor, self.pos) > 420.0
                || dx.abs() < 34.0
            {
                self.cancel_auto_pursuit();
                self.set_state(State::Idle, 0.0);
                return;
            }
            self.facing = if dx < 0.0 { -1.0 } else { 1.0 };
            self.pos.x = (self.pos.x + self.facing * RUN_SPEED * dt)
                .clamp(area.x + BODY_HALF_WIDTH, area.right() - BODY_HALF_WIDTH);
            self.set_state(State::Chase, 0.0);
            return;
        }
        if self.skin == Skin::Claude && self.try_catch_cursor(world) {
            return;
        }
        // Hop toward a cursor that is well above us. The grounded check and
        // cooldown keep the chase from repeatedly launching in midair.
        if self.cooldown <= 0.0 && self.has_support(world) && self.pos.y - world.cursor.y > 130.0 {
            let rise = (self.pos.y - world.cursor.y - CURSOR_HOLD_DY).clamp(100.0, 650.0);
            self.vel = Vec2::new(
                (dx * 1.4).clamp(-320.0, 320.0),
                -(2.0 * GRAVITY * rise).sqrt(),
            );
            self.surface = None;
            self.surface_id = None;
            self.surface_prev = None;
            self.cooldown = 1.25;
            self.squash = 0.25;
            self.set_state(State::Jump, 0.0);
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
            self.grab_cooldown = 1.0;
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
            && !matches!(self.state, State::Fall | State::Jump | State::Thrown)
        {
            self.vel = Vec2::default();
            self.set_state(State::Sleep, 0.0);
        }
    }

    fn update_idle_animation(&mut self, dt: f64, world: &World) {
        if self.state != State::Idle || self.dragging || self.chase || !self.has_support(world) {
            return;
        }
        if self.auto_activities
            && self.auto_activity_cooldown <= 0.0
            && self.random_chance(dt * 0.1)
        {
            let len = crate::activities::CATALOG.len();
            let mut index = (self.random() * len as f64) as usize;
            if self
                .last_activity
                .is_some_and(|last| len > 1 && crate::activities::CATALOG[index].id == last)
            {
                index = (index + 1) % len;
            }
            if let Some(spec) = crate::activities::CATALOG.get(index) {
                self.last_activity = Some(spec.id);
                self.activity = Some(spec.id);
                self.auto_activity_cooldown = 20.0 + self.random() * 15.0;
                self.set_state(State::Activity, spec.duration);
                self.say(spec.phrase, spec.duration.min(3.0));
                return;
            }
        }
        if !self.random_chance(dt * 0.01) {
            return;
        }
        match (self.random() * 3.0) as u8 {
            0 => {
                self.set_state(State::Laptop, 10.0);
                self.say("сейчас допечатаю", 2.0);
            }
            1 => {
                self.set_state(State::Stretch, 2.5);
                self.say("разомнусь немного", 2.0);
            }
            _ => {
                self.set_state(State::LookAround, 2.0);
                self.say("что тут интересного?", 2.0);
            }
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

    fn head_clearance(&self) -> f64 {
        55.0 * self.display_scale.clamp(0.5, 2.0) + 8.0
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

    pub(crate) fn socially_supported(&self, world: &World) -> bool {
        !self.dragging && self.has_support(world)
    }

    fn update_gaze(&mut self, dt: f64, world: &World) {
        let eye = Vec2::new(self.pos.x, self.pos.y - 40.0);
        let dx = world.cursor.x - eye.x;
        let dy = world.cursor.y - eye.y;
        let length = dx.hypot(dy);
        let target =
            if self.state == State::Sleep || !length.is_finite() || !valid_vec(world.cursor) {
                Vec2::default()
            } else if length > 6.0 && length < 240.0 {
                let amount = if length < 140.0 {
                    1.0
                } else {
                    (240.0 - length) / 100.0
                };
                Vec2::new(
                    (dx / length * amount).clamp(-1.0, 1.0),
                    (dy / length * amount).clamp(-1.0, 1.0),
                )
            } else if length >= 240.0 && self.attention_time > 0.0 {
                self.attention_target.map_or(Vec2::default(), |target| {
                    let ax = target.x - eye.x;
                    let ay = target.y - eye.y;
                    let al = ax.hypot(ay);
                    if al > 1.0 && al.is_finite() {
                        Vec2::new(ax / al, ay / al)
                    } else {
                        Vec2::default()
                    }
                })
            } else {
                Vec2::default()
            };
        let blend = 1.0 - (-12.0 * dt).exp();
        self.gaze.x = if self.gaze.x.is_finite() {
            self.gaze.x
        } else {
            0.0
        };
        self.gaze.y = if self.gaze.y.is_finite() {
            self.gaze.y
        } else {
            0.0
        };
        self.gaze.x = (self.gaze.x + (target.x - self.gaze.x) * blend).clamp(-1.0, 1.0);
        self.gaze.y = (self.gaze.y + (target.y - self.gaze.y) * blend).clamp(-1.0, 1.0);
    }

    fn update_reactions(&mut self, dt: f64, world: &World) {
        if !self.auto_reactions {
            self.reaction_baselined = false;
            self.prior_reaction_windows.clear();
            self.attention_target = None;
            self.attention_time = 0.0;
            self.cancel_auto_pursuit();
            self.cursor_dwell = 0.0;
            return;
        }
        if self.dragging || !valid_vec(world.cursor) {
            self.reaction_baselined = false;
            self.cancel_auto_pursuit();
            self.cursor_dwell = 0.0;
            return;
        }
        self.cursor_motion_recent = (self.cursor_motion_recent - dt).max(0.0);
        if distance(world.cursor, self.last_cursor) > 2.5 {
            self.cursor_motion_recent = 0.2;
        }
        if self.reaction_scan <= 0.0 {
            self.reaction_scan = 0.1;
            self.current_reaction_windows.clear();
            self.current_reaction_windows.extend(
                world
                    .windows
                    .iter()
                    .filter(|w| !w.is_desktop_icon)
                    .take(256)
                    .map(|w| {
                        (
                            w.id,
                            w.is_file_manager,
                            w.active,
                            if w.is_file_manager {
                                title_hash(&w.title)
                            } else {
                                0
                            },
                        )
                    }),
            );
            if !self.reaction_baselined {
                std::mem::swap(
                    &mut self.prior_reaction_windows,
                    &mut self.current_reaction_windows,
                );
                self.reaction_baselined = true;
            } else {
                let explorer_event =
                    self.current_reaction_windows
                        .iter()
                        .any(|&(id, explorer, active, title)| {
                            explorer
                                && active
                                && !self.prior_reaction_windows.iter().any(
                                    |&(old_id, old_explorer, old_active, old_title)| {
                                        id == old_id
                                            && old_explorer
                                            && old_active
                                            && old_title == title
                                    },
                                )
                        });
                let new_window = self.current_reaction_windows.iter().any(|&(id, _, _, _)| {
                    !self
                        .prior_reaction_windows
                        .iter()
                        .any(|&(old_id, _, _, _)| id == old_id)
                });
                if self.reaction_cooldown <= 0.0 && self.eligible_for_reaction(world) {
                    if explorer_event {
                        if let Some(w) =
                            world.windows.iter().find(|w| w.is_file_manager && w.active)
                        {
                            self.attention_target =
                                Some(Vec2::new(w.rect.x + w.rect.w * 0.5, w.rect.y + 30.0));
                            self.attention_time = 1.5;
                            self.say("о, папка открылась", 2.0);
                            self.set_state(State::Wave, 1.0);
                            self.reaction_cooldown = 12.0;
                        }
                    } else if new_window && self.random_chance(0.35) {
                        self.say("кто пришёл?", 1.5);
                        self.set_state(State::LookAround, 0.8);
                        self.reaction_cooldown = 12.0;
                    }
                }
                std::mem::swap(
                    &mut self.prior_reaction_windows,
                    &mut self.current_reaction_windows,
                );
            }
        }
        let near = distance(world.cursor, self.pos) < 210.0;
        let eligible = self.eligible_for_reaction(world);
        if near && self.cursor_motion_recent > 0.0 && eligible {
            self.cursor_dwell += dt;
            self.cursor_interest = true;
        } else if !near || !eligible || self.cursor_motion_recent <= 0.0 {
            self.cursor_dwell = 0.0;
            self.cursor_interest = false;
        }
        if self.cursor_dwell >= 0.7 && self.reaction_cooldown <= 0.0 && eligible {
            self.auto_pursuit = true;
            self.pursuit_stationary = 0.0;
            self.auto_pursuit_time = 3.5;
            self.reaction_cooldown = 20.0;
            self.cursor_dwell = 0.0;
            self.set_state(State::Chase, 0.0);
        }
    }

    fn eligible_for_reaction(&self, world: &World) -> bool {
        matches!(self.state, State::Idle | State::Walk)
            && !self.chase
            && self.social.is_none()
            && self.activity.is_none()
            && self.has_support(world)
            && self.satisfaction >= 35.0
    }

    fn cancel_auto_pursuit(&mut self) {
        let was_automatic = self.auto_pursuit;
        self.auto_pursuit = false;
        self.auto_pursuit_time = 0.0;
        self.pursuit_stationary = 0.0;
        if was_automatic && !self.chase && self.state == State::Chase {
            self.set_state(State::Idle, 0.0);
        }
    }

    fn update_social_motion(&mut self, _dt: f64, world: &World) -> bool {
        match self.social_motion {
            Some(SocialMotion::Perch { target, .. }) => {
                if !valid_vec(target) || distance(self.pos, target) > 80.0 || self.dragging {
                    self.social_motion = None;
                    self.social = None;
                    self.surface = None;
                    self.surface_id = None;
                    self.vel = Vec2::default();
                    self.set_state(State::Fall, 0.0);
                    return false;
                }
                self.pos = target;
                self.vel = Vec2::default();
                self.surface = None;
                self.surface_id = None;
                self.surface_prev = None;
                if self.state != State::Social {
                    self.state = State::Social;
                    self.state_time = 0.0;
                }
                true
            }
            Some(SocialMotion::Leap { target, .. }) => {
                if !valid_vec(target) || self.dragging {
                    self.social_motion = None;
                    return false;
                }
                if self.state != State::Jump && !self.is_airborne() {
                    let rise = (self.pos.y - target.y + self.head_clearance()).clamp(80.0, 600.0);
                    let area = world.area_at(self.pos);
                    let available_rise = (self.pos.y - area.y - self.head_clearance()).max(0.0);
                    if rise > available_rise {
                        self.social_motion = None;
                        self.social = None;
                        self.set_state(State::Fall, 0.0);
                        return false;
                    }
                    let launch = (2.0 * GRAVITY * rise).sqrt();
                    let dy = target.y - self.pos.y;
                    let disc = (launch * launch + 2.0 * GRAVITY * dy).max(0.0);
                    let flight = (launch + disc.sqrt()) / GRAVITY;
                    if !flight.is_finite() || flight <= 0.0 {
                        self.social_motion = None;
                        return false;
                    }
                    self.facing = if target.x < self.pos.x { -1.0 } else { 1.0 };
                    self.vel = Vec2::new(
                        ((target.x - self.pos.x) / flight).clamp(-420.0, 420.0),
                        -launch,
                    );
                    self.surface = None;
                    self.surface_id = None;
                    self.surface_prev = None;
                    self.state = State::Jump;
                    self.state_time = 0.0;
                }
                false
            }
            Some(SocialMotion::Run { target_x, speed }) => {
                if self.dragging
                    || !target_x.is_finite()
                    || !speed.is_finite()
                    || !self.has_support(world)
                {
                    self.social_motion = None;
                    self.social = None;
                    return false;
                }
                let area = world.area_at(self.pos);
                let target_x =
                    target_x.clamp(area.x + BODY_HALF_WIDTH, area.right() - BODY_HALF_WIDTH);
                let dx = target_x - self.pos.x;
                if dx.abs() <= 8.0 {
                    self.vel = Vec2::default();
                    self.state = State::Social;
                    self.state_time = 0.0;
                    return true;
                }
                self.facing = dx.signum();
                if self.state != State::Run {
                    self.state = State::Run;
                    self.state_time = 0.0;
                }
                false
            }
            None => false,
        }
    }

    fn nearest_edge<'a>(&self, world: &'a World) -> Option<(usize, &'a WindowInfo, f64)> {
        world
            .windows
            .iter()
            .enumerate()
            .filter(|(_, w)| {
                valid_rect(w.rect)
                    && ((w.is_desktop_icon && w.rect.w >= 16.0 && w.rect.h >= 16.0)
                        || (w.rect.w >= 80.0 && w.rect.h >= 60.0))
            })
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

    fn start_icon_jump(&mut self, index: usize, world: &World) -> bool {
        let Some(icon) = world.windows.get(index).filter(|w| w.is_desktop_icon) else {
            return false;
        };
        if !self.has_support(world) || !valid_rect(icon.rect) {
            return false;
        }
        let target = Vec2::new(icon.rect.x + icon.rect.w * 0.5, icon.rect.y);
        let area = world.area_at(self.pos);
        let head_clearance = self.head_clearance();
        let rise = self.pos.y - target.y + head_clearance;
        let available = (self.pos.y - area.y - head_clearance).clamp(0.0, 900.0);
        if !area.contains(target)
            || rise <= head_clearance
            || rise > available
            || covered(world, Vec2::new(target.x, target.y - 1.0), index)
        {
            return false;
        }
        let launch = (2.0 * GRAVITY * rise).sqrt().min(2_050.0);
        let dy = target.y - self.pos.y;
        let disc = launch * launch + 2.0 * GRAVITY * dy;
        if disc < 0.0 {
            return false;
        }
        let flight = (launch + disc.sqrt()) / GRAVITY;
        let vx = (target.x - self.pos.x) / flight;
        if !flight.is_finite() || flight <= 0.0 || vx.abs() > 360.0 {
            return false;
        }
        self.facing = if vx < 0.0 { -1.0 } else { 1.0 };
        self.vel = Vec2::new(vx, -launch);
        self.surface = None;
        self.surface_id = None;
        self.surface_prev = None;
        self.set_state(State::Jump, 0.0);
        self.target = Some(icon.id as u64);
        self.explore_cooldown = 5.0;
        true
    }

    /// Occasionally take a reachable window-top jump while idle. Candidate
    /// reachability uses a fixed ballistic arc so it stays deterministic.
    fn try_explore_jump(&mut self, world: &World) -> bool {
        if self.explore_cooldown > 0.0 || self.state != State::Idle || !self.has_support(world) {
            return false;
        }
        let mut best: Option<(f64, f64, f64, usize)> = None;
        for (i, w) in world.windows.iter().enumerate() {
            let icon = w.is_desktop_icon && w.rect.w >= 16.0 && w.rect.h >= 16.0;
            if !valid_rect(w.rect) || (!icon && w.rect.w < 80.0) || w.rect.y >= self.pos.y - 60.0 {
                continue;
            }
            let target_x = if icon && w.rect.w < BODY_HALF_WIDTH * 2.0 {
                w.rect.x + w.rect.w * 0.5
            } else {
                self.pos
                    .x
                    .clamp(w.rect.x + BODY_HALF_WIDTH, w.rect.right() - BODY_HALF_WIDTH)
            };
            let area = world.area_at(Vec2::new(target_x, w.rect.y));
            let head_clearance = self.head_clearance();
            let rise = self.pos.y - w.rect.y + head_clearance;
            let available_rise = (self.pos.y - area.y - head_clearance).clamp(0.0, 900.0);
            if !area.contains(Vec2::new(target_x, w.rect.y)) || rise > available_rise {
                continue;
            }
            let launch = (2.0 * GRAVITY * rise).sqrt().min(2_050.0);
            let dy = w.rect.y - self.pos.y;
            let disc = launch * launch + 2.0 * GRAVITY * dy;
            if disc < 0.0 {
                continue;
            }
            let t = (launch + disc.sqrt()) / GRAVITY;
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
        let rise = self.pos.y - world.windows[index].rect.y + self.head_clearance();
        let launch = (2.0 * GRAVITY * rise).sqrt().min(2_050.0);
        self.vel = Vec2::new(vx, -launch);
        self.surface = None;
        self.surface_id = None;
        self.surface_prev = None;
        self.set_state(State::Jump, 0.0);
        self.target = Some(world.windows[index].id as u64);
        self.explore_cooldown = 8.0 + self.random() * 10.0;
        true
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
        self.prior_windows.clear();
        self.prior_windows
            .extend(world.windows.iter().map(|window| (window.id, window.rect)));
    }
    fn set_state(&mut self, state: State, duration: f64) {
        if self.state == State::Activity && state != State::Activity {
            self.activity = None;
        }
        if self.state == State::Social && state != State::Social {
            self.social = None;
        }
        if self.state != state {
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
        if self.state == State::Run || self.state == State::Chase {
            RUN_SPEED * self.skin_speed()
        } else {
            WALK_SPEED * self.skin_speed()
        }
    }
    fn skin_speed(&self) -> f64 {
        1.0
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
            let (left_connected, right_connected) = horizontal_neighbors(a, self.pos.y, world);
            let left = if left_connected { a.x } else { a.x + inset };
            let right = if right_connected {
                a.right()
            } else {
                a.right() - inset
            };
            let before = self.pos.x;
            self.pos.x = self.pos.x.clamp(left, right);
            if self.pos.x != before && matches!(self.state, State::Walk | State::Run) {
                self.facing = if before < left { 1.0 } else { -1.0 };
                self.squash = self.squash.max(0.15);
            }
        }
    }

    fn bounce_from_screen_wall(&mut self, world: &World) {
        let a = world.area_at(self.pos);
        if !valid_rect(a) {
            return;
        }
        let inset = BODY_HALF_WIDTH.min(a.w * 0.5);
        let (left_connected, right_connected) = horizontal_neighbors(a, self.pos.y, world);
        let left = if left_connected { a.x } else { a.x + inset };
        let right = if right_connected {
            a.right()
        } else {
            a.right() - inset
        };
        if self.pos.x < left && self.vel.x < 0.0 {
            self.pos.x = left;
            if self.vel.x.abs() > WALL_BOUNCE_MIN_SPEED {
                self.vel.x = -self.vel.x * WALL_RESTITUTION;
                self.squash = 0.35;
            } else {
                self.vel.x = 0.0;
            }
        } else if self.pos.x > right && self.vel.x > 0.0 {
            self.pos.x = right;
            if self.vel.x.abs() > WALL_BOUNCE_MIN_SPEED {
                self.vel.x = -self.vel.x * WALL_RESTITUTION;
                self.squash = 0.35;
            } else {
                self.vel.x = 0.0;
            }
        } else {
            self.pos.x = self.pos.x.clamp(left, right);
        }
    }

    fn command_pose(&mut self, state: State, duration: f64, world: &World) {
        if !self.has_support(world) {
            self.pending_ground = Some(state);
            self.chase = false;
            if matches!(self.state, State::Hold | State::Chase) {
                self.hold_offset = Vec2::default();
                self.vel.x = 0.0;
                self.set_state(State::Fall, 0.0);
            }
            return;
        }
        self.vel = Vec2::default();
        self.set_state(state, duration);
    }

    fn try_catch_cursor(&mut self, world: &World) -> bool {
        if self.grab_cooldown > 0.0 || self.state == State::Hold || self.skin != Skin::Claude {
            return false;
        }
        let grip = Vec2::new(self.pos.x, self.pos.y - CURSOR_HOLD_DY);
        if distance(world.cursor, grip) > 40.0 {
            return false;
        }
        self.surface = None;
        self.surface_id = None;
        self.surface_prev = None;
        self.vel = Vec2::default();
        self.hold_offset = Vec2::new(0.0, CURSOR_HOLD_DY);
        self.pos = Vec2::new(world.cursor.x, world.cursor.y + CURSOR_HOLD_DY);
        self.set_state(State::Hold, 0.0);
        self.say("вишу!", 1.5);
        true
    }
}

fn valid_vec(p: Vec2) -> bool {
    p.x.is_finite() && p.y.is_finite() && p.x.abs() < MAX_COORD && p.y.abs() < MAX_COORD
}
fn pose_duration(state: State) -> f64 {
    match state {
        State::Dance => 3.0,
        State::Wave | State::LookAround => 2.0,
        State::Laptop => 10.0,
        State::Stretch => 2.5,
        _ => 0.0,
    }
}
fn horizontal_neighbors(area: Rect, y: f64, world: &World) -> (bool, bool) {
    let connected = |other: Rect| valid_rect(other) && y >= other.y && y <= other.bottom();
    let left = world
        .work_areas
        .iter()
        .copied()
        .any(|other| connected(other) && (other.right() - area.x).abs() < 1.0);
    let right = world
        .work_areas
        .iter()
        .copied()
        .any(|other| connected(other) && (other.x - area.right()).abs() < 1.0);
    (left, right)
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
fn distance(a: Vec2, b: Vec2) -> f64 {
    ((a.x - b.x).powi(2) + (a.y - b.y).powi(2)).sqrt()
}
fn distance_sq(a: Vec2, b: Vec2) -> f64 {
    (a.x - b.x).powi(2) + (a.y - b.y).powi(2)
}
fn title_hash(title: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    title.hash(&mut hasher);
    hasher.finish()
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

#[cfg(test)]
mod responsive_engine_tests {
    use super::*;

    fn support_world(area: Rect, platform_y: f64) -> World {
        World {
            work_areas: vec![area],
            windows: vec![WindowInfo {
                id: 9,
                rect: Rect::new(350.0, platform_y, 300.0, 100.0),
                title: String::new(),
                active: false,
                fullscreen: false,
                is_file_manager: false,
                is_desktop_icon: false,
            }],
            ..World::default()
        }
    }

    #[test]
    fn adaptive_window_hop_reaches_high_platforms_and_respects_ceiling() {
        let mut pet = Pet::new(9, Skin::Claude, Vec2::new(400.0, 1100.0));
        pet.state = State::Idle;
        pet.display_scale = 1.5;
        let high = support_world(Rect::new(0.0, 0.0, 1000.0, 1100.0), 300.0);
        assert!(pet.try_explore_jump(&high));
        assert!(pet.vel.y < -1_800.0);

        let mut pet = Pet::new(10, Skin::Claude, Vec2::new(400.0, 1100.0));
        pet.state = State::Idle;
        let constrained = support_world(Rect::new(0.0, 700.0, 1000.0, 400.0), 720.0);
        assert!(!pet.try_explore_jump(&constrained));
    }
}
