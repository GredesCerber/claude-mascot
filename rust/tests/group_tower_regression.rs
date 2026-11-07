use claude_mascot::{
    engine::{Pet, Skin, State},
    social::{SocialCoordinator, SocialKind, SocialMotion},
    Rect, Vec2, World,
};

fn world(floor_y: f64) -> World {
    World {
        work_areas: vec![Rect::new(0.0, 0.0, 2000.0, floor_y)],
        ..World::default()
    }
}

fn pets(count: usize, floor_y: f64, scale: f64) -> Vec<Pet> {
    (0..count)
        .map(|i| {
            let mut p = Pet::new(
                i as u64 + 1,
                Skin::Claude,
                Vec2::new(760.0 + i as f64 * 82.0, floor_y),
            );
            p.state = State::Idle;
            p.display_scale = scale;
            p.auto_activities = false;
            p.auto_reactions = false;
            p
        })
        .collect()
}

#[test]
fn all_eight_build_and_dismount_top_down_on_tall_floor() {
    let floor = 1040.0;
    let w = world(floor);
    let mut ps = pets(8, floor, 1.5);
    let mut c = SocialCoordinator::new();
    c.request_tower(Some(1));
    c.update(0.0, ps.iter_mut(), &w);
    assert_eq!(
        ps.iter().filter(|p| p.social.is_some()).count(),
        8,
        "whole group should reserve atomically"
    );
    let base = ps
        .iter()
        .find(|p| {
            p.social.is_some_and(|s| {
                s.kind == SocialKind::Tower
                    && s.role == claude_mascot::social::SocialRole::Initiator
            })
        })
        .unwrap()
        .id;
    let mut chain = vec![base];
    chain.push(
        ps.iter()
            .find(|p| p.id == base)
            .unwrap()
            .social
            .unwrap()
            .partner_id,
    );
    while chain.len() < 8 {
        let current = *chain.last().unwrap();
        let next = ps
            .iter()
            .find(|p| {
                p.social
                    .is_some_and(|s| s.kind == SocialKind::Tower && s.partner_id == current)
                    && !chain.contains(&p.id)
            })
            .unwrap()
            .id;
        chain.push(next);
    }
    let mut perch_order = Vec::new();
    let mut dismount_order = Vec::new();
    let mut all_formed = false;
    let mut saw_dismount = false;
    let mut last_departed = 8usize;
    for frame in 0..(30 * 240) {
        for p in &mut ps {
            p.update(1.0 / 240.0, &w);
        }
        c.update(1.0 / 240.0, ps.iter_mut(), &w);
        assert_eq!(
            c.stats.cancelled,
            0,
            "cancel frame={frame} pets={:?}",
            ps.iter()
                .map(|p| (p.id, p.pos, p.state, p.social, p.social_motion))
                .collect::<Vec<_>>()
        );
        for i in 1..8 {
            if let Some(p) = ps.iter().find(|p| p.id == chain[i]) {
                if matches!(p.social_motion,Some(SocialMotion::Perch{owner_id,..}) if owner_id==chain[i-1])
                    && !perch_order.contains(&p.id)
                {
                    perch_order.push(p.id);
                }
            }
        }
        for p in &ps {
            if let Some(SocialMotion::Leap { owner_id, .. }) = p.social_motion {
                let owner = ps.iter().find(|q| q.id == owner_id).unwrap();
                let target_y =
                    owner.pos.y - 55.0 * p.display_scale.max(owner.display_scale).clamp(0.5, 2.0);
                if (p.pos.x - owner.pos.x).abs() <= 76.0 {
                    assert!(
                        p.pos.y <= target_y + 5.0,
                        "leap crossed through tower: id={} owner={} feet={} target={}",
                        p.id,
                        owner_id,
                        p.pos.y,
                        target_y
                    );
                }
            }
        }
        let formed=(1..8).all(|i|ps.iter().find(|p|p.id==chain[i]).is_some_and(|p|matches!(p.social_motion,Some(SocialMotion::Perch{owner_id,..}) if owner_id==chain[i-1])));
        all_formed |= formed;
        if all_formed {
            for p in &ps {
                if p.state == State::Jump
                    && p.social.is_some_and(|s| s.kind == SocialKind::Tower)
                    && p.social_motion.is_none()
                    && !dismount_order.contains(&p.id)
                {
                    dismount_order.push(p.id);
                }
                if p.state == State::Jump
                    && p.social.is_some_and(|s| s.kind == SocialKind::Tower)
                    && p.social_motion.is_none()
                {
                    if let Some(level) = chain
                        .iter()
                        .position(|id| *id == p.id)
                        .filter(|level| *level > 0)
                    {
                        let owner = ps.iter().find(|q| q.id == chain[level - 1]).unwrap();
                        let clearance =
                            55.0 * p.display_scale.max(owner.display_scale).clamp(0.5, 2.0);
                        if (p.pos.x - owner.pos.x).abs() <= 76.0 {
                            assert!(p.pos.y<=owner.pos.y-clearance+5.0,"dismount passed through lower pet: upper={} lower={} feet={} lower_head={}",p.id,owner.id,p.pos.y,owner.pos.y-clearance);
                        }
                    }
                }
            }
        }
        if perch_order.len() == 7
            && ps
                .iter()
                .any(|p| p.social.is_some() && p.state == State::Jump)
        {
            saw_dismount = true;
        }
        if saw_dismount {
            let active_perched = ps
                .iter()
                .enumerate()
                .filter(|(_, p)| matches!(p.social_motion, Some(SocialMotion::Perch { .. })))
                .count();
            assert!(
                active_perched <= last_departed,
                "dismount should remove upper levels before lower ones"
            );
            last_departed = active_perched;
        }
        assert!(ps
            .iter()
            .all(|p| p.pos.x.is_finite() && p.pos.y.is_finite()));
        if c.stats.completed > 0 {
            break;
        }
    }
    assert_eq!(
        perch_order,
        chain[1..].to_vec(),
        "each layer must land on its predecessor; stats={:?}, pets={:?}",
        c.stats,
        ps.iter()
            .map(|p| (p.id, p.pos, p.state, p.social_motion))
            .collect::<Vec<_>>()
    );
    assert_eq!(c.stats.completed, 1);
    assert_eq!(
        dismount_order,
        chain[1..].iter().rev().copied().collect::<Vec<_>>(),
        "tower dismantles top first"
    );
    assert!(ps.iter().all(|p| p.social.is_none()));
}

#[test]
fn too_short_ceiling_rejects_the_whole_requested_group() {
    let w = world(550.0);
    let mut ps = pets(8, 550.0, 1.5);
    let mut c = SocialCoordinator::new();
    c.request_tower(Some(1));
    c.update(0.0, ps.iter_mut(), &w);
    assert_eq!(c.stats.started, 0);
    assert!(ps
        .iter()
        .all(|p| p.social.is_none() && p.social_motion.is_none()));
}
