use claude_mascot::{
    dialogue::SpeechEvent,
    engine::{Command, Pet, Skin, State},
    Rect, Vec2, WindowInfo, World,
};
fn world() -> World {
    World {
        ui_surfaces: Vec::new(),
        windows: vec![WindowInfo {
            id: 207,
            rect: Rect::new(100.0, 400.0, 800.0, 180.0),
            title: String::new(),
            active: false,
            fullscreen: false,
            is_file_manager: false,
            is_desktop_icon: false,
        }],
        work_areas: vec![Rect::new(0.0, 0.0, 1000.0, 700.0)],
        cursor: Vec2::new(500.0, 650.0),
        idle_seconds: 0.0,
        video_window: None,
    }
}
#[test]
fn exploration_remains_available_when_wandering_is_disabled() {
    let w = world();
    let mut p = Pet::new(207, Skin::Claude, Vec2::new(500.0, 700.0));
    p.state = State::Idle;
    p.frequencies.wandering = 0;
    p.frequencies.exploration = 100;
    for _ in 0..30000 {
        p.update(1.0 / 60.0, &w);
        if p.surface == Some(0) && (p.pos.y - 400.0).abs() < 1.0 {
            return;
        }
    }
    panic!("exploration should reach the window with wandering disabled");
}
#[test]
fn zero_cursor_frequency_blocks_automatic_but_not_manual_chase() {
    let mut w = world();
    w.windows.clear();
    let mut p = Pet::new(43, Skin::Claude, Vec2::new(500.0, 700.0));
    p.state = State::Idle;
    p.frequencies.cursor = 0;
    for i in 0..30000 {
        w.cursor = Vec2::new(500.0 + (i % 60) as f64, 650.0);
        p.update(1.0 / 60.0, &w);
        assert_ne!(p.state, State::Chase);
    }
    p.command(Command::Chase, &w);
    assert!(p.chase);
}
#[test]
fn quiet_speech_preserves_simulation_rng() {
    let w = world();
    let mut quiet = Pet::new(70, Skin::Claude, Vec2::new(500.0, 700.0));
    quiet.state = State::Idle;
    let mut speaking = quiet.clone();
    quiet.frequencies.speech = 0;
    for step in 0..6000 {
        quiet.update(1.0 / 60.0, &w);
        speaking.update(1.0 / 60.0, &w);
        if step % 100 == 0 {
            quiet.speak(SpeechEvent::Wave, 2.0);
            speaking.speak(SpeechEvent::Wave, 2.0);
        }
        assert_eq!(
            (quiet.pos, quiet.vel, quiet.state),
            (speaking.pos, speaking.vel, speaking.state)
        );
    }
}
