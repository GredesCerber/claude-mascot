//! Knock-back tumble, bounce budget, dizzy recovery and friendly rough play.
use claude_mascot::{
    behavior::{BehaviorCoordinator, BehaviorSettings},
    contacts::ContactCoordinator,
    engine::{Command, Knock, Pet, Skin, State},
    social::{SocialCoordinator, SocialKind},
    Rect, Vec2, World,
};

const DT: f64 = 1.0 / 240.0;
const FLOOR: f64 = 900.0;

fn world() -> World {
    World {
        work_areas: vec![Rect::new(0.0, 0.0, 1600.0, FLOOR)],
        ..World::default()
    }
}

fn idle(id: u64, x: f64) -> Pet {
    let mut p = Pet::new(id, Skin::Claude, Vec2::new(x, FLOOR));
    p.state = State::Idle;
    p.auto_activities = false;
    p.auto_reactions = false;
    p
}

fn finite(p: &Pet) -> bool {
    p.pos.x.is_finite()
        && p.pos.y.is_finite()
        && p.vel.x.is_finite()
        && p.vel.y.is_finite()
        && p.tilt.is_finite()
        && p.tumble_spin().is_finite()
}

fn airborne(p: &Pet) -> bool {
    matches!(p.state, State::Thrown | State::Fall | State::Jump)
}

#[derive(Default, Debug)]
struct Trace {
    max_spin: f64,
    max_tilt: f64,
    bounces: u32,
    dizzy_frames: u32,
    recovered_at: Option<usize>,
    min_y: f64,
}

/// Follow one pet until it has been dizzy and is upright and idle again.
fn follow(p: &mut Pet, w: &World, dt: f64, max_steps: usize) -> Trace {
    let mut trace = Trace {
        min_y: f64::MAX,
        ..Trace::default()
    };
    let mut previous_vy = p.vel.y;
    for step in 0..max_steps {
        p.update(dt, w);
        assert!(finite(p), "non-finite state at step {step}");
        let area = w.area_at(p.pos);
        assert!(
            p.pos.x >= area.x && p.pos.x <= area.right(),
            "left the screen"
        );
        assert!(p.pos.y <= area.bottom() + 0.001, "sank below the floor");
        assert!(p.tumble_bounces_left() <= 2);
        trace.min_y = trace.min_y.min(p.pos.y);
        trace.max_spin = trace.max_spin.max(p.tumble_spin().abs());
        trace.max_tilt = trace.max_tilt.max(p.tilt.abs());
        // A floor bounce turns a fall into a rise.
        if airborne(p) && previous_vy > 0.0 && p.vel.y < 0.0 {
            trace.bounces += 1;
        }
        previous_vy = p.vel.y;
        if p.state == State::Dizzy {
            trace.dizzy_frames += 1;
        }
        if trace.dizzy_frames > 0 && p.state == State::Idle && p.tilt == 0.0 {
            trace.recovered_at = Some(step);
            break;
        }
    }
    trace
}

#[test]
fn hard_side_hit_tumbles_bounces_finitely_then_recovers_upright() {
    let w = world();
    let mut a = idle(1, 400.0);
    let mut b = idle(2, 600.0);
    let mut contacts = ContactCoordinator::new();
    contacts.update(DT, [&mut a, &mut b], &w);
    a.pos.x = 585.0;
    a.vel = Vec2::new(1200.0, 0.0);
    a.state = State::Thrown;
    contacts.update(DT, [&mut a, &mut b], &w);
    assert_eq!(contacts.tumble_impacts, 1);
    assert!(b.is_tumbling(), "the struck pet must be knocked away");
    assert!(
        b.vel.x > 250.0,
        "translational momentum is still transferred"
    );
    assert!(b.vel.y < -200.0, "a grounded pet is popped into the air");
    assert!(b.tumble_spin() > 0.0, "spin follows the push to the right");
    assert!(b.bubble.is_some(), "impact gets a short reaction line");

    let start_x = b.pos.x;
    let trace = follow(&mut b, &w, DT, 240 * 8);
    assert!(trace.max_tilt > 2.0, "the body visibly rolls over");
    assert!(trace.max_spin <= 14.0 + 1e-9, "angular speed is bounded");
    assert!(
        (1..=2).contains(&trace.bounces),
        "bounces: {}",
        trace.bounces
    );
    assert!(trace.dizzy_frames >= 120, "a readable dizzy spell follows");
    assert!(trace.recovered_at.is_some(), "Claude must return upright");
    assert!(trace.min_y < FLOOR - 30.0, "the knock is a visible flight");
    assert!(b.pos.x > start_x + 100.0, "knocked away from the hitter");
    assert_eq!(b.pos.y, FLOOR);
    assert!(!b.is_tumbling());
}

#[test]
fn gentle_and_ordinary_contacts_never_tumble() {
    let w = world();
    // Two runners meeting head on: strong enough to stagger, not to tumble.
    let mut a = idle(1, 400.0);
    let mut b = idle(2, 480.0);
    let mut contacts = ContactCoordinator::new();
    contacts.update(DT, [&mut a, &mut b], &w);
    a.state = State::Run;
    b.state = State::Run;
    a.vel.x = 200.0;
    b.vel.x = -200.0;
    a.pos.x = 410.0;
    contacts.update(DT, [&mut a, &mut b], &w);
    assert!(contacts.contact_count > 0);
    assert_eq!(contacts.tumble_impacts, 0);
    for _ in 0..(240 * 3) {
        a.update(DT, &w);
        b.update(DT, &w);
        contacts.update(DT, [&mut a, &mut b], &w);
        for p in [&a, &b] {
            assert_eq!(p.tilt, 0.0);
            assert_ne!(p.state, State::Dizzy);
            assert!(!p.is_tumbling());
        }
    }

    // A normal hop landing on a friend's head bounces without spinning.
    let mut top = idle(3, 800.0);
    top.pos.y = FLOOR - 70.0;
    top.state = State::Fall;
    top.vel.y = 700.0;
    let mut lower = idle(4, 800.0);
    let mut contacts = ContactCoordinator::new();
    contacts.update(DT, [&mut top, &mut lower], &w);
    top.pos.y = FLOOR - 50.0;
    contacts.update(DT, [&mut top, &mut lower], &w);
    assert_eq!(contacts.tumble_impacts, 0);
    assert!(!top.is_tumbling() && !lower.is_tumbling());

    // Slow low throws and ordinary falls keep the plain landing.
    let mut thrown = idle(5, 300.0);
    thrown.drag_to(Vec2::new(300.0, 800.0));
    thrown.release(Vec2::new(200.0, 0.0));
    let mut falling = Pet::new(6, Skin::Claude, Vec2::new(900.0, 100.0));
    for _ in 0..(240 * 3) {
        thrown.update(DT, &w);
        falling.update(DT, &w);
        for p in [&thrown, &falling] {
            assert_eq!(p.tilt, 0.0);
            assert_ne!(p.state, State::Dizzy);
        }
    }
}

#[test]
fn repeated_contact_inside_the_cooldown_does_not_reinject_energy() {
    let w = world();
    let mut p = idle(1, 500.0);
    assert!(p.knock_back(Knock {
        direction: 1.0,
        strength: 900.0
    }));
    let vel = p.vel;
    let spin = p.tumble_spin();
    let bounces = p.tumble_bounces_left();
    assert!(!p.knock_back(Knock {
        direction: 1.0,
        strength: 2000.0
    }));
    assert_eq!(
        (p.vel, p.tumble_spin(), p.tumble_bounces_left()),
        (vel, spin, bounces)
    );
    assert!(!p.knock_back(Knock {
        direction: f64::NAN,
        strength: 900.0
    }));
    let trace = follow(&mut p, &w, DT, 240 * 8);
    assert!(trace.recovered_at.is_some());
}

#[test]
fn manual_actions_interrupt_tumble_and_dizziness() {
    let w = world();
    let knock = Knock {
        direction: -1.0,
        strength: 700.0,
    };
    // Petting a dizzy Claude cheers it up and stands it upright.
    let mut p = idle(1, 800.0);
    assert!(p.knock_back(knock));
    for _ in 0..(240 * 4) {
        p.update(DT, &w);
        if p.state == State::Dizzy {
            break;
        }
    }
    assert_eq!(p.state, State::Dizzy);
    p.pet();
    assert_eq!(p.state, State::Happy);
    p.update(DT, &w);
    assert_eq!(p.tilt, 0.0);

    // A manual jump launches straight out of dizziness.
    let mut p = idle(2, 800.0);
    assert!(p.knock_back(knock));
    for _ in 0..(240 * 4) {
        p.update(DT, &w);
        if p.state == State::Dizzy {
            break;
        }
    }
    assert_eq!(p.state, State::Dizzy);
    p.jump();
    assert_eq!(p.state, State::Jump);
    for _ in 0..(240 * 3) {
        p.update(DT, &w);
        assert_ne!(
            p.state,
            State::Dizzy,
            "a jump must not fall back into dizziness"
        );
    }
    assert_eq!(p.tilt, 0.0);

    // A command during flight keeps physics but skips the dizzy finale.
    let mut p = idle(3, 800.0);
    assert!(p.knock_back(knock));
    p.update(DT, &w);
    p.command(Command::Sit, &w);
    assert_eq!(p.tumble_bounces_left(), 0);
    let mut sat = false;
    for _ in 0..(240 * 4) {
        p.update(DT, &w);
        assert_ne!(p.state, State::Dizzy);
        sat |= p.state == State::Sit;
    }
    assert!(sat, "the queued command runs after landing");
    assert_eq!(p.tilt, 0.0);

    // Grabbing a tumbling pet stops the rotation at once.
    let mut p = idle(4, 800.0);
    assert!(p.knock_back(knock));
    for _ in 0..30 {
        p.update(DT, &w);
    }
    assert!(p.tilt != 0.0);
    p.drag_to(Vec2::new(700.0, 400.0));
    p.update(DT, &w);
    assert_eq!(p.tilt, 0.0);
    assert!(!p.is_tumbling());
    p.release(Vec2::default());
    for _ in 0..(240 * 3) {
        p.update(DT, &w);
        assert_ne!(p.state, State::Dizzy);
    }
}

#[test]
fn tumble_is_deterministic_and_independent_of_step_rate() {
    let w = world();
    let run = |dt: f64| {
        let mut p = idle(7, 500.0);
        assert!(p.knock_back(Knock {
            direction: 1.0,
            strength: 900.0,
        }));
        let trace = follow(&mut p, &w, dt, (8.0 / dt) as usize);
        (p, trace)
    };
    let (a, ta) = run(DT);
    let (b, tb) = run(DT);
    assert_eq!((a.pos, a.state, a.tilt), (b.pos, b.state, b.tilt));
    assert_eq!(ta.recovered_at, tb.recovered_at);

    let (slow, ts) = run(1.0 / 120.0);
    let (fast, tf) = run(1.0 / 480.0);
    for (p, t) in [(&a, &ta), (&slow, &ts), (&fast, &tf)] {
        assert!(t.recovered_at.is_some());
        assert_eq!(p.state, State::Idle);
        assert_eq!(p.tilt, 0.0);
        assert_eq!(
            t.bounces, ta.bounces,
            "bounce count must not depend on rate"
        );
    }
    assert!(
        (slow.pos.x - a.pos.x).abs() < 40.0,
        "{} vs {}",
        slow.pos.x,
        a.pos.x
    );
    assert!(
        (fast.pos.x - a.pos.x).abs() < 40.0,
        "{} vs {}",
        fast.pos.x,
        a.pos.x
    );
    let seconds = |t: &Trace, dt: f64| t.recovered_at.unwrap() as f64 * dt;
    assert!((seconds(&ts, 1.0 / 120.0) - seconds(&ta, DT)).abs() < 0.25);
    assert!((seconds(&tf, 1.0 / 480.0) - seconds(&ta, DT)).abs() < 0.25);
}

#[test]
fn screen_edges_and_ceiling_contain_a_tumble() {
    // Knocked hard into the right edge of a short monitor.
    let w = World {
        work_areas: vec![Rect::new(0.0, 600.0, 700.0, 300.0)],
        ..World::default()
    };
    let mut p = idle(1, 640.0);
    assert!(p.knock_back(Knock {
        direction: 1.0,
        strength: 2000.0,
    }));
    p.vel.y = -2000.0;
    let mut seen_left = false;
    for _ in 0..(240 * 8) {
        p.update(DT, &w);
        assert!(finite(&p));
        assert!(p.pos.x >= 0.0 && p.pos.x <= 700.0);
        assert!(
            p.pos.y >= 600.0 + 110.0 - 1e-6,
            "tumble must stay below the top edge"
        );
        assert!(p.pos.y <= 900.0 + 1e-6);
        seen_left |= p.vel.x < 0.0;
    }
    assert!(seen_left, "the wall reflects the knock");
    assert!(!p.is_tumbling() && p.state != State::Dizzy);
    assert_eq!(p.tilt, 0.0);

    // A fast throw into the wall starts a tumble; it still recovers.
    let w = world();
    let mut fast = idle(2, 1500.0);
    fast.drag_to(Vec2::new(1500.0, 600.0));
    fast.release(Vec2::new(1500.0, -100.0));
    let mut tumbled = false;
    for _ in 0..240 {
        fast.update(DT, &w);
        tumbled |= fast.is_tumbling() && fast.tumble_spin() < 0.0;
        if tumbled {
            break;
        }
    }
    assert!(
        tumbled,
        "bouncing off the right edge rolls back to the left"
    );
    assert!(follow(&mut fast, &w, DT, 240 * 8).recovered_at.is_some());

    // A hard thrown landing rolls instead of only rebounding.
    let mut dropped = idle(3, 800.0);
    dropped.drag_to(Vec2::new(800.0, 300.0));
    dropped.release(Vec2::new(300.0, 1800.0));
    let mut rolled = false;
    for _ in 0..240 {
        dropped.update(DT, &w);
        rolled |= dropped.is_tumbling() && dropped.tilt != 0.0;
        if rolled {
            break;
        }
    }
    assert!(rolled);
    assert!(follow(&mut dropped, &w, DT, 240 * 8).recovered_at.is_some());
}

#[test]
fn eight_pets_stay_safe_through_a_pile_up() {
    let w = world();
    let mut pets: Vec<Pet> = (0..8)
        .map(|i| idle(i + 1, 300.0 + i as f64 * 120.0))
        .collect();
    let mut contacts = ContactCoordinator::new();
    let mut social = SocialCoordinator::new();
    let mut behavior = BehaviorCoordinator::new(BehaviorSettings::default());
    for step in 0..(240 * 40) {
        if step % (240 * 5) == 0 {
            // Fling one pet hard into its neighbours, alternating direction.
            let round = step / (240 * 5);
            let direction = if round % 2 == 0 { 1.0 } else { -1.0 };
            pets[round % 8].release(Vec2::new(1800.0 * direction, -200.0));
        }
        for p in &mut pets {
            p.update(DT, &w);
        }
        contacts.update(DT, pets.iter_mut(), &w);
        behavior.update(DT, pets.iter_mut(), &w);
        social.update(DT, pets.iter_mut(), &w);
        for p in &pets {
            assert!(finite(p));
            assert!(p.tumble_spin().abs() <= 14.0 + 1e-9);
            assert!(p.tilt.abs() <= std::f64::consts::PI + 1e-9);
            assert!(p.tumble_bounces_left() <= 2);
            assert!(p.pos.x >= 0.0 && p.pos.x <= 1600.0);
            assert!(p.pos.y <= FLOOR + 0.001);
            assert!(
                p.tilt == 0.0 || airborne(p) || matches!(p.state, State::Dizzy | State::Land),
                "a grounded {:?} pet must be upright",
                p.state
            );
        }
    }
    assert!(
        contacts.tumble_impacts >= 1,
        "the pile-up should knock someone over"
    );
    assert!(
        contacts.tumble_impacts < 400,
        "knock-backs must not chain forever"
    );
    for _ in 0..(240 * 6) {
        for p in &mut pets {
            p.update(DT, &w);
        }
        contacts.update(DT, pets.iter_mut(), &w);
    }
    for p in &pets {
        assert!(!p.is_tumbling() && p.state != State::Dizzy && p.tilt == 0.0);
    }
}

fn play_pair() -> Vec<Pet> {
    vec![idle(1, 300.0), idle(2, 450.0)]
}

fn step_all(pets: &mut [Pet], social: &mut SocialCoordinator, contacts: &mut ContactCoordinator) {
    let w = world();
    for p in pets.iter_mut() {
        p.update(DT, &w);
    }
    contacts.update(DT, pets.iter_mut(), &w);
    social.update(DT, pets.iter_mut(), &w);
}

#[test]
fn friendly_rough_play_shoves_the_partner_into_a_short_roll() {
    let w = world();
    let mut pets = play_pair();
    let mut social = SocialCoordinator::new();
    let mut contacts = ContactCoordinator::new();
    social.request_for(1, Some(SocialKind::RoughPlay));
    social.update(0.0, pets.iter_mut(), &w);
    assert_eq!(pets[0].social.map(|s| s.kind), Some(SocialKind::RoughPlay));
    assert_eq!(pets[1].social.map(|s| s.kind), Some(SocialKind::RoughPlay));
    let start_partner = pets[1].pos.x;
    let mut partner_rolled = false;
    let mut partner_dizzy = false;
    for _ in 0..(240 * 5) {
        step_all(&mut pets, &mut social, &mut contacts);
        assert!(!pets[0].is_tumbling(), "the initiator stays on its feet");
        partner_rolled |= pets[1].is_tumbling() && pets[1].tumble_spin() > 0.0;
        partner_dizzy |= pets[1].state == State::Dizzy;
    }
    assert!(partner_rolled, "the partner should roll away");
    assert!(partner_dizzy);
    assert_eq!(social.stats.completed, 1);
    assert_eq!(
        contacts.tumble_impacts, 0,
        "the shove never needs a body crash"
    );
    assert!(pets[1].pos.x > start_partner + 60.0);
    for p in &pets {
        assert!(!p.is_tumbling() && p.state != State::Dizzy);
        assert_eq!(p.tilt, 0.0);
        assert!(p.social.is_none());
    }
}

#[test]
fn rough_play_respects_settings_sleep_dragging_and_commands() {
    let w = world();
    // Disabled social interactions never start it.
    let mut pets = play_pair();
    let mut social = SocialCoordinator::new();
    social.enabled = false;
    social.request_for(1, Some(SocialKind::RoughPlay));
    social.update(0.0, pets.iter_mut(), &w);
    assert!(pets.iter().all(|p| p.social.is_none()));

    // A sleeping partner is never shoved.
    let mut pets = play_pair();
    pets[1].command(Command::Sleep, &w);
    let mut social = SocialCoordinator::new();
    social.request_for(1, Some(SocialKind::RoughPlay));
    social.update(0.0, pets.iter_mut(), &w);
    assert!(pets.iter().all(|p| p.social.is_none()));

    // Dragging or commanding during the square-up cancels without a shove.
    for action in 0..2 {
        let mut pets = play_pair();
        let mut social = SocialCoordinator::new();
        let mut contacts = ContactCoordinator::new();
        social.request_for(1, Some(SocialKind::RoughPlay));
        social.update(0.0, pets.iter_mut(), &w);
        assert!(pets[0].social.is_some());
        for _ in 0..24 {
            step_all(&mut pets, &mut social, &mut contacts);
        }
        if action == 0 {
            pets[1].drag_to(Vec2::new(450.0, 500.0));
        } else {
            pets[0].command(Command::Wave, &w);
        }
        for _ in 0..(240 * 3) {
            step_all(&mut pets, &mut social, &mut contacts);
            assert!(!pets[1].is_tumbling());
            assert_ne!(pets[1].state, State::Dizzy);
        }
        assert_eq!(social.stats.cancelled, 1);
        assert_eq!(social.stats.completed, 0);
    }

    // With random activities off, autonomous play never picks a shove.
    let mut pets: Vec<Pet> = (0..4)
        .map(|i| idle(i + 1, 300.0 + i as f64 * 150.0))
        .collect();
    let mut social = SocialCoordinator::new();
    let mut contacts = ContactCoordinator::new();
    for _ in 0..(240 * 60) {
        step_all(&mut pets, &mut social, &mut contacts);
        for p in &pets {
            assert_ne!(p.social.map(|s| s.kind), Some(SocialKind::RoughPlay));
        }
    }
}
