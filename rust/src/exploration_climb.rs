//! Safe floor-to-window climbing route planning.

use crate::engine::Pet;
use crate::{Rect, Vec2, WindowInfo, World};

const MAX_APPROACH: f64 = 1_500.0;
const BODY_HALF: f64 = 24.0;
/// Climbing hands hold a side edge while the feet are this far below it.
pub(crate) const GRIP_REACH: f64 = 40.0;
/// Extra rise above the grip height so a hop reliably reaches the edge.
pub(crate) const HOP_MARGIN: f64 = 18.0;
/// Highest standing hop used to grab a floating window's side (per scale).
const MAX_GRAB_HOP: f64 = 480.0;

fn pet_scale(pet: &Pet) -> f64 {
    if pet.display_scale.is_finite() {
        pet.display_scale.clamp(0.5, 2.0)
    } else {
        1.0
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ClimbPlan {
    pub window_id: usize,
    /// -1 means the left edge; +1 means the right edge.
    pub side: f64,
    pub approach_x: f64,
    /// Standing hop (feet rise in px) needed before the hands reach the side
    /// edge of a floating window. Zero when the window reaches the floor.
    pub hop: f64,
}

/// Feet rise needed to grab the side of `w` from `floor`, or `None` when the
/// edge is out of a standing hop's reach (or hangs below the floor).
fn grab_hop(floor: f64, w: &WindowInfo, scale: f64) -> Option<f64> {
    let reach = GRIP_REACH * scale;
    if (w.rect.bottom() - floor).abs() <= reach {
        return Some(0.0);
    }
    if w.rect.bottom() > floor {
        return None;
    }
    let hop = floor - (w.rect.bottom() + reach) + HOP_MARGIN * scale;
    (hop <= MAX_GRAB_HOP * scale).then_some(hop)
}

pub(crate) fn plan_climb(pet: &Pet, world: &World, roll: f64) -> Option<ClimbPlan> {
    if !roll.is_finite() || !on_floor(pet, world) {
        return None;
    }
    let floor = world.area_at(pet.pos).bottom();
    let scale = pet_scale(pet);
    let mut candidates = Vec::new();
    for w in &world.windows {
        if !eligible(w) || floor - w.rect.y < 200.0 * scale {
            continue;
        }
        let Some(hop) = grab_hop(floor, w, scale) else {
            continue;
        };
        for side in [-1.0, 1.0] {
            if let Some(x) = approach_x(pet, world, w, side) {
                candidates.push((w.id, side, x, (x - pet.pos.x).abs(), hop));
            }
        }
    }
    candidates.sort_by(|a, b| a.3.total_cmp(&b.3));
    let pick = ((roll.clamp(0.0, 1.0) * candidates.len() as f64) as usize)
        .min(candidates.len().saturating_sub(1));
    candidates
        .get(pick)
        .map(|(window_id, side, approach_x, _, hop)| ClimbPlan {
            window_id: *window_id,
            side: *side,
            approach_x: *approach_x,
            hop: *hop,
        })
}

pub(crate) fn refresh_climb(pet: &Pet, world: &World, plan: ClimbPlan) -> Option<ClimbPlan> {
    let w = world.windows.iter().find(|w| w.id == plan.window_id)?;
    if !on_floor(pet, world) || !eligible(w) {
        return None;
    }
    let floor = world.area_at(pet.pos).bottom();
    let scale = pet_scale(pet);
    if floor - w.rect.y < 200.0 * scale {
        return None;
    }
    let hop = grab_hop(floor, w, scale)?;
    let approach_x = approach_x(pet, world, w, plan.side)?;
    Some(ClimbPlan {
        approach_x,
        hop,
        ..plan
    })
}

/// True only at the physical side attachment point; caller may then enter Climb.
pub(crate) fn can_attach(pet: &Pet, world: &World, plan: ClimbPlan) -> bool {
    let Some(w) = world.windows.iter().find(|w| w.id == plan.window_id) else {
        return false;
    };
    let floor = world.area_at(pet.pos).bottom();
    (pet.pos.x - plan.approach_x).abs() <= 3.0
        && (pet.pos.y - floor).abs() <= 3.0
        && (pet.pos.x
            - (if plan.side < 0.0 {
                w.rect.x - BODY_HALF * pet_scale(pet)
            } else {
                w.rect.right() + BODY_HALF * pet_scale(pet)
            }))
        .abs()
            <= 3.0
        && !covered(
            world,
            Vec2::new(pet.pos.x, floor - 40.0 * pet_scale(pet)),
            w,
        )
}

fn eligible(w: &WindowInfo) -> bool {
    valid(w.rect) && !w.is_desktop_icon && !w.fullscreen && w.rect.w >= 80.0 && w.rect.h >= 60.0
}
fn approach_x(pet: &Pet, world: &World, w: &WindowInfo, side: f64) -> Option<f64> {
    let scale = pet_scale(pet);
    let x = if side < 0.0 {
        w.rect.x - BODY_HALF * scale
    } else {
        w.rect.right() + BODY_HALF * scale
    };
    if (x - pet.pos.x).abs() > MAX_APPROACH * scale {
        return None;
    }
    let area = floor_area(pet, world)?;
    let landing_x = if side < 0.0 {
        w.rect.x + BODY_HALF * scale + 1.0
    } else {
        w.rect.right() - BODY_HALF * scale - 1.0
    };
    if w.rect.y < area.y + 55.0 * scale + 8.0
        || landing_x - BODY_HALF * scale < area.x
        || landing_x + BODY_HALF * scale > area.right()
        || covered(world, Vec2::new(landing_x, w.rect.y + 1.0), w)
    {
        return None;
    }
    // The pet's full horizontal body must remain within this one work area.
    if pet.pos.x.min(x) - BODY_HALF * scale < area.x
        || pet.pos.x.max(x) + BODY_HALF * scale > area.right()
    {
        return None;
    }
    let edge = Vec2::new(
        if side < 0.0 {
            w.rect.x - 3.0 * scale
        } else {
            w.rect.right() + 3.0 * scale
        },
        area.bottom() - 40.0 * scale,
    );
    if covered(world, edge, w) {
        return None;
    }
    let lo = pet.pos.x.min(x);
    let hi = pet.pos.x.max(x);
    let target_index = world.windows.iter().position(|window| window.id == w.id)?;
    let grip_x = if side < 0.0 {
        w.rect.x - 3.0 * scale
    } else {
        w.rect.right() + 3.0 * scale
    };
    if world.windows.iter().skip(target_index + 1).any(|other| {
        valid(other.rect)
            && grip_x >= other.rect.x
            && grip_x <= other.rect.right()
            && other.rect.bottom() > w.rect.y
            && other.rect.y < w.rect.bottom().min(area.bottom())
    }) {
        return None;
    }
    if world.windows.iter().skip(target_index + 1).any(|other| {
        valid(other.rect)
            && other.rect.y < area.bottom()
            && other.rect.bottom() > area.bottom() - 48.0 * scale
            && other.rect.x < hi
            && other.rect.right() > lo
    }) {
        return None;
    }
    Some(x)
}
fn on_floor(pet: &Pet, world: &World) -> bool {
    pet.surface.is_none()
        && pet.surface_id.is_none()
        && floor_area(pet, world).is_some_and(|area| (pet.pos.y - area.bottom()).abs() <= 3.0)
}
fn floor_area(pet: &Pet, world: &World) -> Option<Rect> {
    world.work_areas.iter().copied().find(|area| {
        valid(*area)
            && pet.pos.x >= area.x
            && pet.pos.x <= area.right()
            && (pet.pos.y - area.bottom()).abs() <= 3.0
    })
}
fn covered(world: &World, p: Vec2, target: &WindowInfo) -> bool {
    let Some(i) = world.windows.iter().position(|w| w.id == target.id) else {
        return true;
    };
    world
        .windows
        .iter()
        .skip(i + 1)
        .any(|w| valid(w.rect) && w.rect.contains(p))
}
fn valid(r: Rect) -> bool {
    r.is_valid()
        && r.w < 10_000_000.0
        && r.h < 10_000_000.0
        && r.right().abs() < 10_000_000.0
        && r.bottom().abs() < 10_000_000.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{Skin, State};
    fn win(id: usize, r: Rect) -> WindowInfo {
        WindowInfo {
            id,
            rect: r,
            title: String::new(),
            active: false,
            fullscreen: false,
            is_file_manager: false,
            is_desktop_icon: false,
        }
    }
    fn setup() -> (Pet, World) {
        let mut p = Pet::new(1, Skin::Claude, Vec2::new(300.0, 1000.0));
        p.state = State::Idle;
        let w = World {
            work_areas: vec![Rect::new(0., 0., 1200., 1000.)],
            windows: vec![win(4, Rect::new(500., 400., 300., 600.))],
            ..World::default()
        };
        (p, w)
    }
    #[test]
    fn selects_safe_edge_and_requires_approach_probability() {
        let (p, w) = setup();
        let plan = plan_climb(&p, &w, 0.2).unwrap();
        assert_eq!(plan.side, -1.);
        assert!(plan_climb(&p, &w, 0.8).is_some());
    }
    #[test]
    fn refresh_tracks_move_resize_and_rejects_removed_or_covered() {
        let (p, mut w) = setup();
        let plan = plan_climb(&p, &w, 0.).unwrap();
        w.windows[0].rect.x += 80.;
        let moved = refresh_climb(&p, &w, plan).unwrap();
        assert_ne!(moved.approach_x, plan.approach_x);
        w.windows.clear();
        assert!(refresh_climb(&p, &w, plan).is_none());
    }
    #[test]
    fn rejects_icons_fullscreen_occlusion_and_long_or_wrong_monitor_routes() {
        let (p, mut w) = setup();
        w.windows[0].is_desktop_icon = true;
        assert!(plan_climb(&p, &w, 0.).is_none());
        w.windows[0] = win(4, Rect::new(500., 400., 300., 600.));
        w.windows[0].fullscreen = true;
        assert!(plan_climb(&p, &w, 0.).is_none());
        w.windows[0].fullscreen = false;
        w.windows.push(win(5, Rect::new(400., 850., 250., 150.)));
        assert!(plan_climb(&p, &w, 0.).is_none());
    }
    #[test]
    fn scaled_geometry_and_attachment_are_consistent() {
        let (mut p, w) = setup();
        p.display_scale = 1.5;
        let plan = plan_climb(&p, &w, 0.).unwrap();
        p.pos.x = plan.approach_x;
        assert!(can_attach(&p, &w, plan));
    }
    #[test]
    fn rejects_airborne_pet_and_routes_outside_workarea() {
        let (mut p, w) = setup();
        p.pos.y -= 20.0;
        assert!(plan_climb(&p, &w, 0.0).is_none());
        p.pos.y += 20.0;
        p.pos.x = 1190.0;
        assert!(plan_climb(&p, &w, 0.0).is_none());
    }
    #[test]
    fn route_crossing_a_monitor_gap_is_rejected() {
        let (mut p, mut w) = setup();
        w.work_areas = vec![
            Rect::new(0.0, 0.0, 400.0, 1000.0),
            Rect::new(500.0, 0.0, 700.0, 1000.0),
        ];
        p.pos.x = 300.0;
        assert!(plan_climb(&p, &w, 0.0).is_none());
    }
}
