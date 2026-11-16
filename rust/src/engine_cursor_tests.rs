use super::*;
fn grounded() -> (Pet, World) {
    let world = World::default();
    let mut pet = Pet::new(44, Skin::Claude, Vec2::new(200.0, 1040.0));
    pet.state = State::Idle;
    pet.auto_activities = false;
    pet.frequencies.wandering = 0;
    pet.frequencies.exploration = 0;
    (pet, world)
}
#[test]
fn slow_cursor_motion_is_not_lost_at_high_physics_rates() {
    for fps in [60, 120, 240] {
        let (mut pet, mut world) = grounded();
        pet.cursor_play_cooldown = 1e6;
        pet.activity_intensity = 0;
        for frame in 0..fps {
            world.cursor = Vec2::new(300.0 + 50.0 * frame as f64 / fps as f64, 1020.0);
            pet.update(1.0 / fps as f64, &world);
        }
        assert!(
            pet.cursor_dwell > 0.8,
            "dwell at {fps}Hz: {}",
            pet.cursor_dwell
        );
        assert!(pet.cursor_motion_recent > 0.0);
    }
}
#[test]
fn cursor_games_complete_one_two_or_three_runs_then_rest() {
    for total in 1..=3 {
        let (mut pet, mut world) = grounded();
        pet.cursor_play_runs_left = total - 1;
        pet.cursor_play_remaining = 20.0;
        pet.cursor_play_cooldown = 180.0;
        pet.start_cursor_run();
        let mut runs = 1;
        for tick in 0..4800 {
            let motion = 12.0 * (tick as f64 / 240.0 * std::f64::consts::TAU).sin();
            world.cursor = Vec2::new(pet.pos.x + 110.0 + motion, 1020.0);
            let running = pet.auto_pursuit;
            pet.update(1.0 / 240.0, &world);
            if !running && pet.auto_pursuit {
                runs += 1;
            }
            if pet.cursor_play_remaining == 0.0 {
                break;
            }
        }
        assert_eq!(runs, total);
        assert_eq!(pet.state, State::Sit);
        assert_eq!(pet.cursor_play_runs_left, 0);
        assert!(!pet.chase && !pet.auto_pursuit);
        assert!(pet.cursor_play_cooldown > 150.0);
    }
}
#[test]
fn manual_sleep_cancels_pending_runs() {
    let (mut pet, world) = grounded();
    pet.cursor_play_runs_left = 2;
    pet.cursor_play_remaining = 20.0;
    pet.start_cursor_run();
    pet.command(Command::Sleep, &world);
    assert_eq!(pet.cursor_play_runs_left, 0);
    assert!(!pet.auto_pursuit);
    assert_eq!(pet.state, State::Sleep);
}
#[test]
fn smooth_shakes_throw_in_the_final_flick_direction_at_all_frame_rates() {
    for fps in [60, 120, 240] {
        for axis in [
            Vec2::new(1.0, 0.0),
            Vec2::new(0.0, 1.0),
            Vec2::new(1.0, 1.0),
        ] {
            let (mut pet, mut world) = grounded();
            world.cursor = Vec2::new(700.0, 500.0);
            pet.chase = true;
            pet.initialized = true;
            pet.pos = Vec2::new(700.0, 570.0);
            assert!(pet.try_catch_cursor(&world));
            let mut thrown = false;
            for frame in 1..=fps * 2 {
                let previous = world.cursor;
                let offset = 80.0 * (frame as f64 / fps as f64 * 5.0 * std::f64::consts::TAU).sin();
                world.cursor = Vec2::new(700.0 + axis.x * offset, 500.0 + axis.y * offset);
                pet.update(1.0 / fps as f64, &world);
                if pet.state == State::Thrown {
                    let delta = Vec2::new(world.cursor.x - previous.x, world.cursor.y - previous.y);
                    assert!(
                        pet.vel.x * delta.x + pet.vel.y * delta.y > 0.0,
                        "wrong flick at {fps}Hz"
                    );
                    if axis.x == 0.0 {
                        assert_eq!(pet.vel.x, 0.0);
                    }
                    if axis.y == 0.0 {
                        assert_eq!(pet.vel.y, 0.0);
                    }
                    assert!(pet.vel.x.hypot(pet.vel.y) > 50.0);
                    thrown = true;
                    break;
                }
            }
            assert!(thrown, "smooth shake not detected at {fps}Hz on {axis:?}");
        }
    }
}
