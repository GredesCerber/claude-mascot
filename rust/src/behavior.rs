//! Small, deterministic personal-space and context coordinator.
use crate::{
    engine::{Pet, State},
    World,
};
use serde::{Deserialize, Serialize};

const CAP: usize = 8;
const BODY_HALF: f64 = 40.0;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct SpacingPolicy {
    pub min_free_gap_px: f64,
    pub max_free_gap_px: f64,
}
impl Default for SpacingPolicy {
    fn default() -> Self {
        Self {
            min_free_gap_px: 24.0,
            max_free_gap_px: 96.0,
        }
    }
}
impl SpacingPolicy {
    fn sanitized(self) -> Self {
        let min = if self.min_free_gap_px.is_finite() {
            self.min_free_gap_px.clamp(8.0, 256.0)
        } else {
            24.0
        };
        let raw_max = if self.max_free_gap_px.is_finite() {
            self.max_free_gap_px.clamp(16.0, 512.0)
        } else {
            96.0
        };
        Self {
            min_free_gap_px: min,
            max_free_gap_px: raw_max.max(min + 8.0).min(512.0),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct BehaviorSettings {
    pub enabled: bool,
    pub spacing: SpacingPolicy,
    pub decision_interval_s: f64,
    pub cooldown_s: f64,
}
impl Default for BehaviorSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            spacing: SpacingPolicy::default(),
            decision_interval_s: 0.15,
            cooldown_s: 7.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum BehaviorReason {
    #[default]
    NoChange,
    Crowded,
    CrossingPath,
    JumpOverFriend,
    VoluntaryCompany,
    ExplorerNearby,
    FriendNearby,
    LowMood,
    BusyDesktop,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum BehaviorIntent {
    #[default]
    Hold,
    YieldLeft,
    YieldRight,
    FollowBriefly,
    Observe,
    Rest,
    CalmActivity,
    JumpOver,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BehaviorEvent {
    pub pet_id: u64,
    pub reason: BehaviorReason,
    pub intent: BehaviorIntent,
}

#[derive(Clone, Copy)]
struct Entry {
    id: u64,
    x: f64,
    half: f64,
    scale: f64,
    key: (Option<usize>, u64, i64),
    movable: bool,
    active: bool,
    facing: f64,
}
#[derive(Clone, Copy, Default)]
struct Stored {
    id: u64,
    cooldown: f64,
    intent: BehaviorIntent,
    reason: BehaviorReason,
    active: bool,
    steering: bool,
    hop_cooldown: f64,
    context_cooldown: f64,
}

pub struct BehaviorCoordinator {
    settings: BehaviorSettings,
    accumulator: f64,
    entries: [Option<Entry>; CAP],
    stored: [Stored; CAP],
    events: [Option<BehaviorEvent>; CAP],
    pub decision_count: u64,
    pub steering_count: u64,
}
impl BehaviorCoordinator {
    pub fn new(settings: BehaviorSettings) -> Self {
        Self {
            settings,
            accumulator: 0.0,
            entries: [None; CAP],
            stored: [Stored::default(); CAP],
            events: [None; CAP],
            decision_count: 0,
            steering_count: 0,
        }
    }
    pub fn settings(&self) -> BehaviorSettings {
        self.settings
    }
    pub fn set_settings(&mut self, settings: BehaviorSettings) {
        self.settings = settings;
    }
    pub fn events(&self) -> impl Iterator<Item = &BehaviorEvent> {
        self.events.iter().flatten()
    }
    pub fn last_intent(&self, pet_id: u64) -> Option<(BehaviorReason, BehaviorIntent)> {
        self.stored
            .iter()
            .find(|s| s.active && s.id == pet_id)
            .map(|s| (s.reason, s.intent))
    }

    pub fn update<'a, I>(&mut self, dt: f64, pets: I, world: &World)
    where
        I: IntoIterator<Item = &'a mut Pet>,
    {
        let dt = if dt.is_finite() {
            dt.clamp(0.0, 0.05)
        } else {
            0.0
        };
        self.accumulator = (self.accumulator + dt).min(0.5);
        let decision = self.accumulator >= self.settings.decision_interval_s.clamp(0.1, 0.2);
        let spacing = self.settings.spacing.sanitized();
        if decision {
            self.accumulator = 0.0;
            self.events.fill(None);
            self.decision_count = self.decision_count.saturating_add(1);
        }
        let mut slots: [Option<&mut Pet>; CAP] = std::array::from_fn(|_| None);
        let mut count = 0;
        for pet in pets {
            if count == CAP {
                break;
            }
            slots[count] = Some(pet);
            count += 1;
        }
        for state in &mut self.stored {
            if state.active && !slots.iter().take(count).flatten().any(|p| p.id == state.id) {
                *state = Stored::default();
            }
        }
        self.entries.fill(None);
        for (i, slot) in slots.iter().enumerate().take(count) {
            let p = slot.as_deref().unwrap();
            let Some(key) = p.behavior_support_key(world) else {
                continue;
            };
            let state_owned = p.social.is_some()
                || p.social_motion.is_some()
                || p.dragging
                || p.chase
                || p.auto_pursuit
                || p.activity.is_some()
                || p.forced_sleep
                || p.behavior_manual_lock > 0.0
                || p.pending_ground.is_some()
                || !matches!(p.state, State::Idle | State::Walk);
            let active = matches!(p.state, State::Walk | State::Approach);
            self.entries[i] = Some(Entry {
                id: p.id,
                x: p.pos.x,
                half: BODY_HALF * p.display_scale.clamp(0.5, 2.0),
                scale: p.display_scale.clamp(0.5, 2.0),
                key,
                movable: !state_owned,
                active,
                facing: p.facing,
            });
        }
        // Age timers and keep slots associated with stable IDs despite input reordering.
        for state in &mut self.stored {
            if state.active {
                state.cooldown = (state.cooldown - dt).max(0.0);
                state.hop_cooldown = (state.hop_cooldown - dt).max(0.0);
                state.context_cooldown = (state.context_cooldown - dt).max(0.0);
            }
        }
        for i in 0..count {
            let Some(a) = self.entries[i] else {
                continue;
            };
            let mut vx = 0.0;
            let mut steer = false;
            let mut jumped = false;
            if a.movable {
                let mut nearest: Option<(f64, Entry)> = None;
                for b in self.entries.iter().flatten().copied() {
                    if a.id == b.id
                        || a.key.0 != b.key.0
                        || a.key.1 != b.key.1
                        || (a.key.2 - b.key.2).abs() > 6
                    {
                        continue;
                    }
                    let d = (b.x - a.x).abs();
                    if nearest.is_none_or(|n| d < n.0) {
                        nearest = Some((d, b));
                    }
                }
                if let Some((d, b)) = nearest {
                    let min_gap = spacing.min_free_gap_px * (a.scale + b.scale) * 0.5;
                    let center_gap = a.half + b.half + min_gap;
                    let max_gap = spacing.max_free_gap_px * (a.scale + b.scale) * 0.5;
                    let was_steering = self
                        .stored
                        .iter()
                        .any(|s| s.active && s.id == a.id && s.steering);
                    let free_gap = d - a.half - b.half;
                    let dx = b.x - a.x;
                    let a_approach = a.active
                        && a.facing * dx > 0.0
                        && (!b.active || (a.facing - b.facing) * dx > 0.0);
                    let b_approach = b.active
                        && b.facing * (-dx) > 0.0
                        && (!a.active || (b.facing - a.facing) * (-dx) > 0.0);
                    let both = a_approach && b_approach;
                    let hop_window = free_gap
                        >= spacing.min_free_gap_px * (a.scale + b.scale) * 0.5
                        && free_gap
                            <= spacing.max_free_gap_px.min(70.0) * (a.scale + b.scale) * 0.5;
                    let hop_ready = !self
                        .stored
                        .iter()
                        .any(|s| s.active && s.id == a.id && s.hop_cooldown > 0.0);
                    let friend_can_be_crossed = b.active || b.movable;
                    let selected = a_approach && (!both || a.id < b.id);
                    if self.settings.enabled
                        && hop_window
                        && selected
                        && hop_ready
                        && friend_can_be_crossed
                    {
                        let dir = if dx >= 0.0 { 1.0 } else { -1.0 };
                        let target = b.x
                            + dir
                                * (a.half
                                    + b.half
                                    + spacing.min_free_gap_px * (a.scale + b.scale) * 0.5);
                        let success = slots[i]
                            .as_deref_mut()
                            .is_some_and(|p| p.behavior_jump_over(target, world));
                        if success {
                            jumped = true;
                            self.steering_count = self.steering_count.saturating_add(1);
                            let idx = self
                                .stored
                                .iter()
                                .position(|s| s.active && s.id == a.id)
                                .or_else(|| self.stored.iter().position(|s| !s.active))
                                .unwrap_or(0);
                            self.stored[idx].id = a.id;
                            self.stored[idx].active = true;
                            self.stored[idx].hop_cooldown = 3.0 + (a.id % 6) as f64;
                            self.record(
                                a.id,
                                BehaviorReason::JumpOverFriend,
                                BehaviorIntent::JumpOver,
                                3.0 + (a.id % 6) as f64,
                            );
                        } else {
                            steer = true;
                            vx = -a.facing * 25.0;
                            if decision {
                                self.record(
                                    a.id,
                                    BehaviorReason::CrossingPath,
                                    if vx < 0.0 {
                                        BehaviorIntent::YieldLeft
                                    } else {
                                        BehaviorIntent::YieldRight
                                    },
                                    2.0,
                                );
                            }
                        }
                    } else if hop_window && both && a.id > b.id {
                        steer = true;
                        vx = -a.facing * 25.0;
                        if decision {
                            self.record(
                                a.id,
                                BehaviorReason::CrossingPath,
                                if vx < 0.0 {
                                    BehaviorIntent::YieldLeft
                                } else {
                                    BehaviorIntent::YieldRight
                                },
                                2.0,
                            );
                        }
                    } else if d < center_gap || (was_steering && d < a.half + b.half + max_gap) {
                        steer = true;
                        let side = if a.x < b.x {
                            -1.0
                        } else if a.x > b.x {
                            1.0
                        } else if a.id < b.id {
                            -1.0
                        } else {
                            1.0
                        };
                        let preferred = a.half + b.half + (min_gap + max_gap) * 0.5;
                        vx = side * ((preferred - d) * 1.2).clamp(0.0, 55.0);
                        if decision {
                            self.record(
                                a.id,
                                BehaviorReason::Crowded,
                                if side < 0.0 {
                                    BehaviorIntent::YieldLeft
                                } else {
                                    BehaviorIntent::YieldRight
                                },
                                self.settings.cooldown_s,
                            );
                        }
                    } else if a.active && d < a.half + b.half + 20.0 {
                        let side = if a.id < b.id { -1.0 } else { 1.0 };
                        vx = side * 30.0;
                        if decision {
                            self.record(
                                a.id,
                                BehaviorReason::CrossingPath,
                                if side < 0.0 {
                                    BehaviorIntent::YieldLeft
                                } else {
                                    BehaviorIntent::YieldRight
                                },
                                2.0,
                            );
                        }
                    }
                }
            }
            if let Some(p) = slots[i].as_deref_mut() {
                if a.movable && !jumped && steer {
                    p.behavior_steer(Some(vx), dt);
                } else if a.movable
                    && !jumped
                    && self
                        .stored
                        .iter()
                        .any(|s| s.active && s.id == a.id && s.steering)
                {
                    p.behavior_steer(Some(0.0), dt);
                } else if a.movable && !jumped {
                    p.behavior_steer(None, dt);
                }
            }
            if steer
                && !self
                    .stored
                    .iter()
                    .any(|s| s.active && s.id == a.id && s.steering)
            {
                self.steering_count = self.steering_count.saturating_add(1);
            }
            if let Some(s) = self.stored.iter_mut().find(|s| s.active && s.id == a.id) {
                s.steering = steer;
            }
            if decision {
                self.context_event(i, a, &mut slots, count, world);
            }
        }
    }

    fn record(&mut self, id: u64, reason: BehaviorReason, intent: BehaviorIntent, cooldown: f64) {
        let idx = self
            .stored
            .iter()
            .position(|s| s.active && s.id == id)
            .or_else(|| self.stored.iter().position(|s| !s.active))
            .unwrap_or(0);
        let steering = self.stored[idx].steering;
        let hop_cooldown = self.stored[idx].hop_cooldown;
        let context_cooldown = self.stored[idx].context_cooldown;
        let remaining = if self.stored[idx].active
            && self.stored[idx].reason == reason
            && self.stored[idx].cooldown > 0.0
        {
            self.stored[idx].cooldown
        } else {
            cooldown
        };
        self.stored[idx] = Stored {
            id,
            cooldown: remaining,
            intent,
            reason,
            active: true,
            steering,
            hop_cooldown,
            context_cooldown,
        };
        let out = self.events.iter().position(Option::is_none).unwrap_or(0);
        self.events[out] = Some(BehaviorEvent {
            pet_id: id,
            reason,
            intent,
        });
    }
    fn context_event(
        &mut self,
        i: usize,
        a: Entry,
        pets: &mut [Option<&mut Pet>; CAP],
        count: usize,
        world: &World,
    ) {
        let Some(p) = pets[i].as_deref() else { return };
        if !a.movable || !p.auto_reactions || p.state != State::Idle {
            return;
        }
        let state = self
            .stored
            .iter()
            .find(|s| s.active && s.id == a.id)
            .copied()
            .unwrap_or_default();
        if state.context_cooldown > 0.0 {
            return;
        }
        let explorer_window = world.windows.iter().find(|w| {
            w.is_file_manager
                && w.active
                && w.rect.is_valid()
                && (w.rect.x + w.rect.w * 0.5 - p.pos.x).abs() < 260.0
        });
        let explorer = explorer_window.is_some();
        let friend = pets.iter().take(count).flatten().any(|q| {
            q.id != a.id
                && (q.pos.x - p.pos.x).abs() < 300.0
                && q.behavior_support_key(world) == Some(a.key)
        });
        let visible = world
            .windows
            .iter()
            .filter(|w| w.rect.is_valid() && !w.is_desktop_icon)
            .count();
        let busy = visible >= 6 && world.area_at(p.pos).w < 500.0 && world.idle_seconds < 20.0;
        let (reason, intent) = if p.satisfaction < 32.0 {
            (BehaviorReason::LowMood, BehaviorIntent::Rest)
        } else if explorer {
            (BehaviorReason::ExplorerNearby, BehaviorIntent::Observe)
        } else if friend {
            (BehaviorReason::FriendNearby, BehaviorIntent::CalmActivity)
        } else if busy && p.satisfaction < 50.0 {
            (BehaviorReason::BusyDesktop, BehaviorIntent::Rest)
        } else {
            return;
        };
        let target =
            explorer_window.map(|w| crate::Vec2::new(w.rect.x + w.rect.w * 0.5, w.rect.y + 30.0));
        if let Some(pet) = pets[i].as_deref_mut() {
            pet.behavior_context(intent, target);
        }
        self.record(
            a.id,
            reason,
            intent,
            self.settings.cooldown_s.clamp(10.0, 24.0) + (a.id % 5) as f64 * 0.7,
        );
        if let Some(stored) = self.stored.iter_mut().find(|s| s.active && s.id == a.id) {
            stored.context_cooldown =
                self.settings.cooldown_s.clamp(10.0, 24.0) + (a.id % 5) as f64 * 0.7;
        }
    }
}
