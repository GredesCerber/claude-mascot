use claude_mascot::{
    engine::{Command, Pet, Skin, State},
    Vec2, World,
};

fn grounded_pet(id: u64) -> Pet {
    let mut pet = Pet::new(id, Skin::Claude, Vec2::new(500.0, 1040.0));
    pet.state = State::Idle;
    pet
}

#[test]
fn autonomous_walk_bouts_pause_and_resume() {
    for id in [1, 7, 31, 71, 143, 250, 531, 1_001] {
        check_walk_bout(id);
    }
}

fn check_walk_bout(id: u64) {
    let world = World::default();
    let mut pet = grounded_pet(id);
    let mut saw_bout = false;
    let mut saw_bout_end = false;
    let mut saw_resume = false;
    let mut bout_state = None;
    let mut bout_ticks = 0;
    let mut previous_state = pet.state;
    let mut stopped_at = pet.pos.x;

    for _ in 0..5_000 {
        pet.update(0.05, &world);
        if matches!(pet.state, State::Walk | State::Run) {
            if bout_state != Some(pet.state) {
                bout_state = Some(pet.state);
                bout_ticks = 0;
            }
            bout_ticks += 1;
            if saw_bout_end {
                saw_resume = true;
                break;
            }
            saw_bout = true;
        }
        if saw_bout && previous_state != State::Idle && pet.state == State::Idle {
            let min_ticks = if bout_state == Some(State::Run) {
                19
            } else {
                59
            };
            let max_ticks = if bout_state == Some(State::Run) {
                54
            } else {
                164
            };
            assert!(
                (min_ticks..=max_ticks).contains(&bout_ticks),
                "autonomous {:?} bout lasted {} ticks",
                bout_state.unwrap(),
                bout_ticks
            );
            saw_bout_end = true;
            stopped_at = pet.pos.x;
        }
        if saw_bout_end && pet.state == State::Idle {
            assert!((pet.pos.x - stopped_at).abs() < 1.0);
        }
        previous_state = pet.state;
    }

    assert!(
        saw_bout,
        "autonomous behavior should start a walk or run bout"
    );
    assert!(saw_bout_end, "a bounded bout should end in a pause");
    assert!(
        saw_resume,
        "the pet should make a new decision after pausing"
    );
}

#[test]
fn autonomous_scheduler_leaves_sleep_and_sit_alone() {
    let world = World::default();
    let mut sleeper = grounded_pet(72);
    sleeper.command(Command::Sleep, &world);
    let mut sleep_world = world.clone();
    sleep_world.idle_seconds = 60.0;
    let sleep_position = sleeper.pos;
    for _ in 0..100 {
        sleeper.update(0.05, &sleep_world);
    }
    assert_eq!(sleeper.state, State::Sleep);
    assert_eq!(sleeper.pos, sleep_position);

    let mut sitter = grounded_pet(73);
    sitter.command(Command::Sit, &world);
    for _ in 0..20 {
        sitter.update(0.05, &world);
    }
    assert_eq!(sitter.state, State::Sit);
}

#[test]
fn setting_zero_intensity_during_a_walk_stops_further_autonomous_choices() {
    let world = World::default();
    let mut pet = grounded_pet(71);
    pet.auto_activities = false;
    pet.auto_reactions = false;
    for _ in 0..5_000 {
        pet.update(0.05, &world);
        if matches!(pet.state, State::Walk | State::Run) {
            break;
        }
    }
    assert!(matches!(pet.state, State::Walk | State::Run));
    pet.activity_intensity = 0;
    for _ in 0..200 {
        pet.update(0.05, &world);
    }
    assert_eq!(pet.state, State::Idle);
    let position = pet.pos;
    for _ in 0..1_000 {
        pet.update(0.05, &world);
        assert_eq!(pet.state, State::Idle);
        assert_eq!(pet.pos, position);
    }
}
