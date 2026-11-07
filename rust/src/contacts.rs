//! Bounded pairwise body contact resolution for the desktop simulation.
use crate::{
    engine::{Pet, State},
    social::SocialKind,
    Vec2, World,
};

const CAP: usize = 8;
const RESTITUTION: f64 = 0.55;
const HALF_WIDTH: f64 = 38.0;
const HALF_HEIGHT: f64 = 27.5;

#[derive(Clone, Copy, Debug)]
struct Body {
    id: u64,
    pos: Vec2,
    old: Vec2,
    vel: Vec2,
    scale: f64,
    dragged: bool,
    state: State,
    tower: bool,
    tag: bool,
    supported: bool,
}

pub struct ContactCoordinator {
    previous: [Option<(u64, Vec2)>; CAP],
    pub contact_count: u64,
    pub strong_impacts: u64,
}
impl ContactCoordinator {
    pub fn new() -> Self {
        Self {
            previous: [None; CAP],
            contact_count: 0,
            strong_impacts: 0,
        }
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
        let mut slots: [Option<&mut Pet>; CAP] = std::array::from_fn(|_| None);
        let mut count = 0;
        for pet in pets {
            if count == CAP {
                break;
            }
            slots[count] = Some(pet);
            count += 1;
        }
        let mut bodies = [None; CAP];
        for i in 0..count {
            let p = slots[i].as_deref().unwrap();
            let old = self
                .previous
                .iter()
                .flatten()
                .find(|(id, _)| *id == p.id)
                .map(|(_, pos)| *pos)
                .unwrap_or(p.pos);
            let scale = if p.display_scale.is_finite() {
                p.display_scale.clamp(0.5, 2.0)
            } else {
                1.0
            };
            let vel = if p.dragging && dt > 0.0 {
                Vec2::new(
                    ((p.pos.x - old.x) / dt).clamp(-2200.0, 2200.0),
                    ((p.pos.y - old.y) / dt).clamp(-2200.0, 2200.0),
                )
            } else {
                p.vel
            };
            bodies[i] = Some(Body {
                id: p.id,
                pos: p.pos,
                old,
                vel,
                scale,
                dragged: p.dragging,
                state: p.state,
                tower: p.social.is_some_and(|s| s.kind == SocialKind::Tower),
                tag: p.social.is_some_and(|s| s.kind == SocialKind::Tag),
                supported: p.socially_supported(world),
            });
        }
        for i in 0..count {
            for j in i + 1..count {
                let (Some(a), Some(b)) = (bodies[i], bodies[j]) else {
                    continue;
                };
                if a.tower && b.tower {
                    continue;
                }
                let Some(hit) = contact(a, b) else {
                    continue;
                };
                self.contact_count = self.contact_count.saturating_add(1);
                let n = hit.normal;
                let vertical = n.y.abs() > 0.5;
                let inv_a = if a.dragged || (vertical && a.supported && !b.supported) {
                    0.0
                } else {
                    1.0 / (a.scale * a.scale)
                };
                let inv_b = if b.dragged || (vertical && b.supported && !a.supported) {
                    0.0
                } else {
                    1.0 / (b.scale * b.scale)
                };
                let inv_sum = inv_a + inv_b;
                if inv_sum <= 0.0 {
                    continue;
                }
                let relative = Vec2::new(b.vel.x - a.vel.x, b.vel.y - a.vel.y);
                let normal_speed = relative.x * n.x + relative.y * n.y;
                let mut new_a = a.vel;
                let mut new_b = b.vel;
                let mut strong = false;
                if normal_speed < 0.0 {
                    let impact = normal_speed.abs();
                    let restitution = if is_tag_pair(a, b) { 0.25 } else { RESTITUTION };
                    let impulse = -(1.0 + restitution) * normal_speed / inv_sum;
                    new_a.x -= impulse * n.x * inv_a;
                    new_a.y -= impulse * n.y * inv_a;
                    new_b.x += impulse * n.x * inv_b;
                    new_b.y += impulse * n.y * inv_b;
                    strong = impact >= 260.0
                        || matches!(a.state, State::Thrown) && impact >= 150.0
                        || matches!(b.state, State::Thrown) && impact >= 150.0;
                    if strong {
                        self.strong_impacts = self.strong_impacts.saturating_add(1);
                    }
                }
                if let Some((pa, pb)) = pair_mut(&mut slots, i, j) {
                    if hit.swept {
                        if !a.dragged {
                            pa.contact_position(hit.at_a, world, HALF_WIDTH * a.scale);
                        }
                        if !b.dragged {
                            pb.contact_position(hit.at_b, world, HALF_WIDTH * b.scale);
                        }
                    } else if hit.penetration > 0.0 {
                        let correction = (hit.penetration - 0.05).max(0.0) / inv_sum;
                        pa.contact_position(
                            Vec2::new(
                                a.pos.x - n.x * correction * inv_a,
                                a.pos.y - n.y * correction * inv_a,
                            ),
                            world,
                            HALF_WIDTH * a.scale,
                        );
                        pb.contact_position(
                            Vec2::new(
                                b.pos.x + n.x * correction * inv_b,
                                b.pos.y + n.y * correction * inv_b,
                            ),
                            world,
                            HALF_WIDTH * b.scale,
                        );
                    }
                    if normal_speed < 0.0 {
                        if inv_a > 0.0 {
                            pa.contact_impact(new_a, strong);
                        }
                        if inv_b > 0.0 {
                            pb.contact_impact(new_b, strong);
                        }
                    }
                }
                if let Some(pa) = slots[i].as_deref() {
                    if let Some(mut body) = bodies[i] {
                        body.pos = pa.pos;
                        body.old = pa.pos;
                        body.vel = if pa.dragging { a.vel } else { pa.vel };
                        body.state = pa.state;
                        body.tower = pa.social.is_some_and(|s| s.kind == SocialKind::Tower);
                        body.tag = pa.social.is_some_and(|s| s.kind == SocialKind::Tag);
                        body.supported = pa.socially_supported(world);
                        bodies[i] = Some(body);
                    }
                }
                if let Some(pb) = slots[j].as_deref() {
                    if let Some(mut body) = bodies[j] {
                        body.pos = pb.pos;
                        body.old = pb.pos;
                        body.vel = if pb.dragging { b.vel } else { pb.vel };
                        body.state = pb.state;
                        body.tower = pb.social.is_some_and(|s| s.kind == SocialKind::Tower);
                        body.tag = pb.social.is_some_and(|s| s.kind == SocialKind::Tag);
                        body.supported = pb.socially_supported(world);
                        bodies[j] = Some(body);
                    }
                }
            }
        }
        self.previous.fill(None);
        for (i, slot) in slots.iter().enumerate().take(count) {
            if let Some(p) = slot.as_deref() {
                self.previous[i] = Some((p.id, p.pos));
            }
        }
    }
}
impl Default for ContactCoordinator {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy)]
struct Hit {
    normal: Vec2,
    penetration: f64,
    swept: bool,
    at_a: Vec2,
    at_b: Vec2,
}
fn contact(a: Body, b: Body) -> Option<Hit> {
    let ax = HALF_WIDTH * a.scale;
    let ay = HALF_HEIGHT * a.scale;
    let bx = HALF_WIDTH * b.scale;
    let by = HALF_HEIGHT * b.scale;
    let dx = b.pos.x - a.pos.x;
    let dy = (b.pos.y - by) - (a.pos.y - ay);
    let ox = ax + bx - dx.abs();
    let oy = ay + by - dy.abs();
    if ox > 0.0 && oy > 0.0 {
        let horizontal_motion = (a.vel.x - b.vel.x).abs() >= (a.vel.y - b.vel.y).abs();
        if (a.supported && b.supported)
            || ((a.supported || b.supported) && horizontal_motion)
            || ox <= oy
        {
            Some(Hit {
                normal: Vec2::new(
                    if dx > 0.0 || (dx == 0.0 && a.id < b.id) {
                        1.0
                    } else {
                        -1.0
                    },
                    0.0,
                ),
                penetration: ox,
                swept: false,
                at_a: a.pos,
                at_b: b.pos,
            })
        } else {
            Some(Hit {
                normal: Vec2::new(
                    0.0,
                    if dy > 0.0 || (dy == 0.0 && a.id < b.id) {
                        1.0
                    } else {
                        -1.0
                    },
                ),
                penetration: oy,
                swept: false,
                at_a: a.pos,
                at_b: b.pos,
            })
        }
    } else {
        swept_hit(a, b, ax + bx, ay + by)
    }
}

fn swept_hit(a: Body, b: Body, hx: f64, hy: f64) -> Option<Hit> {
    let ay = HALF_HEIGHT * a.scale;
    let by = HALF_HEIGHT * b.scale;
    let start = Vec2::new(b.old.x - a.old.x, (b.old.y - by) - (a.old.y - ay));
    let end = Vec2::new(b.pos.x - a.pos.x, (b.pos.y - by) - (a.pos.y - ay));
    let delta = Vec2::new(end.x - start.x, end.y - start.y);
    let (mut enter, mut exit) = (0.0f64, 1.0f64);
    let mut normal = Vec2::default();
    for (s, d, extent, x_axis) in [(start.x, delta.x, hx, true), (start.y, delta.y, hy, false)] {
        if d.abs() < 1e-9 {
            if s.abs() > extent {
                return None;
            }
            continue;
        }
        let t1 = (-extent - s) / d;
        let t2 = (extent - s) / d;
        let near = t1.min(t2);
        let far = t1.max(t2);
        if near > enter {
            enter = near;
            normal = if x_axis {
                Vec2::new(if d > 0.0 { -1.0 } else { 1.0 }, 0.0)
            } else {
                Vec2::new(0.0, if d > 0.0 { -1.0 } else { 1.0 })
            };
        }
        exit = exit.min(far);
        if enter > exit {
            return None;
        }
    }
    if !(0.0..=1.0).contains(&enter) || normal == Vec2::default() {
        return None;
    }
    let ta = Vec2::new(
        a.old.x + (a.pos.x - a.old.x) * enter,
        a.old.y + (a.pos.y - a.old.y) * enter,
    );
    let tb = Vec2::new(
        b.old.x + (b.pos.x - b.old.x) * enter,
        b.old.y + (b.pos.y - b.old.y) * enter,
    );
    Some(Hit {
        normal,
        penetration: 0.0,
        swept: true,
        at_a: ta,
        at_b: tb,
    })
}

fn is_tag_pair(a: Body, b: Body) -> bool {
    a.tag && b.tag
}
fn pair_mut<'a>(
    slots: &'a mut [Option<&mut Pet>; CAP],
    i: usize,
    j: usize,
) -> Option<(&'a mut Pet, &'a mut Pet)> {
    let (left, right) = slots.split_at_mut(j);
    Some((left[i].as_deref_mut()?, right[0].as_deref_mut()?))
}
