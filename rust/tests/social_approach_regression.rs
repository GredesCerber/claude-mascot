use claude_mascot::{
    behavior::{BehaviorCoordinator, BehaviorSettings},
    contacts::ContactCoordinator,
    dialogue::SpeechEvent,
    engine::{Command, Pet, Skin, State},
    social::{SocialCoordinator, SocialKind, SocialMotion},
    Rect, Vec2, World,
};

fn setup(
    scale: f64,
) -> (
    World,
    Vec<Pet>,
    SocialCoordinator,
    ContactCoordinator,
    BehaviorCoordinator,
) {
    let w = World {
        work_areas: vec![Rect::new(0.0, 0.0, 2400.0, 1040.0)],
        ..World::default()
    };
    let pets = [(1, 300.0), (2, 1100.0)]
        .into_iter()
        .map(|(id, x)| {
            let mut p = Pet::new(id, Skin::Claude, Vec2::new(x, 1040.0));
            p.state = State::Idle;
            p.display_scale = scale;
            p.auto_activities = false;
            p.auto_reactions = false;
            p
        })
        .collect();
    let mut social = SocialCoordinator::new();
    social.frequency = 0;
    (
        w,
        pets,
        social,
        ContactCoordinator::new(),
        BehaviorCoordinator::new(BehaviorSettings::default()),
    )
}
fn frame(
    w: &World,
    pets: &mut [Pet],
    s: &mut SocialCoordinator,
    c: &mut ContactCoordinator,
    b: &mut BehaviorCoordinator,
) {
    let dt = 1.0 / 240.0;
    for p in pets.iter_mut() {
        p.update(dt, w);
    }
    c.update(dt, pets.iter_mut(), w);
    b.update(dt, pets.iter_mut(), w);
    s.update(dt, pets.iter_mut(), w);
}
#[test]
fn far_invitation_walks_safely_then_starts_requested_game_at_each_scale() {
    for scale in [0.75, 1.0, 1.5] {
        for kind in [SocialKind::Greeting, SocialKind::Tag, SocialKind::Leapfrog] {
            let (w, mut pets, mut s, mut c, mut b) = setup(scale);
            s.request_for(1, Some(kind));
            s.update(0.0, pets.iter_mut(), &w);
            assert!(
                matches!(pets[0].social_motion, Some(SocialMotion::Run { .. })),
                "{kind:?} {scale}"
            );
            let initial = pets[0].pos.x;
            let mut previous = initial;
            for _ in 0..(12 * 240) {
                frame(&w, &mut pets, &mut s, &mut c, &mut b);
                assert!((pets[0].pos.x - previous).abs() < 5.0, "no teleport");
                previous = pets[0].pos.x;
                if s.stats.started > 0 {
                    break;
                }
            }
            assert_eq!(
                s.stats.started,
                1,
                "{kind:?} {scale} {:?}",
                pets.iter()
                    .map(|p| (p.pos, p.state, p.bubble.clone()))
                    .collect::<Vec<_>>()
            );
            assert!(pets[0].pos.x > initial);
            assert!(pets
                .iter()
                .all(|p| p.social.is_some_and(|x| x.kind == kind)));
            assert_eq!(c.strong_impacts, 0);
        }
    }
}
#[test]
fn manual_command_or_drag_cancels_both_reservations_without_overriding_the_command() {
    for drag in [false, true] {
        let (w, mut pets, mut s, mut c, mut b) = setup(1.0);
        s.request_for(1, Some(SocialKind::Tag));
        s.update(0.0, pets.iter_mut(), &w);
        for _ in 0..120 {
            frame(&w, &mut pets, &mut s, &mut c, &mut b);
        }
        if drag {
            let pos = pets[0].pos;
            pets[0].drag_to(pos);
        } else {
            pets[1].command(Command::Sleep, &w);
        }
        s.update(1.0 / 240.0, pets.iter_mut(), &w);
        assert!(pets
            .iter()
            .all(|p| p.social.is_none() && p.social_motion.is_none()));
        assert_eq!(s.stats.started, 0);
        if !drag {
            assert_eq!(pets[1].state, State::Sleep);
        }
    }
}
#[test]
fn lost_support_and_timeout_clear_the_reservation() {
    for timeout in [false, true] {
        let (mut w, mut pets, mut s, _, _) = setup(1.0);
        s.request_for(1, Some(SocialKind::Tag));
        s.update(0.0, pets.iter_mut(), &w);
        if timeout {
            for _ in 0..241 {
                s.update(0.05, pets.iter_mut(), &w);
            }
        } else {
            w.work_areas[0].h = 900.0;
            s.update(0.05, pets.iter_mut(), &w);
        }
        assert!(pets
            .iter()
            .all(|p| p.social.is_none() && p.social_motion.is_none()));
        assert_eq!(s.stats.started, 0);
        if timeout {
            assert!(SpeechEvent::SocialNoRoom
                .lines()
                .contains(&pets[0].bubble.as_deref().unwrap()));
        }
    }
}
#[test]
fn repeated_immediate_refusals_are_visible_even_inside_dialogue_cooldown() {
    let (w, mut pets, mut s, _, _) = setup(1.0);
    pets[0].state = State::Sleep;
    for _ in 0..2 {
        s.request_for(1, Some(SocialKind::Tag));
        s.update(0.05, pets.iter_mut(), &w);
        let bubble = pets[0]
            .bubble
            .take()
            .expect("every explicit request needs feedback");
        assert!(SpeechEvent::SocialSelfBusy
            .lines()
            .contains(&bubble.as_str()));
    }
}

#[test]
fn a_friend_entering_the_path_stops_the_approach_before_contact() {
    let (w, mut pets, mut s, mut c, mut b) = setup(1.0);
    let mut third = Pet::new(3, Skin::Claude, Vec2::new(1800.0, 1040.0));
    third.state = State::Idle;
    third.auto_activities = false;
    third.auto_reactions = false;
    pets.push(third);
    s.request_for(1, Some(SocialKind::Tag));
    s.update(0.0, pets.iter_mut(), &w);
    assert!(pets[0].social_motion.is_some());
    pets[2].pos.x = 650.0;
    s.update(1.0 / 240.0, pets.iter_mut(), &w);
    assert!(pets[0].social.is_none() && pets[1].social.is_none());
    assert_eq!(s.stats.started, 0);
    assert!(SpeechEvent::SocialNoRoom
        .lines()
        .contains(&pets[0].bubble.as_deref().unwrap()));
    for _ in 0..240 {
        frame(&w, &mut pets, &mut s, &mut c, &mut b);
    }
    assert_eq!(c.strong_impacts, 0);
}

#[test]
fn closing_window_during_approach_releases_both_pets() {
    let (mut w, mut pets, mut s, mut c, mut b) = setup(1.0);
    w.windows.push(claude_mascot::WindowInfo {
        id: 44,
        rect: Rect::new(100.0, 600.0, 1400.0, 300.0),
        title: "ledge".into(),
        active: false,
        fullscreen: false,
        is_file_manager: false,
        is_desktop_icon: false,
    });
    for p in &mut pets {
        p.pos.y = 600.0;
        p.surface = Some(0);
        p.update(0.0, &w);
    }
    s.request_for(1, Some(SocialKind::Tag));
    s.update(0.0, pets.iter_mut(), &w);
    assert!(pets[0].social_motion.is_some());
    for _ in 0..120 {
        frame(&w, &mut pets, &mut s, &mut c, &mut b);
    }
    w.windows.clear();
    for _ in 0..4 {
        frame(&w, &mut pets, &mut s, &mut c, &mut b);
    }
    assert!(pets
        .iter()
        .all(|p| p.social.is_none() && p.social_motion.is_none()));
    assert_eq!(s.stats.started, 0);
}
