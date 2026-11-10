//! Captured high-DPI desktop geometry (rust/work/opus/native-world-*.json,
//! geometry only) and desktop-icon support visibility.
use claude_mascot::{
    behavior::{BehaviorCoordinator, BehaviorSettings},
    engine::{Command, Pet, Skin, State},
    Rect, Vec2, WindowInfo, World,
};

const FLOATING: usize = 4_523_940;
const BACKGROUND: usize = 132_504;
const FLOOR: f64 = 1620.0;

fn window(id: usize, rect: Rect, active: bool) -> WindowInfo {
    WindowInfo {
        id,
        rect,
        title: String::new(),
        active,
        fullscreen: false,
        is_file_manager: false,
        is_desktop_icon: false,
    }
}

fn icon(id: usize, rect: Rect) -> WindowInfo {
    WindowInfo {
        is_desktop_icon: true,
        ..window(id, rect, false)
    }
}

/// native-world-geometry.json: 2880x1620 work area at 200% DPI, an inactive
/// full-work-area window at the bottom and an active floating window whose
/// top is 1483 px above the floor and whose bottom floats 281 px above it.
fn captured_world() -> World {
    World {
        windows: vec![
            window(BACKGROUND, Rect::new(0.0, 0.0, 2880.0, 1620.0), false),
            window(FLOATING, Rect::new(117.0, 137.0, 1867.0, 1202.0), true),
        ],
        work_areas: vec![Rect::new(0.0, 0.0, 2880.0, FLOOR)],
        cursor: Vec2::new(2700.0, 300.0),
        ..World::default()
    }
}

/// native-world-baseline.json: a maximized active window covers everything.
fn baseline_world() -> World {
    World {
        windows: vec![
            window(3_738_332, Rect::new(213.0, 295.0, 2495.0, 1259.0), false),
            window(264_634, Rect::new(426.0, 59.0, 2277.0, 1378.0), false),
            window(BACKGROUND, Rect::new(0.0, 0.0, 2880.0, 1620.0), false),
            window(FLOATING, Rect::new(0.0, 0.0, 2880.0, 1618.0), true),
        ],
        work_areas: vec![Rect::new(0.0, 0.0, 2880.0, FLOOR)],
        cursor: Vec2::new(2700.0, 300.0),
        ..World::default()
    }
}

fn desktop_pet(id: u64, x: f64, scale: f64) -> Pet {
    let mut p = Pet::new(id, Skin::Claude, Vec2::new(x, FLOOR));
    p.state = State::Idle;
    p.display_scale = scale;
    p.activity_intensity = 100;
    p
}

fn support_id(p: &Pet, w: &World) -> Option<usize> {
    p.surface.and_then(|i| w.windows.get(i)).map(|w| w.id)
}

#[derive(Default)]
struct Journey {
    crouched_for_window: bool,
    climbed: bool,
    reached_top: bool,
    left_again: bool,
}

fn run_desktop(pets: &mut [Pet], w: &World, seconds: f64, coordinate: bool) -> Vec<Journey> {
    let dt = 1.0 / 120.0;
    let mut journeys: Vec<Journey> = pets.iter().map(|_| Journey::default()).collect();
    let mut behavior = BehaviorCoordinator::new(BehaviorSettings::default());
    for _ in 0..(seconds / dt) as usize {
        for (p, j) in pets.iter_mut().zip(journeys.iter_mut()) {
            let old = p.pos;
            p.update(dt, w);
            assert!(p.pos.x.is_finite() && p.pos.y.is_finite());
            assert!(p.pos.x >= 0.0 && p.pos.x <= 2880.0, "left the monitor");
            assert!(
                p.pos.y >= 0.0 && p.pos.y <= FLOOR + 0.01,
                "left the monitor"
            );
            assert!(
                (p.pos.x - old.x).abs() <= 2200.0 * dt + 1e-6
                    && (p.pos.y - old.y).abs() <= 2200.0 * dt + 1e-6,
                "teleport in {:?}: {:?} -> {:?}",
                p.state,
                old,
                p.pos
            );
            j.crouched_for_window |=
                p.state == State::Crouch && p.target_id() == Some(FLOATING as u64);
            j.climbed |= p.state == State::Climb;
            let on_top = support_id(p, w) == Some(FLOATING)
                && (p.pos.y - 137.0).abs() < 0.5
                && p.state != State::Climb;
            j.reached_top |= on_top;
            if j.reached_top && p.surface.is_none() && (p.pos.y - FLOOR).abs() < 0.5 {
                j.left_again = true;
            }
        }
        if coordinate {
            behavior.update(dt, pets.iter_mut(), w);
        }
    }
    journeys
}

#[test]
fn captured_high_dpi_floating_window_is_explored_and_left_under_defaults() {
    let w = captured_world();
    let mut reached = 0;
    let mut left = 0;
    for (seed, x) in [(1u64, 1400.0), (2, 2400.0), (3, 600.0)] {
        let mut pets = [desktop_pet(seed, x, 1.5)];
        let journey = run_desktop(&mut pets, &w, 600.0, false).remove(0);
        assert!(
            journey.crouched_for_window && journey.climbed,
            "seed {seed}: the pet must hop to the floating window's edge and climb"
        );
        reached += journey.reached_top as u32;
        left += journey.left_again as u32;
    }
    assert_eq!(
        reached, 3,
        "every seed should reach the floating window top"
    );
    assert!(
        left >= 1,
        "a pet must also leave the window again on its own"
    );
}

#[test]
fn settled_friends_still_explore_instead_of_freezing_in_place() {
    let w = captured_world();
    // Close enough that the spacing coordinator holds them apart at zero speed.
    let mut pets = [desktop_pet(11, 1400.0, 1.5), desktop_pet(12, 1500.0, 1.5)];
    let journeys = run_desktop(&mut pets, &w, 600.0, true);
    assert!(
        journeys.iter().any(|j| j.reached_top),
        "a spacing hold must not starve autonomous exploration"
    );
}

#[test]
fn covered_baseline_desktop_offers_no_unsafe_route() {
    let w = baseline_world();
    let mut pets = [desktop_pet(4, 1400.0, 1.5)];
    let dt = 1.0 / 120.0;
    for _ in 0..(180.0 / dt) as usize {
        pets[0].update(dt, &w);
        assert_eq!(pets[0].target_id(), None, "every window top is covered");
        assert_eq!(pets[0].surface, None);
        assert_ne!(pets[0].state, State::Climb);
    }
}

#[test]
fn floating_windows_are_reachable_at_150_and_200_percent_dpi() {
    for scale in [1.5, 2.0] {
        let w = captured_world();
        let mut pets = [desktop_pet(21, 1000.0, scale)];
        let journey = run_desktop(&mut pets, &w, 600.0, false).remove(0);
        assert!(journey.reached_top, "scale {scale}");
    }
}

#[test]
fn one_window_has_random_direct_and_climbing_routes() {
    let w = World {
        windows: vec![window(77, Rect::new(100.0, 400.0, 800.0, 180.0), true)],
        work_areas: vec![Rect::new(0.0, 0.0, 1000.0, 700.0)],
        ..World::default()
    };
    let mut direct = 0;
    let mut climbing = 0;
    for seed in 1..=24 {
        let mut p = Pet::new(seed, Skin::Claude, Vec2::new(500.0, 700.0));
        p.state = State::Idle;
        p.auto_reactions = false;
        p.activity_intensity = 100;
        let mut used_climb = false;
        let mut arrived = false;
        for _ in 0..(180 * 120) {
            p.update(1.0 / 120.0, &w);
            used_climb |= p.state == State::Climb;
            if support_id(&p, &w) == Some(77) && (p.pos.y - 400.0).abs() < 0.5 {
                arrived = true;
                break;
            }
        }
        assert!(arrived, "seed {seed} must complete its route");
        if used_climb {
            climbing += 1;
        } else {
            direct += 1;
        }
    }
    assert!(
        direct > 0 && climbing > 0,
        "routes must vary: {direct} direct, {climbing} climbs"
    );
}

// --- desktop icon support visibility -------------------------------------

const ICON: usize = (1usize << (usize::BITS - 1)) | 5;

fn icon_world() -> World {
    World {
        windows: vec![icon(ICON, Rect::new(100.0, 800.0, 150.0, 157.0))],
        work_areas: vec![Rect::new(0.0, 0.0, 1600.0, 900.0)],
        cursor: Vec2::new(1500.0, 100.0),
        ..World::default()
    }
}

/// Drop a calm pet (no autonomous decisions) onto the icon and let it settle.
fn perched_on_icon(w: &World) -> Pet {
    let mut p = Pet::new(31, Skin::Claude, Vec2::new(175.0, 700.0));
    p.activity_intensity = 0;
    p.auto_activities = false;
    p.auto_reactions = false;
    for _ in 0..240 {
        p.update(1.0 / 120.0, w);
    }
    assert_eq!(support_id(&p, w), Some(ICON));
    assert_eq!(p.pos.y, 800.0);
    p
}

fn settle(p: &mut Pet, w: &World, seconds: f64) {
    for _ in 0..(seconds * 120.0) as usize {
        let old = p.pos;
        p.update(1.0 / 120.0, w);
        assert!(
            (p.pos.y - old.y).abs() <= 2200.0 / 120.0 + 1e-6,
            "falls instead of teleporting"
        );
        assert!((p.pos.x - old.x).abs() < 1e-9, "x is preserved");
    }
}

#[test]
fn alt_tab_foreground_window_over_an_occupied_icon_drops_the_pet() {
    let mut w = icon_world();
    let mut p = perched_on_icon(&w);
    // Alt+Tab brings a large window in front of the desktop icon.
    w.windows
        .push(window(9, Rect::new(0.0, 0.0, 1600.0, 900.0), true));
    p.update(1.0 / 120.0, &w);
    assert_eq!(p.surface, None, "a hidden icon is no longer support");
    assert_eq!(p.state, State::Fall);
    assert!(
        (p.pos.y - 800.0).abs() < 1.0,
        "starts from its real position"
    );
    settle(&mut p, &w, 2.0);
    assert_eq!(p.pos.y, 900.0);
    assert_eq!(p.pos.x, 175.0);
}

#[test]
fn manual_sit_and_sleep_resume_after_falling_from_a_hidden_icon() {
    for command in [Command::Sit, Command::Sleep] {
        let mut w = icon_world();
        let mut p = perched_on_icon(&w);
        p.command(command, &w);
        let resting = if command == Command::Sit {
            State::Sit
        } else {
            State::Sleep
        };
        assert_eq!(p.state, resting);
        w.windows
            .push(window(9, Rect::new(0.0, 0.0, 1600.0, 900.0), true));
        settle(&mut p, &w, 2.0);
        assert_eq!(p.pos.y, 900.0, "{command:?} falls with gravity");
        assert_eq!(p.state, resting, "{command:?} intent survives the fall");
    }
}

#[test]
fn exposed_icons_stay_valid_through_focus_changes_and_partial_covers() {
    let mut w = icon_world();
    let mut p = perched_on_icon(&w);
    // Focus moves to a window elsewhere and another one covers only the
    // icon's far edge, away from Claude's feet.
    w.windows
        .push(window(9, Rect::new(600.0, 100.0, 700.0, 500.0), true));
    w.windows
        .push(window(10, Rect::new(220.0, 700.0, 300.0, 200.0), false));
    for _ in 0..240 {
        p.update(1.0 / 120.0, &w);
        assert_eq!(support_id(&p, &w), Some(ICON));
        assert_eq!(p.pos.y, 800.0);
    }
    w.windows[1].active = false;
    w.windows[2].active = true;
    for _ in 0..240 {
        p.update(1.0 / 120.0, &w);
        assert_eq!(support_id(&p, &w), Some(ICON));
    }
}

#[test]
fn deleted_or_expired_icon_drops_without_teleporting() {
    let mut w = icon_world();
    let mut p = perched_on_icon(&w);
    w.windows.clear();
    p.update(1.0 / 120.0, &w);
    assert_eq!(p.surface, None);
    assert!((p.pos.y - 800.0).abs() < 1.0);
    settle(&mut p, &w, 2.0);
    assert_eq!(p.pos.y, 900.0);
}

#[test]
fn deleting_an_icon_preserves_manual_rest_until_actual_landing() {
    for command in [Command::Sit, Command::Sleep] {
        let mut w = icon_world();
        let mut p = perched_on_icon(&w);
        p.command(command, &w);
        let resting = p.state;
        w.windows.clear();
        p.update(1.0 / 120.0, &w);
        assert_eq!(p.state, State::Fall);
        assert!((p.pos.y - 800.0).abs() < 1.0);
        settle(&mut p, &w, 2.0);
        assert_eq!(p.pos.y, 900.0);
        assert_eq!(p.state, resting);
    }
}

#[test]
fn ordinary_window_support_is_not_affected_by_covering_windows() {
    let mut w = World {
        windows: vec![window(40, Rect::new(100.0, 500.0, 400.0, 300.0), false)],
        work_areas: vec![Rect::new(0.0, 0.0, 1600.0, 900.0)],
        cursor: Vec2::new(1500.0, 100.0),
        ..World::default()
    };
    let mut p = Pet::new(32, Skin::Claude, Vec2::new(250.0, 400.0));
    p.activity_intensity = 0;
    p.auto_activities = false;
    p.auto_reactions = false;
    for _ in 0..240 {
        p.update(1.0 / 120.0, &w);
    }
    assert_eq!(support_id(&p, &w), Some(40));
    w.windows
        .push(window(41, Rect::new(0.0, 0.0, 1600.0, 900.0), true));
    for _ in 0..240 {
        p.update(1.0 / 120.0, &w);
        assert_eq!(support_id(&p, &w), Some(40));
        assert_eq!(p.pos.y, 500.0);
    }
}
