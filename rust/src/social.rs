//! Deterministic, bounded coordinator for friendly interactions between pets.
use crate::{
    engine::{Pet, Skin, State},
    Rect, Vec2, World,
};

const CAP: usize = 8;
const TICK: f64 = 0.1;
const INITIAL_DELAY: f64 = 2.0;
const MAX_DISTANCE: f64 = 140.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SocialKind {
    Greeting,
    HighFive,
    DanceTogether,
    ShareLaptop,
    Comfort,
    Tag,
    Tower,
}

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
    cooldowns: [Cooldown; CAP],
    request: Option<(Option<u64>, Option<SocialKind>)>,
    request_age: f64,
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
            cooldowns: [Cooldown::default(); CAP],
            request: None,
            request_age: 0.0,
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
                            )
                        })
                    })
                })
            })
        {
            self.request = None;
        }
        self.advance_pairs(&mut refs, count, world, step);
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
                    if (pa.pos.x - pb.pos.x).abs() <= 64.0 && pair.phase <= 0.0 {
                        pair.chaser = if pair.chaser == pair.a {
                            pair.b
                        } else {
                            pair.a
                        };
                        pair.phase = 0.7;
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
                        && (pa.pos.x - pb.pos.x).abs() <= MAX_DISTANCE
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
        if self.age < INITIAL_DELAY && self.request.is_none() {
            return;
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
                    let mut kind = requested_kind
                        .unwrap_or_else(|| choose(a, refs[j].as_deref().unwrap(), self.age));
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
                let mut kind = self
                    .request
                    .take()
                    .and_then(|(_, k)| k)
                    .unwrap_or_else(|| choose(a, b, self.age));
                if kind == SocialKind::HighFive && (a.pos.x - b.pos.x).abs() > 96.0 {
                    if let Some(request) = req {
                        self.request = Some(request);
                        continue;
                    } else {
                        kind = SocialKind::Greeting;
                    }
                }
                if self.start(i, j, kind, refs, world) {
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
        let duration = duration(kind);
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
            phase: initial_tag_separation,
            chaser: a_id,
            support: support_rect(refs[i].as_deref().unwrap(), world),
            upper: if kind == SocialKind::Tower { b_id } else { 0 },
            tower_anchor_dx: 0.0,
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
        }
        cooldown(&mut self.cooldowns, a_id, 12.0 + (a_id % 19) as f64);
        cooldown(&mut self.cooldowns, b_id, 12.0 + (b_id % 19) as f64);
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
                    if matches!(p.kind, SocialKind::Tag | SocialKind::Tower) {
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
                            || (p.kind == SocialKind::Tag && pet.state == State::Run)
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
                            SocialKind::DanceTogether => {
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
fn free_eligible(
    refs: &[Option<&mut Pet>; CAP],
    i: usize,
    w: &World,
    s: &SocialCoordinator,
    bypass_cooldown: bool,
) -> bool {
    let p = refs[i].as_deref().unwrap();
    eligible(p, w, false)
        && !s.pairs.iter().flatten().any(|x| x.a == p.id || x.b == p.id)
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
fn choose(a: &Pet, b: &Pet, age: f64) -> SocialKind {
    if a.satisfaction < 35.0 || b.satisfaction < 35.0 {
        SocialKind::Comfort
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
    }
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
        Some(SocialKind::Tag) => distance <= 240.0,
        Some(SocialKind::Tower) => distance <= 175.0,
        _ => distance <= MAX_DISTANCE,
    }
}

fn can_start_kind(kind: SocialKind, a: &Pet, b: &Pet, w: &World) -> bool {
    if !same_support(a, b, w) {
        return false;
    }
    let support = support_rect(a, w);
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
    fn say_social(&mut self, k: SocialKind, role: SocialRole) {
        let text = match k {
            SocialKind::Greeting => {
                if role == SocialRole::Initiator {
                    "привет!"
                } else {
                    "привет :)"
                }
            }
            SocialKind::HighFive => {
                if role == SocialRole::Initiator {
                    "дай пять!"
                } else {
                    "пять!"
                }
            }
            SocialKind::DanceTogether => "танцуем?",
            SocialKind::ShareLaptop => {
                if role == SocialRole::Initiator {
                    "смотри, что делаю"
                } else {
                    "интересно!"
                }
            }
            SocialKind::Comfort => {
                if self.satisfaction < 35.0 {
                    "спасибо, полегче"
                } else {
                    "всё хорошо?"
                }
            }
            SocialKind::Tag => {
                if role == SocialRole::Initiator {
                    "догоню!"
                } else {
                    "не поймаешь!"
                }
            }
            SocialKind::Tower => "я сверху!",
        };
        self.bubble = Some(text.to_owned());
        self.bubble_time = 2.0;
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
