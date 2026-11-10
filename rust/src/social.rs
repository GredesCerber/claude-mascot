//! Deterministic, bounded coordinator for friendly interactions between pets.
use crate::{
    engine::{Pet, Skin, State},
    Rect, Vec2, World,
};

const CAP: usize = 8;
const TICK: f64 = 0.1;
const INITIAL_DELAY: f64 = 2.0;
const MAX_DISTANCE: f64 = 600.0;
const GRAVITY: f64 = crate::engine::GRAVITY;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SocialKind {
    Greeting,
    HighFive,
    DanceTogether,
    ShareLaptop,
    Comfort,
    Tag,
    Tower,
    Leapfrog,
    /// Friendly shove: square up, a short charge, then the partner tumbles.
    RoughPlay,
}

/// Distance at which the rough-play charge stops just short of body contact.
const ROUGH_PLAY_REACH: f64 = 80.0;
/// Square-up pause before the rough-play charge.
const ROUGH_PLAY_SQUARE_UP: f64 = 0.8;

/// Per-frame movement owned by the social coordinator. The normal pet physics
/// still advances these motions; this is only an intent/landing contract.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SocialMotion {
    Run { target_x: f64, speed: f64 },
    Leap { target: Vec2, owner_id: u64 },
    Perch { target: Vec2, owner_id: u64 },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SocialRole {
    Initiator,
    Partner,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SocialActivity {
    pub kind: SocialKind,
    pub role: SocialRole,
    pub partner_id: u64,
    pub elapsed: f64,
    /// Position relative to the partner for renderers that need a shared pose.
    pub partner_offset: Vec2,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SocialStats {
    pub started: u64,
    pub completed: u64,
    pub cancelled: u64,
}

#[derive(Clone, Copy, Debug)]
struct Pair {
    a: u64,
    b: u64,
    kind: SocialKind,
    elapsed: f64,
    duration: f64,
    phase: f64,
    chaser: u64,
    support: Rect,
    upper: u64,
    tower_anchor_dx: f64,
    leapfrog_jumps: u8,
}

#[derive(Clone, Copy, Debug)]
struct TowerGroup {
    ids: [u64; CAP],
    len: usize,
    support: Rect,
    stack_x: f64,
    next_upper: usize,
    phase: TowerPhase,
    elapsed: f64,
    formation_time: f64,
    dismounting: usize,
    direction: f64,
    clear_wait: f64,
    active_id: u64,
    active_target: Vec2,
    active_vx: f64,
    horizontal_started: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TowerPhase {
    Forming,
    Holding,
    Dismounting,
}
impl Default for Pair {
    fn default() -> Self {
        Self {
            a: 0,
            b: 0,
            kind: SocialKind::Greeting,
            elapsed: 0.0,
            duration: 0.0,
            phase: 0.0,
            chaser: 0,
            support: Rect::default(),
            upper: 0,
            tower_anchor_dx: 0.0,
            leapfrog_jumps: 4,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Cooldown {
    id: u64,
    remaining: f64,
}
impl Default for Cooldown {
    fn default() -> Self {
        Self {
            id: 0,
            remaining: 0.0,
        }
    }
}

/// Fixed-size social scheduler. Pet identity is always resolved by stable id.
pub struct SocialCoordinator {
    pub enabled: bool,
    accumulator: f64,
    age: f64,
    pairs: [Option<Pair>; 4],
    tower: Option<TowerGroup>,
    tower_invitation_at: f64,
    cooldowns: [Cooldown; CAP],
    request: Option<(Option<u64>, Option<SocialKind>)>,
    request_age: f64,
    last_kind: Option<SocialKind>,
    pub stats: SocialStats,
}
impl Default for SocialCoordinator {
    fn default() -> Self {
        Self::new()
    }
}
impl SocialCoordinator {
    pub fn new() -> Self {
        Self {
            enabled: true,
            accumulator: 0.0,
            age: 0.0,
            pairs: [None; 4],
            tower: None,
            tower_invitation_at: 20.0,
            cooldowns: [Cooldown::default(); CAP],
            request: None,
            request_age: 0.0,
            last_kind: None,
            stats: SocialStats::default(),
        }
    }
    /// Request an interaction on the next scheduler tick. `None` chooses by deterministic id order.
    pub fn request(&mut self, kind: Option<SocialKind>) {
        self.request = Some((None, kind));
        self.request_age = 0.0;
    }
    /// Request a specific interaction with a selected pet and its nearest eligible friend.
    pub fn request_for(&mut self, pet_id: u64, kind: Option<SocialKind>) {
        self.request = Some((Some(pet_id), kind));
        self.request_age = 0.0;
    }

    /// Convenience request used by the desktop menu.
    pub fn request_tag(&mut self, pet_id: Option<u64>) {
        self.request = Some((pet_id, Some(SocialKind::Tag)));
        self.request_age = 0.0;
    }

    /// Convenience request used by the desktop menu.
    pub fn request_tower(&mut self, pet_id: Option<u64>) {
        self.request = Some((pet_id, Some(SocialKind::Tower)));
        self.request_age = 0.0;
    }

    pub fn update<'a>(
        &mut self,
        dt: f64,
        pets: impl IntoIterator<Item = &'a mut Pet>,
        world: &World,
    ) {
        let mut refs: [Option<&'a mut Pet>; CAP] = std::array::from_fn(|_| None);
        let mut count = 0;
        for pet in pets.into_iter().take(CAP) {
            refs[count] = Some(pet);
            count += 1;
        }
        let step = if dt.is_finite() {
            dt.clamp(0.0, 0.05)
        } else {
            0.0
        };
        self.age += step;
        self.accumulator += step;
        if self.request.is_some() {
            self.request_age += step;
        }
        for c in &mut self.cooldowns {
            c.remaining = (c.remaining - step).max(0.0);
        }
        if !self.enabled {
            self.cancel_all(&mut refs, count, world);
            self.request = None;
            return;
        }
        if self.request_age > 3.0
            || self.request.is_some_and(|(id, _)| {
                id.is_some_and(|target| {
                    find(&refs, count, target).is_none_or(|i| {
                        refs[i].as_deref().is_some_and(|p| {
                            matches!(
                                p.state,
                                State::Sit
                                    | State::Sleep
                                    | State::Climb
                                    | State::Fall
                                    | State::Jump
                                    | State::Thrown
                                    | State::Hold
                                    | State::Peek
                                    | State::Dance
                                    | State::Wave
                                    | State::Activity
                                    | State::Dizzy
                            )
                        })
                    })
                })
            })
        {
            self.request = None;
        }
        self.advance_pairs(&mut refs, count, world, step);
        self.advance_tower(&mut refs, count, world, step);
        if step == 0.0 {
            if self.request.is_some() {
                self.tick(&mut refs, count, world)
            }
            return;
        }
        while self.accumulator >= TICK {
            self.accumulator -= TICK;
            self.tick(&mut refs, count, world);
        }
    }

    fn advance_pairs(
        &mut self,
        refs: &mut [Option<&mut Pet>; CAP],
        n: usize,
        world: &World,
        dt: f64,
    ) {
        for ix in 0..self.pairs.len() {
            let Some(mut pair) = self.pairs[ix] else {
                continue;
            };
            if pair.kind == SocialKind::Leapfrog {
                let ai = find(refs, n, pair.a);
                let bi = find(refs, n, pair.b);
                let valid = match (ai, bi) {
                    (Some(a), Some(b)) => {
                        let pa = refs[a].as_deref().unwrap();
                        let pb = refs[b].as_deref().unwrap();
                        pa.social.is_some_and(|s| {
                            s.kind == SocialKind::Leapfrog && s.partner_id == pair.b
                        }) && pb.social.is_some_and(|s| {
                            s.kind == SocialKind::Leapfrog && s.partner_id == pair.a
                        }) && !pa.dragging
                            && !pb.dragging
                            && !pa.chase
                            && !pb.chase
                            && support_exists(
                                pair.support,
                                world,
                                Vec2::new(pa.pos.x, pair.support.y),
                            )
                    }
                    _ => false,
                };
                if !valid {
                    self.pairs[ix] = None;
                    self.finish_pair(pair, refs, n, world, false);
                    continue;
                }
                pair.elapsed += dt;
                let jumper_id = pair.upper;
                let croucher_id = if jumper_id == pair.a { pair.b } else { pair.a };
                let landed=find(refs,n,jumper_id).is_some_and(|i|matches!(refs[i].as_deref().unwrap().social_motion,Some(SocialMotion::Perch{owner_id,..}) if owner_id==croucher_id));
                if landed {
                    if let Some(i) = find(refs, n, jumper_id) {
                        let p = refs[i].as_deref_mut().unwrap();
                        p.social_motion = None;
                        p.state = State::Social;
                        p.state_time = 0.0;
                    }
                    pair.chaser = pair.chaser.saturating_add(1);
                    if pair.chaser >= pair.leapfrog_jumps as u64 {
                        self.pairs[ix] = None;
                        self.finish_pair(pair, refs, n, world, true);
                        continue;
                    }
                    pair.phase = 0.4;
                } else if pair.phase > 0.0 {
                    pair.phase = (pair.phase - dt).max(0.0);
                    if pair.phase == 0.0 {
                        let target_for = |id: u64| {
                            let other = if id == pair.a { pair.b } else { pair.a };
                            let ji = find(refs, n, id)?;
                            let ci = find(refs, n, other)?;
                            leapfrog_target(
                                refs[ji].as_deref().unwrap(),
                                refs[ci].as_deref().unwrap(),
                                pair.support,
                            )
                            .and_then(|target| {
                                crate::leapfrog::plan(
                                    refs[ji].as_deref().unwrap(),
                                    refs[ci].as_deref().unwrap(),
                                    target,
                                    world,
                                )
                                .map(|arc| (id, other, target, arc))
                            })
                        };
                        // At a ledge, hop back over the crouching friend instead
                        // of abandoning the game after just one or two jumps.
                        let Some((next, other_id, target, arc)) =
                            target_for(croucher_id).or_else(|| target_for(jumper_id))
                        else {
                            self.pairs[ix] = None;
                            self.finish_pair(pair, refs, n, world, false);
                            continue;
                        };
                        pair.upper = next;
                        for (id, role, other) in [
                            (
                                pair.a,
                                if pair.a == next {
                                    SocialRole::Initiator
                                } else {
                                    SocialRole::Partner
                                },
                                pair.b,
                            ),
                            (
                                pair.b,
                                if pair.b == next {
                                    SocialRole::Initiator
                                } else {
                                    SocialRole::Partner
                                },
                                pair.a,
                            ),
                        ] {
                            if let Some(i) = find(refs, n, id) {
                                if let Some(s) = refs[i].as_deref_mut().unwrap().social.as_mut() {
                                    s.role = role;
                                    s.partner_id = other;
                                    s.elapsed = pair.elapsed;
                                }
                                refs[i]
                                    .as_deref_mut()
                                    .unwrap()
                                    .say_social(SocialKind::Leapfrog, role);
                            }
                        }
                        if let Some(i) = find(refs, n, next) {
                            let jumper = refs[i].as_deref_mut().unwrap();
                            jumper.prepare_leapfrog(target, other_id, arc);
                        }
                    }
                }
                if pair.elapsed >= pair.duration {
                    self.pairs[ix] = None;
                    self.finish_pair(pair, refs, n, world, true);
                } else {
                    self.pairs[ix] = Some(pair);
                }
                continue;
            }
            if pair.kind == SocialKind::RoughPlay {
                self.advance_rough_play(ix, pair, refs, n, world, dt);
                continue;
            }
            let ai = find(refs, n, pair.a);
            let bi = find(refs, n, pair.b);
            if matches!(pair.kind, SocialKind::Tag | SocialKind::Tower) {
                let valid = match (ai, bi) {
                    (Some(a), Some(b)) => {
                        let pa = refs[a].as_deref().unwrap();
                        let pb = refs[b].as_deref().unwrap();
                        let common = active_marker(pa, pair, pair.a, pair.b)
                            && active_marker(pb, pair, pair.b, pair.a)
                            && !pa.dragging
                            && !pb.dragging
                            && !pa.chase
                            && !pb.chase;
                        common
                            && if pair.kind == SocialKind::Tower {
                                pa.socially_supported(world)
                                    && support_rect_matches(pa, pair.support, world)
                                    && matches!(pb.state, State::Jump | State::Social | State::Fall)
                            } else {
                                pa.socially_supported(world)
                                    && pb.socially_supported(world)
                                    && pa.surface_id == pb.surface_id
                                    && same_support(pa, pb, world)
                                    && support_rect_matches(pa, pair.support, world)
                                    && support_rect_matches(pb, pair.support, world)
                            }
                    }
                    _ => false,
                };
                let landed = if pair.kind == SocialKind::Tower {
                    find(refs, n, pair.upper).is_some_and(|i| {
                        refs[i].as_deref().is_some_and(|p| {
                            matches!(p.social_motion, Some(crate::social::SocialMotion::Perch { owner_id, .. }) if owner_id == if pair.upper == pair.a { pair.b } else { pair.a })
                        })
                    })
                } else {
                    false
                };
                if !valid {
                    self.pairs[ix] = None;
                    self.finish_pair(pair, refs, n, world, false);
                    continue;
                }
                pair.elapsed += dt;
                pair.phase = if pair.kind == SocialKind::Tower {
                    if landed {
                        pair.phase + dt
                    } else {
                        0.0
                    }
                } else {
                    (pair.phase - dt).max(0.0)
                };
                if pair.kind == SocialKind::Tag {
                    let pa = refs[ai.unwrap()].as_deref().unwrap();
                    let pb = refs[bi.unwrap()].as_deref().unwrap();
                    // Catch before physical body contacts keep the actors apart.
                    let catch_distance =
                        84.0 * pa.display_scale.max(pb.display_scale).clamp(0.5, 2.0);
                    if (pa.pos.x - pb.pos.x).abs() <= catch_distance && pair.phase <= 0.0 {
                        pair.chaser = if pair.chaser == pair.a {
                            pair.b
                        } else {
                            pair.a
                        };
                        pair.phase = 0.7;
                        for id in [pair.a, pair.b] {
                            if let Some(i) = find(refs, n, id) {
                                refs[i].as_deref_mut().unwrap().speak(
                                    if id == pair.chaser {
                                        crate::dialogue::SpeechEvent::TagChaser
                                    } else {
                                        crate::dialogue::SpeechEvent::TagRunner
                                    },
                                    2.0,
                                );
                            }
                        }
                    }
                    self.set_tag_motion(pair, refs, n);
                } else if landed {
                    self.set_perch_anchor(pair, refs, n);
                }
                let finished = if pair.kind == SocialKind::Tag {
                    pair.elapsed >= pair.duration
                } else {
                    pair.phase >= 2.5
                };
                for id in [pair.a, pair.b] {
                    if let Some(i) = find(refs, n, id) {
                        if let Some(s) = refs[i].as_deref_mut().and_then(|p| p.social.as_mut()) {
                            s.elapsed = pair.elapsed;
                        }
                    }
                }
                if finished {
                    self.pairs[ix] = None;
                    self.finish_pair(pair, refs, n, world, true);
                } else {
                    self.pairs[ix] = Some(pair);
                }
                continue;
            }
            let valid = match (ai, bi) {
                (Some(a), Some(b)) => {
                    let pa = refs[a].as_deref().unwrap();
                    let pb = refs[b].as_deref().unwrap();
                    active_marker(pa, pair, pair.a, pair.b)
                        && active_marker(pb, pair, pair.b, pair.a)
                        && !pa.dragging
                        && !pb.dragging
                        && !pa.chase
                        && !pb.chase
                        && active_state(pa, pair, dt)
                        && active_state(pb, pair, dt)
                        && pa.socially_supported(world)
                        && pb.socially_supported(world)
                        && same_support(pa, pb, world)
                        && (pa.pos.y - pb.pos.y).abs() <= 20.0
                        && (pa.pos.x - pb.pos.x).hypot(pa.pos.y - pb.pos.y)
                            <= social_distance_range(
                                pair.kind,
                                pa.display_scale.max(pb.display_scale).clamp(0.5, 2.0),
                            )
                            .1 * 1.1
                }
                _ => false,
            };
            if !valid {
                self.pairs[ix] = None;
                self.finish_pair(pair, refs, n, world, false);
                continue;
            }
            pair.elapsed += dt;
            for (idx, id) in [(ai.unwrap(), pair.a), (bi.unwrap(), pair.b)] {
                if let Some(pet) = refs[idx].as_deref_mut() {
                    if let Some(s) = pet.social.as_mut() {
                        if s.partner_id == if id == pair.a { pair.b } else { pair.a } {
                            s.elapsed = pair.elapsed;
                        }
                    }
                }
            }
            if pair.elapsed >= pair.duration {
                self.pairs[ix] = None;
                self.finish_pair(pair, refs, n, world, true)
            } else {
                self.pairs[ix] = Some(pair)
            }
        }
    }

    /// Rough play: both square up, the initiator charges and stops just short
    /// of body contact, then the partner receives a friendly tumble. Any loss
    /// of ownership (drag, command, chase, lost support) cancels without a shove.
    fn advance_rough_play(
        &mut self,
        ix: usize,
        mut pair: Pair,
        refs: &mut [Option<&mut Pet>; CAP],
        n: usize,
        world: &World,
        dt: f64,
    ) {
        let (Some(ai), Some(bi)) = (find(refs, n, pair.a), find(refs, n, pair.b)) else {
            self.pairs[ix] = None;
            self.finish_pair(pair, refs, n, world, false);
            return;
        };
        let valid = {
            let pa = refs[ai].as_deref().unwrap();
            let pb = refs[bi].as_deref().unwrap();
            active_marker(pa, pair, pair.a, pair.b)
                && active_marker(pb, pair, pair.b, pair.a)
                && !pa.dragging
                && !pb.dragging
                && !pa.chase
                && !pb.chase
                && matches!(pa.state, State::Social | State::Run)
                && pb.state == State::Social
                && pa.socially_supported(world)
                && pb.socially_supported(world)
                && same_support(pa, pb, world)
        };
        if !valid {
            self.pairs[ix] = None;
            self.finish_pair(pair, refs, n, world, false);
            return;
        }
        pair.elapsed += dt;
        for i in [ai, bi] {
            if let Some(s) = refs[i].as_deref_mut().and_then(|p| p.social.as_mut()) {
                s.elapsed = pair.elapsed;
            }
        }
        let (ax, bx, scale) = {
            let pa = refs[ai].as_deref().unwrap();
            let pb = refs[bi].as_deref().unwrap();
            (
                pa.pos.x,
                pb.pos.x,
                pa.display_scale.max(pb.display_scale).clamp(0.5, 2.0),
            )
        };
        let direction = if bx >= ax { 1.0 } else { -1.0 };
        if pair.chaser == 0 {
            pair.phase = (pair.phase - dt).max(0.0);
            if pair.phase <= 0.0 {
                pair.chaser = 1;
                let p = refs[ai].as_deref_mut().unwrap();
                p.facing = direction;
                p.social_motion = Some(SocialMotion::Run {
                    target_x: bx - direction * ROUGH_PLAY_REACH * scale,
                    speed: 230.0,
                });
            }
        } else if (bx - ax).abs() <= (ROUGH_PLAY_REACH + 8.0) * scale {
            self.pairs[ix] = None;
            self.finish_pair(pair, refs, n, world, true);
            if let Some(p) = refs[ai].as_deref_mut() {
                p.vel = Vec2::default();
                p.squash = p.squash.max(0.4);
            }
            if let Some(p) = refs[bi].as_deref_mut() {
                p.play_knock(direction);
            }
            return;
        }
        if pair.elapsed >= pair.duration {
            self.pairs[ix] = None;
            self.finish_pair(pair, refs, n, world, true);
        } else {
            self.pairs[ix] = Some(pair);
        }
    }

    fn start_tower(
        &mut self,
        target: Option<u64>,
        refs: &mut [Option<&mut Pet>; CAP],
        n: usize,
        world: &World,
        manual: bool,
    ) -> bool {
        if self.tower.is_some() {
            return false;
        }
        let base_i = target
            .and_then(|id| {
                find(refs, n, id).filter(|&i| tower_eligible(refs, i, world, self, manual))
            })
            .or_else(|| {
                (0..n)
                    .filter(|&i| tower_eligible(refs, i, world, self, manual))
                    .min_by_key(|&i| refs[i].as_deref().unwrap().id)
            });
        let Some(mut base_i) = base_i else {
            return false;
        };
        let base = refs[base_i].as_deref().unwrap();
        let base_id = base.id;
        let support = support_rect(base, world);
        let mut candidates: Vec<(u64, usize)> = (0..n)
            .filter_map(|i| {
                let p = refs[i].as_deref()?;
                (tower_eligible(refs, i, world, self, manual) && same_support(base, p, world))
                    .then_some((p.id, i))
            })
            .collect();
        candidates.sort_by_key(|x| x.0);
        let mut chosen = [0u64; CAP];
        let mut len = 0;
        chosen[len] = base_id;
        len += 1;
        for (id, _) in candidates {
            if id != base_id && len < CAP {
                chosen[len] = id;
                len += 1;
            }
        }
        if len < 2 {
            return false;
        }
        if len > 2 {
            let mid = support.x + support.w * 0.5;
            let slot = (0..len)
                .min_by(|&a, &b| {
                    let ax = find(refs, n, chosen[a])
                        .map(|i| refs[i].as_deref().unwrap().pos.x)
                        .unwrap_or(mid);
                    let bx = find(refs, n, chosen[b])
                        .map(|i| refs[i].as_deref().unwrap().pos.x)
                        .unwrap_or(mid);
                    (ax - mid)
                        .abs()
                        .total_cmp(&(bx - mid).abs())
                        .then_with(|| chosen[a].cmp(&chosen[b]))
                })
                .unwrap_or(0);
            chosen.swap(0, slot);
            base_i = find(refs, n, chosen[0]).unwrap_or(base_i);
            let center = refs[base_i].as_deref().unwrap().pos.x;
            chosen[1..len].sort_by(|a, b| {
                let ax = find(refs, n, *a)
                    .map(|i| refs[i].as_deref().unwrap().pos.x)
                    .unwrap_or(center);
                let bx = find(refs, n, *b)
                    .map(|i| refs[i].as_deref().unwrap().pos.x)
                    .unwrap_or(center);
                (ax - center)
                    .abs()
                    .total_cmp(&(bx - center).abs())
                    .then_with(|| a.cmp(b))
            });
        }
        if !manual && len > 2 {
            // Vary autonomous tower size instead of requiring every free pet.
            let seed = (self.age * 10.0) as u64 ^ self.stats.started.wrapping_mul(37);
            len = 2 + (seed % (len - 1) as u64) as usize;
        }
        let base_pos = refs[base_i].as_deref().unwrap().pos;
        // Tower height is checked for the complete requested group before any reservation.
        let scales: Vec<f64> = chosen[..len]
            .iter()
            .filter_map(|id| {
                find(refs, n, *id)
                    .map(|i| refs[i].as_deref().unwrap().display_scale.clamp(0.5, 2.0))
            })
            .collect();
        let height: f64 = (1..len)
            .map(|j| 55.0 * scales[j - 1].max(scales[j]))
            .sum::<f64>()
            + 55.0 * scales[len - 1]
            + 8.0;
        let area = world.area_at(Vec2::new(base_pos.x, support.y));
        let max_scale = scales.iter().copied().fold(1.0_f64, f64::max);
        let dismount_room =
            (base_pos.x - support.x - 35.0).max(support.right() - 35.0 - base_pos.x);
        if support.w < 180.0
            || support.y - area.y < height + 100.0
            || dismount_room < 80.0 * max_scale + 35.0
        {
            if support.y - area.y < height + 100.0 {
                if let Some(i) = find(refs, n, target.unwrap_or(base_id)) {
                    let p = refs[i].as_deref_mut().unwrap();
                    p.speak(crate::dialogue::SpeechEvent::TowerLowCeiling, 3.0);
                }
            }
            return false;
        }
        let stack_x = base_pos.x;
        let direction = if stack_x - support.x >= support.right() - stack_x {
            -1.0
        } else {
            1.0
        };
        let mut group = TowerGroup {
            ids: chosen,
            len,
            support,
            stack_x,
            next_upper: 1,
            phase: TowerPhase::Forming,
            elapsed: 0.0,
            formation_time: 0.0,
            dismounting: len,
            direction,
            clear_wait: 0.0,
            active_id: 0,
            active_target: Vec2::default(),
            active_vx: 0.0,
            horizontal_started: false,
        };
        if !tower_path_fits(refs, n, &group) {
            if let Some(i) = find(refs, n, target.unwrap_or(chosen[0])) {
                let p = refs[i].as_deref_mut().unwrap();
                p.speak(crate::dialogue::SpeechEvent::TowerNoRoom, 2.5);
            }
            return false;
        }
        for pos in 0..len {
            let id = chosen[pos];
            let Some(i) = find(refs, n, id) else { continue };
            let p = refs[i].as_deref_mut().unwrap();
            p.social = Some(SocialActivity {
                kind: SocialKind::Tower,
                role: if pos == 0 {
                    SocialRole::Initiator
                } else {
                    SocialRole::Partner
                },
                partner_id: chosen[if pos == 0 { 1 } else { pos - 1 }],
                elapsed: 0.0,
                partner_offset: Vec2::default(),
            });
            p.state = State::Social;
            p.state_time = 0.0;
            p.state_duration = 8.0;
            p.vel = Vec2::default();
            if pos == 0 {
                p.bubble = None;
                p.bubble_time = 0.0;
            } else {
                p.speak(crate::dialogue::SpeechEvent::TowerTop, 2.0);
            }
        }
        if let Some(i) = find(refs, n, chosen[1]) {
            let target_y = tower_target_y(refs, n, &group, 1);
            let target = Vec2::new(stack_x, target_y);
            group.active_vx =
                set_tower_leap(refs[i].as_deref_mut().unwrap(), target, chosen[0]).unwrap_or(0.0);
            group.active_id = chosen[1];
            group.active_target = target;
            group.next_upper = 2;
        }
        self.tower = Some(group);
        let intensity = chosen[..len]
            .iter()
            .filter_map(|id| {
                find(refs, n, *id).map(|i| refs[i].as_deref().unwrap().activity_intensity as f64)
            })
            .sum::<f64>()
            / len as f64;
        for id in chosen[..len].iter().copied() {
            cooldown(
                &mut self.cooldowns,
                id,
                (12.0 + (id % 19) as f64) * (1.5 - intensity.min(100.0) / 100.0),
            );
        }
        self.last_kind = Some(SocialKind::Tower);
        self.stats.started += 1;
        true
    }

    fn advance_tower(
        &mut self,
        refs: &mut [Option<&mut Pet>; CAP],
        n: usize,
        world: &World,
        dt: f64,
    ) {
        let Some(mut g) = self.tower else { return };
        g.formation_time += dt;
        if g.active_id != 0 && !g.horizontal_started {
            if let Some(i) = find(refs, n, g.active_id) {
                let p = refs[i].as_deref_mut().unwrap();
                if p.pos.y <= g.active_target.y {
                    p.vel.x = g.active_vx;
                    g.horizontal_started = true;
                }
            }
        }
        let valid = (0..g.len).all(|k| {
            find(refs, n, g.ids[k]).is_some_and(|i| {
                let p = refs[i].as_deref().unwrap();
                !p.dragging
                    && !p.chase
                    && active_marker(
                        p,
                        Pair {
                            a: g.ids[0],
                            b: g.ids[1],
                            kind: SocialKind::Tower,
                            ..Pair::default()
                        },
                        p.id,
                        if k == 0 { g.ids[1] } else { g.ids[k - 1] },
                    )
            })
        }) && support_exists(g.support, world, Vec2::new(g.stack_x, g.support.y));
        if !valid || g.formation_time > 20.0 {
            self.tower = None;
            self.finish_tower(g, refs, n, world, false);
            return;
        }
        match g.phase {
            TowerPhase::Forming => {
                let landed=(1..g.next_upper).all(|k| find(refs,n,g.ids[k]).is_some_and(|i| matches!(refs[i].as_deref().unwrap().social_motion,Some(SocialMotion::Perch{owner_id,..}) if owner_id==g.ids[k-1])));
                if g.active_id != 0
                    && find(refs, n, g.active_id).is_some_and(|i| {
                        matches!(
                            refs[i].as_deref().unwrap().social_motion,
                            Some(SocialMotion::Perch { .. })
                        )
                    })
                {
                    g.active_id = 0;
                    g.horizontal_started = false;
                }
                if landed && g.next_upper < g.len {
                    let k = g.next_upper;
                    if let Some(i) = find(refs, n, g.ids[k]) {
                        let target = Vec2::new(
                            g.stack_x,
                            g.support.y
                                - (0..k)
                                    .map(|j| {
                                        let lower = find(refs, n, g.ids[j])
                                            .map(|ii| {
                                                refs[ii]
                                                    .as_deref()
                                                    .unwrap()
                                                    .display_scale
                                                    .clamp(0.5, 2.0)
                                            })
                                            .unwrap_or(1.0);
                                        let upper = find(refs, n, g.ids[j + 1])
                                            .map(|ii| {
                                                refs[ii]
                                                    .as_deref()
                                                    .unwrap()
                                                    .display_scale
                                                    .clamp(0.5, 2.0)
                                            })
                                            .unwrap_or(1.0);
                                        55.0 * lower.max(upper)
                                    })
                                    .sum::<f64>(),
                        );
                        let p = refs[i].as_deref_mut().unwrap();
                        g.active_vx = set_tower_leap(p, target, g.ids[k - 1]).unwrap_or(0.0);
                        g.active_id = g.ids[k];
                        g.active_target = target;
                        g.horizontal_started = false;
                        g.next_upper += 1;
                    }
                }
                if g.next_upper == g.len
                    && (1..g.len).all(|k| {
                        find(refs, n, g.ids[k]).is_some_and(|i| {
                            matches!(
                                refs[i].as_deref().unwrap().social_motion,
                                Some(SocialMotion::Perch { .. })
                            )
                        })
                    })
                {
                    g.phase = TowerPhase::Holding;
                    g.elapsed = 0.0;
                }
            }
            TowerPhase::Holding => {
                g.elapsed += dt;
                if g.elapsed >= 2.5 {
                    g.phase = TowerPhase::Dismounting;
                    g.dismounting = g.len - 1;
                    self.launch_tower_top(g, refs, n);
                    g.dismounting -= 1;
                }
            }
            TowerPhase::Dismounting => {
                g.clear_wait += dt;
                if g.dismounting > 0 {
                    let just_launched = g.ids[g.dismounting + 1];
                    if find(refs, n, just_launched).is_some_and(|i| {
                        refs[i].as_deref().unwrap().pos.x - g.stack_x
                            >= 80.0 * refs[i].as_deref().unwrap().display_scale.clamp(0.5, 2.0)
                            || g.stack_x - refs[i].as_deref().unwrap().pos.x
                                >= 80.0 * refs[i].as_deref().unwrap().display_scale.clamp(0.5, 2.0)
                    }) {
                        g.clear_wait = 0.0;
                        self.launch_tower_top(g, refs, n);
                        g.dismounting -= 1;
                    } else if g.clear_wait > 2.5 {
                        self.tower = None;
                        self.finish_tower(g, refs, n, world, false);
                        return;
                    }
                } else if g.dismounting == 0 {
                    self.tower = None;
                    self.finish_tower(g, refs, n, world, true);
                    return;
                }
            }
        }
        if g.phase == TowerPhase::Forming {
            for k in 1..g.next_upper.min(g.len) {
                let owner_id = g.ids[k - 1];
                let Some(owner_i) = find(refs, n, owner_id) else {
                    continue;
                };
                let owner_pos = refs[owner_i].as_deref().unwrap().pos;
                let owner_scale = refs[owner_i]
                    .as_deref()
                    .unwrap()
                    .display_scale
                    .clamp(0.5, 2.0);
                let Some(top_i) = find(refs, n, g.ids[k]) else {
                    continue;
                };
                if !matches!(
                    refs[top_i].as_deref().unwrap().social_motion,
                    Some(SocialMotion::Perch { .. })
                ) {
                    continue;
                }
                let scale = refs[top_i]
                    .as_deref()
                    .unwrap()
                    .display_scale
                    .clamp(0.5, 2.0);
                refs[top_i].as_deref_mut().unwrap().social_motion = Some(SocialMotion::Perch {
                    target: Vec2::new(owner_pos.x, owner_pos.y - 55.0 * scale.max(owner_scale)),
                    owner_id,
                });
            }
        }
        self.tower = Some(g);
    }

    fn launch_tower_top(&self, g: TowerGroup, refs: &mut [Option<&mut Pet>; CAP], n: usize) {
        if g.dismounting == 0 {
            return;
        }
        let id = g.ids[g.dismounting];
        if let Some(i) = find(refs, n, id) {
            let p = refs[i].as_deref_mut().unwrap();
            let scale = p.display_scale.clamp(0.5, 2.0);
            p.social_motion = None;
            p.vel = Vec2::new(g.direction * 310.0, -330.0 * scale);
            p.state = State::Jump;
            p.state_time = 0.0;
        }
    }

    fn finish_tower(
        &mut self,
        g: TowerGroup,
        refs: &mut [Option<&mut Pet>; CAP],
        n: usize,
        world: &World,
        completed: bool,
    ) {
        for k in 0..g.len {
            if let Some(i) = find(refs, n, g.ids[k]) {
                let p = refs[i].as_deref_mut().unwrap();
                let expected_partner = if k == 0 { g.ids[1] } else { g.ids[k - 1] };
                let owned = p.social.is_some_and(|s| {
                    s.kind == SocialKind::Tower && s.partner_id == expected_partner
                });
                if owned {
                    p.social_motion = None;
                    p.social = None;
                    if p.state == State::Social {
                        p.set_social_end(world);
                    }
                }
            }
        }
        if completed {
            self.stats.completed += 1;
        } else {
            self.stats.cancelled += 1;
        }
    }

    fn set_tag_motion(&self, pair: Pair, refs: &mut [Option<&mut Pet>; CAP], n: usize) {
        let Some(ai) = find(refs, n, pair.a) else {
            return;
        };
        let Some(bi) = find(refs, n, pair.b) else {
            return;
        };
        let (a, b) = (
            refs[ai].as_deref().unwrap().pos.x,
            refs[bi].as_deref().unwrap().pos.x,
        );
        let bounds = (pair.support.x + 25.0, pair.support.right() - 25.0);
        for (id, other_x) in [(pair.a, b), (pair.b, a)] {
            let Some(i) = find(refs, n, id) else { continue };
            let pet = refs[i].as_deref_mut().unwrap();
            let chasing = id == pair.chaser;
            let pausing = chasing && pair.phase > 0.0;
            let target_x = if pausing {
                pet.pos.x
            } else if chasing {
                other_x
            } else if pet.pos.x < other_x {
                bounds.0
            } else {
                bounds.1
            };
            pet.social_motion = Some(crate::social::SocialMotion::Run {
                target_x,
                speed: if pausing {
                    0.0
                } else if chasing {
                    155.0
                } else {
                    190.0
                },
            });
            pet.facing = if target_x < pet.pos.x { -1.0 } else { 1.0 };
        }
    }

    fn set_perch_anchor(&self, pair: Pair, refs: &mut [Option<&mut Pet>; CAP], n: usize) {
        let owner_id = if pair.upper == pair.a { pair.b } else { pair.a };
        let Some(owner_i) = find(refs, n, owner_id) else {
            return;
        };
        let Some(upper_i) = find(refs, n, pair.upper) else {
            return;
        };
        let owner = refs[owner_i].as_deref().unwrap();
        let scale = refs[upper_i]
            .as_deref()
            .unwrap()
            .display_scale
            .clamp(0.5, 2.5);
        let target = Vec2::new(
            owner.pos.x + pair.tower_anchor_dx,
            owner.pos.y - 55.0 * scale,
        );
        refs[upper_i].as_deref_mut().unwrap().social_motion =
            Some(crate::social::SocialMotion::Perch { target, owner_id });
    }

    fn tick(&mut self, refs: &mut [Option<&mut Pet>; CAP], n: usize, world: &World) {
        if self.tower.is_some() {
            return;
        }
        if let Some((target, Some(SocialKind::Tower))) = self.request {
            if self.start_tower(target, refs, n, world, true) {
                self.request = None;
            }
            return;
        }
        if self.age < INITIAL_DELAY && self.request.is_none() {
            return;
        }
        // A group invitation must not depend on the generic pair gate finding
        // two walkers first. Resting friends can join without being paired off.
        if self.request.is_none() && self.age >= self.tower_invitation_at {
            self.tower_invitation_at =
                self.age + 35.0 + (self.stats.started.wrapping_mul(17) % 46) as f64;
            let available = (0..n)
                .filter(|&i| tower_eligible(refs, i, world, self, false))
                .count();
            if available >= 3 && self.start_tower(None, refs, n, world, false) {
                return;
            }
        }
        let mut ids = [(u64::MAX, 0usize); CAP];
        for i in 0..n {
            if let Some(p) = refs[i].as_deref() {
                ids[i] = (p.id, i);
            }
        }
        ids[..n].sort_by_key(|x| x.0);
        let req = self.request;
        if let Some((Some(target), requested_kind)) = req {
            if let Some(i) =
                find(refs, n, target).filter(|&i| free_eligible(refs, i, world, self, true))
            {
                let a = refs[i].as_deref().unwrap();
                let mut best: Option<(f64, u64, usize)> = None;
                for j in 0..n {
                    if i == j || !free_eligible(refs, j, world, self, true) {
                        continue;
                    }
                    let b = refs[j].as_deref().unwrap();
                    if b.id == a.id {
                        continue;
                    }
                    let d = (a.pos.x - b.pos.x).hypot(a.pos.y - b.pos.y);
                    if !distance_allowed(requested_kind, d)
                        || (a.pos.y - b.pos.y).abs() > 20.0
                        || !same_support(a, b, world)
                    {
                        continue;
                    }
                    if requested_kind == Some(SocialKind::HighFive) && d > 96.0 {
                        continue;
                    }
                    let candidate = (d, b.id, j);
                    if best.is_none_or(|old| {
                        candidate.0 < old.0 || (candidate.0 == old.0 && candidate.1 < old.1)
                    }) {
                        best = Some(candidate)
                    }
                }
                if let Some((_, _, j)) = best {
                    let mut kind = requested_kind.unwrap_or_else(|| {
                        choose(
                            a,
                            refs[j].as_deref().unwrap(),
                            self.age,
                            self.last_kind,
                            (a.pos.x - refs[j].as_deref().unwrap().pos.x)
                                .hypot(a.pos.y - refs[j].as_deref().unwrap().pos.y),
                        )
                    });
                    if kind == SocialKind::HighFive
                        && (a.pos.x - refs[j].as_deref().unwrap().pos.x).abs() > 96.0
                    {
                        kind = SocialKind::Greeting
                    }
                    if self.start(i, j, kind, refs, world) {
                        self.request = None;
                    }
                    return;
                }
            }
            return;
        }
        'outer: for x in 0..n {
            let i = ids[x].1;
            if ids[x].0 == u64::MAX || !free_eligible(refs, i, world, self, req.is_some()) {
                continue;
            }
            for y in 0..n {
                let j = ids[y].1;
                if i == j
                    || ids[y].0 == u64::MAX
                    || ids[x].0 == ids[y].0
                    || !free_eligible(refs, j, world, self, req.is_some())
                {
                    continue;
                }
                let a = refs[i].as_deref().unwrap();
                let b = refs[j].as_deref().unwrap();
                let requested = self.request.and_then(|(_, k)| k);
                if !same_support(a, b, world)
                    || (a.pos.y - b.pos.y).abs() > 20.0
                    || !distance_allowed(requested, (a.pos.x - b.pos.x).abs())
                {
                    continue;
                }
                let mut kind = self.request.take().and_then(|(_, k)| k).unwrap_or_else(|| {
                    choose(
                        a,
                        b,
                        self.age,
                        self.last_kind,
                        (a.pos.x - b.pos.x).hypot(a.pos.y - b.pos.y),
                    )
                });
                if kind == SocialKind::HighFive && (a.pos.x - b.pos.x).abs() > 96.0 {
                    if let Some(request) = req {
                        self.request = Some(request);
                        continue;
                    } else {
                        kind = SocialKind::Greeting;
                    }
                }
                // An autonomous shove without enough landing room becomes a
                // greeting instead of blocking this pair until the hash changes.
                if self.start(i, j, kind, refs, world)
                    || (kind == SocialKind::RoughPlay
                        && req.is_none()
                        && self.start(i, j, SocialKind::Greeting, refs, world))
                {
                    break 'outer;
                }
            }
        }
    }
    fn start(
        &mut self,
        i: usize,
        j: usize,
        kind: SocialKind,
        refs: &mut [Option<&mut Pet>; CAP],
        world: &World,
    ) -> bool {
        if kind == SocialKind::Tower {
            let id = refs[i].as_deref().unwrap().id;
            return self.start_tower(Some(id), refs, CAP, world, false);
        }
        if !can_start_kind(
            kind,
            refs[i].as_deref().unwrap(),
            refs[j].as_deref().unwrap(),
            world,
        ) {
            return false;
        }
        let slot = self.pairs.iter().position(Option::is_none);
        let Some(slot) = slot else { return false };
        let a_id = refs[i].as_deref().unwrap().id;
        let b_id = refs[j].as_deref().unwrap().id;
        let leapfrog_jumps = 2 + ((a_id ^ b_id ^ self.age as u64) % 9) as u8;
        let duration = if kind == SocialKind::Leapfrog {
            leapfrog_jumps as f64 * 1.8 + 3.0
        } else {
            duration(kind)
        };
        let ax = refs[i].as_deref().unwrap().pos.x;
        let bx = refs[j].as_deref().unwrap().pos.x;
        let initial_tag_separation = if kind == SocialKind::Tag && (ax - bx).abs() < 64.0 {
            0.7
        } else {
            0.0
        };
        for (idx, role, partner, other_x) in [
            (i, SocialRole::Initiator, b_id, bx),
            (j, SocialRole::Partner, a_id, ax),
        ] {
            let p = refs[idx].as_deref_mut().unwrap();
            let offset = match kind {
                SocialKind::ShareLaptop => Vec2::new(
                    if role == SocialRole::Initiator {
                        42.0
                    } else {
                        -42.0
                    },
                    0.0,
                ),
                SocialKind::HighFive => Vec2::new(
                    if role == SocialRole::Initiator {
                        30.0
                    } else {
                        -30.0
                    },
                    0.0,
                ),
                _ => Vec2::default(),
            };
            p.social = Some(SocialActivity {
                kind,
                role,
                partner_id: partner,
                elapsed: 0.0,
                partner_offset: offset,
            });
            p.vel = Vec2::default();
            p.facing = if p.pos.x < other_x { 1.0 } else { -1.0 };
            p.set_social_state(kind, role, duration, world);
            if kind == SocialKind::Tower && role == SocialRole::Initiator {
                p.bubble = None;
                p.bubble_time = 0.0;
            } else {
                p.say_social(kind, role);
            }
        }
        self.pairs[slot] = Some(Pair {
            a: a_id,
            b: b_id,
            kind,
            elapsed: 0.0,
            duration,
            phase: if kind == SocialKind::RoughPlay {
                ROUGH_PLAY_SQUARE_UP
            } else {
                initial_tag_separation
            },
            chaser: if matches!(kind, SocialKind::Leapfrog | SocialKind::RoughPlay) {
                0
            } else {
                a_id
            },
            support: support_rect(refs[i].as_deref().unwrap(), world),
            upper: if kind == SocialKind::Tower {
                b_id
            } else if kind == SocialKind::Leapfrog {
                a_id
            } else {
                0
            },
            tower_anchor_dx: 0.0,
            leapfrog_jumps,
        });
        let pair = self.pairs[slot].unwrap();
        if kind == SocialKind::Tag {
            self.set_tag_motion(pair, refs, CAP);
        } else if kind == SocialKind::Tower {
            let lower_id = a_id;
            let upper_id = b_id;
            let lower_pos = refs[i].as_deref().unwrap().pos;
            if let Some(upper_i) = find(refs, CAP, upper_id) {
                let upper = refs[upper_i].as_deref_mut().unwrap();
                let scale = upper.display_scale.clamp(0.5, 2.5);
                upper.social_motion = Some(crate::social::SocialMotion::Leap {
                    target: Vec2::new(lower_pos.x, lower_pos.y - 55.0 * scale),
                    owner_id: lower_id,
                });
            }
        } else if kind == SocialKind::Leapfrog {
            let jumper = refs[i].as_deref().unwrap();
            let croucher = refs[j].as_deref().unwrap();
            if let Some((target, arc)) =
                leapfrog_target(jumper, croucher, pair.support).and_then(|target| {
                    crate::leapfrog::plan(jumper, croucher, target, world).map(|arc| (target, arc))
                })
            {
                if let Some(jumper_i) = find(refs, CAP, a_id) {
                    refs[jumper_i]
                        .as_deref_mut()
                        .unwrap()
                        .prepare_leapfrog(target, b_id, arc);
                }
            }
        }
        let intensity = (refs[i].as_deref().unwrap().activity_intensity as f64
            + refs[j].as_deref().unwrap().activity_intensity as f64)
            * 0.5;
        cooldown(
            &mut self.cooldowns,
            a_id,
            (12.0 + (a_id % 19) as f64) * (1.5 - intensity.min(100.0) / 100.0),
        );
        cooldown(
            &mut self.cooldowns,
            b_id,
            (12.0 + (b_id % 19) as f64) * (1.5 - intensity.min(100.0) / 100.0),
        );
        self.last_kind = Some(kind);
        self.stats.started += 1;
        true
    }
    fn finish_pair(
        &mut self,
        p: Pair,
        refs: &mut [Option<&mut Pet>; CAP],
        n: usize,
        world: &World,
        completed: bool,
    ) {
        for id in [p.a, p.b] {
            if let Some(i) = find(refs, n, id) {
                if let Some(pet) = refs[i].as_deref_mut() {
                    let was_perched = matches!(
                        pet.social_motion,
                        Some(crate::social::SocialMotion::Perch { .. })
                    );
                    let owns_marker = pet.social.is_some_and(|s| {
                        s.kind == p.kind && s.partner_id == if id == p.a { p.b } else { p.a }
                    });
                    if owns_marker
                        && matches!(
                            p.kind,
                            SocialKind::Tag
                                | SocialKind::Tower
                                | SocialKind::Leapfrog
                                | SocialKind::RoughPlay
                        )
                    {
                        pet.social_motion = None;
                        if p.kind == SocialKind::Tower && id == p.upper {
                            if completed && was_perched {
                                let room_left = pet.pos.x - (p.support.x + 25.0);
                                let room_right = p.support.right() - 25.0 - pet.pos.x;
                                let direction = if room_right >= room_left { 1.0 } else { -1.0 };
                                let scale = pet.display_scale.clamp(0.5, 2.5);
                                pet.vel = Vec2::new(direction * 230.0, -330.0 * scale);
                                pet.state = State::Jump;
                                pet.state_time = 0.0;
                            } else {
                                pet.vel.x = pet.vel.x.clamp(-45.0, 45.0);
                                pet.vel.y = pet.vel.y.max(0.0);
                            }
                        }
                    }
                    if pet
                        .social
                        .is_some_and(|s| s.partner_id == if id == p.a { p.b } else { p.a })
                    {
                        pet.social = None;
                        if pet.state == State::Social
                            || (matches!(p.kind, SocialKind::Tag | SocialKind::RoughPlay)
                                && pet.state == State::Run)
                            || (p.kind == SocialKind::ShareLaptop
                                && id == p.a
                                && pet.state == State::Laptop)
                        {
                            pet.set_social_end(world);
                        }
                    }
                }
            }
        }
        if completed {
            for id in [p.a, p.b] {
                if let Some(i) = find(refs, n, id) {
                    if let Some(pet) = refs[i].as_deref_mut() {
                        match p.kind {
                            SocialKind::Comfort => {
                                pet.satisfaction = (pet.satisfaction
                                    + if pet.satisfaction < 35.0 { 8.0 } else { 4.0 })
                                .min(100.0)
                            }
                            SocialKind::DanceTogether | SocialKind::RoughPlay => {
                                pet.satisfaction = (pet.satisfaction + 2.0).min(100.0)
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
        if completed {
            self.stats.completed += 1;
        } else {
            self.stats.cancelled += 1;
        }
    }
    fn cancel_all(&mut self, refs: &mut [Option<&mut Pet>; CAP], n: usize, world: &World) {
        for i in 0..self.pairs.len() {
            if let Some(p) = self.pairs[i].take() {
                self.finish_pair(p, refs, n, world, false);
            }
        }
        if let Some(g) = self.tower.take() {
            self.finish_tower(g, refs, n, world, false);
        }
    }
}
fn find(refs: &[Option<&mut Pet>; CAP], n: usize, id: u64) -> Option<usize> {
    (0..n).find(|&i| refs[i].as_deref().is_some_and(|p| p.id == id))
}
fn active_marker(p: &Pet, pair: Pair, id: u64, other: u64) -> bool {
    p.social.is_some_and(|s| {
        s.partner_id == other
            && s.kind == pair.kind
            && s.role
                == if id == pair.a {
                    SocialRole::Initiator
                } else {
                    SocialRole::Partner
                }
    })
}
fn active_state(p: &Pet, pair: Pair, dt: f64) -> bool {
    matches!(p.state, State::Social | State::Laptop)
        || (p.state == State::Idle && pair.elapsed + dt >= pair.duration)
}
fn cooldown(c: &mut [Cooldown; CAP], id: u64, seconds: f64) {
    let i = c
        .iter()
        .position(|x| x.id == id)
        .or_else(|| c.iter().position(|x| x.remaining <= 0.0))
        .unwrap_or(0);
    c[i] = Cooldown {
        id,
        remaining: seconds,
    };
}
fn cooling(c: &[Cooldown; CAP], id: u64) -> bool {
    c.iter().any(|x| x.id == id && x.remaining > 0.0)
}
fn tower_eligible(
    refs: &[Option<&mut Pet>; CAP],
    i: usize,
    w: &World,
    s: &SocialCoordinator,
    manual: bool,
) -> bool {
    let p = refs[i].as_deref().unwrap();
    // The generic pair gate only admits Idle/Walk/Happy. That accidentally
    // excludes most friends when they are resting or looking around.
    p.skin == Skin::Claude
        && !p.dragging
        && !p.chase
        && p.social.is_none()
        && p.activity.is_none()
        && p.socially_supported(w)
        && (manual || (p.activity_intensity > 0 && p.behavior_manual_lock <= 0.0))
        && matches!(
            p.state,
            State::Idle
                | State::Walk
                | State::Run
                | State::Happy
                | State::Sit
                | State::LookAround
                | State::Wave
                | State::Stretch
                | State::Dance
        )
        && !s
            .pairs
            .iter()
            .flatten()
            .any(|pair| pair.a == p.id || pair.b == p.id)
        && !s
            .tower
            .is_some_and(|group| group.ids[..group.len].contains(&p.id))
        && (manual || !cooling(&s.cooldowns, p.id))
}

fn free_eligible(
    refs: &[Option<&mut Pet>; CAP],
    i: usize,
    w: &World,
    s: &SocialCoordinator,
    bypass_cooldown: bool,
) -> bool {
    let p = refs[i].as_deref().unwrap();
    eligible(p, w, false)
        && (bypass_cooldown || p.activity_intensity > 0)
        && !s.pairs.iter().flatten().any(|x| x.a == p.id || x.b == p.id)
        && !s.tower.is_some_and(|g| g.ids[..g.len].contains(&p.id))
        && (bypass_cooldown || !cooling(&s.cooldowns, p.id))
}
fn eligible(p: &Pet, w: &World, active: bool) -> bool {
    p.skin == Skin::Claude
        && !p.dragging
        && !p.chase
        && p.social.is_some() == active
        && p.socially_supported(w)
        && if active {
            matches!(p.state, State::Social | State::Laptop | State::Idle)
        } else {
            matches!(p.state, State::Idle | State::Walk | State::Happy)
        }
}
fn choose(a: &Pet, b: &Pet, age: f64, previous: Option<SocialKind>, distance: f64) -> SocialKind {
    // Rough play is a rare, separately hashed choice so the established mix of
    // calmer interactions keeps its usual frequencies.
    let rough_play = rough_play_willing(a)
        && rough_play_willing(b)
        && (a.id.wrapping_mul(0x9E37) ^ b.id.wrapping_mul(0x85EB))
            .wrapping_add(age as u64 / 4)
            .is_multiple_of(13);
    let preferred = if a.satisfaction < 35.0 || b.satisfaction < 35.0 {
        SocialKind::Comfort
    } else if rough_play {
        SocialKind::RoughPlay
    } else {
        match ((a.id ^ b.id).wrapping_add(age as u64)) % 23 {
            0 => SocialKind::Tag,
            1 if a.satisfaction >= 65.0 && b.satisfaction >= 65.0 => SocialKind::Tower,
            2 => SocialKind::Greeting,
            3..=7 => SocialKind::HighFive,
            8..=12 => SocialKind::DanceTogether,
            13..=17 => SocialKind::ShareLaptop,
            _ => SocialKind::Comfort,
        }
    };
    let scale = a.display_scale.max(b.display_scale).clamp(0.5, 2.0);
    let fits = |kind: SocialKind| {
        let (min, max) = social_distance_range(kind, scale);
        distance >= min && distance <= max
    };
    if Some(preferred) != previous && fits(preferred) {
        return preferred;
    }
    [
        SocialKind::Comfort,
        SocialKind::Greeting,
        SocialKind::DanceTogether,
        SocialKind::ShareLaptop,
        SocialKind::HighFive,
        SocialKind::Tag,
        SocialKind::Tower,
    ]
    .into_iter()
    .find(|kind| Some(*kind) != previous && fits(*kind))
    .unwrap_or(preferred)
}

fn social_distance_range(kind: SocialKind, scale: f64) -> (f64, f64) {
    match kind {
        SocialKind::Greeting | SocialKind::Comfort | SocialKind::DanceTogether => {
            (104.0 * scale, 300.0 * scale)
        }
        SocialKind::HighFive => (74.0 * scale, 110.0 * scale),
        SocialKind::ShareLaptop => (100.0 * scale, 160.0 * scale),
        SocialKind::Tag => (64.0 * scale, 240.0 * scale),
        SocialKind::Tower => (60.0 * scale, f64::INFINITY),
        SocialKind::Leapfrog => (60.0 * scale, 260.0 * scale),
        SocialKind::RoughPlay => (100.0 * scale, 240.0 * scale),
    }
}

/// Autonomous rough play needs two content Claudes with random activities on.
fn rough_play_willing(p: &Pet) -> bool {
    p.auto_activities
        && p.activity_intensity >= 25
        && p.satisfaction >= 55.0
        && p.behavior_manual_lock <= 0.0
}
fn duration(k: SocialKind) -> f64 {
    match k {
        SocialKind::Greeting => 4.0,
        SocialKind::HighFive => 2.0,
        SocialKind::DanceTogether => 4.0,
        SocialKind::ShareLaptop => 10.0,
        SocialKind::Comfort => 4.0,
        SocialKind::Tag => 7.5,
        SocialKind::Tower => 8.0,
        SocialKind::Leapfrog => 10.0,
        SocialKind::RoughPlay => 3.5,
    }
}
fn same_support(a: &Pet, b: &Pet, w: &World) -> bool {
    match (a.surface_id, b.surface_id) {
        (Some(x), Some(y)) => x == y,
        (None, None) => {
            let aa = w.area_at(a.pos);
            let bb = w.area_at(b.pos);
            aa == bb
        }
        _ => false,
    }
}

fn distance_allowed(kind: Option<SocialKind>, distance: f64) -> bool {
    match kind {
        Some(SocialKind::Tag) => distance <= 480.0,
        Some(SocialKind::Tower) => distance <= MAX_DISTANCE,
        Some(SocialKind::HighFive) => distance <= 220.0,
        Some(SocialKind::ShareLaptop) => distance <= 320.0,
        Some(SocialKind::Leapfrog) => distance <= 520.0,
        Some(SocialKind::RoughPlay) => distance <= 260.0,
        _ => distance <= MAX_DISTANCE,
    }
}

fn can_start_kind(kind: SocialKind, a: &Pet, b: &Pet, w: &World) -> bool {
    if !same_support(a, b, w) {
        return false;
    }
    let support = support_rect(a, w);
    let scale = a.display_scale.max(b.display_scale).clamp(0.5, 2.0);
    let d = (a.pos.x - b.pos.x).hypot(a.pos.y - b.pos.y);
    let range = social_distance_range(kind, scale);
    if d < range.0 || d > range.1 {
        return false;
    }
    match kind {
        SocialKind::Tag => {
            support.w >= 320.0
                && a.pos.x >= support.x + 25.0
                && a.pos.x <= support.right() - 25.0
                && b.pos.x >= support.x + 25.0
                && b.pos.x <= support.right() - 25.0
        }
        SocialKind::Tower => {
            let work_area = w.area_at(Vec2::new(a.pos.x, support.y));
            let scale = a.display_scale.max(b.display_scale).clamp(0.5, 2.5);
            let headroom = support.y - work_area.y;
            let dismount_room =
                (a.pos.x - (support.x + 25.0)).max(support.right() - 25.0 - a.pos.x);
            support.w >= 180.0
                && headroom >= 110.0 * scale + 53.0
                && dismount_room >= 75.0 * scale
                && a.pos.x >= support.x + 35.0
                && a.pos.x <= support.right() - 35.0
                && b.pos.x >= support.x + 35.0
                && b.pos.x <= support.right() - 35.0
        }
        SocialKind::Leapfrog => {
            support.y - w.area_at(a.pos).y
                >= 180.0 * a.display_scale.max(b.display_scale).clamp(0.5, 2.0)
                && support.w >= 320.0
                && d >= 38.0 * (a.display_scale + b.display_scale) + 4.0 * scale
                && leapfrog_target(a, b, support)
                    .and_then(|target| crate::leapfrog::plan(a, b, target, w))
                    .is_some()
        }
        SocialKind::RoughPlay => {
            // The partner rolls away from the initiator; keep that landing on
            // the same support and inside the work area, and never shove an
            // upset Claude.
            let direction = if b.pos.x >= a.pos.x { 1.0 } else { -1.0 };
            let room = if direction > 0.0 {
                support.right() - b.pos.x
            } else {
                b.pos.x - support.x
            };
            let area = w.area_at(b.pos);
            let area_room = if direction > 0.0 {
                area.right() - b.pos.x
            } else {
                b.pos.x - area.x
            };
            support.w >= 320.0
                && room >= 180.0 * scale
                && area_room >= 180.0 * scale
                && a.pos.x >= support.x + 25.0
                && a.pos.x <= support.right() - 25.0
                && a.satisfaction >= 40.0
                && b.satisfaction >= 40.0
        }
        _ => true,
    }
}

fn support_rect(p: &Pet, w: &World) -> Rect {
    if let Some(id) = p.surface_id {
        if let Some(window) = w.windows.iter().find(|x| x.id == id && x.rect.is_valid()) {
            return window.rect;
        }
    }
    let area = w.area_at(p.pos);
    Rect::new(area.x, area.bottom() - 1.0, area.w, 1.0)
}

fn support_rect_matches(p: &Pet, expected: Rect, w: &World) -> bool {
    let actual = support_rect(p, w);
    (actual.x - expected.x).abs() <= 1.0
        && (actual.y - expected.y).abs() <= 1.0
        && (actual.w - expected.w).abs() <= 1.0
        && (actual.h - expected.h).abs() <= 1.0
}

fn support_exists(expected: Rect, world: &World, at: Vec2) -> bool {
    let same = |actual: Rect| {
        (actual.x - expected.x).abs() <= 1.0
            && (actual.y - expected.y).abs() <= 1.0
            && (actual.w - expected.w).abs() <= 1.0
    };
    if world
        .windows
        .iter()
        .any(|win| win.rect.is_valid() && same(win.rect))
    {
        return true;
    }
    let area = world.area_at(at);
    same(Rect::new(area.x, area.bottom() - 1.0, area.w, 1.0))
}

fn leapfrog_target(jumper: &Pet, croucher: &Pet, support: Rect) -> Option<Vec2> {
    let scale = jumper
        .display_scale
        .max(croucher.display_scale)
        .clamp(0.5, 2.0);
    let direction = if jumper.pos.x < croucher.pos.x {
        1.0
    } else {
        -1.0
    };
    let x = croucher.pos.x + direction * 160.0 * scale;
    let margin = 35.0 * scale;
    if x < support.x + margin
        || x > support.right() - margin
        || (x - jumper.pos.x).abs() < 60.0 * scale
    {
        return None;
    }
    Some(Vec2::new(
        x,
        support.y + if support.h <= 1.0 { 1.0 } else { 0.0 },
    ))
}

fn tower_target_y(refs: &[Option<&mut Pet>; CAP], n: usize, g: &TowerGroup, k: usize) -> f64 {
    let rise = (0..k)
        .map(|j| {
            let a = find(refs, n, g.ids[j])
                .map(|i| refs[i].as_deref().unwrap().display_scale.clamp(0.5, 2.0))
                .unwrap_or(1.0);
            let b = find(refs, n, g.ids[j + 1])
                .map(|i| refs[i].as_deref().unwrap().display_scale.clamp(0.5, 2.0))
                .unwrap_or(1.0);
            55.0 * a.max(b)
        })
        .sum::<f64>();
    g.support.y - rise
}

fn tower_path_fits(refs: &[Option<&mut Pet>; CAP], n: usize, g: &TowerGroup) -> bool {
    for k in 1..g.len {
        let Some(i) = find(refs, n, g.ids[k]) else {
            return false;
        };
        let p = refs[i].as_deref().unwrap();
        let target = Vec2::new(g.stack_x, tower_target_y(refs, n, g, k));
        if tower_launch(p, target).is_none() {
            return false;
        }
    }
    true
}

fn tower_launch(pet: &Pet, target: Vec2) -> Option<(f64, f64)> {
    let scale = pet.display_scale.clamp(0.5, 2.0);
    let rise = (pet.pos.y - target.y + 65.0 * scale + 100.0 * scale).clamp(80.0, 900.0);
    let vy = (2.0 * GRAVITY * rise).sqrt().min(2050.0);
    let dy = target.y - pet.pos.y;
    let disc = vy * vy + 2.0 * GRAVITY * dy;
    if disc < 0.0 {
        return None;
    }
    let root = disc.sqrt();
    let t_up = (vy - root) / GRAVITY;
    let t_down = (vy + root) / GRAVITY;
    let flight = t_down - t_up;
    if flight <= 0.0 || !flight.is_finite() {
        return None;
    }
    let vx = (target.x - pet.pos.x) / flight;
    (vx.is_finite() && vx.abs() <= 600.0).then_some((vx, vy))
}

fn set_tower_leap(pet: &mut Pet, target: Vec2, owner_id: u64) -> Option<f64> {
    let (vx, launch) = tower_launch(pet, target)?;
    pet.social_motion = Some(SocialMotion::Leap { target, owner_id });
    pet.vel = Vec2::new(0.0, -launch);
    pet.state = State::Jump;
    pet.state_time = 0.0;
    Some(vx)
}
impl Pet {
    fn set_social_state(&mut self, k: SocialKind, role: SocialRole, d: f64, _w: &World) {
        self.state = if k == SocialKind::ShareLaptop && role == SocialRole::Initiator {
            State::Laptop
        } else {
            State::Social
        };
        self.state_time = 0.0;
        self.state_duration = d;
    }
    fn say_social(&mut self, kind: SocialKind, role: SocialRole) {
        use crate::dialogue::SpeechEvent as E;
        let initiator = role == SocialRole::Initiator;
        let event = match kind {
            SocialKind::Greeting => {
                if initiator {
                    E::Greeting
                } else {
                    E::GreetingReply
                }
            }
            SocialKind::HighFive => {
                if initiator {
                    E::HighFive
                } else {
                    E::HighFiveReply
                }
            }
            SocialKind::DanceTogether => E::DanceTogether,
            SocialKind::ShareLaptop => {
                if initiator {
                    E::ShareLaptop
                } else {
                    E::WatchLaptop
                }
            }
            SocialKind::Comfort => {
                if self.satisfaction < 35.0 {
                    E::ComfortReply
                } else {
                    E::Comfort
                }
            }
            SocialKind::Tag => {
                if initiator {
                    E::TagChaser
                } else {
                    E::TagRunner
                }
            }
            SocialKind::Tower => E::TowerTop,
            SocialKind::Leapfrog => {
                if initiator {
                    E::LeapfrogJump
                } else {
                    E::LeapfrogWait
                }
            }
            SocialKind::RoughPlay => {
                if initiator {
                    E::RoughPlay
                } else {
                    E::RoughPlayReply
                }
            }
        };
        self.speak(event, 2.0);
    }
    fn set_social_end(&mut self, w: &World) {
        self.state = if self.socially_supported(w) {
            State::Idle
        } else {
            State::Fall
        };
        self.state_time = 0.0;
        self.state_duration = 0.0;
    }
}

#[cfg(test)]
mod tower_selection_tests {
    use super::*;
    #[test]
    fn group_invitation_does_not_need_two_idle_walkers() {
        let world = World::default();
        let mut pets: Vec<_> = (0..5)
            .map(|i| {
                let mut p = Pet::new(
                    i + 1,
                    Skin::Claude,
                    Vec2::new(700.0 + i as f64 * 82.0, 1040.0),
                );
                p.state = if i == 0 { State::Idle } else { State::Sit };
                p
            })
            .collect();
        let mut refs: [Option<&mut Pet>; CAP] = std::array::from_fn(|_| None);
        for (slot, pet) in refs.iter_mut().zip(pets.iter_mut()) {
            *slot = Some(pet);
        }
        let mut coordinator = SocialCoordinator::new();
        coordinator.age = 20.2;
        coordinator.tick(&mut refs, 5, &world);
        assert!(coordinator.tower.is_some());
        assert!(coordinator.tower_invitation_at > coordinator.age);
    }
    #[test]
    fn autonomous_towers_can_choose_two_three_four_and_five_relaxed_friends() {
        let world = World::default();
        let mut seen = std::collections::BTreeSet::new();
        for seed in 0..12 {
            let mut pets: Vec<_> = (0..5)
                .map(|i| {
                    let mut p = Pet::new(
                        i + 1,
                        Skin::Claude,
                        Vec2::new(700.0 + i as f64 * 82.0, 1040.0),
                    );
                    p.state = [
                        State::Idle,
                        State::Sit,
                        State::LookAround,
                        State::Stretch,
                        State::Run,
                    ][i as usize];
                    p
                })
                .collect();
            let mut refs: [Option<&mut Pet>; CAP] = std::array::from_fn(|_| None);
            for (slot, pet) in refs.iter_mut().zip(pets.iter_mut()) {
                *slot = Some(pet);
            }
            let mut coordinator = SocialCoordinator::new();
            coordinator.age = seed as f64 / 10.0;
            assert!(coordinator.start_tower(Some(1), &mut refs, 5, &world, false));
            seen.insert(coordinator.tower.unwrap().len);
        }
        assert_eq!(seen.into_iter().collect::<Vec<_>>(), vec![2, 3, 4, 5]);
    }
}
