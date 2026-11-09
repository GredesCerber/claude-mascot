//! Geometry-only planning for autonomous jumps between desktop surfaces.
use crate::engine::{Pet, GRAVITY};
use crate::{Vec2, WindowInfo, World};

const X_POINTS: [f64; 5] = [0.15, 0.30, 0.50, 0.70, 0.85];
const MAX_LAUNCH_SPEED: f64 = 2050.0;
const MAX_HORIZONTAL_SPEED: f64 = 360.0;
const MAX_DISTANCE: f64 = 1500.0;
const HALF_WIDTH: f64 = 24.0;
const HEAD: f64 = 70.0;

#[derive(Clone, Copy, Debug)]
pub(crate) struct JumpPlan {
    /// Stable `WindowInfo.id`; `None` denotes a work-area floor.
    pub window_id: Option<usize>,
    pub target: Vec2,
    pub velocity: Vec2,
}

pub(crate) fn plan_jump(
    pet: &Pet,
    world: &World,
    window_roll: f64,
    point_roll: f64,
) -> Option<JumpPlan> {
    if world.fullscreen_active() {
        return None;
    }
    let scale = pet.display_scale.clamp(0.5, 2.0);
    let mut targets = Vec::new();
    for (i, w) in world.windows.iter().enumerate().take(256) {
        if !valid_window(w) || pet.surface_id == Some(w.id) || pet.surface == Some(i) {
            continue;
        }
        for f in X_POINTS {
            if w.is_desktop_icon && f != 0.5 {
                continue;
            }
            let x = if w.is_desktop_icon {
                w.rect.x + w.rect.w * 0.5
            } else {
                w.rect.x + w.rect.w * f
            };
            let p = Vec2::new(x, w.rect.y);
            if is_top_at(world, i, p.x)
                && target_valid(world, p, Some(w.id), scale)
                && reachable(pet.pos, p, scale)
                && clear_path(world, pet.pos, p, Some(w.id), scale)
            {
                targets.push((w.id, p));
            }
        }
    }
    let mut ids = Vec::new();
    for (id, _) in &targets {
        if !ids.contains(id) {
            ids.push(*id);
        }
    }
    if ids.is_empty() {
        return None;
    }
    let id = ids[(unit(window_roll) * ids.len() as f64) as usize];
    let points: Vec<Vec2> = targets
        .iter()
        .filter(|(wid, _)| *wid == id)
        .map(|(_, p)| *p)
        .collect();
    let point = points[(unit(point_roll) * points.len() as f64) as usize];
    jump_to(pet, world, Some(id), point)
}

pub(crate) fn jump_to(
    pet: &Pet,
    world: &World,
    window_id: Option<usize>,
    target: Vec2,
) -> Option<JumpPlan> {
    let scale = pet.display_scale.clamp(0.5, 2.0);
    if world.fullscreen_active()
        || !target_valid(world, target, window_id, scale)
        || !reachable(pet.pos, target, scale)
        || !clear_path(world, pet.pos, target, window_id, scale)
    {
        return None;
    }
    let (velocity, target) = ballistic(pet.pos, target, scale)?;
    Some(JumpPlan {
        window_id,
        target,
        velocity,
    })
}

fn unit(v: f64) -> f64 {
    if v.is_finite() {
        v.clamp(0.0, 1.0 - f64::EPSILON)
    } else {
        0.0
    }
}
fn valid_window(w: &WindowInfo) -> bool {
    w.rect.is_valid()
        && !w.fullscreen
        && if w.is_desktop_icon {
            w.rect.w >= 16.0 && w.rect.h >= 16.0
        } else {
            w.rect.w >= 80.0 && w.rect.h >= 30.0
        }
}
fn is_top_at(world: &World, i: usize, x: f64) -> bool {
    let y = world.windows[i].rect.y;
    !world
        .windows
        .iter()
        .enumerate()
        .skip(i + 1)
        .any(|(_, w)| w.rect.is_valid() && w.rect.contains(Vec2::new(x, y + 1.0)))
}
fn target_valid(world: &World, p: Vec2, id: Option<usize>, scale: f64) -> bool {
    if !p.x.is_finite() || !p.y.is_finite() {
        return false;
    }
    let area_ok = world.work_areas.iter().any(|a| {
        a.is_valid()
            && p.x >= a.x + HALF_WIDTH * scale
            && p.x <= a.right() - HALF_WIDTH * scale
            && p.y >= a.y + HEAD * scale
            && p.y <= a.bottom()
    });
    if !area_ok {
        return false;
    }
    if id.is_none()
        && !world.work_areas.iter().any(|a| {
            a.is_valid()
                && (p.y - a.bottom()).abs() <= 0.01
                && p.x >= a.x + HALF_WIDTH * scale
                && p.x <= a.right() - HALF_WIDTH * scale
        })
    {
        return false;
    }
    if let Some(id) = id {
        let Some(w) = world.windows.iter().find(|w| w.id == id) else {
            return false;
        };
        if !valid_window(w) || (p.y - w.rect.y).abs() > 0.01 {
            return false;
        }
        let inset = if w.is_desktop_icon {
            0.0
        } else {
            HALF_WIDTH * scale
        };
        let Some(index) = world.windows.iter().position(|a| a.id == id) else {
            return false;
        };
        if p.x < w.rect.x + inset || p.x > w.rect.right() - inset || !is_top_at(world, index, p.x) {
            return false;
        }
    }
    true
}
fn reachable(a: Vec2, b: Vec2, scale: f64) -> bool {
    (a.x - b.x).hypot(a.y - b.y) <= MAX_DISTANCE * scale
}

// Aim for a scaled head-clearance apex. Solve the descending root at target height.
fn trajectory(a: Vec2, b: Vec2, scale: f64) -> Option<(f64, f64, f64)> {
    let rise = (a.y - b.y).max(0.0) + HEAD * scale;
    let vy = -(2.0 * GRAVITY * rise).sqrt();
    if -vy > MAX_LAUNCH_SPEED {
        return None;
    }
    let disc = vy * vy + 2.0 * GRAVITY * (b.y - a.y);
    if disc < 0.0 {
        return None;
    }
    let t = (-vy + disc.sqrt()) / GRAVITY;
    if t <= 0.0 {
        return None;
    }
    let vx = (b.x - a.x) / t;
    if vx.abs() > MAX_HORIZONTAL_SPEED * scale.sqrt() {
        return None;
    }
    Some((vx, vy, t))
}
fn ballistic(a: Vec2, b: Vec2, scale: f64) -> Option<(Vec2, Vec2)> {
    let (vx, vy, _) = trajectory(a, b, scale)?;
    Some((Vec2::new(vx, vy), b))
}

fn clear_path(world: &World, a: Vec2, b: Vec2, target_id: Option<usize>, scale: f64) -> bool {
    let Some((vx, vy, duration)) = trajectory(a, b, scale) else {
        return false;
    };
    // Monitor/headroom sampling is independent of the number of windows.
    for step in 1..=96 {
        let time = duration * step as f64 / 96.0;
        let x = a.x + vx * time;
        let y = a.y + vy * time + 0.5 * GRAVITY * time * time;
        if !world.work_areas.iter().any(|r| {
            r.is_valid()
                && x - HALF_WIDTH * scale >= r.x - 1e-6
                && x + HALF_WIDTH * scale <= r.right() + 1e-6
                && y - HEAD * scale >= r.y - 1e-6
                && y <= r.bottom() + 1e-6
        }) {
            return false;
        }
    }
    // Solve each foreign platform crossing once, rather than N checks per sample.
    for (index, w) in world.windows.iter().enumerate() {
        if !w.rect.is_valid() || Some(w.id) == target_id {
            continue;
        }
        let discriminant = vy * vy + 2.0 * GRAVITY * (w.rect.y - a.y);
        if discriminant < 0.0 {
            continue;
        }
        let time = (-vy + discriminant.sqrt()) / GRAVITY;
        if time <= 1e-6 || time >= duration - 1e-6 {
            continue;
        }
        let x = a.x + vx * time;
        if x + HALF_WIDTH * scale > w.rect.x
            && x - HALF_WIDTH * scale < w.rect.right()
            && !crate::engine::covered(world, Vec2::new(x, w.rect.y - 1.0), index)
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
    use crate::Rect;
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
    fn pet(pos: Vec2) -> Pet {
        let mut p = Pet::new(1, Skin::Claude, pos);
        p.state = State::Idle;
        p.surface = None;
        p.surface_id = None;
        p
    }
    #[test]
    fn choices_vary_by_stable_id_and_point() {
        let world = World {
            windows: vec![
                window(91, Rect::new(200.0, 500.0, 300.0, 200.0)),
                window(45, Rect::new(600.0, 500.0, 300.0, 200.0)),
            ],
            ..World::default()
        };
        let p = pet(Vec2::new(500.0, 900.0));
        let a = plan_jump(&p, &world, 0.0, 0.0).unwrap();
        let b = plan_jump(&p, &world, 0.8, 0.8).unwrap();
        assert_eq!(a.window_id, Some(91));
        assert_eq!(b.window_id, Some(45));
        assert_ne!(a.target.x, b.target.x);
        let reordered = World {
            windows: vec![world.windows[1].clone(), world.windows[0].clone()],
            ..world.clone()
        };
        let j = jump_to(&p, &reordered, Some(91), a.target).unwrap();
        assert_eq!(j.window_id, Some(91));
    }
    #[test]
    fn trajectory_reaches_target_while_descending() {
        // Exercise several rises and horizontal offsets through the actual planner.
        for start_y in [800.0, 900.0, 1100.0] {
            for target_y in [400.0, 500.0, 600.0] {
                let p = pet(Vec2::new(500.0, start_y));
                let w = World {
                    windows: vec![window(7, Rect::new(250.0, target_y, 500.0, 200.0))],
                    work_areas: vec![Rect::new(0.0, 0.0, 1920.0, 1200.0)],
                    ..World::default()
                };
                let j = jump_to(&p, &w, Some(7), Vec2::new(450.0, target_y)).unwrap();
                let disc = j.velocity.y * j.velocity.y + 2.0 * GRAVITY * (j.target.y - p.pos.y);
                let t = (-j.velocity.y + disc.sqrt()) / GRAVITY;
                let mut last_y = p.pos.y;
                for step in 1..=96 {
                    let time = t * step as f64 / 96.0;
                    let y = p.pos.y + j.velocity.y * time + 0.5 * GRAVITY * time * time;
                    if step == 96 {
                        assert!((y - j.target.y).abs() < 0.01);
                    }
                    last_y = y;
                }
                assert!(j.velocity.y + GRAVITY * t > 0.0);
                assert!(last_y > p.pos.y - 2000.0);
            }
        }
    }
    #[test]
    fn topmost_partial_cover_and_icon_center() {
        let mut w = World {
            windows: vec![
                window(1, Rect::new(200.0, 500.0, 300.0, 200.0)),
                window(2, Rect::new(300.0, 500.0, 100.0, 100.0)),
            ],
            ..World::default()
        };
        let p = pet(Vec2::new(500.0, 900.0));
        assert!(jump_to(&p, &w, Some(1), Vec2::new(250.0, 500.0)).is_some());
        assert!(jump_to(&p, &w, Some(1), Vec2::new(350.0, 500.0)).is_none());
        let mut icon = window(3, Rect::new(-500.0, 400.0, 40.0, 60.0));
        icon.is_desktop_icon = true;
        w.work_areas = vec![Rect::new(-1280.0, 0.0, 1280.0, 1024.0)];
        w.windows = vec![icon];
        let j = jump_to(
            &pet(Vec2::new(-600.0, 800.0)),
            &w,
            Some(3),
            Vec2::new(-480.0, 400.0),
        )
        .unwrap();
        assert_eq!(j.target.x, -480.0);
    }
    #[test]
    fn fullscreen_unreachable_and_floor_are_checked() {
        let p = pet(Vec2::new(500.0, 500.0));
        let w = World::default();
        assert!(jump_to(&p, &w, None, Vec2::new(500.0, 900.0)).is_none());
        let floor = jump_to(
            &pet(Vec2::new(500.0, 500.0)),
            &w,
            None,
            Vec2::new(500.0, 1040.0),
        );
        assert!(floor.is_some());
        assert!(jump_to(&p, &w, None, Vec2::new(1800.0, 1040.0)).is_none());
        let mut fw = w;
        let mut f = window(1, Rect::new(0.0, 0.0, 1920.0, 1040.0));
        f.active = true;
        f.fullscreen = true;
        fw.windows.push(f);
        assert!(plan_jump(&p, &fw, 0.0, 0.0).is_none());
    }
}
