use claude_mascot::{
    engine::{Pet, Skin, State},
    social::{SocialCoordinator, SocialKind, SocialRole},
    Rect, Vec2, World,
};

fn world() -> World {
    World {
        work_areas: vec![Rect::new(0.0, 0.0, 1000.0, 700.0)],
        ..World::default()
    }
}
fn pair() -> Vec<Pet> {
    let mut a = Pet::new(1, Skin::Claude, Vec2::new(300.0, 700.0));
    let mut b = Pet::new(2, Skin::Claude, Vec2::new(380.0, 700.0));
    a.state = State::Idle;
    b.state = State::Idle;
    vec![a, b]
}
fn advance(c: &mut SocialCoordinator, p: &mut [Pet], w: &World, secs: f64) {
    let n = (secs * 240.0) as usize;
    if n == 0 {
        c.update(0.0, p.iter_mut(), w);
    }
    for _ in 0..n {
        c.update(1.0 / 240.0, p.iter_mut(), w);
    }
}

#[test]
fn all_kinds_start_and_complete_with_owned_states() {
    let w = world();
    for (idx, k) in [
        SocialKind::Greeting,
        SocialKind::HighFive,
        SocialKind::DanceTogether,
        SocialKind::ShareLaptop,
        SocialKind::Comfort,
    ]
    .into_iter()
    .enumerate()
    {
        let mut p = pair();
        let mut c = SocialCoordinator::new();
        c.request(Some(k));
        advance(&mut c, &mut p, &w, 0.11);
        assert_eq!(p[0].social.unwrap().kind, k);
        assert_eq!(p[0].social.unwrap().role, SocialRole::Initiator);
        if k == SocialKind::ShareLaptop {
            assert_eq!(p[0].state, State::Laptop)
        } else {
            assert_eq!(p[0].state, State::Social)
        }
        advance(&mut c, &mut p, &w, [4.1, 2.1, 4.1, 10.1, 4.1][idx]);
        assert!(p.iter().all(|x| x.social.is_none()));
        assert_eq!(c.stats.completed, 1);
    }
}

#[test]
fn drag_and_command_cancel_both_sides() {
    let w = world();
    for action in 0..3 {
        let mut p = pair();
        let mut c = SocialCoordinator::new();
        c.request(Some(SocialKind::Greeting));
        advance(&mut c, &mut p, &w, 0.11);
        if action == 0 {
            p[1].drag_to(Vec2::new(400.0, 300.0));
        } else if action == 1 {
            p[1].command(claude_mascot::engine::Command::Sit, &w);
        } else {
            p[1].jump();
        }
        advance(&mut c, &mut p, &w, 0.11);
        assert!(p.iter().all(|x| x.social.is_none()));
        assert_eq!(c.stats.cancelled, 1);
    }
}

#[test]
fn pair_selection_is_order_independent_and_support_matters() {
    let mut w = world();
    let mut a = pair();
    let mut b = pair();
    a.reverse();
    let mut ca = SocialCoordinator::new();
    let mut cb = SocialCoordinator::new();
    advance(&mut ca, &mut a, &w, 2.2);
    advance(&mut cb, &mut b, &w, 2.2);
    let ids = |p: &[Pet]| {
        p.iter()
            .filter_map(|x| x.social.map(|s| (x.id, s.partner_id)))
            .collect::<Vec<_>>()
    };
    let mut ia = ids(&a);
    let mut ib = ids(&b);
    ia.sort();
    ib.sort();
    assert_eq!(ia, ib);
    assert_eq!(ca.stats.started, 1);
    assert_eq!(cb.stats.started, 1);
    let mut c = SocialCoordinator::new();
    let mut p = pair();
    p[1].pos.x = 900.0;
    advance(&mut c, &mut p, &w, 4.0);
    assert!(p.iter().all(|x| x.social.is_none()));
    w.work_areas = vec![
        Rect::new(0.0, 0.0, 400.0, 700.0),
        Rect::new(500.0, 0.0, 400.0, 700.0),
    ];
    p[0].pos = Vec2::new(350.0, 700.0);
    p[1].pos = Vec2::new(530.0, 700.0);
    advance(&mut c, &mut p, &w, 2.0);
    assert!(p.iter().all(|x| x.social.is_none()));
}

#[test]
fn manual_and_air_states_are_excluded_and_duplicate_ids_never_pair() {
    let w = world();
    for state in [
        State::Sleep,
        State::Climb,
        State::Fall,
        State::Jump,
        State::Hold,
        State::Peek,
        State::Sit,
        State::Dance,
    ] {
        let mut p = pair();
        p[0].state = state;
        let mut c = SocialCoordinator::new();
        c.request(Some(SocialKind::Greeting));
        advance(&mut c, &mut p, &w, 0.2);
        assert!(p.iter().all(|x| x.social.is_none()));
    }
    let mut p = pair();
    p[1].id = p[0].id;
    let mut c = SocialCoordinator::new();
    c.request(Some(SocialKind::Greeting));
    advance(&mut c, &mut p, &w, 0.2);
    assert!(p.iter().all(|x| x.social.is_none()));
}

#[test]
fn gaze_tracks_near_cursor_and_stays_finite_bounded() {
    let mut p = Pet::new(90, Skin::Claude, Vec2::new(300.0, 700.0));
    p.state = State::Sit;
    let mut w = World {
        work_areas: vec![Rect::new(0.0, 0.0, 1000.0, 700.0)],
        ..World::default()
    };
    w.cursor = Vec2::new(400.0, 660.0);
    p.update(0.05, &w);
    assert!(p.gaze.x > 0.0);
    w.cursor = Vec2::new(200.0, 660.0);
    for _ in 0..200 {
        p.update(0.05, &w);
    }
    assert!(p.gaze.x < 0.0);
    w.cursor = Vec2::new(300.0, 660.0);
    for _ in 0..100 {
        p.update(0.05, &w);
    }
    assert!(p.gaze.x.abs() < 0.01);
    w.cursor = Vec2::new(f64::NAN, 0.0);
    for _ in 0..10 {
        p.update(0.05, &w);
    }
    assert!(
        p.gaze.x.is_finite()
            && p.gaze.y.is_finite()
            && p.gaze.x.abs() <= 1.0
            && p.gaze.y.abs() <= 1.0
    );
    w.cursor = Vec2::new(300.0, 560.0);
    for _ in 0..100 {
        p.update(0.05, &w);
    }
    assert!(p.gaze.y < 0.0);
    w.cursor = Vec2::new(900.0, 660.0);
    for _ in 0..100 {
        p.update(0.05, &w);
    }
    assert!(p.gaze.x.abs() < 0.02 && p.gaze.y.abs() < 0.02);
}

#[test]
fn request_for_uses_nearest_friend_and_manual_laptop_is_excluded() {
    let w = world();
    let mut p = vec![
        Pet::new(1, Skin::Claude, Vec2::new(300.0, 700.0)),
        Pet::new(2, Skin::Claude, Vec2::new(390.0, 700.0)),
        Pet::new(3, Skin::Claude, Vec2::new(340.0, 700.0)),
    ];
    for x in &mut p {
        x.state = State::Idle;
    }
    let mut c = SocialCoordinator::new();
    c.request_for(1, Some(SocialKind::Greeting));
    advance(&mut c, &mut p, &w, 0.0);
    assert_eq!(p[0].social.unwrap().partner_id, 3);
    let mut p = pair();
    p[0].state = State::Laptop;
    let mut c = SocialCoordinator::new();
    c.request(Some(SocialKind::Greeting));
    advance(&mut c, &mut p, &w, 3.0);
    assert!(p.iter().all(|x| x.social.is_none()));
}

#[test]
fn elapsed_tracks_physics_updates_and_removal_or_disable_cancels_pair() {
    let w = world();
    let mut p = pair();
    let mut c = SocialCoordinator::new();
    c.request(Some(SocialKind::Greeting));
    advance(&mut c, &mut p, &w, 0.0);
    for _ in 0..24 {
        c.update(1.0 / 240.0, p.iter_mut(), &w);
    }
    assert!((p[0].social.unwrap().elapsed - 0.1).abs() < 0.02);
    c.update(1.0 / 240.0, p[..1].iter_mut(), &w);
    assert!(p[0].social.is_none());
    assert_eq!(c.stats.cancelled, 1);
    let mut p = pair();
    let mut c = SocialCoordinator::new();
    c.request(Some(SocialKind::Greeting));
    advance(&mut c, &mut p, &w, 0.0);
    c.enabled = false;
    c.update(0.0, p.iter_mut(), &w);
    assert!(p.iter().all(|x| x.social.is_none()));
}

#[test]
fn active_pair_cancels_after_separation_and_cooldown_prevents_immediate_repeat() {
    let w = world();
    let mut p = pair();
    let mut c = SocialCoordinator::new();
    c.request(Some(SocialKind::Greeting));
    advance(&mut c, &mut p, &w, 0.0);
    advance(&mut c, &mut p, &w, 4.1);
    assert_eq!(c.stats.completed, 1);
    advance(&mut c, &mut p, &w, 5.0);
    assert_eq!(c.stats.started, 1);
    let mut p = pair();
    let mut c = SocialCoordinator::new();
    c.request(Some(SocialKind::Greeting));
    advance(&mut c, &mut p, &w, 0.0);
    p[1].pos.x += 200.0;
    c.update(0.0, p.iter_mut(), &w);
    assert!(p.iter().all(|pet| pet.social.is_none()));
    assert_eq!(c.stats.cancelled, 1);
}

#[test]
fn activity_command_runs_and_manual_actions_cancel_it() {
    let w = world();
    let id = claude_mascot::activities::CATALOG[0].id;
    let mut p = Pet::new(1, Skin::Claude, Vec2::new(300.0, 700.0));
    p.state = State::Idle;
    p.command(claude_mascot::engine::Command::Activity(id), &w);
    assert_eq!(p.activity, Some(id));
    assert_eq!(p.state, State::Activity);
    p.jump();
    assert_eq!(p.activity, None);
    assert_eq!(p.state, State::Jump);
}

#[test]
fn petting_or_jump_cancels_an_activity_waiting_for_landing() {
    let w = world();
    for jump in [false, true] {
        let mut pet = Pet::new(900, Skin::Claude, Vec2::new(300.0, 150.0));
        pet.auto_activities = false;
        pet.command(
            claude_mascot::engine::Command::Activity(claude_mascot::activities::CATALOG[0].id),
            &w,
        );
        if jump {
            pet.jump();
        } else {
            pet.pet();
        }
        for _ in 0..600 {
            pet.update(1.0 / 240.0, &w);
            assert_ne!(
                pet.state,
                State::Activity,
                "cancelled activity must not reappear on landing"
            );
            assert!(pet.activity.is_none());
        }
    }
}

#[test]
fn every_pack_activity_ends_on_its_catalog_duration_and_can_defer_until_landing() {
    let w = world();
    let mut pet = Pet::new(800, Skin::Claude, Vec2::new(300.0, 700.0));
    pet.state = State::Idle;
    for spec in claude_mascot::activities::CATALOG {
        pet.state = State::Idle;
        pet.command(claude_mascot::engine::Command::Activity(spec.id), &w);
        assert_eq!(pet.activity, Some(spec.id));
        for _ in 0..((spec.duration / 0.05).ceil() as usize + 2) {
            pet.update(0.05, &w);
        }
        assert_eq!(pet.activity, None, "activity {} did not finish", spec.slug);
        assert_ne!(pet.state, State::Activity);
    }
    pet.state = State::Fall;
    pet.pos = Vec2::new(300.0, 200.0);
    let requested = claude_mascot::activities::CATALOG[0].id;
    pet.command(claude_mascot::engine::Command::Activity(requested), &w);
    for _ in 0..80 {
        pet.update(0.05, &w);
    }
    assert_eq!(pet.activity, Some(requested));
    assert_eq!(pet.state, State::Activity);
}

#[test]
fn eight_pet_physics_and_social_run_safely_for_ten_minutes() {
    let w = world();
    let mut pets: Vec<_> = (0..8)
        .map(|i| {
            let x = 100.0 + (i / 2) as f64 * 220.0 + (i % 2) as f64 * 55.0;
            let mut pet = Pet::new(i as u64 + 100, Skin::Claude, Vec2::new(x, 700.0));
            pet.state = State::Idle;
            pet.auto_activities = false;
            pet
        })
        .collect();
    let mut social = SocialCoordinator::new();
    social.request(Some(SocialKind::Greeting));
    for _ in 0..(600 * 240) {
        for pet in &mut pets {
            pet.update(1.0 / 240.0, &w);
        }
        social.update(1.0 / 240.0, pets.iter_mut(), &w);
        assert!(pets
            .iter()
            .all(|p| p.pos.x.is_finite() && p.pos.y.is_finite()));
    }
    assert!(social.stats.started >= 1);
    assert!(pets
        .iter()
        .all(|p| p.pos.x.abs() < 10_000_000.0 && p.pos.y.abs() < 10_000_000.0));
}

#[test]
fn disabling_auto_activities_suppresses_catalog_dance_and_wave_idles() {
    let w = world();
    let mut pet = Pet::new(909, Skin::Claude, Vec2::new(300.0, 700.0));
    pet.state = State::Idle;
    pet.auto_activities = false;
    for _ in 0..(90 * 240) {
        pet.update(1.0 / 240.0, &w);
        assert!(!matches!(
            pet.state,
            State::Activity | State::Dance | State::Wave
        ));
    }
}
