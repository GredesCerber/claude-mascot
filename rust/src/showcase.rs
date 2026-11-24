//! Original procedural activities inspired by the Codrops Claude motion study.
//! Timelines produce poses and props only; desktop position stays physics-owned.
use crate::{
    render::{pose_time, Canvas, FEET_X, FEET_Y, UNIT},
    Vec2,
};
pub(crate) const FLAG: usize = 75;
pub(crate) const CONFETTI: usize = 76;
pub(crate) const GYM: usize = 77;
#[derive(Clone, Copy, Debug)]
pub(crate) struct Pose {
    pub body_x: f64,
    pub bob: f64,
    pub hands: [Vec2; 2],
    pub lifted_pair: Option<usize>,
}
fn tick(t: f64, hz: f64) -> usize {
    (pose_time(t) * hz + 1e-8).floor() as usize
}
fn flag_frame(t: f64) -> usize {
    let frame = tick(t, 12.0);
    if frame < 3 {
        frame
    } else {
        3 + (frame - 3) % 9
    }
}
/// Two repetitions with pauses at the top and a final put-down. Heights are
/// authored in whole logical pixels, not interpolated between key poses.
fn gym_height(t: f64) -> f64 {
    let t = pose_time(t);
    match t {
        t if t < 0.75 => -3.0,
        t if t < 1.25 => -4.0,
        t if t < 1.75 => -7.0,
        t if t < 2.0 => -9.0,
        t if t < 2.25 => -12.0,
        t if t < 2.9 => -15.0,
        t if t < 3.25 => -11.0,
        t if t < 3.75 => -7.0,
        t if t < 4.0 => -9.0,
        t if t < 4.25 => -12.0,
        t if t < 4.9 => -15.0,
        t if t < 5.25 => -11.0,
        t if t < 6.0 => -7.0,
        t if t < 6.5 => -4.0,
        _ => -3.0,
    }
}
pub(crate) fn pose(id: usize, t: f64) -> Option<Pose> {
    let mut p = Pose {
        body_x: 0.0,
        bob: 0.0,
        hands: [Vec2::new(-8.0, -8.0), Vec2::new(6.0, -8.0)],
        lifted_pair: None,
    };
    match id {
        FLAG => {
            let f = flag_frame(t);
            p.body_x = [
                0.0, 0.0, -1.0, -1.0, 0.0, 1.0, 1.0, 0.0, -1.0, -1.0, 0.0, 1.0,
            ][f];
            p.hands[1] = Vec2::new(7.0 + if (4..8).contains(&f) { 1.0 } else { 0.0 }, -10.0);
        }
        CONFETTI => {
            let beat = tick(t, 6.0) % 8;
            p.body_x = if beat < 4 { -1.0 } else { 1.0 };
            p.bob = if beat == 1 || beat == 5 { -1.0 } else { 0.0 };
            p.lifted_pair = Some(usize::from(beat >= 4));
            if (1..=2).contains(&beat) {
                p.hands[1] = Vec2::new(7.0, -15.0);
            }
            if (5..=6).contains(&beat) {
                p.hands[0] = Vec2::new(-9.0, -15.0);
            }
        }
        GYM => {
            let h = gym_height(t);
            if (0.25..0.5).contains(&t)
                || (0.75..1.25).contains(&t)
                || (6.0..6.5).contains(&t)
                || (6.75..7.0).contains(&t)
            {
                p.bob = 1.0;
            }
            if (0.5..0.75).contains(&t) || (6.5..6.75).contains(&t) {
                p.bob = 2.0;
            }
            if (0.25..0.5).contains(&t) || (6.75..7.0).contains(&t) {
                p.hands[1] = Vec2::new(7.0, -6.0 - p.bob);
            } else if (0.5..6.75).contains(&t) {
                p.hands[1] = Vec2::new(10.0, h - p.bob);
            }
        }
        _ => return None,
    }
    Some(p)
}
fn cell(c: &mut Canvas, facing: f64, x: f64, y: f64, w: f64, h: f64, color: u32) {
    let x = if facing < 0.0 { -x - w } else { x };
    c.rect(
        FEET_X + x * UNIT,
        FEET_Y + y * UNIT,
        w * UNIT,
        h * UNIT,
        color,
    );
}
pub(crate) fn draw(c: &mut Canvas, id: usize, elapsed: f64, facing: f64, duration: f64) {
    let t = pose_time(elapsed);
    if t <= 0.0 || t >= duration {
        return;
    }
    let p = pose(id, t).unwrap();
    let opacity = ((t / 0.25).min((duration - t) / 0.3).clamp(0.0, 1.0) * 3.0).ceil() as u32 * 85;
    let color = |rgb| opacity << 24 | rgb;
    match id {
        FLAG => {
            let frame = tick(t, 12.0);
            let hand = p.hands[1];
            let pole_x = hand.x + p.body_x + 1.0;
            cell(c, facing, pole_x, -24.0, 1.0, 16.0, color(0xB7A28B));
            let height = (frame + 1).min(5) as f64;
            // Columns overlap vertically: one connected fabric, no separated stripes.
            for column in 0..8 {
                let ripple = [0.0, 1.0, 0.0, -1.0][(frame + column) % 4];
                cell(
                    c,
                    facing,
                    pole_x + 1.0 + column as f64,
                    -24.0 + ripple,
                    1.0,
                    height,
                    color(if column % 3 == 0 { 0xF4DF9D } else { 0xE9C76A }),
                );
            }
        }
        CONFETTI => {
            // Each burst begins at the corresponding raised palm. Its clock is
            // separate from the stomp, but both use this same pose tick.
            let elapsed_tick = tick(t, 12.0);
            let cycle = elapsed_tick % 16;
            for (launch, side) in [(2_usize, 1.0), (10, -1.0)] {
                if elapsed_tick < launch {
                    continue;
                }
                let age = (cycle + 16 - launch) % 16;
                if age >= 8 {
                    continue;
                }
                let rise = [-1.0, -2.0, -3.0, -3.0, -2.0, 0.0, 2.0, 4.0][age];
                let launch_pose = pose(CONFETTI, launch as f64 / 12.0).unwrap();
                let palm = launch_pose.hands[usize::from(side > 0.0)];
                let origin = Vec2::new(
                    launch_pose.body_x + palm.x + 1.0,
                    launch_pose.bob + palm.y + 1.0,
                );
                for n in 0..6 {
                    if age >= 5 && n % 2 == 0 {
                        continue;
                    }
                    let spread = age.div_ceil(3) as f64;
                    let (dx, dy) = [
                        (0.0, 0.0),
                        (1.0, -1.0),
                        (-1.0, -2.0),
                        (2.0, 1.0),
                        (-2.0, 0.0),
                        (0.0, 2.0),
                    ][n];
                    let x = origin.x + side * dx * spread;
                    let y = origin.y + rise + dy;
                    let rgb = [0xF2C65B, 0x72C79C, 0xB394D9, 0xE98D9D][n % 4];
                    cell(c, facing, x, y, 1.0, 1.0, color(rgb));
                }
            }
        }
        GYM => {
            let h = gym_height(t);
            let x = 10.0;
            cell(c, facing, x - 1.0, h + 1.0, 5.0, 1.0, color(0xB5BBC2));
            cell(c, facing, x - 2.0, h - 1.0, 2.0, 4.0, color(0x64778B));
            cell(c, facing, x + 3.0, h - 1.0, 2.0, 4.0, color(0x64778B));
            cell(c, facing, x - 2.0, h - 1.0, 1.0, 4.0, color(0x90A5B9));
            cell(c, facing, x + 3.0, h - 1.0, 1.0, 4.0, color(0x90A5B9));
        }
        _ => unreachable!(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        engine::{Pet, Skin, State},
        render::{Renderer, HEIGHT, WIDTH},
    };
    #[test]
    fn props_mirror_exactly_and_hold_each_pose_tick() {
        for id in FLAG..=GYM {
            let duration = crate::activities::CATALOG[id].duration;
            for t in [0.5, 1.0, 2.0, 3.0, 4.0] {
                let mut right = Canvas::new(WIDTH, HEIGHT);
                let mut left = Canvas::new(WIDTH, HEIGHT);
                let mut held = Canvas::new(WIDTH, HEIGHT);
                draw(&mut right, id, t, 1.0, duration);
                draw(&mut left, id, t, -1.0, duration);
                draw(&mut held, id, t + 0.05, 1.0, duration);
                assert_eq!(right.pixels, held.pixels);
                for y in 0..HEIGHT {
                    for x in 0..WIDTH {
                        assert_eq!(
                            right.pixels[y * WIDTH + x],
                            left.pixels[y * WIDTH + WIDTH - 1 - x],
                            "mirror {id} at {t}"
                        );
                    }
                }
                assert!(
                    right.pixels[(FEET_Y as usize) * WIDTH..]
                        .iter()
                        .all(|p| *p == 0),
                    "props below floor"
                );
            }
        }
    }
    #[test]
    fn flag_pole_stays_in_the_canonical_palm_in_both_directions() {
        let mut r = Renderer::new().unwrap();
        r.show_mood = false;
        let mut pet = Pet::new(1, Skin::Claude, Vec2::default());
        pet.state = State::Activity;
        pet.activity = crate::activities::ActivityId::new(FLAG);
        for facing in [-1.0, 1.0] {
            for frame in 1..60 {
                let t = frame as f64 / 12.0;
                pet.facing = facing;
                pet.time = t;
                pet.state_time = t;
                let p = pose(FLAG, t).unwrap();
                let hand = p.hands[1];
                let x = (FEET_X + facing * (p.body_x + hand.x + 1.5) * UNIT).floor() as usize;
                let y = (FEET_Y + (hand.y + 1.5) * UNIT).floor() as usize;
                assert_eq!(
                    r.frame(&pet, false).pixels[y * WIDTH + x],
                    0xFFD97757,
                    "grip at {facing}/{frame}"
                );
            }
        }
    }
    #[test]
    fn confetti_waits_for_the_correct_raised_hand() {
        let mut c = Canvas::new(WIDTH, HEIGHT);
        draw(&mut c, CONFETTI, 1.0 / 12.0, 1.0, 5.0);
        assert!(c.pixels.iter().all(|p| *p == 0));
        for (tick, right) in [(2, true), (10, false)] {
            c.clear();
            draw(&mut c, CONFETTI, tick as f64 / 12.0, 1.0, 5.0);
            assert!(c.pixels.iter().any(|p| *p != 0));
            for (index, color) in c.pixels.iter().enumerate() {
                if *color != 0 {
                    assert_eq!(index % WIDTH >= FEET_X as usize, right);
                }
            }
            let p = pose(CONFETTI, tick as f64 / 12.0).unwrap();
            assert_eq!(p.hands[usize::from(right)].y, -15.0);
        }
    }
    #[test]
    fn gym_has_two_held_repetitions_and_returns_the_weight_to_the_floor() {
        let mut reps = 0;
        let mut raised = false;
        for frame in 0..96 {
            let high = gym_height(frame as f64 / 12.0) == -15.0;
            if high && !raised {
                reps += 1;
            }
            raised = high;
        }
        assert_eq!(reps, 2);
        assert_eq!(gym_height(7.5), -3.0);
    }
}

#[cfg(test)]
mod rig_regression_tests {
    use super::*;
    use crate::{
        engine::{Pet, Skin, State},
        render::Renderer,
    };
    #[test]
    fn residual_squash_does_not_separate_activity_hands_and_props() {
        let mut r = Renderer::new().unwrap();
        r.show_mood = false;
        let mut p = Pet::new(1, Skin::Claude, Vec2::default());
        p.state = State::Activity;
        for id in FLAG..=GYM {
            for facing in [-1.0, 1.0] {
                for t in [0.5, 1.0, 2.5, 4.5, 5.0] {
                    p.activity = crate::activities::ActivityId::new(id);
                    p.facing = facing;
                    p.time = t;
                    p.state_time = t;
                    p.squash = 0.0;
                    let normal = r.frame(&p, false).pixels.clone();
                    p.squash = 1.0;
                    assert_eq!(normal, r.frame(&p, false).pixels, "rig id={id} t={t}");
                }
            }
        }
    }
    #[test]
    fn unfolded_flag_is_one_connected_object_including_the_pole() {
        for frame in 3..24 {
            let mut c = Canvas::new(280, 240);
            draw(&mut c, FLAG, frame as f64 / 12.0, 1.0, 6.0);
            let Some(first) = c.pixels.iter().position(|p| *p != 0) else {
                panic!("missing flag")
            };
            let mut seen = vec![false; c.pixels.len()];
            let mut pending = vec![first];
            seen[first] = true;
            while let Some(at) = pending.pop() {
                let x = at % c.width;
                let y = at / c.width;
                for (nx, ny) in [
                    (x.wrapping_sub(1), y),
                    (x + 1, y),
                    (x, y.wrapping_sub(1)),
                    (x, y + 1),
                ] {
                    if nx < c.width && ny < c.height {
                        let next = ny * c.width + nx;
                        if c.pixels[next] != 0 && !seen[next] {
                            seen[next] = true;
                            pending.push(next);
                        }
                    }
                }
            }
            assert!(
                c.pixels.iter().enumerate().all(|(i, p)| *p == 0 || seen[i]),
                "torn fabric at {frame}"
            );
        }
    }
}
