use claude_mascot::{
    behavior::{BehaviorCoordinator, BehaviorSettings},
    contacts::ContactCoordinator,
    engine::{Pet, Skin, State},
    social::{SocialCoordinator, SocialKind},
    Rect, Vec2, WindowInfo, World,
};

#[test]
fn five_pets_keep_finite_safe_states_with_coordinators_and_changing_desktop() {
    for scale in [0.75, 1.0, 1.5] {
        for game in [SocialKind::Tower, SocialKind::Leapfrog] {
            let mut world = World {
                work_areas: vec![Rect::new(0.0, 0.0, 2400.0, 1040.0)],
                windows: vec![WindowInfo {
                    id: 44,
                    rect: Rect::new(1400.0, 400.0, 600.0, 400.0),
                    title: "window".into(),
                    active: false,
                    fullscreen: false,
                    is_file_manager: false,
                    is_desktop_icon: false,
                }],
                ..World::default()
            };
            let mut pets: Vec<_> = (0..5)
                .map(|i| {
                    let mut p = Pet::new(
                        if i == 1 { 8 } else { i + 1 },
                        Skin::Claude,
                        Vec2::new(
                            if game == SocialKind::Tower {
                                350.0 + i as f64 * 82.0 * scale
                            } else if i < 2 {
                                350.0 + i as f64 * 100.0 * scale
                            } else {
                                1650.0 + (i - 2) as f64 * 200.0
                            },
                            1040.0,
                        ),
                    );
                    p.state = State::Idle;
                    p.display_scale = scale;
                    p.auto_activities = false;
                    p.auto_reactions = false;
                    p
                })
                .collect();
            let mut behavior = BehaviorCoordinator::new(BehaviorSettings::default());
            let mut contacts = ContactCoordinator::new();
            let mut social = SocialCoordinator::new();
            social.frequency = 0;
            social.request_for(1, Some(game));
            social.update(0.0, pets.iter_mut(), &world);
            assert_eq!(social.stats.started, 1, "{game:?} scale={scale}");
            for frame in 0..(30 * 240) {
                if frame == 20 * 240 {
                    world.work_areas[0].h = 1028.0;
                }
                if frame == 24 * 240 {
                    world.windows.clear();
                }
                for p in &mut pets {
                    p.update(1.0 / 240.0, &world);
                }
                contacts.update(1.0 / 240.0, pets.iter_mut(), &world);
                behavior.update(1.0 / 240.0, pets.iter_mut(), &world);
                social.update(1.0 / 240.0, pets.iter_mut(), &world);
                assert!(
                    pets.iter().all(|p| p.pos.x.is_finite()
                        && p.pos.y.is_finite()
                        && p.vel.x.is_finite()
                        && p.vel.y.is_finite()),
                    "{game:?} scale={scale} frame={frame}"
                );
            }
            assert_eq!(
                social.stats.completed, 1,
                "{game:?} scale={scale}, {:?}",
                social.stats
            );
            assert!(pets
                .iter()
                .all(|p| p.social.is_none() && p.social_motion.is_none()));
        }
    }
}

#[test]
fn leapfrog_does_not_launch_into_a_third_pet_at_the_landing() {
    let w = World {
        work_areas: vec![Rect::new(0.0, 0.0, 2400.0, 1040.0)],
        ..World::default()
    };
    let mut pets: Vec<_> = [(1, 350.0), (8, 500.0), (3, 1000.0)]
        .into_iter()
        .map(|(id, x)| {
            let mut p = Pet::new(id, Skin::Claude, Vec2::new(x, 1040.0));
            p.state = State::Idle;
            p.display_scale = 1.5;
            p.auto_activities = false;
            p.auto_reactions = false;
            p
        })
        .collect();
    let mut social = SocialCoordinator::new();
    let mut contacts = ContactCoordinator::new();
    let mut behavior = BehaviorCoordinator::new(BehaviorSettings::default());
    social.frequency = 0;
    social.request_for(1, Some(SocialKind::Leapfrog));
    social.update(0.0, pets.iter_mut(), &w);
    assert_eq!(social.stats.started, 1);
    for _ in 0..(12 * 240) {
        for p in &mut pets {
            p.update(1.0 / 240.0, &w);
        }
        contacts.update(1.0 / 240.0, pets.iter_mut(), &w);
        behavior.update(1.0 / 240.0, pets.iter_mut(), &w);
        social.update(1.0 / 240.0, pets.iter_mut(), &w);
        assert!(pets.iter().all(|p| p.state != State::Thrown));
        if social.stats.completed + social.stats.cancelled > 0 {
            break;
        }
    }
    assert_eq!(social.stats.completed + social.stats.cancelled, 1);
}
