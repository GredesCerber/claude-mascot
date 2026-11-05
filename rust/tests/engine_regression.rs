use claude_mascot::engine::{update_interactions, Attack, Command, Pet, Skin, State};
use claude_mascot::{Rect, Vec2, WindowInfo, World};

fn world() -> World {
    World {
        windows: vec![],
        work_areas: vec![Rect::new(0.0, 0.0, 1000.0, 700.0)],
        cursor: Vec2::new(500.0, 300.0),
        idle_seconds: 0.0,
        video_window: None,
    }
}

fn window(id: usize, rect: Rect) -> WindowInfo {
    WindowInfo {
        id,
        rect,
        title: String::new(),
        active: false,
        fullscreen: false,
    }
}

fn step(p: &mut Pet, w: &World, n: usize) {
    for _ in 0..n {
        p.update(1.0 / 60.0, w);
    }
}

#[test]
fn falling_pet_lands_on_desktop_floor() {
    let w = world();
    let mut p = Pet::new(1, Skin::Claude, Vec2::new(200.0, 100.0));
    step(&mut p, &w, 120);
    assert_eq!(p.surface, None);
    assert!((p.pos.y - 700.0).abs() < 0.1);
    assert_ne!(p.state, State::Fall);
}

#[test]
fn grounded_pet_tracks_taskbar_work_area_changes_without_falling() {
    let mut w = world();
    let mut p = Pet::new(101, Skin::Claude, Vec2::new(200.0, 700.0));
    p.state = State::Idle;
    p.update(1.0 / 60.0, &w);

    // Taskbar shown: the usable desktop becomes shorter.
    w.work_areas[0].h = 660.0;
    p.update(1.0 / 60.0, &w);
    assert_eq!(p.state, State::Idle);
    assert!((p.pos.y - 660.0).abs() < 0.1);

    // Taskbar hidden: usable desktop expands. The floor follows the pet in
    // the same update, rather than leaving it airborne until a later crossing.
    w.work_areas[0].h = 700.0;
    p.update(1.0 / 60.0, &w);
    assert_eq!(p.state, State::Idle);
    assert!((p.pos.y - 700.0).abs() < 0.1);
}

#[test]
fn landing_keeps_horizontal_inertia_and_ground_friction_slows_it() {
    let w = world();
    let mut p = Pet::new(102, Skin::Claude, Vec2::new(200.0, 700.0));
    p.state = State::Land;
    p.vel.x = 300.0;
    let start_x = p.pos.x;
    p.update(0.05, &w);
    assert!(
        p.pos.x > start_x + 10.0,
        "landing should carry horizontal momentum"
    );
    let speed_after_first_step = p.vel.x.abs();
    for _ in 0..8 {
        p.update(0.05, &w);
    }
    assert!(p.vel.x.abs() < speed_after_first_step);
    assert!(
        p.vel.x.abs() > 0.0,
        "friction should slow the slide progressively"
    );
}

#[test]
fn releasing_dragged_pet_below_screen_recovers_to_desktop_floor() {
    let w = world();
    let mut p = Pet::new(103, Skin::Claude, Vec2::new(300.0, 300.0));
    p.drag_to(Vec2::new(300.0, 1000.0));
    p.release(Vec2::default());

    step(&mut p, &w, 120);
    assert!((p.pos.y - 700.0).abs() < 0.1);
    assert_ne!(p.state, State::Fall);
    assert!(p.pos.x >= 24.0 && p.pos.x <= 976.0);
}

#[test]
fn offscreen_release_uses_remaining_negative_origin_monitor() {
    let mut w = world();
    let mut p = Pet::new(104, Skin::Claude, Vec2::new(300.0, 300.0));
    p.drag_to(Vec2::new(5000.0, 9000.0));
    // The monitor under the drag position is removed before release/update.
    w.work_areas = vec![Rect::new(-1920.0, -100.0, 1920.0, 1080.0)];
    p.release(Vec2::default());

    step(&mut p, &w, 120);
    assert!((p.pos.y - 980.0).abs() < 0.1);
    assert!(p.pos.x >= -1896.0 && p.pos.x <= -24.0);
    assert_ne!(p.state, State::Fall);
}

#[test]
fn dragged_release_above_monitor_starts_a_normal_visible_fall() {
    let w = world();
    let mut p = Pet::new(105, Skin::Claude, Vec2::new(300.0, 300.0));
    p.drag_to(Vec2::new(300.0, -1000.0));
    p.release(Vec2::default());
    p.update(1.0 / 60.0, &w);
    assert!(p.pos.y >= 0.0 && p.pos.y < 10.0);
    assert_eq!(p.state, State::Fall);
}

#[test]
fn falling_pet_lands_on_topmost_unoccluded_window() {
    let mut w = world();
    w.windows
        .push(window(10, Rect::new(100.0, 300.0, 500.0, 200.0)));
    w.windows
        .push(window(11, Rect::new(150.0, 200.0, 300.0, 80.0)));
    let mut p = Pet::new(2, Skin::Claude, Vec2::new(250.0, 50.0));
    for _ in 0..120 {
        p.update(1.0 / 60.0, &w);
        if p.surface.is_some() {
            break;
        }
    }
    assert_eq!(p.surface, Some(1));
    assert!((p.pos.y - 200.0).abs() < 0.1);
}

#[test]
fn occluded_lower_platform_is_not_selected() {
    let mut w = world();
    w.windows
        .push(window(10, Rect::new(100.0, 250.0, 500.0, 200.0)));
    // Upper window covers only the crossing point on the lower window's top.
    w.windows
        .push(window(11, Rect::new(200.0, 240.0, 100.0, 140.0)));
    let mut p = Pet::new(3, Skin::Claude, Vec2::new(240.0, 40.0));
    for _ in 0..120 {
        p.update(1.0 / 60.0, &w);
        if p.surface.is_some() {
            break;
        }
    }
    assert_eq!(p.surface, Some(1));
    assert!((p.pos.y - 240.0).abs() < 0.1);
}

#[test]
fn missing_surface_recovers_to_fall_and_lands() {
    let mut w = world();
    w.windows
        .push(window(10, Rect::new(100.0, 300.0, 500.0, 200.0)));
    let mut p = Pet::new(4, Skin::Claude, Vec2::new(200.0, 300.0));
    p.surface = Some(0);
    p.state = State::Idle;
    p.update(1.0 / 60.0, &w);
    w.windows.clear();
    p.update(1.0 / 60.0, &w);
    assert_eq!(p.surface, None);
    assert_eq!(p.state, State::Fall);
    step(&mut p, &w, 120);
    assert!((p.pos.y - 700.0).abs() < 0.1);
}

#[test]
fn moving_window_carries_pet_and_large_shake_throws_it() {
    let mut w = world();
    w.windows
        .push(window(10, Rect::new(100.0, 300.0, 500.0, 200.0)));
    let mut p = Pet::new(5, Skin::Claude, Vec2::new(200.0, 300.0));
    p.surface = Some(0);
    p.state = State::Idle;
    p.update(1.0 / 60.0, &w); // establish the prior window position
    w.windows[0].rect.x += 10.0;
    w.windows[0].rect.y += 5.0;
    p.update(1.0 / 60.0, &w);
    assert!((p.pos.x - 210.0).abs() < 0.01 && (p.pos.y - 305.0).abs() < 0.01);
    w.windows[0].rect.x += 80.0;
    p.update(1.0 / 60.0, &w);
    assert_eq!(p.surface, None);
    assert_eq!(p.state, State::Thrown);
}

#[test]
fn surface_stays_with_window_when_enumeration_order_changes() {
    let mut w = world();
    w.windows
        .push(window(10, Rect::new(100.0, 300.0, 500.0, 200.0)));
    w.windows
        .push(window(11, Rect::new(700.0, 250.0, 200.0, 150.0)));
    let mut p = Pet::new(51, Skin::Claude, Vec2::new(200.0, 300.0));
    p.surface = Some(0);
    p.state = State::Idle;
    p.update(1.0 / 60.0, &w); // caches native window ID 10
    w.windows.swap(0, 1);
    w.windows[1].rect.x += 15.0;
    p.update(1.0 / 60.0, &w);
    assert_eq!(p.surface, Some(1));
    assert!((p.pos.x - 215.0).abs() < 0.01);
    assert!((p.pos.y - 300.0).abs() < 0.01);
}

#[test]
fn dt_is_bounded_and_invalid_world_values_recover() {
    let w = world();
    let mut p = Pet::new(6, Skin::Claude, Vec2::new(100.0, 100.0));
    p.update(10_000.0, &w);
    assert!(
        p.pos.y < 200.0,
        "a long frame must not tunnel or apply 10 seconds of gravity"
    );
    p.pos = Vec2::new(f64::NAN, 0.0);
    p.update(0.016, &w);
    assert!(p.pos.x.is_finite() && p.pos.y.is_finite());
    assert_eq!(p.surface, None);
}

#[test]
fn idle_sleep_starts_at_sixty_seconds_and_wakes_on_input() {
    let mut w = world();
    let mut p = Pet::new(7, Skin::Claude, Vec2::new(100.0, 700.0));
    p.state = State::Idle;
    w.idle_seconds = 60.0;
    p.update(0.016, &w);
    assert_eq!(p.state, State::Sleep);
    w.idle_seconds = 0.0;
    p.update(0.016, &w);
    assert_eq!(p.state, State::Idle);
}

#[test]
fn manual_sleep_persists_until_a_new_input_after_sleep_begins() {
    let mut w = world();
    let mut p = Pet::new(70, Skin::Claude, Vec2::new(100.0, 700.0));
    p.command(Command::Sleep, &w);
    p.update(0.016, &w);
    assert_eq!(p.state, State::Sleep);
    w.idle_seconds = 2.0;
    p.update(0.016, &w);
    assert_eq!(p.state, State::Sleep);
    w.idle_seconds = 0.0;
    p.update(0.016, &w);
    assert_eq!(p.state, State::Idle);
}

#[test]
fn fullscreen_is_a_host_visibility_decision_not_an_engine_pause() {
    let mut w = world();
    let mut active = window(12, Rect::new(0.0, 0.0, 1000.0, 700.0));
    active.active = true;
    active.fullscreen = true;
    w.windows.push(active);
    let mut p = Pet::new(71, Skin::Claude, Vec2::new(200.0, 100.0));
    let before = p.pos.y;
    p.update(0.05, &w);
    assert!(p.pos.y > before);
}

#[test]
fn cursor_chase_clings_then_mouse_shake_knocks_pet_off() {
    let mut w = world();
    let mut p = Pet::new(8, Skin::Claude, Vec2::new(490.0, 300.0));
    p.chase = true;
    p.state = State::Idle;
    p.update(0.016, &w);
    assert_eq!(p.state, State::Hold);
    for i in 0..12 {
        w.cursor.x = if i % 2 == 0 { 400.0 } else { 500.0 };
        w.cursor.y = 300.0;
        p.update(0.1, &w);
        if p.state != State::Hold {
            break;
        }
    }
    assert_ne!(p.state, State::Hold);
    assert_eq!(p.state, State::Thrown);
}

#[test]
fn confirmed_video_can_trigger_peek_and_stops_when_session_ends() {
    let mut w = world();
    w.windows
        .push(window(99, Rect::new(100.0, 400.0, 500.0, 200.0)));
    w.video_window = Some(99);
    let mut p = Pet::new(9, Skin::Claude, Vec2::new(50.0, 700.0));
    // Deterministic seed eventually hits the intentionally rare idle peek draw.
    for _ in 0..10000 {
        p.update(0.05, &w);
        if p.state == State::Peek {
            break;
        }
    }
    assert_eq!(p.state, State::Peek);
    assert!((p.pos.y - 400.0).abs() < 1.0);
    assert!(
        (76.0..=100.0).contains(&p.pos.x) || (600.0..=624.0).contains(&p.pos.x),
        "peek should finish at a supported edge beside the confirmed media window"
    );
    w.video_window = None;
    p.update(0.016, &w);
    assert_ne!(p.state, State::Peek);
}

#[test]
fn tagilla_hunts_and_pairwise_outcome_is_reproducible() {
    let w = world();
    let mut pets = [
        Pet::new(10, Skin::Tagilla, Vec2::new(100.0, 700.0)),
        Pet::new(11, Skin::Claude, Vec2::new(500.0, 700.0)),
    ];
    pets[0].rage = true;
    update_interactions(&mut pets, 0.05, &w);
    assert_eq!(pets[0].target_id(), Some(11));
    assert_eq!(pets[0].state, State::Hunt);
    assert_eq!(pets[1].state, State::Flee);
    let first = (pets[0].pos, pets[1].pos, pets[0].state, pets[1].state);

    let mut again = [
        Pet::new(10, Skin::Tagilla, Vec2::new(100.0, 700.0)),
        Pet::new(11, Skin::Claude, Vec2::new(500.0, 700.0)),
    ];
    again[0].rage = true;
    update_interactions(&mut again, 0.05, &w);
    assert_eq!(
        first,
        (again[0].pos, again[1].pos, again[0].state, again[1].state)
    );
}

#[test]
fn tagilla_never_targets_itself_or_another_tagilla() {
    let w = world();
    let mut pets = [
        Pet::new(80, Skin::Tagilla, Vec2::new(100.0, 700.0)),
        Pet::new(81, Skin::Tagilla, Vec2::new(130.0, 700.0)),
    ];
    pets[0].rage = true;
    update_interactions(&mut pets, 0.05, &w);
    assert_eq!(pets[0].target_id(), None);
    assert_eq!(pets[0].state, State::Fall);
    assert_eq!(pets[1].state, State::Fall);
}

#[test]
fn commands_drag_release_pet_and_jump_are_simulation_only() {
    let w = world();
    let mut p = Pet::new(12, Skin::Claude, Vec2::new(100.0, 700.0));
    p.command(Command::Sit, &w);
    assert_eq!(p.state, State::Sit);
    p.pet();
    assert!(p.hearts > 0.0);
    p.jump();
    assert_eq!(p.state, State::Jump);
    p.drag_to(Vec2::new(200.0, 200.0));
    assert!(p.dragging);
    assert_eq!(p.pos, Vec2::new(200.0, 200.0));
    p.release(Vec2::new(400.0, -200.0));
    assert!(!p.dragging);
    assert_eq!(p.state, State::Thrown);
    assert_eq!(p.vel.x, 400.0);
}

#[test]
fn unsupported_sleep_command_falls_to_floor_before_sleeping() {
    let w = world();
    let mut p = Pet::new(120, Skin::Claude, Vec2::new(100.0, 200.0));
    p.command(Command::Sleep, &w);
    assert_eq!(p.state, State::Fall);
    step(&mut p, &w, 120);
    assert_eq!(p.state, State::Sleep);
    assert!((p.pos.y - 700.0).abs() < 0.1);
}

#[test]
fn climb_uses_selected_edge_and_ledge_pose_finishes() {
    let mut w = world();
    w.windows
        .push(window(121, Rect::new(100.0, 300.0, 500.0, 200.0)));
    let mut p = Pet::new(121, Skin::Claude, Vec2::new(200.0, 450.0));
    p.command(Command::Climb, &w);
    assert_eq!(p.state, State::Climb);
    assert!(
        p.facing < 0.0,
        "the nearer left edge determines climb direction"
    );
    for _ in 0..240 {
        p.update(1.0 / 60.0, &w);
        if p.state == State::Ledge {
            break;
        }
    }
    assert_eq!(p.state, State::Ledge);
    assert!((p.pos.x - 76.0).abs() < 0.1);
    step(&mut p, &w, 40);
    assert_eq!(p.state, State::Idle);
}

#[test]
fn smash_without_a_window_does_not_invent_an_attack_target() {
    let w = world();
    let mut p = Pet::new(122, Skin::Tagilla, Vec2::new(200.0, 700.0));
    p.state = State::Idle;
    p.command(Command::Smash, &w);
    assert_eq!(p.state, State::Idle);
    assert_eq!(p.attack, None);
    assert_eq!(p.attack_serial, 0);
}

#[test]
fn window_smash_emits_one_typed_impact_for_real_window() {
    let mut w = world();
    w.windows
        .push(window(123, Rect::new(100.0, 300.0, 500.0, 200.0)));
    let mut p = Pet::new(123, Skin::Tagilla, Vec2::new(200.0, 300.0));
    p.surface = Some(0);
    p.state = State::Idle;
    p.command(Command::Smash, &w);
    assert_eq!(p.state, State::Smash);
    assert!(matches!(
        p.attack,
        Some(Attack::Window { window_id: 123, .. })
    ));
    step(&mut p, &w, 22);
    assert_eq!(p.attack_serial, 1);
    step(&mut p, &w, 45);
    assert_eq!(p.attack_serial, 1, "one swing emits one impact");
    assert_eq!(p.state, State::Idle);
    assert_eq!(p.attack, None);
}

#[test]
fn window_smash_approaches_then_leaps_to_reachable_window_top() {
    let mut w = world();
    w.windows
        .push(window(125, Rect::new(100.0, 300.0, 500.0, 200.0)));
    let mut p = Pet::new(125, Skin::Tagilla, Vec2::new(900.0, 700.0));
    p.state = State::Idle;
    p.command(Command::Smash, &w);
    assert_eq!(p.state, State::Walk);
    assert!(matches!(
        p.attack,
        Some(Attack::Window { window_id: 125, .. })
    ));
    for _ in 0..600 {
        p.update(0.05, &w);
        if p.state == State::Smash {
            break;
        }
    }
    assert_eq!(
        p.state,
        State::Smash,
        "route ended at {:?} pos={:?} vel={:?}",
        p.state,
        p.pos,
        p.vel
    );
    assert_eq!(p.surface, Some(0));
    assert!((p.pos.y - 300.0).abs() < 1.0);
}

#[test]
fn tagilla_cursor_attack_requires_chase_and_emits_typed_impact() {
    let mut w = world();
    w.cursor = Vec2::new(500.0, 300.0);
    let mut p = Pet::new(124, Skin::Tagilla, Vec2::new(480.0, 300.0));
    p.chase = true;
    p.state = State::Idle;
    p.update(0.016, &w);
    assert_eq!(p.state, State::Smash);
    assert!(matches!(p.attack, Some(Attack::Cursor { point }) if point == w.cursor));
    step(&mut p, &w, 22);
    assert_eq!(p.attack_serial, 1);
}

#[test]
fn changing_skin_clears_skin_specific_state_but_keeps_position() {
    let mut p = Pet::new(90, Skin::Tagilla, Vec2::new(230.0, 300.0));
    p.rage = true;
    p.rage_forever = true;
    p.chase = true;
    p.state = State::Smash;
    p.bubble = Some("БАХ!".into());
    p.surface = Some(3);
    p.set_skin(Skin::Claude);
    assert_eq!(p.skin, Skin::Claude);
    assert_eq!(p.pos, Vec2::new(230.0, 300.0));
    assert_eq!(p.surface, Some(3));
    assert_eq!(p.state, State::Idle);
    assert!(!p.rage && !p.rage_forever && !p.chase);
    assert!(p.bubble.is_none());
}
