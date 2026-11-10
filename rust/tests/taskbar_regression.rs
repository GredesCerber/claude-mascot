use claude_mascot::{
    engine::{Pet, Skin, State},
    Rect, Vec2, World,
};
fn setup() -> (Pet, World) {
    let w = World {
        work_areas: vec![Rect::new(0.0, 0.0, 1920.0, 1080.0)],
        ..World::default()
    };
    let mut p = Pet::new(7, Skin::Claude, Vec2::new(500.0, 1080.0));
    p.state = State::Idle;
    p.auto_activities = false;
    p.auto_reactions = false;
    p.activity_intensity = 0;
    p.update(0.0, &w);
    (p, w)
}
#[test]
fn rising_taskbar_gives_one_small_bounce_and_a_stable_landing() {
    let (mut p, mut w) = setup();
    w.work_areas[0].h = 1068.0;
    p.update(1.0 / 240.0, &w);
    assert_eq!(p.state, State::Jump);
    let initial_velocity = p.vel.y;
    assert!(initial_velocity < 0.0);
    for height in [1060.0, 1052.0, 1040.0] {
        w.work_areas[0].h = height;
        p.update(1.0 / 240.0, &w);
        assert!(
            p.vel.y > initial_velocity,
            "animated rise must not reset launch speed"
        );
        assert!(p.pos.y <= height);
    }
    let mut highest = p.pos.y;
    let mut landed = false;
    for _ in 0..480 {
        p.update(1.0 / 240.0, &w);
        highest = highest.min(p.pos.y);
        if p.state == State::Land {
            landed = true;
        }
    }
    assert!(landed);
    assert!(1040.0 - highest < 40.0, "taskbar bounce must stay gentle");
    assert_eq!(p.pos.y, 1040.0);
    assert_eq!(p.state, State::Idle);
}
#[test]
fn static_floor_dragging_airborne_and_monitor_switch_do_not_trigger_bounce() {
    let (mut p, mut w) = setup();
    for _ in 0..20 {
        p.update(1.0 / 240.0, &w);
    }
    assert_eq!(p.state, State::Idle);
    p.state = State::Jump;
    p.pos.y = 850.0;
    p.vel.y = -100.0;
    w.work_areas[0].h = 1040.0;
    p.update(1.0 / 240.0, &w);
    assert!(p.vel.y > -100.0);
    let (mut p, mut w) = setup();
    p.drag_to(Vec2::new(500.0, 1080.0));
    w.work_areas[0].h = 1040.0;
    p.update(1.0 / 240.0, &w);
    assert!(p.dragging);
    let (mut p, mut w) = setup();
    w.work_areas[0].w = 1600.0;
    w.work_areas[0].h = 1040.0;
    p.update(1.0 / 240.0, &w);
    assert_ne!(p.state, State::Jump);
}
#[test]
fn hiding_taskbar_releases_floor_support_to_gravity() {
    let (mut p, mut w) = setup();
    w.work_areas[0].h = 1120.0;
    p.update(1.0 / 240.0, &w);
    assert_eq!(p.state, State::Fall);
    assert!(p.pos.y > 1080.0 && p.pos.y < 1120.0);
}
