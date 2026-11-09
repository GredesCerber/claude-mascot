use claude_mascot::{
    engine::{Command, Pet, Skin, State},
    social::{SocialCoordinator, SocialKind, SocialMotion},
    Rect, Vec2, WindowInfo, World,
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
    a.auto_activities = false;
    b.auto_activities = false;
    a.auto_reactions = false;
    b.auto_reactions = false;
    vec![a, b]
}

fn step(c: &mut SocialCoordinator, pets: &mut [Pet], w: &World, frames: usize) {
    for _ in 0..frames {
        for p in pets.iter_mut() {
            p.update(1.0 / 240.0, w);
        }
        c.update(1.0 / 240.0, pets.iter_mut(), w);
    }
}

#[test]
fn tag_runs_swaps_chaser_and_stays_on_the_shared_runway() {
    let w = world();
    let mut pets = pair();
    let mut c = SocialCoordinator::new();
    c.request_tag(Some(1));
    step(&mut c, &mut pets, &w, 2 * 240);
    assert_eq!(c.stats.started, 1);
    assert_eq!(pets[0].social.unwrap().kind, SocialKind::Tag);
    assert!(pets.iter().all(|p| p.pos.x >= 25.0 && p.pos.x <= 975.0));
    let mut speeds = [false; 2];
    for frame in 0..(8 * 240) {
        for p in pets.iter_mut() {
            p.update(1.0 / 240.0, &w);
        }
        let reorder = frame % 120 == 0;
        if reorder {
            pets.reverse();
        }
        c.update(1.0 / 240.0, pets.iter_mut(), &w);
        if reorder {
            pets.reverse();
        }
        for (i, p) in pets.iter().enumerate() {
            if let Some(SocialMotion::Run { speed, .. }) = p.social_motion {
                if speed >= 180.0 {
                    speeds[i] = true;
                }
            }
            if p.social.is_some() {
                assert!(p.pos.x >= 24.0 && p.pos.x <= 976.0);
                assert!(
                    (p.pos.y - 700.0).abs() < 3.0,
                    "tag must stay grounded id={} state={:?} x={} motion={:?}",
                    p.id,
                    p.state,
                    p.pos.x,
                    p.social_motion
                );
            }
        }
        if pets[0].social.is_some() && pets[1].social.is_some() {
            assert!(
                (pets[0].pos.x - pets[1].pos.x).abs() >= 58.0,
                "tag actors must keep visual clearance"
            );
        }
    }
    assert!(
        speeds.into_iter().all(|seen| seen),
        "both pets should take the runaway role"
    );
    assert_eq!(
        c.stats.completed,
        1,
        "stats={:?} pets={:?}",
        c.stats,
        pets.iter()
            .map(|p| (p.id, p.pos, p.state, p.social, p.social_motion))
            .collect::<Vec<_>>()
    );
    assert!(pets
        .iter()
        .all(|p| p.social.is_none() && p.social_motion.is_none()));
}

#[test]
fn leapfrog_jumps_over_partner_then_swaps_crouch_and_jump_roles() {
    let w = world();
    let mut pets = pair();
    let mut c = SocialCoordinator::new();
    c.request_for(1, Some(SocialKind::Leapfrog));
    c.update(0.0, pets.iter_mut(), &w);
    assert_eq!(pets[0].social.unwrap().kind, SocialKind::Leapfrog);
    assert!(matches!(
        pets[0].social_motion,
        Some(SocialMotion::Leap { owner_id: 2, .. })
    ));
    let mut saw_swap = false;
    for _ in 0..(12 * 240) {
        for p in &mut pets {
            p.update(1.0 / 240.0, &w);
        }
        c.update(1.0 / 240.0, pets.iter_mut(), &w);
        if pets[1]
            .social
            .is_some_and(|s| s.role == claude_mascot::social::SocialRole::Initiator)
        {
            saw_swap = true;
        }
        assert!(pets
            .iter()
            .all(|p| p.pos.x.is_finite() && p.pos.y.is_finite()));
    }
    assert!(saw_swap, "the partner should take a turn jumping");
    assert_eq!(
        c.stats.completed,
        1,
        "stats={:?} pets={:?}",
        c.stats,
        pets.iter()
            .map(|p| (p.id, p.pos, p.state, p.social, p.social_motion))
            .collect::<Vec<_>>()
    );
    assert!(pets
        .iter()
        .all(|p| p.social.is_none() && p.social_motion.is_none()));
    assert!(pets.iter().all(|p| (p.pos.y - 700.0).abs() < 3.0));
}

#[test]
fn tower_uses_a_real_jump_perch_hold_and_gravity_release() {
    let w = world();
    let mut pets = pair();
    pets[1].display_scale = 1.5;
    let original_upper = pets[1].pos;
    let mut c = SocialCoordinator::new();
    c.request_tower(Some(1));
    c.update(0.0, pets.iter_mut(), &w);
    assert_eq!(pets[1].pos, original_upper, "forming must not teleport");
    assert!(matches!(
        pets[1].social_motion,
        Some(SocialMotion::Leap { owner_id: 1, .. })
    ));
    assert!(pets[0].bubble.is_none());
    assert!(pets[1].bubble.as_deref().is_some_and(|line| {
        claude_mascot::dialogue::SpeechEvent::TowerTop
            .lines()
            .contains(&line)
    }));

    let mut saw_perch = false;
    let mut saw_dismount_apex = false;
    let mut saw_clear_dismount = false;
    for frame in 0..(8 * 240) {
        for p in pets.iter_mut() {
            p.update(1.0 / 240.0, &w);
        }
        let reorder = frame % 120 == 0;
        if reorder {
            pets.reverse();
        }
        c.update(1.0 / 240.0, pets.iter_mut(), &w);
        if reorder {
            pets.reverse();
        }
        if matches!(
            pets[1].social_motion,
            Some(SocialMotion::Perch { owner_id: 1, .. })
        ) {
            saw_perch = true;
            assert!((pets[0].pos.y - pets[1].pos.y - 82.5).abs() < 2.0);
        }
        if c.stats.completed > 0 && pets[1].pos.y < pets[0].pos.y - 65.0 {
            saw_dismount_apex = true;
        }
        if saw_dismount_apex && pets[1].pos.y >= pets[0].pos.y - 55.0 {
            assert!((pets[1].pos.x - pets[0].pos.x).abs() >= 82.5);
            saw_clear_dismount = true;
        }
        assert!(pets
            .iter()
            .all(|p| p.pos.x.is_finite() && p.pos.y.is_finite()));
    }
    assert!(saw_perch, "upper pet must physically land on the lower pet");
    assert!(
        saw_clear_dismount,
        "upper pet must hop sideways before descending past the lower pet"
    );
    assert_eq!(c.stats.completed, 1);
    assert!(pets.iter().all(|p| p.social.is_none()));
    assert!(pets.iter().all(|p| p.pos.y >= 0.0 && p.pos.y <= 700.0));
    assert_eq!(
        pets[1].state,
        State::Idle,
        "the upper pet should fall back to support"
    );
}

#[test]
fn tag_and_tower_cancel_on_drag_jump_removal_and_command() {
    let w = world();
    for kind in [SocialKind::Tag, SocialKind::Tower] {
        for action in 0..4 {
            let mut pets = pair();
            let mut c = SocialCoordinator::new();
            c.request(Some(kind));
            c.update(0.0, pets.iter_mut(), &w);
            if action == 0 {
                pets[1].drag_to(Vec2::new(420.0, 500.0));
            } else if action == 1 {
                pets[0].jump();
            } else if action == 2 {
                pets[1].command(Command::Sit, &w);
            }
            if action == 3 {
                c.update(1.0 / 240.0, pets[..1].iter_mut(), &w);
            } else {
                c.update(1.0 / 240.0, pets.iter_mut(), &w);
            }
            let live = if action == 3 { &pets[..1] } else { &pets[..] };
            assert!(
                live.iter().all(|p| p.social.is_none()),
                "{kind:?} action {action}"
            );
            assert_eq!(c.stats.cancelled, 1);
            assert!(
                live.iter().all(|p| p.social_motion.is_none()),
                "{kind:?} action {action}"
            );
        }
    }
}

#[test]
fn tower_uses_a_real_window_ledge_and_cancels_when_it_closes() {
    let mut w = world();
    w.windows.push(WindowInfo {
        id: 44,
        rect: Rect::new(200.0, 300.0, 600.0, 320.0),
        title: "visible platform".into(),
        active: false,
        fullscreen: false,
        is_file_manager: false,
        is_desktop_icon: false,
    });
    let mut a = Pet::new(11, Skin::Claude, Vec2::new(350.0, 300.0));
    let mut b = Pet::new(12, Skin::Claude, Vec2::new(430.0, 300.0));
    for p in [&mut a, &mut b] {
        p.state = State::Idle;
        p.surface = Some(0);
        p.auto_activities = false;
        p.auto_reactions = false;
        p.update(0.0, &w);
    }
    let mut pets = vec![a, b];
    assert_eq!(pets[0].surface, Some(0));
    let mut c = SocialCoordinator::new();
    c.request_tower(Some(11));
    c.update(0.0, pets.iter_mut(), &w);
    assert_eq!(pets[0].social.unwrap().kind, SocialKind::Tower);
    assert!(matches!(
        pets[1].social_motion,
        Some(SocialMotion::Leap { .. })
    ));

    w.windows.clear();
    step(&mut c, &mut pets, &w, 8);
    assert_eq!(c.stats.cancelled, 1);
    assert!(pets
        .iter()
        .all(|p| p.social.is_none() && p.social_motion.is_none()));
}

#[test]
fn tag_respects_an_offset_window_runway_and_tower_rejects_low_ceiling() {
    let mut w = world();
    w.windows.push(WindowInfo {
        id: 9,
        rect: Rect::new(200.0, 300.0, 320.0, 320.0),
        title: "runway".into(),
        active: false,
        fullscreen: false,
        is_file_manager: false,
        is_desktop_icon: false,
    });
    let mut pets = vec![
        Pet::new(21, Skin::Claude, Vec2::new(280.0, 300.0)),
        Pet::new(22, Skin::Claude, Vec2::new(350.0, 300.0)),
    ];
    for p in &mut pets {
        p.state = State::Idle;
        p.surface = Some(0);
        p.auto_activities = false;
        p.auto_reactions = false;
        p.update(0.0, &w);
    }
    let mut c = SocialCoordinator::new();
    c.request_tag(Some(21));
    c.update(0.0, pets.iter_mut(), &w);
    let mut bounded_to_runway = true;
    for _ in 0..(8 * 240) {
        for pet in &mut pets {
            pet.update(1.0 / 240.0, &w);
        }
        c.update(1.0 / 240.0, pets.iter_mut(), &w);
        if pets.iter().any(|pet| pet.social.is_some()) {
            bounded_to_runway &= pets
                .iter()
                .all(|pet| pet.pos.x >= 225.0 && pet.pos.x <= 495.0);
        }
    }
    assert_eq!(
        c.stats.started,
        1,
        "pets={:?} stats={:?}",
        pets.iter()
            .map(|p| (p.id, p.pos, p.state))
            .collect::<Vec<_>>(),
        c.stats
    );
    assert_eq!(c.stats.completed, 1);
    assert!(bounded_to_runway);

    w.windows[0].rect.y = 200.0;
    let mut upper = Pet::new(24, Skin::Claude, Vec2::new(430.0, 200.0));
    let mut lower = Pet::new(23, Skin::Claude, Vec2::new(350.0, 200.0));
    for p in [&mut lower, &mut upper] {
        p.display_scale = 1.5;
        p.state = State::Idle;
        p.surface = Some(0);
        p.update(1.0 / 240.0, &w);
    }
    let mut elevated = [lower, upper];
    let mut tower = SocialCoordinator::new();
    tower.request_tower(Some(23));
    tower.update(0.0, elevated.iter_mut(), &w);
    assert_eq!(
        tower.stats.started, 0,
        "scaled tower needs ceiling clearance"
    );
    assert!(elevated.iter().all(|p| p.social.is_none()));
}
