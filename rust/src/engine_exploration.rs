//! Runtime ownership of optional exploration: approach, prepare, travel, arrive.
use super::{covered, Pet, Rect, State, Vec2, World, BODY_HALF_WIDTH, WALK_SPEED};
use crate::exploration::{self, JumpPlan};
use crate::exploration_climb::{self, ClimbPlan};
use crate::exploration_descent;
use std::hash::{Hash, Hasher};

#[derive(Clone, Copy, Debug)]
enum Phase {
    ApproachClimb(ClimbPlan),
    ApproachDescent { x: f64, plan: JumpPlan },
    Crouch(JumpPlan),
    Flight(JumpPlan),
    Climbing(ClimbPlan),
    Rollover(ClimbPlan),
}
#[derive(Clone, Debug)]
pub(super) struct ExploreRoute {
    phase: Phase,
    age: f64,
    prepared: f64,
    source: Option<usize>,
    destination: Option<Rect>,
    scene: u64,
    scale: f64,
}

impl Pet {
    pub(super) fn clear_exploration(&mut self) {
        if self.exploration_route.take().is_none() {
            return;
        }
        self.target = None;
        if matches!(self.state, State::Approach | State::Crouch) {
            self.set_state(State::Idle, 0.0);
        } else if self.state == State::Climb {
            self.surface = None;
            self.surface_id = None;
            self.surface_prev = None;
            self.vel = Vec2::default();
            self.set_state(State::Fall, 0.0);
        }
    }

    fn abort_exploration(&mut self, world: &World) {
        self.exploration_route = None;
        self.target = None;
        self.explore_cooldown = self.explore_cooldown.max(3.0);
        if matches!(
            self.state,
            State::Approach | State::Crouch | State::Climb | State::Ledge
        ) {
            if self.has_support(world) {
                self.vel = Vec2::default();
                self.set_state(State::Idle, 1.5);
            } else {
                self.surface = None;
                self.surface_id = None;
                self.surface_prev = None;
                self.vel.y = self.vel.y.max(0.0);
                self.set_state(State::Fall, 0.0);
            }
        }
    }

    pub(super) fn try_start_exploration(&mut self, world: &World) -> bool {
        if !self.display_scale.is_finite()
            || self.exploration_route.is_some()
            || self.explore_cooldown > 0.0
            || self.behavior_manual_lock > 0.0
            || self.behavior_velocity_ttl > 0.0
            || !self.auto_activities
            || self.activity_intensity == 0
            || self.dragging
            || self.chase
            || self.social.is_some()
            || self.social_motion.is_some()
            || self.peek_goal.is_some()
            || self.state != State::Idle
            || world.fullscreen_active()
            || !self.has_support(world)
        {
            return false;
        }
        let choice = self.random();
        if choice < 0.17 {
            if self.random() < 0.6 {
                let roll = self.random();
                if let Some(plan) = exploration_climb::plan_climb(self, world, roll) {
                    if self.begin_climb_route(plan, world) {
                        return true;
                    }
                }
            }
            if self.try_start_window_jump(world) {
                return true;
            }
        }
        if choice < 0.25 && self.current_surface_index(world).is_some() {
            let roll = self.random();
            if let Some(plan) = exploration_descent::plan_descent(self, world, roll) {
                self.begin_exploration_jump(plan, world);
                return true;
            }
            if let Some((x, plan)) = exploration_descent::descent_approach(self, world, roll) {
                let mut route = self.jump_route(plan, world);
                route.phase = Phase::ApproachDescent { x, plan };
                self.exploration_route = Some(route);
                self.target = plan.window_id.map(|id| id as u64);
                self.behavior_velocity = None;
                self.behavior_velocity_ttl = 0.0;
                self.set_state(State::Approach, 0.0);
                self.explore_cooldown = 12.0;
                return true;
            }
        }
        false
    }

    pub(super) fn try_start_window_jump(&mut self, world: &World) -> bool {
        if !self.has_support(world) {
            return false;
        }
        let window_roll = self.random();
        let point_roll = self.random();
        let Some(plan) = exploration::plan_jump(self, world, window_roll, point_roll) else {
            return false;
        };
        self.begin_exploration_jump(plan, world);
        true
    }

    fn jump_route(&self, plan: JumpPlan, world: &World) -> ExploreRoute {
        ExploreRoute {
            phase: Phase::Crouch(plan),
            age: 0.0,
            prepared: 0.0,
            source: self
                .current_surface_index(world)
                .map(|i| world.windows[i].id),
            destination: plan
                .window_id
                .and_then(|id| world.windows.iter().find(|w| w.id == id).map(|w| w.rect)),
            scene: scene_hash(world),
            scale: self.display_scale,
        }
    }

    fn begin_exploration_jump(&mut self, plan: JumpPlan, world: &World) {
        self.exploration_route = Some(self.jump_route(plan, world));
        self.target = plan.window_id.map(|id| id as u64);
        self.behavior_velocity = None;
        self.behavior_velocity_ttl = 0.0;
        self.vel = Vec2::default();
        self.facing = if plan.velocity.x < 0.0 { -1.0 } else { 1.0 };
        self.set_state(State::Crouch, 0.22);
        self.explore_cooldown = 12.0 + self.random() * 6.0;
    }

    fn begin_climb_route(&mut self, plan: ClimbPlan, world: &World) -> bool {
        let Some(index) = world.windows.iter().position(|w| w.id == plan.window_id) else {
            return false;
        };
        let rect = world.windows[index].rect;
        let area = world.area_at(self.pos);
        let scale = self.display_scale.clamp(0.5, 2.0);
        let edge_x = if plan.side < 0.0 {
            rect.x
        } else {
            rect.right()
        };
        if rect.y < area.y + self.head_clearance()
            || (0..=32).any(|step| {
                let y = rect.y + 1.0 + (self.pos.y - rect.y - 36.0 * scale) * step as f64 / 32.0;
                covered(world, Vec2::new(edge_x + plan.side * 3.0 * scale, y), index)
            })
        {
            return false;
        }
        self.exploration_route = Some(ExploreRoute {
            phase: Phase::ApproachClimb(plan),
            age: 0.0,
            prepared: 0.0,
            source: None,
            destination: None,
            scene: scene_hash(world),
            scale: self.display_scale,
        });
        self.target = Some(plan.window_id as u64);
        self.behavior_velocity = None;
        self.behavior_velocity_ttl = 0.0;
        self.vel = Vec2::default();
        self.set_state(State::Approach, 0.0);
        self.explore_cooldown = 18.0;
        true
    }

    /// True when the route owns this tick; flight and climbing use normal physics.
    pub(super) fn advance_exploration(&mut self, dt: f64, world: &World) -> bool {
        let Some(mut route) = self.exploration_route.take() else {
            return false;
        };
        route.age += dt;
        if !self.display_scale.is_finite()
            || (self.display_scale - route.scale).abs() > 1e-6
            || route.age > 45.0
            || !self.auto_activities
            || self.activity_intensity == 0
            || self.dragging
            || self.chase
            || self.social.is_some()
            || self.social_motion.is_some()
            || world.fullscreen_active()
        {
            self.abort_exploration(world);
            return false;
        }
        match route.phase {
            Phase::ApproachClimb(plan) => {
                if self.state != State::Approach {
                    self.abort_exploration(world);
                    return false;
                }
                let Some(plan) = exploration_climb::refresh_climb(self, world, plan) else {
                    self.abort_exploration(world);
                    return false;
                };
                self.walk_route_toward(plan.approach_x, dt);
                if (self.pos.x - plan.approach_x).abs() < 0.01
                    && exploration_climb::can_attach(self, world, plan)
                {
                    let Some(i) = world.windows.iter().position(|w| w.id == plan.window_id) else {
                        self.abort_exploration(world);
                        return false;
                    };
                    // Attach at the current feet height: no vertical or horizontal teleport.
                    if self.pos.y
                        > world.windows[i].rect.bottom()
                            + 40.0 * self.display_scale.clamp(0.5, 2.0)
                            + 1e-6
                    {
                        self.abort_exploration(world);
                        return false;
                    }
                    self.surface = Some(i);
                    self.surface_id = Some(plan.window_id);
                    self.surface_prev = Some(world.windows[i].rect);
                    self.facing = plan.side;
                    self.set_state(State::Climb, 0.0);
                    route.phase = Phase::Climbing(plan);
                } else {
                    route.phase = Phase::ApproachClimb(plan);
                }
                self.exploration_route = Some(route);
                true
            }
            Phase::ApproachDescent { x, plan } => {
                if self.state != State::Approach
                    || !self.source_supported(&route, world)
                    || scene_hash(world) != route.scene
                {
                    self.abort_exploration(world);
                    return false;
                }
                self.walk_route_toward(x, dt);
                if (self.pos.x - x).abs() < 0.1 {
                    self.vel = Vec2::default();
                    self.facing = plan.velocity.x.signum();
                    self.set_state(State::Crouch, 0.22);
                    route.phase = Phase::Crouch(plan);
                    route.prepared = 0.0;
                }
                self.exploration_route = Some(route);
                true
            }
            Phase::Crouch(plan) => {
                if self.state != State::Crouch
                    || !self.source_supported(&route, world)
                    || !destination_exists(plan, route.destination, world)
                    || scene_hash(world) != route.scene
                {
                    self.abort_exploration(world);
                    return false;
                }
                if let Some(i) = self.current_surface_index(world) {
                    self.follow_surface(i, world);
                }
                route.prepared += dt;
                if route.prepared + 1e-9 >= 0.22 {
                    self.surface = None;
                    self.surface_id = None;
                    self.surface_prev = None;
                    self.vel = plan.velocity;
                    self.set_state(State::Jump, 0.0);
                    route.phase = Phase::Flight(plan);
                }
                self.exploration_route = Some(route);
                true
            }
            Phase::Flight(plan) => {
                if !matches!(self.state, State::Jump | State::Fall)
                    || !destination_exists(plan, route.destination, world)
                {
                    // Real landing/collision or target changes end ownership. Retain momentum.
                    self.exploration_route = None;
                    self.target = None;
                    return false;
                }
                self.exploration_route = Some(route);
                false
            }
            Phase::Climbing(plan) | Phase::Rollover(plan) => {
                let Some(i) = world.windows.iter().position(|w| w.id == plan.window_id) else {
                    self.abort_exploration(world);
                    return false;
                };
                let rect = world.windows[i].rect;
                let scale = self.display_scale.clamp(0.5, 2.0);
                let edge = if plan.side < 0.0 {
                    rect.x
                } else {
                    rect.right()
                };
                let edge_point = Vec2::new(
                    edge + plan.side * 3.0 * scale,
                    (self.pos.y - 35.0 * scale).max(rect.y + 1.0),
                );
                let area = world.area_at(self.pos);
                if !rect.is_valid()
                    || world.windows[i].fullscreen
                    || rect.y < area.y + self.head_clearance()
                    || covered(world, edge_point, i)
                    || self.pos.x - BODY_HALF_WIDTH * scale < area.x
                    || self.pos.x + BODY_HALF_WIDTH * scale > area.right()
                    || !matches!(self.state, State::Climb | State::Ledge | State::Approach)
                {
                    self.abort_exploration(world);
                    return false;
                }
                if self.state == State::Climb {
                    self.exploration_route = Some(route);
                    return false;
                }
                if (self.pos.y - rect.y).abs() > 3.0 {
                    self.abort_exploration(world);
                    return false;
                }
                let x = if plan.side < 0.0 {
                    rect.x + BODY_HALF_WIDTH * scale + 1.0
                } else {
                    rect.right() - BODY_HALF_WIDTH * scale - 1.0
                };
                self.set_state(State::Approach, 0.0);
                self.walk_route_toward(x, dt);
                if (self.pos.x - x).abs() < 0.1 {
                    self.vel = Vec2::default();
                    self.target = None;
                    self.set_state(State::Ledge, 0.45);
                    self.exploration_route = None;
                } else {
                    route.phase = Phase::Rollover(plan);
                    self.exploration_route = Some(route);
                }
                true
            }
        }
    }
    fn source_supported(&self, route: &ExploreRoute, world: &World) -> bool {
        self.has_support(world)
            && self
                .current_surface_index(world)
                .map(|i| world.windows[i].id)
                == route.source
    }
    fn walk_route_toward(&mut self, x: f64, dt: f64) {
        let dx = x - self.pos.x;
        if dx.abs() > 0.01 {
            self.facing = dx.signum();
        }
        self.pos.x += dx.clamp(-WALK_SPEED * dt, WALK_SPEED * dt);
        self.vel = Vec2::default();
    }
}
fn destination_exists(plan: JumpPlan, expected: Option<Rect>, world: &World) -> bool {
    if let Some(id) = plan.window_id {
        world
            .windows
            .iter()
            .any(|w| w.id == id && Some(w.rect) == expected && !w.fullscreen)
    } else {
        world.work_areas.iter().any(|a| {
            a.is_valid()
                && (a.bottom() - plan.target.y).abs() < 0.01
                && plan.target.x >= a.x
                && plan.target.x <= a.right()
        })
    }
}
fn scene_hash(world: &World) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for w in &world.windows {
        w.id.hash(&mut h);
        w.fullscreen.hash(&mut h);
        w.is_desktop_icon.hash(&mut h);
        for x in [w.rect.x, w.rect.y, w.rect.w, w.rect.h] {
            x.to_bits().hash(&mut h);
        }
    }
    for a in &world.work_areas {
        for x in [a.x, a.y, a.w, a.h] {
            x.to_bits().hash(&mut h);
        }
    }
    h.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{Command, Skin};
    use crate::WindowInfo;
    const DT: f64 = 1.0 / 240.0;
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
    fn world() -> World {
        World {
            work_areas: vec![Rect::new(0.0, 0.0, 1400.0, 1000.0)],
            windows: vec![window(77, Rect::new(500.0, 400.0, 400.0, 600.0))],
            ..World::default()
        }
    }
    fn pet(pos: Vec2) -> Pet {
        let mut p = Pet::new(19, Skin::Claude, pos);
        p.state = State::Idle;
        p.auto_reactions = false;
        p.auto_activity_cooldown = 1e6;
        p
    }
    #[test]
    fn prepared_jump_holds_pose_then_reaches_actual_window() {
        let w = world();
        let mut p = pet(Vec2::new(430.0, 1000.0));
        assert!(p.try_start_window_jump(&w));
        let before = p.pos;
        for _ in 0..50 {
            p.update(DT, &w);
            assert_eq!(p.state, State::Crouch);
            assert_eq!(p.pos, before);
        }
        for _ in 0..4 {
            p.update(DT, &w);
        }
        assert_eq!(p.state, State::Jump);
        assert!(p.vel.y < 0.0);
        let mut landed = false;
        for _ in 0..600 {
            p.update(DT, &w);
            if p.state == State::Land {
                landed = true;
                break;
            }
        }
        assert!(landed);
        assert_eq!(p.surface_id, Some(77));
        assert!((p.pos.y - 400.0).abs() < 0.1);
    }
    #[test]
    fn moved_or_closed_destination_cancels_preparation_on_current_support() {
        for remove in [true, false] {
            let mut w = world();
            let mut p = pet(Vec2::new(430.0, 1000.0));
            assert!(p.try_start_window_jump(&w));
            if remove {
                w.windows.clear();
            } else {
                w.windows[0].rect.x += 10.0;
            }
            p.update(DT, &w);
            assert!(p.exploration_route.is_none());
            assert_eq!(p.state, State::Idle);
            assert_eq!(p.pos.y, 1000.0);
            assert_eq!(p.vel, Vec2::default());
        }
    }
    #[test]
    fn lost_support_during_crouch_falls_from_real_position() {
        let mut w = world();
        let mut p = pet(Vec2::new(650.0, 400.0));
        p.surface = Some(0);
        p.surface_id = Some(77);
        p.surface_prev = Some(w.windows[0].rect);
        let (_, plan) = exploration_descent::descent_approach(&p, &w, 0.0).unwrap();
        p.begin_exploration_jump(plan, &w);
        w.windows.clear();
        p.update(DT, &w);
        assert_eq!(p.state, State::Fall);
        assert!(p.pos.y < 405.0);
        assert!(p.exploration_route.is_none());
    }
    #[test]
    fn drag_and_manual_commands_cancel_pending_routes() {
        let w = world();
        for cmd in [Command::Sit, Command::Sleep] {
            let mut p = pet(Vec2::new(430.0, 1000.0));
            assert!(p.try_start_window_jump(&w));
            p.command(cmd, &w);
            for _ in 0..100 {
                p.update(DT, &w);
            }
            assert!(p.exploration_route.is_none());
            assert_eq!(
                p.state,
                if cmd == Command::Sit {
                    State::Sit
                } else {
                    State::Sleep
                }
            );
        }
        let mut p = pet(Vec2::new(430.0, 1000.0));
        assert!(p.try_start_window_jump(&w));
        p.drag_to(Vec2::new(700.0, 700.0));
        p.update(DT, &w);
        assert!(p.exploration_route.is_none());
        assert_eq!(p.pos, Vec2::new(700.0, 700.0));
    }
    #[test]
    fn changed_settings_cancel_preparation_instead_of_launching_a_stale_arc() {
        for condition in 0..4 {
            let mut w = world();
            let mut p = pet(Vec2::new(430.0, 1000.0));
            assert!(p.try_start_window_jump(&w));
            match condition {
                0 => p.auto_activities = false,
                1 => p.activity_intensity = 0,
                2 => p.display_scale = 1.5,
                _ => {
                    w.windows[0].active = true;
                    w.windows[0].fullscreen = true;
                }
            }
            p.update(DT, &w);
            assert!(p.exploration_route.is_none());
            assert_eq!(p.state, State::Idle);
            assert_eq!(p.pos.y, 1000.0);
            assert_eq!(p.vel, Vec2::default());
        }
    }
    #[test]
    fn approach_climb_and_rollover_have_no_position_snap_at_any_scale() {
        for (scale, gap) in [
            (0.75, 0.0),
            (1.0, 0.0),
            (1.5, 0.0),
            (0.75, 30.0),
            (1.0, 30.0),
            (1.5, 30.0),
        ] {
            let mut w = world();
            w.windows[0].rect.h -= gap * scale;
            let mut p = pet(Vec2::new(300.0, 1000.0));
            p.display_scale = scale;
            let plan = exploration_climb::plan_climb(&p, &w, 0.0).unwrap();
            assert!(p.begin_climb_route(plan, &w));
            let mut saw_climb = false;
            let mut arrived = false;
            for _ in 0..6000 {
                let old = p.pos;
                p.update(DT, &w);
                assert!(
                    (p.pos.x - old.x).abs() <= WALK_SPEED * DT + 1e-6,
                    "horizontal teleport at {scale}: {:?}",
                    p.state
                );
                assert!(
                    (p.pos.y - old.y).abs() <= 65.0 * DT + 1e-6,
                    "vertical teleport at {scale}: {:?}",
                    p.state
                );
                saw_climb |= p.state == State::Climb;
                if p.exploration_route.is_none() {
                    arrived = true;
                    break;
                }
            }
            assert!(saw_climb && arrived);
            assert_eq!(p.surface_id, Some(77));
            assert_eq!(p.state, State::Ledge);
            assert!(p.pos.x > 500.0 && p.pos.x < 900.0);
            assert!((p.pos.y - 400.0).abs() < 0.1);
        }
    }
    #[test]
    fn closing_climbed_window_drops_pet_without_floor_teleport() {
        let mut w = world();
        let mut p = pet(Vec2::new(480.0, 1000.0));
        let plan = exploration_climb::plan_climb(&p, &w, 0.0).unwrap();
        assert!(p.begin_climb_route(plan, &w));
        for _ in 0..1500 {
            p.update(DT, &w);
            if p.state == State::Climb && p.pos.y < 800.0 {
                break;
            }
        }
        let y = p.pos.y;
        assert!(y < 800.0);
        w.windows.clear();
        p.update(DT, &w);
        assert!(p.exploration_route.is_none());
        assert!(p.pos.y < y + 1.0);
        assert_eq!(p.state, State::Fall);
    }
    #[test]
    fn voluntary_descent_walks_to_edge_crouches_and_lands_on_floor() {
        let w = world();
        let mut p = pet(Vec2::new(700.0, 400.0));
        p.surface = Some(0);
        p.surface_id = Some(77);
        p.surface_prev = Some(w.windows[0].rect);
        let (x, plan) = exploration_descent::descent_approach(&p, &w, 0.0).unwrap();
        let mut route = p.jump_route(plan, &w);
        route.phase = Phase::ApproachDescent { x, plan };
        p.exploration_route = Some(route);
        p.state = State::Approach;
        let mut crouched = false;
        let mut jumped = false;
        let mut landed = false;
        for _ in 0..4000 {
            p.update(DT, &w);
            crouched |= p.state == State::Crouch;
            jumped |= p.state == State::Jump;
            if jumped && p.state == State::Land {
                landed = true;
                break;
            }
        }
        assert!(crouched && jumped && landed);
        assert_eq!(p.surface_id, None);
        assert!((p.pos.y - 1000.0).abs() < 0.1);
    }
    #[test]
    fn moving_target_cannot_keep_route_owned_forever() {
        let mut w = world();
        w.work_areas[0].w = 8000.0;
        let mut p = pet(Vec2::new(100.0, 1000.0));
        w.windows[0].rect.x = 1000.0;
        let plan = exploration_climb::plan_climb(&p, &w, 0.0).unwrap();
        assert!(p.begin_climb_route(plan, &w));
        for _ in 0..11000 {
            w.windows[0].rect.x += 56.0 * DT;
            p.update(DT, &w);
            if p.exploration_route.is_none() {
                break;
            }
        }
        assert!(p.exploration_route.is_none());
        assert_eq!(p.state, State::Idle);
    }
}
