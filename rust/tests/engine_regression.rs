use claude_mascot::engine::{Command, Pet, Skin, State};
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
        is_file_manager: false,
        is_desktop_icon: false,
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
fn fast_thrown_pet_bounces_off_outer_wall_and_squashes() {
    let w = world();
    let mut p = Pet::new(201, Skin::Claude, Vec2::new(950.0, 300.0));
    p.release(Vec2::new(1200.0, 0.0));
    p.update(0.05, &w);
    assert!(p.pos.x <= 976.0);
    assert!(
        p.vel.x < 0.0,
        "wall impact should reverse horizontal velocity"
    );
    assert!(p.squash > 0.0, "wall impact should trigger squash feedback");
}

#[test]
fn slow_thrown_pet_keeps_horizontal_momentum_in_open_air() {
    let w = world();
    let mut p = Pet::new(211, Skin::Claude, Vec2::new(300.0, 300.0));
    p.release(Vec2::new(200.0, 0.0));
    p.update(1.0 / 60.0, &w);
    assert!(
        p.pos.x > 300.0,
        "slow horizontal throw should keep drifting"
    );
    assert_eq!(p.vel.x, 200.0, "only wall contact should stop the drift");
}

#[test]
fn thrown_pet_crosses_adjacent_monitor_seam_without_bouncing() {
    let mut w = world();
    w.work_areas = vec![
        Rect::new(0.0, 0.0, 500.0, 700.0),
        Rect::new(500.0, 0.0, 500.0, 700.0),
    ];
    let mut p = Pet::new(202, Skin::Claude, Vec2::new(450.0, 300.0));
    p.release(Vec2::new(900.0, 0.0));
    for _ in 0..40 {
        p.update(1.0 / 240.0, &w);
    }
    assert!(p.pos.x > 500.0);
    assert!(p.vel.x > 0.0, "monitor seam should remain traversable");
}

#[test]
fn horizontal_floor_throw_rebounds_during_landing_slide() {
    let w = world();
    let mut p = Pet::new(221, Skin::Claude, Vec2::new(950.0, 700.0));
    p.release(Vec2::new(1200.0, 0.0));
    for _ in 0..12 {
        p.update(1.0 / 240.0, &w);
    }
    assert!(p.vel.x < 0.0, "a grounded throw should rebound too");
    assert!(p.pos.x <= 976.0);
}

#[test]
fn walking_pet_turns_at_outer_wall_instead_of_walking_in_place() {
    let w = world();
    let mut p = Pet::new(222, Skin::Claude, Vec2::new(975.0, 700.0));
    p.state = State::Walk;
    p.facing = 1.0;
    p.update(0.05, &w);
    assert!(p.facing < 0.0);
    let boundary = p.pos.x;
    p.update(0.05, &w);
    assert!(p.pos.x < boundary);
}

#[test]
fn cursor_chase_jumps_toward_cursor_above_and_returns_to_chasing_after_landing() {
    let mut w = world();
    w.cursor = Vec2::new(700.0, 400.0);
    let mut p = Pet::new(203, Skin::Claude, Vec2::new(300.0, 700.0));
    p.chase = true;
    p.state = State::Idle;
    p.update(1.0 / 60.0, &w);
    assert_eq!(p.state, State::Jump);
    assert!(p.vel.y < 0.0);
    let start_y = p.pos.y;
    p.update(1.0 / 60.0, &w);
    assert!(
        p.pos.y < start_y,
        "launched chase hop should follow normal airborne physics"
    );
}

#[test]
fn cursor_chase_catches_cursor_at_jump_apex_and_hangs_from_hand_anchor() {
    let mut w = world();
    w.cursor = Vec2::new(350.0, 400.0);
    let mut p = Pet::new(206, Skin::Claude, Vec2::new(300.0, 700.0));
    p.chase = true;
    p.state = State::Idle;
    p.update(1.0 / 60.0, &w);
    assert_eq!(p.state, State::Jump);
    for _ in 0..80 {
        p.update(1.0 / 60.0, &w);
        if p.state == State::Hold {
            break;
        }
    }
    assert_eq!(
        p.state,
        State::Hold,
        "the airborne chase should grab the cursor near the apex"
    );
    assert!((p.pos.x - w.cursor.x).abs() < 1.0);
    assert!((p.pos.y - (w.cursor.y + 70.0)).abs() < 1.0);
}

#[test]
fn autonomous_pet_eventually_leaps_to_a_visible_reachable_window_top() {
    let mut w = world();
    w.windows
        .push(window(207, Rect::new(100.0, 400.0, 800.0, 180.0)));
    let mut p = Pet::new(207, Skin::Claude, Vec2::new(500.0, 700.0));
    p.state = State::Idle;
    for _ in 0..12_000 {
        p.update(1.0 / 60.0, &w);
        if p.surface == Some(0) {
            break;
        }
    }
    assert_eq!(
        p.surface,
        Some(0),
        "pet should autonomously reach the supported window top"
    );
    assert!((p.pos.y - 400.0).abs() < 1.0);
}

#[test]
fn autonomous_window_hops_skip_occluded_and_offscreen_tops() {
    let mut w = world();
    w.windows
        .push(window(208, Rect::new(100.0, 400.0, 800.0, 180.0)));
    // This top is above the work area and covers the lower candidate's landing point.
    w.windows
        .push(window(209, Rect::new(100.0, -100.0, 800.0, 600.0)));
    let mut p = Pet::new(208, Skin::Claude, Vec2::new(500.0, 700.0));
    p.state = State::Idle;
    for _ in 0..12_000 {
        p.update(1.0 / 60.0, &w);
        if p.target_id() == Some(208) || p.target_id() == Some(209) {
            break;
        }
    }
    assert_ne!(
        p.target_id(),
        Some(208),
        "occluded window top must not be selected"
    );
    assert_ne!(
        p.target_id(),
        Some(209),
        "offscreen window top must not be selected"
    );
}

#[test]
fn explicit_sit_and_sleep_do_not_autonomously_window_hop() {
    let mut w = world();
    w.windows
        .push(window(210, Rect::new(100.0, 400.0, 800.0, 180.0)));
    let mut p = Pet::new(210, Skin::Claude, Vec2::new(500.0, 700.0));
    p.command(Command::Sit, &w);
    step(&mut p, &w, 600);
    assert_eq!(p.state, State::Sit);
    assert_eq!(p.target_id(), None);
    p.command(Command::Sleep, &w);
    step(&mut p, &w, 600);
    assert_eq!(p.state, State::Sleep);
    assert_eq!(p.target_id(), None);
}

#[test]
fn dance_and_wave_commands_are_timed_and_mood_changes_are_bounded() {
    let w = world();
    let mut p = Pet::new(204, Skin::Claude, Vec2::new(300.0, 700.0));
    p.command(Command::Dance, &w);
    assert_eq!(p.state, State::Dance);
    step(&mut p, &w, 190);
    assert_eq!(p.state, State::Idle);
    p.command(Command::Wave, &w);
    assert_eq!(p.state, State::Wave);
    step(&mut p, &w, 130);
    assert_eq!(p.state, State::Idle);

    let before = p.satisfaction;
    p.release(Vec2::new(50.0, 20.0));
    assert_eq!(
        p.satisfaction, before,
        "slow repositioning should not lower mood"
    );
    p.release(Vec2::new(1800.0, 0.0));
    assert!(p.satisfaction < before);
    assert!(p.satisfaction >= 0.0 && p.satisfaction <= 100.0);
    let after_throw = p.satisfaction;
    p.pet();
    assert!(p.satisfaction > after_throw);
    let petted = p.satisfaction;
    p.update(1.0 / 60.0, &w);
    assert!(
        p.satisfaction >= petted,
        "passive recovery must not erase the petting bonus"
    );
    assert!(p.satisfaction <= 100.0);
}

#[test]
fn mood_recovers_slowly_after_a_rough_throw() {
    let w = world();
    let mut p = Pet::new(205, Skin::Claude, Vec2::new(300.0, 700.0));
    p.release(Vec2::new(1800.0, 0.0));
    let thrown = p.satisfaction;
    step(&mut p, &w, 300);
    assert!(p.satisfaction > thrown);
    assert!(p.satisfaction <= 75.0);
}

#[test]
fn repeated_rough_throws_make_pet_unhappy_and_petting_calms_it_faster() {
    let w = world();
    let mut p = Pet::new(220, Skin::Claude, Vec2::new(300.0, 500.0));
    for _ in 0..6 {
        p.drag_to(Vec2::new(300.0, 500.0));
        p.release(Vec2::new(1800.0, 0.0));
    }
    assert!(p.satisfaction < 30.0);
    let upset = p.satisfaction;
    step(&mut p, &w, 120);
    assert!(p.satisfaction > upset && p.satisfaction < 30.0);
    let after_wait = p.satisfaction;
    for _ in 0..3 {
        p.pet();
    }
    assert!(p.satisfaction >= 30.0);
    assert!(p.satisfaction - after_wait > 30.0);
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
fn sleeping_pet_tracks_taskbar_work_area_changes() {
    let mut w = world();
    let mut p = Pet::new(106, Skin::Claude, Vec2::new(200.0, 700.0));
    p.command(Command::Sleep, &w);
    p.update(1.0 / 60.0, &w);

    w.work_areas[0].h = 660.0;
    p.update(1.0 / 60.0, &w);
    assert_eq!(p.state, State::Sleep);
    assert!((p.pos.y - 660.0).abs() < 0.1);

    w.work_areas[0].h = 700.0;
    p.update(1.0 / 60.0, &w);
    assert_eq!(p.state, State::Sleep);
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
    w.cursor = Vec2::new(500.0, 230.0);
    let mut p = Pet::new(8, Skin::Claude, Vec2::new(490.0, 300.0));
    p.chase = true;
    p.state = State::Idle;
    p.update(0.016, &w);
    assert_eq!(p.state, State::Hold);
    for i in 0..12 {
        w.cursor.x = if i % 2 == 0 { 400.0 } else { 500.0 };
        w.cursor.y = 230.0;
        p.update(0.1, &w);
        if p.state != State::Hold {
            break;
        }
    }
    assert_ne!(p.state, State::Hold);
    assert_eq!(p.state, State::Thrown);
}

#[test]
fn turning_off_chase_while_holding_drops_without_teleporting_to_floor() {
    let mut w = world();
    w.cursor = Vec2::new(500.0, 230.0);
    let mut p = Pet::new(212, Skin::Claude, Vec2::new(490.0, 300.0));
    p.chase = true;
    p.state = State::Idle;
    p.update(1.0 / 60.0, &w);
    assert_eq!(p.state, State::Hold);
    let held_y = p.pos.y;

    p.chase = false;
    p.update(1.0 / 240.0, &w);
    assert_eq!(p.state, State::Fall);
    assert!(
        (p.pos.y - held_y).abs() < 1.0,
        "drop should begin at the held position"
    );
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
fn quick_jump_launches_from_run_dance_wave_sleep_and_hold() {
    for state in [
        State::Run,
        State::Dance,
        State::Wave,
        State::Sleep,
        State::Hold,
    ] {
        let mut p = Pet::new(220 + state as u64, Skin::Claude, Vec2::new(300.0, 700.0));
        p.state = state;
        p.chase = state == State::Hold;
        p.jump();
        assert_eq!(
            p.state,
            State::Jump,
            "quick jump should launch from {state:?}"
        );
        assert!(p.vel.y < 0.0);
    }
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
fn legacy_tagilla_skin_deserializes_as_claude() {
    let skin: Skin = serde_json::from_str("\"Tagilla\"").unwrap();
    assert_eq!(skin, Skin::Claude);
}

#[test]
fn laptop_open_typing_and_close_cancel_on_drag_jump_or_lost_support() {
    for phase in [0.5, 3.0, 9.5] {
        let mut w = world();
        w.windows
            .push(window(501, Rect::new(100.0, 300.0, 600.0, 300.0)));
        let mut pet = Pet::new(501, Skin::Claude, Vec2::new(300.0, 100.0));
        step(&mut pet, &w, 60);
        assert_eq!(pet.surface, Some(0));
        pet.command(Command::Laptop, &w);
        pet.state_time = phase;

        let mut dragged = pet.clone();
        dragged.drag_to(Vec2::new(400.0, 200.0));
        assert_ne!(dragged.state, State::Laptop);
        assert!(dragged.dragging);

        let mut jumping = pet.clone();
        jumping.jump();
        assert_eq!(jumping.state, State::Jump);

        w.windows.clear();
        pet.update(1.0 / 60.0, &w);
        assert_eq!(pet.state, State::Fall);
    }
}

#[test]
fn laptop_pose_runs_for_ten_seconds_and_manual_sleep_overrides_it() {
    let w = world();
    let mut p = Pet::new(301, Skin::Claude, Vec2::new(300.0, 700.0));
    p.command(Command::Laptop, &w);
    assert_eq!(p.state, State::Laptop);
    step(&mut p, &w, 599);
    assert_eq!(p.state, State::Laptop);
    let mut timed = Pet::new(304, Skin::Claude, Vec2::new(350.0, 700.0));
    timed.command(Command::Laptop, &w);
    step(&mut timed, &w, 599);
    assert_eq!(timed.state, State::Laptop);
    timed.update(1.0 / 60.0, &w);
    assert_eq!(timed.state, State::Idle);
    p.command(Command::Sleep, &w);
    assert_eq!(p.state, State::Sleep);
    step(&mut p, &w, 5);
    assert_eq!(p.state, State::Sleep);
}

#[test]
fn grounded_short_poses_finish_and_jump_cancels_them() {
    let w = world();
    let mut p = Pet::new(302, Skin::Claude, Vec2::new(300.0, 700.0));
    p.command(Command::Stretch, &w);
    assert_eq!(p.state, State::Stretch);
    step(&mut p, &w, 151);
    assert_eq!(p.state, State::Idle);

    p.command(Command::LookAround, &w);
    assert_eq!(p.state, State::LookAround);
    p.jump();
    assert_eq!(p.state, State::Jump);
    step(&mut p, &w, 240);
    assert_ne!(p.state, State::LookAround);
}

#[test]
fn airborne_pose_waits_for_landing_and_drag_cancels_pending_pose() {
    let w = world();
    let mut p = Pet::new(303, Skin::Claude, Vec2::new(300.0, 150.0));
    p.command(Command::Stretch, &w);
    for _ in 0..120 {
        p.update(1.0 / 60.0, &w);
        if p.state == State::Stretch {
            break;
        }
    }
    assert_eq!(p.state, State::Stretch);

    let mut pending = Pet::new(305, Skin::Claude, Vec2::new(300.0, 150.0));
    pending.command(Command::Laptop, &w);
    pending.drag_to(Vec2::new(400.0, 250.0));
    pending.release(Vec2::default());
    step(&mut pending, &w, 180);
    assert_ne!(pending.state, State::Laptop);
}
