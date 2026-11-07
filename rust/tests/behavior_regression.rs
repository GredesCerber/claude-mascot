use claude_mascot::{
    behavior::{BehaviorCoordinator, BehaviorReason, BehaviorSettings},
    engine::{Pet, Skin, State},
    Rect, Vec2, WindowInfo, World,
};

fn pet(id: u64, x: f64, y: f64) -> Pet {
    let mut p = Pet::new(id, Skin::Claude, Vec2::new(x, y));
    p.state = State::Idle;
    p
}

#[test]
fn crowded_pets_are_smoothly_spaced_and_ids_survive_reordering() {
    let world = World::default();
    let mut a = pet(1, 500.0, 1040.0);
    let mut b = pet(2, 510.0, 1040.0);
    let mut c = BehaviorCoordinator::new(BehaviorSettings::default());
    for _ in 0..300 {
        a.update(0.05, &world);
        b.update(0.05, &world);
        c.update(0.05, [&mut b, &mut a], &world);
    }
    assert!((a.pos.x - b.pos.x).abs() >= 100.0);
    assert!(c.events().all(|e| e.pet_id == 1 || e.pet_id == 2));
}

#[test]
fn manual_and_different_level_pets_are_untouched() {
    let world = World::default();
    let mut a = pet(1, 500.0, 1040.0);
    let mut baseline = pet(1, 500.0, 1040.0);
    let mut b = pet(2, 505.0, 700.0);
    b.state = State::Sleep;
    let mut c = BehaviorCoordinator::new(BehaviorSettings::default());
    for _ in 0..20 {
        a.update(0.05, &world);
        baseline.update(0.05, &world);
        c.update(0.05, [&mut a, &mut b], &world);
    }
    assert_eq!(a.pos.x, baseline.pos.x);
    assert_eq!(b.state, State::Sleep);
    assert!(!c.events().any(|e| e.reason == BehaviorReason::Crowded));
}

#[test]
fn scale_changes_edge_gap_and_context_tracks_world_snapshot() {
    let world = World::default();
    let mut a = pet(1, 500.0, 1040.0);
    let mut b = pet(2, 520.0, 1040.0);
    a.display_scale = 2.0;
    let mut c = BehaviorCoordinator::new(BehaviorSettings::default());
    for _ in 0..200 {
        a.update(0.05, &world);
        b.update(0.05, &world);
        c.update(0.05, [&mut a, &mut b], &world);
    }
    assert!((a.pos.x - b.pos.x).abs() >= 160.0);
}

#[test]
fn eight_pet_simulation_stays_finite_for_ten_minutes() {
    let world = World::default();
    let mut pets: Vec<Pet> = (0..8)
        .map(|i| pet(i + 1, 300.0 + i as f64 * 15.0, 1040.0))
        .collect();
    let mut coordinator = BehaviorCoordinator::new(BehaviorSettings::default());
    for _ in 0..12_000 {
        for p in &mut pets {
            p.update(0.05, &world);
        }
        coordinator.update(0.05, pets.iter_mut(), &world);
    }
    assert!(pets.iter().all(|p| p.pos.x.is_finite()
        && p.pos.y.is_finite()
        && p.vel.x.is_finite()
        && p.vel.y.is_finite()));
}

#[test]
fn active_explorer_causes_a_nonmoving_lookaround() {
    let mut world = World::default();
    world.windows.push(WindowInfo {
        id: 7,
        rect: Rect::new(500.0, 600.0, 200.0, 300.0),
        title: String::new(),
        active: true,
        fullscreen: false,
        is_file_manager: true,
        is_desktop_icon: false,
    });
    let mut p = pet(1, 500.0, 1040.0);
    let mut coordinator = BehaviorCoordinator::new(BehaviorSettings::default());
    for _ in 0..8 {
        p.update(0.05, &world);
        coordinator.update(0.05, [&mut p], &world);
        if p.state == State::LookAround {
            break;
        }
    }
    assert_eq!(p.state, State::LookAround);
    assert!(coordinator
        .events()
        .any(|e| e.reason == BehaviorReason::ExplorerNearby));
}

#[test]
fn uncoordinated_walk_is_not_stopped() {
    let world = World::default();
    let mut p = pet(11, 600.0, 1040.0);
    let mut baseline = pet(11, 600.0, 1040.0);
    p.state = State::Walk;
    baseline.state = State::Walk;
    p.facing = 1.0;
    baseline.facing = 1.0;
    let mut coordinator = BehaviorCoordinator::new(BehaviorSettings::default());
    for _ in 0..30 {
        p.update(0.05, &world);
        baseline.update(0.05, &world);
        coordinator.update(0.05, [&mut p], &world);
    }
    assert!((p.pos.x - baseline.pos.x).abs() < 1.0);
}

#[test]
fn walking_toward_friend_hops_over_and_lands_on_far_side() {
    let world = World::default();
    let mut walker = pet(1, 400.0, 1040.0);
    walker.state = State::Walk;
    walker.facing = 1.0;
    let friend = pet(2, 505.0, 1040.0);
    let mut friend = friend;
    let mut coordinator = BehaviorCoordinator::new(BehaviorSettings::default());
    coordinator.update(0.05, [&mut walker, &mut friend], &world);
    assert_eq!(walker.state, State::Jump);
    assert!(walker.vel.y < 0.0 && walker.vel.x > 0.0);
    let friend_x = friend.pos.x;
    let friend_head = friend.pos.y - 55.0;
    let mut crossed_above = false;
    for _ in 0..60 {
        walker.update(1.0 / 60.0, &world);
        friend.update(1.0 / 60.0, &world);
        if walker.pos.x > friend_x - 35.0
            && walker.pos.x < friend_x + 35.0
            && walker.pos.y < friend_head
        {
            crossed_above = true;
        }
        coordinator.update(1.0 / 60.0, [&mut walker, &mut friend], &world);
    }
    assert!(crossed_above);
    assert!(walker.pos.x > friend_x + 35.0);
    assert!(walker.pos.x.is_finite() && walker.pos.y.is_finite() && walker.vel.y.is_finite());
}
