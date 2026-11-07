use claude_mascot::{
    contacts::ContactCoordinator,
    engine::{Pet, Skin, State},
    social::{SocialActivity, SocialKind, SocialRole},
    Vec2, World,
};

fn pet(id: u64, x: f64, y: f64) -> Pet {
    let mut p = Pet::new(id, Skin::Claude, Vec2::new(x, y));
    p.state = State::Idle;
    p
}

#[test]
fn strong_horizontal_hit_transfers_inertia_and_cancels_idle_pose() {
    let world = World::default();
    let mut a = pet(1, 400.0, 1040.0);
    let mut b = pet(2, 500.0, 1040.0);
    let mut c = ContactCoordinator::new();
    c.update(0.05, [&mut a, &mut b], &world);
    a.pos.x = 485.0;
    a.vel.x = 1200.0;
    a.state = State::Thrown;
    c.update(0.05, [&mut a, &mut b], &world);
    assert!(b.vel.x > 250.0);
    assert_eq!(b.state, State::Thrown);
    assert_eq!(c.strong_impacts, 1);
}

#[test]
fn separating_overlap_does_not_add_energy() {
    let world = World::default();
    let mut a = pet(1, 490.0, 1040.0);
    let mut b = pet(2, 500.0, 1040.0);
    a.vel.x = -120.0;
    b.vel.x = 120.0;
    let mut c = ContactCoordinator::new();
    c.update(0.05, [&mut a, &mut b], &world);
    assert_eq!(a.vel.x, -120.0);
    assert_eq!(b.vel.x, 120.0);
    assert_eq!(c.strong_impacts, 0);
}

#[test]
fn scale_mass_makes_larger_pet_harder_to_move() {
    let world = World::default();
    let mut a = pet(1, 400.0, 1040.0);
    let mut b = pet(2, 500.0, 1040.0);
    b.display_scale = 2.0;
    let mut c = ContactCoordinator::new();
    c.update(0.05, [&mut a, &mut b], &world);
    a.pos.x = 485.0;
    a.vel.x = 1000.0;
    a.state = State::Thrown;
    c.update(0.05, [&mut a, &mut b], &world);
    assert!(
        b.vel.x > 200.0 && b.vel.x < 500.0,
        "larger target should receive less velocity, got {}",
        b.vel.x
    );
}

#[test]
fn dragged_pet_is_kinematic_but_can_push_other_pet() {
    let world = World::default();
    let mut a = pet(1, 400.0, 1040.0);
    let mut b = pet(2, 500.0, 1040.0);
    let mut c = ContactCoordinator::new();
    c.update(0.05, [&mut a, &mut b], &world);
    a.pos.x = 485.0;
    a.dragging = true;
    a.state = State::Thrown;
    c.update(0.05, [&mut a, &mut b], &world);
    assert!(b.vel.x > 100.0);
    assert_eq!(a.pos.x, 485.0);
}

#[test]
fn swept_hit_catches_fast_crossing_in_one_maximum_step() {
    let world = World::default();
    let mut a = pet(1, 400.0, 1040.0);
    let mut b = pet(2, 500.0, 1040.0);
    let mut c = ContactCoordinator::new();
    c.update(0.05, [&mut a, &mut b], &world);
    a.pos.x = 600.0;
    a.vel.x = 2000.0;
    a.state = State::Thrown;
    c.update(0.05, [&mut a, &mut b], &world);
    assert_eq!(c.contact_count, 1);
    assert!(a.pos.x < 600.0);
    assert!(b.vel.x > 0.0);
}

#[test]
fn vertical_head_contact_bounces_without_nonfinite_motion() {
    let world = World::default();
    let mut top = pet(1, 500.0, 980.0);
    let mut lower = pet(2, 500.0, 1040.0);
    top.state = State::Fall;
    top.vel.y = 700.0;
    let mut c = ContactCoordinator::new();
    c.update(0.05, [&mut top, &mut lower], &world);
    top.pos.y = 1000.0;
    c.update(0.05, [&mut top, &mut lower], &world);
    assert!(top.vel.y < 700.0);
    assert!(top.pos.y.is_finite() && lower.pos.y.is_finite());
    for _ in 0..120 {
        top.update(0.05, &world);
        lower.update(0.05, &world);
        c.update(0.05, [&mut top, &mut lower], &world);
    }
    assert!(top.pos.x.is_finite() && top.pos.y.is_finite() && top.vel.y.is_finite());
}

#[test]
fn distant_pets_keep_their_movement_and_eight_pet_run_is_finite() {
    let world = World::default();
    let mut near_a = pet(90, 300.0, 1040.0);
    let mut near_b = pet(91, 700.0, 1040.0);
    near_a.state = State::Walk;
    near_a.facing = 1.0;
    let before = near_a.pos;
    let mut one = ContactCoordinator::new();
    one.update(0.05, [&mut near_a, &mut near_b], &world);
    assert_eq!(near_a.pos, before);
    let mut pets: Vec<Pet> = (0..8)
        .map(|i| pet(i + 1, 100.0 + i as f64 * 200.0, 1040.0))
        .collect();
    let mut c = ContactCoordinator::new();
    for _ in 0..12_000 {
        for p in &mut pets {
            p.update(0.05, &world);
        }
        c.update(0.05, pets.iter_mut(), &world);
    }
    assert!(pets.iter().all(|p| p.pos.x.is_finite()
        && p.pos.y.is_finite()
        && p.vel.x.is_finite()
        && p.vel.y.is_finite()));
}

#[test]
fn static_overlap_resolves_once_without_jitter_or_edge_escape() {
    let world = World::default();
    let mut a = pet(1, 25.0, 1040.0);
    let mut b = pet(2, 30.0, 1040.0);
    let mut c = ContactCoordinator::new();
    for _ in 0..20 {
        a.update(0.05, &world);
        b.update(0.05, &world);
        c.update(0.05, [&mut a, &mut b], &world);
    }
    let pos = (a.pos.x, b.pos.x);
    let velocities = (a.vel.x, b.vel.x);
    for _ in 0..20 {
        c.update(0.05, [&mut a, &mut b], &world);
    }
    assert!((a.pos.x - pos.0).abs() < 0.001 && (b.pos.x - pos.1).abs() < 0.001);
    assert_eq!((a.vel.x, b.vel.x), velocities);
    assert!(a.pos.x >= 0.0 && b.pos.x >= 0.0 && a.pos.x < 100.0 && b.pos.x < 150.0);
}

#[test]
fn occupants_of_the_same_tower_ignore_internal_contacts() {
    let world = World::default();
    let mut a = pet(1, 500.0, 1040.0);
    let mut b = pet(2, 500.0, 1040.0);
    a.social = Some(SocialActivity {
        kind: SocialKind::Tower,
        role: SocialRole::Initiator,
        partner_id: 2,
        elapsed: 1.0,
        partner_offset: Vec2::default(),
    });
    b.social = Some(SocialActivity {
        kind: SocialKind::Tower,
        role: SocialRole::Partner,
        partner_id: 1,
        elapsed: 1.0,
        partner_offset: Vec2::default(),
    });
    let mut c = ContactCoordinator::new();
    c.update(0.05, [&mut a, &mut b], &world);
    assert_eq!(c.contact_count, 0);
    assert_eq!(a.pos, b.pos);
    assert_eq!(a.vel, b.vel);
}
