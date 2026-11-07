use claude_mascot::{
    behavior::{BehaviorCoordinator, BehaviorSettings},
    contacts::ContactCoordinator,
    engine::{Pet, Skin, State},
    social::SocialCoordinator,
    Vec2, World,
};

#[test]
fn a_fresh_manual_wander_prevents_context_from_replacing_it() {
    let world = World::default();
    let mut pet = Pet::new(1, Skin::Claude, Vec2::new(400.0, 1040.0));
    pet.state = State::Idle;
    pet.satisfaction = 20.0;
    pet.command(claude_mascot::engine::Command::Wander, &world);
    let mut behavior = BehaviorCoordinator::new(BehaviorSettings::default());
    for _ in 0..10 {
        behavior.update(0.05, [&mut pet], &world);
    }
    assert_eq!(pet.state, State::Idle);
    assert!(behavior.events().next().is_none());
}

#[test]
fn replacing_a_pet_reclaims_its_context_slot_without_resetting_live_friends() {
    let world = World::default();
    let mut pets: Vec<_> = (0..8)
        .map(|i| {
            let mut p = Pet::new(
                i + 1,
                Skin::Claude,
                Vec2::new(100.0 + i as f64 * 200.0, 1040.0),
            );
            p.state = State::Idle;
            p.satisfaction = 20.0;
            p
        })
        .collect();
    let mut behavior = BehaviorCoordinator::new(BehaviorSettings::default());
    for _ in 0..4 {
        behavior.update(0.05, pets.iter_mut(), &world);
    }
    let first = behavior.last_intent(1);
    assert!(first.is_some());
    let mut replacement = Pet::new(9, Skin::Claude, pets[3].pos);
    replacement.state = State::Idle;
    replacement.satisfaction = 20.0;
    pets[3] = replacement;
    for _ in 0..4 {
        behavior.update(0.05, pets.iter_mut(), &world);
    }
    assert_eq!(behavior.last_intent(1), first);
    assert!(behavior.last_intent(9).is_some());
    assert!(behavior.last_intent(4).is_none());
}

#[test]
fn tag_can_swap_chasers_while_physical_contacts_are_enabled() {
    use claude_mascot::{
        social::{SocialKind, SocialMotion},
        Rect,
    };
    let world = World {
        work_areas: vec![Rect::new(0.0, 0.0, 1000.0, 700.0)],
        ..World::default()
    };
    let mut pets = [
        Pet::new(1, Skin::Claude, Vec2::new(300.0, 700.0)),
        Pet::new(2, Skin::Claude, Vec2::new(410.0, 700.0)),
    ];
    for p in &mut pets {
        p.state = State::Idle;
        p.auto_reactions = false;
        p.auto_activities = false;
    }
    let mut social = SocialCoordinator::new();
    social.request_for(1, Some(SocialKind::Tag));
    social.update(0.0, pets.iter_mut(), &world);
    let mut contacts = ContactCoordinator::new();
    let mut fled = [false; 2];
    for _ in 0..(8 * 240) {
        for p in &mut pets {
            p.update(1.0 / 240.0, &world);
        }
        contacts.update(1.0 / 240.0, pets.iter_mut(), &world);
        social.update(1.0 / 240.0, pets.iter_mut(), &world);
        for (i, p) in pets.iter().enumerate() {
            if matches!(p.social_motion,Some(SocialMotion::Run{speed,..}) if speed>=180.0) {
                fled[i] = true;
            }
        }
    }
    assert!(
        fled.into_iter().all(|seen| seen),
        "both pets must take the fleeing role despite collision separation"
    );
    assert_eq!(social.stats.completed, 1);
}

#[test]
fn ordinary_avoidance_clears_a_friend_with_contacts_enabled() {
    let world = World::default();
    let mut a = Pet::new(1, Skin::Claude, Vec2::new(400.0, 1040.0));
    let mut b = Pet::new(2, Skin::Claude, Vec2::new(530.0, 1040.0));
    a.state = State::Walk;
    a.facing = 1.0;
    b.state = State::Idle;
    for p in [&mut a, &mut b] {
        p.auto_reactions = false;
        p.auto_activities = false;
    }
    let mut behavior = BehaviorCoordinator::new(BehaviorSettings::default());
    let mut contacts = ContactCoordinator::new();
    let mut crossed = false;
    for _ in 0..480 {
        a.update(1.0 / 240.0, &world);
        b.update(1.0 / 240.0, &world);
        contacts.update(1.0 / 240.0, [&mut a, &mut b], &world);
        behavior.update(1.0 / 240.0, [&mut a, &mut b], &world);
        if (a.pos.x - b.pos.x).abs() < 70.0 {
            assert!(
                a.pos.y <= b.pos.y - 55.0,
                "the moving pet must clear the body before crossing"
            );
            crossed = true;
        }
    }
    assert!(crossed);
    assert!(a.pos.x > b.pos.x + 76.0);
    assert_eq!(contacts.strong_impacts, 0);
}

#[test]
fn all_three_coordinators_share_eight_pets_for_ten_minutes() {
    let world = World::default();
    let mut pets: Vec<_> = (0..8)
        .map(|i| {
            Pet::new(
                i + 1,
                Skin::Claude,
                Vec2::new(400.0 + i as f64 * 70.0, 50.0),
            )
        })
        .collect();
    let mut behavior = BehaviorCoordinator::new(BehaviorSettings::default());
    let mut contacts = ContactCoordinator::new();
    let mut social = SocialCoordinator::new();
    for step in 0..36_000 {
        if step % 600 == 0 {
            pets.reverse();
        }
        for p in &mut pets {
            p.update(1.0 / 60.0, &world);
        }
        contacts.update(1.0 / 60.0, pets.iter_mut(), &world);
        behavior.update(1.0 / 60.0, pets.iter_mut(), &world);
        social.update(1.0 / 60.0, pets.iter_mut(), &world);
        assert!(pets.iter().all(|p| p.pos.x.is_finite()
            && p.pos.y.is_finite()
            && p.vel.x.is_finite()
            && p.vel.y.is_finite()
            && p.gaze.x.is_finite()));
    }
    assert!(behavior.decision_count > 1000);
}
