use claude_mascot::{
    engine::{Pet, Skin, State},
    Rect, Vec2, World,
};
const DT: f64 = 1.0 / 240.0;
fn thrown(pos: Vec2) -> Pet {
    let mut p = Pet::new(1, Skin::Claude, pos);
    p.auto_activities = false;
    p.auto_reactions = false;
    p.drag_to(pos);
    p.release(Vec2::new(100.0, -1800.0));
    p
}
#[test]
fn straight_throw_bonks_without_needing_an_existing_spin() {
    for scale in [0.75, 1.0, 1.5] {
        let w = World {
            work_areas: vec![Rect::new(-800.0, -600.0, 1600.0, 1500.0)],
            ..World::default()
        };
        let mut p = thrown(Vec2::new(0.0, -300.0));
        p.display_scale = scale;
        let top = -600.0 + 55.0 * scale + 8.0;
        let mut bounced = false;
        for _ in 0..480 {
            let vy = p.vel.y;
            p.update(DT, &w);
            assert!(p.pos.y >= top - 0.001);
            if vy < 0.0 && p.vel.y > 0.0 {
                assert!(p.squash > 0.0);
                assert!(p.bubble.is_some());
                bounced = true;
                break;
            }
        }
        assert!(bounced, "scale {scale}");
        for _ in 0..2400 {
            p.update(DT, &w);
        }
        assert_eq!(p.pos.y, 900.0);
        assert!(!matches!(p.state, State::Thrown | State::Dizzy));
    }
}
#[test]
fn vertically_adjoining_monitor_is_an_open_passage() {
    let w = World {
        work_areas: vec![
            Rect::new(0.0, 0.0, 1600.0, 900.0),
            Rect::new(0.0, -900.0, 1600.0, 900.0),
        ],
        ..World::default()
    };
    let mut p = thrown(Vec2::new(800.0, 200.0));
    for _ in 0..120 {
        p.update(DT, &w);
        if p.pos.y < 0.0 {
            assert!(p.vel.y < 0.0);
            return;
        }
        assert!(p.vel.y < 0.0, "false ceiling at monitor seam");
    }
    panic!("never crossed the seam");
}
