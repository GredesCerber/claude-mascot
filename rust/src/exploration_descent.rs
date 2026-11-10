//! Voluntary, physically simulated hops from a window ledge to the desktop.

use crate::engine::{Pet, GRAVITY};
use crate::{Rect, Vec2, World};

const HALF_WIDTH: f64 = 24.0;
const BODY_TOP: f64 = 90.0;
const HOP_SPEED: f64 = 650.0;
const MAX_HORIZONTAL_SPEED: f64 = 300.0;
const LANDING_OFFSETS: [f64; 5] = [60.0, 90.0, 120.0, 150.0, 180.0];

/// Plan from the pet's current position when it is already near a safe edge.
pub(crate) fn plan_descent(
    pet: &Pet,
    world: &World,
    roll: f64,
) -> Option<crate::exploration::JumpPlan> {
    plan_from_position(pet, world, roll)
}

/// Find an on-window staging point and a validated jump to begin from there.
/// The caller can walk to the returned x before starting its crouch timer.
pub(crate) fn descent_approach(
    pet: &Pet,
    world: &World,
    roll: f64,
) -> Option<(f64, crate::exploration::JumpPlan)> {
    let source_id = pet.surface_id?;
    let (source_index, source) = world
        .windows
        .iter()
        .enumerate()
        .find(|(_, w)| w.id == source_id)?;
    if !source.rect.is_valid() || source.fullscreen || source.is_desktop_icon {
        return None;
    }
    let scale = display_scale(pet);
    let inset = (HALF_WIDTH + 4.0) * scale;
    for direction in direction_order(roll) {
        let edge_x = if direction < 0.0 {
            source.rect.x
        } else {
            source.rect.right()
        };
        let approach_x = edge_x - direction * inset;
        if (approach_x - pet.pos.x).abs() < 2.0 * scale
            || !supported_path_clear(
                world,
                source_index,
                source.rect,
                pet.pos.x,
                approach_x,
                scale,
            )
        {
            continue;
        }
        let mut staged = pet.clone();
        staged.pos.x = approach_x;
        let Some(plan) =
            plan_from_position(&staged, world, if direction > 0.0 { 0.0 } else { 0.75 })
        else {
            continue;
        };
        if (plan.target.x - approach_x) * direction > 0.0 {
            return Some((approach_x, plan));
        }
    }
    None
}

fn plan_from_position(pet: &Pet, world: &World, roll: f64) -> Option<crate::exploration::JumpPlan> {
    if world.fullscreen_active() || !pet.pos.x.is_finite() || !pet.pos.y.is_finite() {
        return None;
    }
    let source_id = pet.surface_id?;
    let (source_index, source) = world
        .windows
        .iter()
        .enumerate()
        .find(|(_, w)| w.id == source_id)?;
    if !source.rect.is_valid() || source.fullscreen || source.is_desktop_icon {
        return None;
    }
    let scale = display_scale(pet);
    let half = HALF_WIDTH * scale;
    if (pet.pos.y - source.rect.y).abs() > 4.0 * scale
        || pet.pos.x < source.rect.x + half
        || pet.pos.x > source.rect.right() - half
        || covered_at(world, source_index, pet.pos.x, source.rect.y + 1.0)
    {
        return None;
    }
    let area = world
        .work_areas
        .iter()
        .copied()
        .find(|a| a.is_valid() && a.contains(pet.pos))
        .or_else(|| {
            world
                .work_areas
                .iter()
                .copied()
                .filter(|a| a.is_valid())
                .min_by(|a, b| distance(*a, pet.pos).total_cmp(&distance(*b, pet.pos)))
        })?;
    if area.w <= 2.0 * half {
        return None;
    }
    let floor_y = area.bottom();
    for direction in direction_order(roll) {
        let edge_x = if direction < 0.0 {
            source.rect.x
        } else {
            source.rect.right()
        };
        for offset in LANDING_OFFSETS {
            let target_x = edge_x + direction * offset * scale;
            if target_x - half < area.x || target_x + half > area.right() {
                continue;
            }
            let target = Vec2::new(target_x, floor_y);
            if (target.x - pet.pos.x) * direction <= half || target.y <= pet.pos.y + 40.0 * scale {
                continue;
            }
            let dy = target.y - pet.pos.y;
            let hop_speed = HOP_SPEED * scale.sqrt();
            let vy = -hop_speed;
            let time = (hop_speed + (hop_speed.powi(2) + 2.0 * GRAVITY * dy).sqrt()) / GRAVITY;
            if !time.is_finite() || time <= 0.0 {
                continue;
            }
            let vx = (target.x - pet.pos.x) / time;
            if vx.abs() > MAX_HORIZONTAL_SPEED * scale.sqrt() {
                continue;
            }
            let velocity = Vec2::new(vx, vy);
            // On the descending crossing of the source height, clear its edge
            // by the full body width so ordinary platform collision can resume.
            let return_time = 2.0 * hop_speed / GRAVITY;
            let return_x = pet.pos.x + vx * return_time;
            let clears_source = if direction < 0.0 {
                return_x + half <= source.rect.x
            } else {
                return_x - half >= source.rect.right()
            };
            if !clears_source || !safe_arc(pet, world, source_index, area, velocity, time) {
                continue;
            }
            return Some(crate::exploration::JumpPlan {
                window_id: None,
                target,
                velocity,
            });
        }
    }
    None
}

fn display_scale(pet: &Pet) -> f64 {
    if pet.display_scale.is_finite() {
        pet.display_scale.clamp(0.75, 1.5)
    } else {
        1.0
    }
}

fn direction_order(roll: f64) -> [f64; 2] {
    if roll.is_finite() && roll.rem_euclid(1.0) >= 0.5 {
        [-1.0, 1.0]
    } else {
        [1.0, -1.0]
    }
}

fn covered_at(world: &World, index: usize, x: f64, y: f64) -> bool {
    world
        .windows
        .iter()
        .enumerate()
        .any(|(i, w)| i > index && w.rect.is_valid() && w.rect.contains(Vec2::new(x, y)))
}

fn supported_path_clear(
    world: &World,
    source_index: usize,
    source: Rect,
    from_x: f64,
    to_x: f64,
    scale: f64,
) -> bool {
    let half = HALF_WIDTH * scale;
    (0..=16).all(|step| {
        let q = step as f64 / 16.0;
        let x = from_x + (to_x - from_x) * q;
        x >= source.x + half
            && x <= source.right() - half
            && !covered_at(world, source_index, x, source.y + 1.0)
    })
}

fn distance(r: Rect, p: Vec2) -> f64 {
    (p.x - p.x.clamp(r.x, r.right())).hypot(p.y - p.y.clamp(r.y, r.bottom()))
}

fn safe_arc(
    pet: &Pet,
    world: &World,
    source_index: usize,
    area: Rect,
    velocity: Vec2,
    duration: f64,
) -> bool {
    let source = world.windows[source_index].rect;
    let start = pet.pos;
    let half = HALF_WIDTH * display_scale(pet);
    let top = BODY_TOP * display_scale(pet);
    let mut previous_y = start.y;
    for step in 1..=64 {
        let t = duration * step as f64 / 64.0;
        let x = start.x + velocity.x * t;
        let y = start.y + velocity.y * t + 0.5 * GRAVITY * t * t;
        let descending = velocity.y + GRAVITY * t > 0.0;
        let prev = previous_y;
        // The pet is drawn above every window, so it only collides with a
        // window lower in the stacking order by landing on its visible top
        // edge while falling. A large window behind the source (for example a
        // maximized app behind a floating one) is not an obstacle for the arc.
        let blocks = |i: usize, w: &crate::WindowInfo| {
            if i < source_index {
                descending
                    && prev <= w.rect.y
                    && y >= w.rect.y
                    && x + half > w.rect.x
                    && x - half < w.rect.right()
                    && !covered_at(world, i, x, w.rect.y - 1.0)
            } else {
                // Windows in front of the source keep the conservative body
                // test: the arc must not pass through them.
                x + half > w.rect.x
                    && x - half < w.rect.right()
                    && y > w.rect.y
                    && y - top < w.rect.bottom()
            }
        };
        previous_y = y;
        if x - half < area.x
            || x + half > area.right()
            || y > area.bottom() + 1e-6
            || y - top < area.y - 1e-6
        {
            return false;
        }
        if y >= source.y && x + half > source.x && x - half < source.right() {
            return false;
        }
        if world
            .windows
            .iter()
            .enumerate()
            .any(|(i, w)| i != source_index && w.rect.is_valid() && blocks(i, w))
        {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{Skin, State};
    use crate::WindowInfo;

    fn window(id: usize, rect: Rect) -> WindowInfo {
        WindowInfo {
            id,
            rect,
            title: String::new(),
            active: false,
            fullscreen: false,
            is_file_manager: false,
            is_desktop_icon: false,
        }
    }

    fn setup(x: f64, area: Rect, source: Rect) -> (Pet, World) {
        let mut pet = Pet::new(1, Skin::Claude, Vec2::new(x, source.y));
        pet.state = State::Idle;
        pet.surface_id = Some(7);
        let world = World {
            windows: vec![window(7, source)],
            work_areas: vec![area],
            ..World::default()
        };
        (pet, world)
    }

    #[test]
    fn approaches_from_center_then_plans_clear_floor_landing() {
        let (pet, world) = setup(
            250.0,
            Rect::new(0.0, 0.0, 1000.0, 900.0),
            Rect::new(100.0, 400.0, 300.0, 180.0),
        );
        let (x, plan) = descent_approach(&pet, &world, 0.0).unwrap();
        assert!(x > 340.0 && x < 400.0);
        assert!(plan.target.x > 400.0 && plan.target.y == 900.0);
        assert!(plan.velocity.y < 0.0 && plan.velocity.x > 0.0);
    }

    #[test]
    fn screen_wall_uses_safe_side_and_negative_coordinates() {
        let (pet, world) = setup(
            -850.0,
            Rect::new(-1000.0, 0.0, 1000.0, 900.0),
            Rect::new(-1000.0, 400.0, 300.0, 180.0),
        );
        let (_, plan) = descent_approach(&pet, &world, 0.9).unwrap();
        assert!(plan.target.x > -700.0 && plan.target.x < 0.0);
    }

    #[test]
    fn rejects_unsupported_covered_or_occluded_approach_path() {
        let (mut pet, mut world) = setup(
            250.0,
            Rect::new(0.0, 0.0, 1000.0, 900.0),
            Rect::new(100.0, 400.0, 300.0, 180.0),
        );
        pet.surface_id = None;
        assert!(descent_approach(&pet, &world, 0.0).is_none());
        pet.surface_id = Some(7);
        world
            .windows
            .push(window(8, Rect::new(140.0, 380.0, 50.0, 150.0)));
        world
            .windows
            .push(window(9, Rect::new(310.0, 380.0, 50.0, 150.0)));
        assert!(descent_approach(&pet, &world, 0.0).is_none());
    }

    #[test]
    fn rejects_floor_too_close_to_source() {
        let (pet, world) = setup(
            350.0,
            Rect::new(0.0, 0.0, 600.0, 430.0),
            Rect::new(100.0, 400.0, 300.0, 180.0),
        );
        assert!(descent_approach(&pet, &world, 0.0).is_none());
    }
}
