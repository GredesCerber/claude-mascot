use claude_mascot::{
    dialogue::SpeechEvent,
    engine::{Command, Pet, Skin, State},
    Vec2, World,
};

#[test]
fn speech_choices_do_not_change_simulation_randomness() {
    let world = World::default();
    let mut speaking = Pet::new(70, Skin::Claude, Vec2::new(500.0, 1040.0));
    speaking.state = State::Idle;
    let mut quiet = speaking.clone();
    for step in 0..6000 {
        speaking.update(1.0 / 60.0, &world);
        quiet.update(1.0 / 60.0, &world);
        if step % 100 == 0 {
            speaking.speak(SpeechEvent::Wave, 2.0);
        }
        assert_eq!(
            (
                speaking.pos,
                speaking.vel,
                speaking.state,
                speaking.satisfaction
            ),
            (quiet.pos, quiet.vel, quiet.state, quiet.satisfaction)
        );
    }
}

#[test]
fn repeated_real_laptop_actions_receive_different_lines() {
    let world = World::default();
    let mut pet = Pet::new(71, Skin::Claude, Vec2::new(500.0, 1040.0));
    pet.state = State::Idle;
    pet.auto_activities = false;
    pet.auto_reactions = false;
    let mut previous = None;
    let mut lines = std::collections::HashSet::new();
    for _ in 0..12 {
        pet.command(Command::Laptop, &world);
        let line = pet.bubble.clone().unwrap();
        assert_ne!(Some(&line), previous.as_ref());
        lines.insert(line.clone());
        previous = Some(line);
        for _ in 0..120 {
            pet.update(1.0 / 60.0, &world);
        }
    }
    assert!(lines.len() >= 4);
}
