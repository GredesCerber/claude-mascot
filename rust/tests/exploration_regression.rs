use claude_mascot::{
    engine::{Pet, Skin, State},
    Rect, Vec2, WindowInfo, World,
};
use std::collections::BTreeSet;

#[test]
fn normal_updates_autonomously_explore_climb_and_leave_windows() {
    let world = World {
        windows: vec![
            window(11, Rect::new(400.0, 350.0, 320.0, 650.0)),
            window(22, Rect::new(850.0, 500.0, 300.0, 300.0)),
            window(33, Rect::new(50.0, 680.0, 240.0, 160.0)),
        ],
        work_areas: vec![Rect::new(0.0, 0.0, 1400.0, 1000.0)],
        ..World::default()
    };
    let mut targets = BTreeSet::new();
    let mut saw_crouch = false;
    let mut saw_climb = false;
    let mut saw_approach = false;
    let mut saw_voluntary_descent = false;
    for id in [5, 19, 37, 71] {
        let mut pet = Pet::new(id, Skin::Claude, Vec2::new(320.0, 1000.0));
        pet.auto_reactions = false;
        for _ in 0..36_000 {
            pet.update(1.0 / 60.0, &world);
            assert!(pet.pos.x.is_finite() && pet.pos.y.is_finite());
            assert!(pet.pos.x >= 24.0 - 0.01 && pet.pos.x <= 1376.0 + 0.01);
            assert!(pet.pos.y >= 0.0 && pet.pos.y <= 1000.0 + 0.01);
            saw_crouch |= pet.state == State::Crouch;
            saw_climb |= pet.state == State::Climb;
            saw_approach |= pet.state == State::Approach;
            saw_voluntary_descent |=
                pet.state == State::Crouch && pet.surface.is_some() && pet.target_id().is_none();
            if let Some(id) = pet.target_id() {
                targets.insert(id);
            }
        }
    }
    assert!(
        saw_crouch && saw_climb && saw_approach,
        "routes must start through normal behavior decisions"
    );
    assert!(
        saw_voluntary_descent,
        "a pet must decide to leave its window without a manual command"
    );
    assert!(
        targets.len() >= 2,
        "exploration must choose more than one window: {targets:?}"
    );
}
fn window(id: usize, rect: Rect) -> WindowInfo {
    WindowInfo {
        id,
        rect,
        title: String::new(),
        active: false,
        fullscreen: false,
        is_file_manager: false,
        is_desktop_icon: false,
    }
}
