use claude_mascot::engine::{Command, Pet, Skin, State};
use claude_mascot::{Rect, Vec2, WindowInfo, World};

fn window(id: usize, active: bool, explorer: bool, x: f64) -> WindowInfo {
    WindowInfo {
        id,
        rect: Rect::new(x, 100.0, 300.0, 220.0),
        title: String::new(),
        active,
        fullscreen: false,
        is_file_manager: explorer,
        is_desktop_icon: false,
    }
}

fn world() -> World {
    World {
        work_areas: vec![Rect::new(0.0, 0.0, 1000.0, 700.0)],
        ..World::default()
    }
}

fn step(pet: &mut Pet, world: &World, count: usize) {
    for _ in 0..count {
        pet.update(1.0 / 60.0, world);
    }
}

fn settled() -> Pet {
    let mut pet = Pet::new(81, Skin::Claude, Vec2::new(400.0, 100.0));
    step(&mut pet, &world(), 180);
    pet
}

#[test]
fn newly_active_file_manager_gets_a_brief_notice_and_attention_point() {
    let mut pet = settled();
    let mut w = world();
    step(&mut pet, &w, 15); // establish initial window baseline
    w.windows.push(window(22, true, true, 600.0));
    step(&mut pet, &w, 8);
    assert_eq!(pet.state, State::Wave);
    assert!(pet.attention_target.is_some());
    step(&mut pet, &w, 20);
    assert!(pet.gaze.x > 0.0 && pet.gaze.y < 0.0);
}

#[test]
fn active_file_manager_navigation_title_change_is_not_missed() {
    let mut w = world();
    w.windows.push(window(22, true, true, 600.0));
    let mut pet = Pet::new(83, Skin::Claude, Vec2::new(400.0, 100.0));
    step(&mut pet, &w, 180);
    assert_ne!(pet.state, State::Wave);
    w.windows[0].title = "New folder".to_owned();
    step(&mut pet, &w, 10);
    assert_eq!(pet.state, State::Wave);
}

#[test]
fn startup_windows_and_reordering_do_not_trigger_reactions() {
    let mut pet = Pet::new(82, Skin::Claude, Vec2::new(400.0, 100.0));
    // Isolate focus/reordering from optional exploration and voluntary descent.
    pet.activity_intensity = 0;
    let mut w = world();
    w.windows = vec![window(1, false, true, 50.0), window(2, true, false, 400.0)];
    step(&mut pet, &w, 190);
    assert_ne!(pet.state, State::Wave);
    w.windows.reverse();
    step(&mut pet, &w, 20);
    assert_ne!(pet.state, State::Wave);
    w.windows[1].active = true;
    step(&mut pet, &w, 10);
    assert_eq!(pet.state, State::Wave);
}

#[test]
fn cursor_play_is_bounded_and_does_not_change_persistent_manual_chase() {
    let mut pet = settled();
    let mut w = world();
    w.cursor = Vec2::new(500.0, 680.0);
    step(&mut pet, &w, 5);
    for i in 0..55 {
        w.cursor.x = 505.0 + (i % 8) as f64 * 3.0;
        pet.update(1.0 / 60.0, &w);
    }
    assert_eq!(pet.state, State::Chase);
    assert!(!pet.chase);
    step(&mut pet, &w, 100);
    assert!(!pet.chase);
    assert_ne!(pet.state, State::Chase);

    pet.command(Command::Chase, &w);
    assert!(pet.chase);
    step(&mut pet, &w, 80);
    assert!(pet.chase);
    pet.jump();
    assert!(!pet.chase);
    assert_eq!(pet.state, State::Jump);
}

#[test]
fn cursor_dwell_survives_30hz_world_updates_with_240hz_physics() {
    let mut pet = settled();
    let mut w = world();
    let mut saw_auto_chase = false;
    for frame in 0..45 {
        w.cursor = Vec2::new(565.0 + (frame % 6) as f64 * 3.0, 680.0);
        step(&mut pet, &w, 8);
        saw_auto_chase |= pet.state == State::Chase;
    }
    assert!(saw_auto_chase);
    assert!(!pet.chase);
}

#[test]
fn explicit_sleep_is_not_interrupted_by_cursor_or_window_events() {
    let mut pet = settled();
    let mut w = world();
    pet.command(Command::Sleep, &w);
    w.windows.push(window(7, true, true, 100.0));
    w.cursor = Vec2::new(410.0, 680.0);
    step(&mut pet, &w, 80);
    assert_eq!(pet.state, State::Sleep);
}

#[test]
fn window_reactions_leave_manual_activity_and_dragging_alone() {
    let mut w = world();
    let mut activity_pet = settled();
    activity_pet.command(
        Command::Activity(claude_mascot::activities::CATALOG[0].id),
        &w,
    );
    w.windows.push(window(71, true, true, 600.0));
    step(&mut activity_pet, &w, 10);
    assert_eq!(activity_pet.state, State::Activity);

    let mut dragged = settled();
    let mut drag_world = world();
    dragged.drag_to(Vec2::new(400.0, 400.0));
    drag_world.windows.push(window(72, true, true, 600.0));
    step(&mut dragged, &drag_world, 8);
    assert!(dragged.dragging);
    assert_eq!(dragged.state, State::Thrown);
}

#[test]
fn disabled_reactions_rebaseline_windows_without_replaying_them() {
    let mut pet = settled();
    let mut w = world();
    pet.auto_reactions = false;
    w.windows.push(window(51, true, true, 600.0));
    step(&mut pet, &w, 30);
    pet.auto_reactions = true;
    step(&mut pet, &w, 20);
    assert_ne!(pet.state, State::Wave);
}

#[test]
fn petting_cancels_short_autonomous_cursor_play() {
    let mut pet = settled();
    let mut w = world();
    let mut began = false;
    for i in 0..55 {
        w.cursor = Vec2::new(505.0 + (i % 8) as f64 * 3.0, 680.0);
        pet.update(1.0 / 60.0, &w);
        if pet.state == State::Chase {
            began = true;
            pet.pet();
            break;
        }
    }
    assert!(began);
    assert_eq!(pet.state, State::Happy);
    assert!(!pet.chase);
}

#[test]
fn manual_climb_to_a_distant_icon_uses_physics_without_teleporting() {
    let mut pet = settled();
    let before = pet.pos;
    let mut w = world();
    w.windows.push(WindowInfo {
        id: usize::MAX - 12,
        rect: Rect::new(540.0, 480.0, 40.0, 48.0),
        title: String::new(),
        active: false,
        fullscreen: false,
        is_file_manager: false,
        is_desktop_icon: true,
    });
    pet.command(Command::Climb, &w);
    assert_eq!(pet.state, State::Jump);
    assert_eq!(pet.pos, before);
    assert!(pet.vel.y < 0.0);
}

#[test]
fn nearby_cursor_gaze_is_strong_directional_and_far_cursor_fades() {
    let mut pet = settled();
    let mut w = world();
    w.cursor = Vec2::new(pet.pos.x + 30.0, pet.pos.y - 40.0);
    step(&mut pet, &w, 20);
    assert!(pet.gaze.x > 0.8);
    w.cursor = Vec2::new(pet.pos.x - 60.0, pet.pos.y - 40.0);
    step(&mut pet, &w, 30);
    assert!(pet.gaze.x < -0.8);
    w.cursor = Vec2::new(900.0, 100.0);
    step(&mut pet, &w, 40);
    assert!(pet.gaze.x.abs() < 0.02);
    assert!(pet.gaze.x.is_finite() && pet.gaze.y.is_finite());
}
