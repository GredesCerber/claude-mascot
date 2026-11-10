use claude_mascot::{
    behavior::{BehaviorCoordinator, BehaviorSettings},
    contacts::ContactCoordinator,
    engine::{Pet, Skin, State},
    social::{SocialCoordinator, SocialKind},
    Rect, Vec2, World,
};
#[test]
fn leapfrog_completes_with_real_contacts_at_every_supported_scale_and_gap() {
    for (left_scale, right_scale) in [
        (0.75_f64, 0.75_f64),
        (1.0, 1.0),
        (1.5, 1.5),
        (0.75, 1.5),
        (1.5, 0.75),
    ] {
        for gap in [100.0, 240.0] {
            for second_id in [8, 9] {
                let scale = left_scale.max(right_scale);
                let world = World {
                    work_areas: vec![Rect::new(0.0, 0.0, 3000.0, 1040.0)],
                    ..World::default()
                };
                let mut pets: Vec<_> = [
                    (1, left_scale, 600.0),
                    (second_id, right_scale, 600.0 + gap * scale),
                ]
                .into_iter()
                .map(|(id, s, x)| {
                    let mut p = Pet::new(id, Skin::Claude, Vec2::new(x, 1040.0));
                    p.state = State::Idle;
                    p.display_scale = s;
                    p.auto_activities = false;
                    p.auto_reactions = false;
                    p
                })
                .collect();
                let mut social = SocialCoordinator::new();
                let mut contacts = ContactCoordinator::new();
                let mut behavior = BehaviorCoordinator::new(BehaviorSettings::default());
                social.request_for(1, Some(SocialKind::Leapfrog));
                social.update(0.0, pets.iter_mut(), &world);
                assert!(pets[0].social.is_some());
                let mut launches = 0;
                let mut jumping = [false; 2];
                for _ in 0..7200 {
                    for (i, p) in pets.iter_mut().enumerate() {
                        p.update(1.0 / 240.0, &world);
                        let now = p.state == State::Jump;
                        if now && !jumping[i] {
                            launches += 1;
                        }
                        jumping[i] = now;
                    }
                    contacts.update(1.0 / 240.0, pets.iter_mut(), &world);
                    behavior.update(1.0 / 240.0, pets.iter_mut(), &world);
                    social.update(1.0 / 240.0, pets.iter_mut(), &world);
                    if social.stats.completed > 0 || social.stats.cancelled > 0 {
                        break;
                    }
                }
                assert_eq!(
                    social.stats.completed,
                    1,
                    "scales={left_scale}/{right_scale} gap={gap} contacts={} stats={:?} pets={:?}",
                    contacts.contact_count,
                    social.stats,
                    pets.iter()
                        .map(|p| (p.state, p.pos, p.social_motion))
                        .collect::<Vec<_>>()
                );
                assert_eq!(social.stats.cancelled, 0);
                assert_eq!(launches, if second_id == 8 { 2 } else { 10 });
                assert_eq!(
                    contacts.contact_count, 0,
                    "a planned leap should clear its partner"
                );
            }
        }
    }
}
