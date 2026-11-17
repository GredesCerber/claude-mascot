//! Authored object timelines. The renderer alone owns bodies, faces and palms.
use super::{alpha, cell, stage, Frame};
use crate::{render::Canvas, Vec2};

const IDS: [usize; 52] = [
    0, 2, 4, 7, 8, 9, 10, 12, 13, 14, 15, 17, 19, 21, 22, 24, 25, 26, 27, 28, 29, 31, 32, 33, 34,
    35, 39, 41, 43, 44, 46, 47, 48, 49, 50, 51, 52, 53, 55, 56, 57, 58, 60, 62, 63, 65, 66, 68, 69,
    70, 71, 74,
];
const INK: u32 = 0x44506C;
const GREEN: u32 = 0x63C493;
const BLUE: u32 = 0x668ED4;
const PURPLE: u32 = 0x9873D2;
const GOLD: u32 = 0xE7BF57;
const PAPER: u32 = 0xEDE0B9;
const PINK: u32 = 0xDE78A8;
fn phase(t: f64, d: f64) -> usize {
    stage(t.max(0.), d.max(0.01), 12)
}
fn active(q: usize) -> bool {
    (2..=9).contains(&q)
}
fn opacity(t: f64, d: f64, q: usize) -> u32 {
    alpha(t, d).min(match q {
        1 | 10 => 85,
        2 | 9 => 170,
        3..=8 => 255,
        _ => 0,
    })
}
fn rh(f: &mut Frame, x: f64, y: f64) {
    f.body.hands[1] = Vec2::new(x, y);
}
fn lh(f: &mut Frame, x: f64, y: f64) {
    f.body.hands[0] = Vec2::new(x, y);
}
fn both(f: &mut Frame, y: f64) {
    lh(f, -8., y);
    rh(f, 6., y);
}
fn palm(f: &Frame, n: usize) -> Vec2 {
    let h = f.body.hands[n];
    Vec2::new(h.x + f.body.body_x, h.y + f.body.bob)
}
fn bar_height(q: usize) -> f64 {
    [
        -5., -5., -5., -8., -13., -18., -18., -13., -8., -5., -5., -5.,
    ][q]
}
pub(crate) fn frame(id: usize, t: f64, d: f64) -> Option<Frame> {
    if !IDS.contains(&id) {
        return None;
    }
    let mut f = Frame::neutral();
    let q = phase(t, d);
    if t <= 0. || t >= d || q == 0 || q == 11 {
        return Some(f);
    }
    if !active(q) {
        if id == 44 {
            f.body.bob = 1.;
            both(&mut f, -6.);
        } else {
            rh(&mut f, 7., -7.);
        }
        return Some(f);
    }
    match id {
        0 => {
            rh(&mut f, 9., if q.is_multiple_of(2) { -5. } else { -6. });
            lh(&mut f, 5., -5.);
            f.gaze = Vec2::new(2., 1.);
        }
        2 => {
            both(&mut f, if (4..=7).contains(&q) { -16. } else { -12. });
            f.body.bob = if q == 5 || q == 7 { -1. } else { 0. };
        }
        4 => {
            both(
                &mut f,
                if q < 5 {
                    -12.
                } else if q < 9 {
                    -16.
                } else {
                    -10.
                },
            );
            f.gaze.y = -2.;
        }
        7 => {
            f.body.body_x = [-1., 0., 1., 0.][q % 4];
            f.body.lifted_pair = Some(q % 2);
            lh(&mut f, -8., if q.is_multiple_of(2) { -10. } else { -7. });
            rh(&mut f, 6., if q.is_multiple_of(2) { -7. } else { -10. });
        }
        8 => {
            lh(&mut f, -4., if q.is_multiple_of(2) { -4. } else { -5. });
            rh(&mut f, 3., if q.is_multiple_of(2) { -5. } else { -4. });
            f.gaze.y = 2.;
        }
        9 => {
            rh(&mut f, 10., if q < 5 { -5. } else { -6. });
            f.body.bob = if q < 5 { 1. } else { 0. };
            f.gaze = Vec2::new(2., 2.);
        }
        10 => {
            rh(&mut f, 9., -11. - q.saturating_sub(3).min(4) as f64);
            f.gaze = Vec2::new(2., -1.);
        }
        12 => {
            both(&mut f, if q < 5 { -10. } else { -7. });
            f.body.body_x = if q == 6 || q == 7 { 1. } else { 0. };
        }
        13 => {
            if q < 6 {
                lh(&mut f, -9., -13.);
                f.gaze.x = -2.;
            } else {
                rh(&mut f, 8., -14.);
                f.gaze.x = 2.;
            }
        }
        14 => {
            if q.is_multiple_of(2) {
                lh(&mut f, -9., -15.);
            } else {
                rh(&mut f, 8., -15.);
            }
            f.body.lifted_pair = Some(q % 2);
        }
        15 => {
            rh(&mut f, 4., -12.);
            f.gaze = Vec2::new(1., if q < 6 { -2. } else { -4. });
        }
        17 => {
            f.body.body_x = [-1., 0., 1., 0.][q % 4];
            both(&mut f, if q.is_multiple_of(2) { -12. } else { -7. });
            f.gaze.x = if q.is_multiple_of(2) { -2. } else { 2. };
        }
        19 => {
            both(&mut f, -3.);
            f.gaze.y = 2.;
            if q == 6 {
                f.body.body_x = -1.;
            }
        }
        21 => {
            rh(
                &mut f,
                10.,
                [
                    -8., -8., -4., -8., -8., -12., -12., -16., -16., -16., -8., -8.,
                ][q],
            );
            f.gaze.x = 2.;
        }
        22 => {
            rh(&mut f, 5., -12.);
            f.gaze.x = [-2., 0., 2., 0.][q % 4];
        }
        24 => {
            rh(
                &mut f,
                if q < 6 { 10. } else { 16. },
                if q.is_multiple_of(3) { -5. } else { -8. },
            );
            f.gaze = Vec2::new(2., 2.);
        }
        25 => {
            rh(&mut f, 9., if q < 5 { -12. } else { -9. });
            lh(&mut f, -8., -10.);
            f.gaze.x = 2.;
        }
        26 => {
            lh(&mut f, 5., if q.is_multiple_of(2) { -5. } else { -6. });
            rh(&mut f, 9., if q.is_multiple_of(2) { -6. } else { -5. });
            f.gaze = Vec2::new(2., 1.);
        }
        27 => {
            rh(
                &mut f,
                if q < 5 {
                    9.
                } else if q < 9 {
                    13.
                } else {
                    9.
                },
                -9.,
            );
            f.gaze.x = 2.;
        }
        28 => {
            f.body.body_x = [-1., 0., 1., 0.][q % 4];
            both(&mut f, -10.);
            f.gaze.x = if q < 6 { -2. } else { 2. };
        }
        29 => {
            rh(&mut f, 9., -10.);
            f.gaze = Vec2::new(2., -1.);
        }
        31 => {
            rh(&mut f, 9., if q == 5 || q == 8 { -10. } else { -8. });
            f.body.bob = if q == 6 { 1. } else { 0. };
        }
        32 => {
            f.body.body_x = if q < 5 { -1. } else { 1. };
            f.body.lifted_pair = Some(q % 2);
            lh(&mut f, -8., if q.is_multiple_of(2) { -11. } else { -7. });
            rh(&mut f, 6., if q.is_multiple_of(2) { -7. } else { -11. });
        }
        33 => {
            rh(&mut f, 9., if q == 5 || q == 6 { -11. } else { -7. });
            f.body.bob = if q == 3 || q == 8 { 1. } else { 0. };
            f.gaze.y = 1.;
        }
        34 => {
            both(&mut f, if q < 5 { -5. } else { -11. });
            f.gaze.y = if q < 5 { 2. } else { -2. };
        }
        35 => {
            f.body.body_x = if q.is_multiple_of(2) { -1. } else { 1. };
            f.body.lifted_pair = Some(q % 2);
            both(&mut f, if q.is_multiple_of(3) { -15. } else { -12. });
        }
        39 => {
            rh(
                &mut f,
                if q < 6 { 9. } else { 7. },
                if q < 6 { -13. } else { -9. },
            );
            f.gaze.x = if q < 6 { 2. } else { -2. };
        }
        41 => {
            both(&mut f, if q < 5 { -12. } else { -15. });
            f.gaze.y = -2.;
        }
        43 => {
            lh(&mut f, -9., -10.);
            rh(
                &mut f,
                if (4..7).contains(&q) { -2. } else { 9. },
                if q < 4 {
                    -10.
                } else if q < 7 {
                    -14.
                } else {
                    -9.
                },
            );
            f.gaze.x = if q < 7 { 2. } else { -2. };
        }
        44 => {
            let h = bar_height(q);
            f.body.bob = if q == 2 || q == 9 { 2. } else { 0. };
            let y = h - f.body.bob;
            both(&mut f, y);
            f.gaze.y = if q < 4 { 1. } else { -2. };
        }
        46 => {
            lh(&mut f, -9., -9.);
            rh(&mut f, 9., if q < 5 { -8. } else { -12. });
            f.gaze.x = -2.;
        }
        47 => {
            rh(
                &mut f,
                11.,
                if q < 5 {
                    -6.
                } else if q < 8 {
                    -12.
                } else {
                    -17.
                },
            );
            f.gaze.x = 2.;
        }
        48 => {
            rh(&mut f, 7., -15.);
            f.gaze.y = -2.;
        }
        49 => {
            if q < 5 {
                lh(&mut f, -10., -13.);
            } else if q < 8 {
                rh(&mut f, 10., -16.);
            } else {
                rh(&mut f, 10., -6.);
            }
            f.gaze.x = if q < 5 { -2. } else { 2. };
        }
        50 => {
            both(&mut f, if q < 5 { -6. } else { -12. });
            f.gaze.y = if q < 5 { 2. } else { -3. };
        }
        51 => {
            rh(&mut f, 10., if q < 5 { -4. } else { -6. });
            f.body.bob = if q < 5 { 1. } else { 0. };
            f.gaze.y = 2.;
        }
        52 => {
            lh(&mut f, -9., if q < 6 { -13. } else { -9. });
            rh(&mut f, 8., if q < 6 { -9. } else { -14. });
            f.gaze.x = if q < 6 { -2. } else { 2. };
        }
        53 => {
            lh(&mut f, -8., -10.);
            rh(
                &mut f,
                -7.,
                [
                    -8., -8., -14., -14., -11., -11., -8., -8., -8., -8., -8., -8.,
                ][q],
            );
            f.gaze.y = 1.;
        }
        55 => {
            if q < 6 {
                lh(&mut f, -9., -14.);
            } else {
                rh(&mut f, 8., -6.);
            }
            f.gaze.x = if q < 6 { -1. } else { 1. };
        }
        56 => {
            lh(&mut f, -9., -8.);
            rh(&mut f, 13., if q == 4 || q == 6 { -13. } else { -8. });
            f.gaze.x = 2.;
        }
        57 => {
            lh(&mut f, -8., -13.);
            rh(&mut f, -9., if q.is_multiple_of(2) { -8. } else { -10. });
            f.gaze.y = 1.;
        }
        58 => {
            rh(
                &mut f,
                9.,
                if q < 5 {
                    -10.
                } else if q < 8 {
                    -15.
                } else {
                    -12.
                },
            );
            f.gaze = Vec2::new(2., if q < 5 { -1. } else { -4. });
        }
        60 => {
            lh(
                &mut f,
                -9.,
                if q < 5 {
                    -5.
                } else if q < 8 {
                    -11.
                } else {
                    -17.
                },
            );
            f.gaze.x = -2.;
        }
        62 => {
            f.body.body_x = if q < 6 { 1. } else { -1. };
            both(&mut f, -9.);
            f.body.lifted_pair = Some(q % 2);
        }
        63 => {
            rh(&mut f, 9., if q < 6 { -10. } else { -15. });
            f.gaze.y = -2.;
        }
        65 => {
            rh(&mut f, 10., if q < 5 { -11. } else { -7. });
            f.gaze.x = 2.;
        }
        66 => {
            rh(&mut f, 4., -11.);
            f.gaze.y = -2.;
            if q == 7 {
                lh(&mut f, -9., -12.);
            }
        }
        68 => {
            rh(
                &mut f,
                9.,
                if q < 5 {
                    -6.
                } else if q < 8 {
                    -10.
                } else {
                    -14.
                },
            );
            f.gaze.x = 2.;
        }
        69 => {
            rh(&mut f, 9., -8. - q.saturating_sub(3).min(5) as f64);
            f.gaze.y = -2.;
        }
        70 => {
            lh(&mut f, -9., -9.);
            rh(&mut f, 8., if q < 6 { -7. } else { -11. });
            f.gaze.x = -2.;
        }
        71 => {
            rh(
                &mut f,
                10.,
                if q < 5 {
                    -8.
                } else if q < 8 {
                    -13.
                } else {
                    -18.
                },
            );
            f.gaze.x = 2.;
        }
        74 => {
            lh(&mut f, 5., if q.is_multiple_of(2) { -6. } else { -5. });
            rh(&mut f, 9., if q.is_multiple_of(2) { -5. } else { -6. });
            f.gaze = Vec2::new(2., 1.);
        }
        _ => unreachable!(),
    }
    Some(f)
}

struct Ink<'a> {
    c: &'a mut Canvas,
    facing: f64,
    a: u32,
}
impl Ink<'_> {
    fn r(&mut self, x: f64, y: f64, w: f64, h: f64, col: u32) {
        cell(self.c, self.facing, x, y, w, h, col, self.a);
    }
    fn panel(&mut self, x: f64, y: f64, w: f64, h: f64, col: u32) {
        self.r(x, y, w, h, col);
        self.r(x, y, w, 2., INK);
        for i in 0..3 {
            self.r(x + 1. + i as f64 * 2., y, 1., 1., PINK);
        }
    }
    fn line(&mut self, x: f64, y: f64, w: f64, col: u32) {
        self.r(x, y, w, 1., col);
    }
    fn plus(&mut self, x: f64, y: f64, col: u32) {
        self.r(x + 1., y, 1., 3., col);
        self.r(x, y + 1., 3., 1., col);
    }
    fn check(&mut self, x: f64, y: f64) {
        self.r(x, y + 2., 2., 2., GREEN);
        self.r(x + 2., y, 2., 3., GREEN);
        self.r(x + 4., y - 2., 2., 3., GREEN);
    }
    fn leaf(&mut self, x: f64, y: f64, col: u32) {
        self.r(x + 1., y, 3., 2., col);
        self.r(x, y + 1., 3., 2., col);
        self.r(x + 1., y + 2., 1., 2., GREEN);
    }
    fn plant(&mut self, x: f64, base: f64, h: f64) {
        self.r(x, base - h, 1., h, GREEN);
        if h > 3. {
            self.leaf(x + 1., base - h, GREEN);
            self.leaf(x - 3., base - h + 2., 0x389C78);
        }
    }
    fn cube(&mut self, x: f64, y: f64, col: u32) {
        self.r(x, y, 4., 4., col);
        self.r(x, y, 4., 1., PAPER);
        self.r(x + 3., y + 1., 1., 3., INK);
    }
    fn server(&mut self, x: f64, y: f64, col: u32, on: bool) {
        self.r(x, y, 9., 5., col);
        self.r(x + 1., y + 1., 5., 1., 0xB1C9CD);
        self.r(x + 1., y + 3., 3., 1., INK);
        self.r(x + 7., y + 2., 1., 1., if on { GREEN } else { PINK });
    }
    fn flask(&mut self, x: f64, y: f64, col: u32, fill: f64) {
        self.r(x + 2., y, 2., 4., 0xA6C8CA);
        self.r(x + 1., y, 4., 1., INK);
        self.r(x, y + 4., 6., 4., 0xA6C8CA);
        self.r(x + 1., y + 8. - fill, 4., fill, col);
    }
    fn laptop(&mut self, open: bool, col: u32) {
        self.r(6., -3., 17., 2., INK);
        self.r(7., -4., 15., 1., 0xB2A5C7);
        // Only matte lid back faces the viewer, never an outward-facing screen.
        if open {
            self.r(9., -12., 13., 8., col);
            self.r(10., -11., 1., 6., 0xAEA0C4);
            self.r(15., -9., 2., 1., 0xC8BED5);
        }
    }
    fn arrow(&mut self, x: f64, y: f64, col: u32) {
        self.r(x, y + 2., 2., 5., col);
        self.r(x, y, 5., 2., col);
        self.r(x + 3., y + 2., 2., 2., col);
    }
}

pub(crate) fn draw(c: &mut Canvas, id: usize, t: f64, facing: f64, d: f64) {
    let Some(f) = frame(id, t, d) else {
        return;
    };
    let q = phase(t, d);
    if t <= 0. || t >= d || q == 0 || q == 11 {
        return;
    }
    let mut p = Ink {
        c,
        facing,
        a: opacity(t, d, q),
    };
    // Keep completed structures and marks during cleanup. They fade as a whole;
    // the tower must not suddenly lose three blocks and the map must retain pins.
    let exit = q == 10;
    let q = q.min(9);
    let on = active(q);
    let k = if on { q - 2 } else { 0 };
    let r = palm(&f, 1);
    let l = palm(&f, 0);
    match id {
        0 => {
            p.laptop(on && !exit, PURPLE);
            if (4..=7).contains(&q) {
                for n in 0..3 {
                    let x = 11. + n as f64 * 4.;
                    let y = -17. - ((q + n) % 3) as f64;
                    p.r(x, y, 2., 3., GOLD);
                    p.r(x + 1., y - 3., 1., 3., PINK);
                }
            }
        }
        2 => {
            if on {
                for j in 0..(k.min(4) + 1) {
                    p.r(
                        if j % 2 == 0 { -1. } else { 1. },
                        -26. + j as f64 * 2.,
                        3.,
                        2.,
                        if q < 7 { GREEN } else { GOLD },
                    );
                }
                if q >= 5 {
                    p.plus(-13., -21., GOLD);
                    p.plus(11., -21., GREEN);
                }
            }
        }
        4 => {
            p.r(-10., -1., 20., 1., GREEN);
            if on {
                for n in 0..(k + 1).min(4) {
                    p.r(-8. + n as f64 * 5., -21., 3., 2., GOLD);
                    p.r(-8. + n as f64 * 5., -23., 1., 2., GOLD);
                }
                if q >= 5 {
                    for n in 0..4 {
                        p.plus(-15. + n as f64 * 9., -10. - ((q + n) % 3) as f64 * 3., GOLD);
                    }
                }
            }
        }
        7 => {
            if on {
                for n in 0..(k + 1).min(4) {
                    p.r(
                        -15. - n as f64 * 3.,
                        -2.,
                        2.,
                        1.,
                        if n % 2 == 0 { GOLD } else { PINK },
                    );
                }
            }
        }
        8 => {
            p.r(-7., -4., 14., 3., BLUE);
            p.r(-6., -5., 12., 1., BLUE);
            p.plus(-5., -4., INK);
            p.r(
                3.,
                -3.,
                1.,
                1.,
                if q.is_multiple_of(2) { GREEN } else { PINK },
            );
            p.r(5., -2., 1., 1., GOLD);
            if (5..10).contains(&q) {
                p.panel(-5., -24., 10., 7., INK);
                p.r(-3. + (k % 4) as f64, -21., 2., 2., GREEN);
            }
        }
        9 => {
            p.r(9., -1., 14., 1., 0x8A805B);
            if on {
                for n in 0..3 {
                    p.plant(
                        10. + n as f64 * 4.,
                        -2.,
                        (k.saturating_sub(n) + 1).min(6) as f64,
                    );
                }
                if q < 6 {
                    p.r(r.x + 1., r.y, 3., 3., GOLD);
                    p.r(14., -4., 1., 2., BLUE);
                }
            }
        }
        10 => {
            p.panel(10., -22., 12., 17., 0xD9E7D3);
            if on {
                for n in 0..3 {
                    let h = (k.saturating_sub(n) + 1).min(5) as f64 * 2.;
                    p.r(12. + n as f64 * 3., -6. - h, 2., h, GREEN);
                }
                if q >= 8 {
                    p.check(13., -27.);
                }
            }
        }
        12 => {
            let x = f.body.body_x;
            p.r(-8. + x, -4., 16., 1., PURPLE);
            if on {
                p.r(-8. + x, -6., 2., 2., BLUE);
                p.r(6. + x, -6., 2., 2., BLUE);
                if q >= 4 {
                    p.r(-5. + x, -14., 10., 2., PURPLE);
                    p.r(-3. + x, -16., 6., 2., PURPLE);
                }
                if q >= 7 {
                    p.plus(12., -15., GOLD);
                }
            }
        }
        13 => {
            if on {
                if q < 8 {
                    p.panel(-18., -23., 11., 6., GREEN);
                    p.line(-16., -20., 6., PAPER);
                    p.r(-9., -17., 2., 2., GREEN);
                }
                if q >= 5 {
                    p.panel(7., -25., 12., 6., PINK);
                    p.line(9., -22., 8., PAPER);
                    p.r(8., -19., 2., 2., PINK);
                }
            }
        }
        14 => {
            if on {
                let side = if q.is_multiple_of(2) { -1. } else { 1. };
                let age = (k % 4) as f64;
                for n in 0..5 {
                    p.r(
                        side * (9. + n as f64 * 2.),
                        -17. - age * 2. + (n % 3) as f64,
                        1.,
                        2.,
                        [GREEN, GOLD, PINK, BLUE, PURPLE][n],
                    );
                }
            }
        }
        15 => {
            if on {
                p.r(-1., -22., 3., 2., if q >= 6 { GOLD } else { BLUE });
                if q >= 4 {
                    p.line(-6., -25., 3., GOLD);
                    p.r(7., -25., 1., 3., GREEN);
                }
                if q >= 6 {
                    for n in 0..4 {
                        p.r(-2. + n as f64, -21. + n as f64 * 2., 1., 2., PAPER);
                    }
                    p.plus(3., -29., GOLD);
                }
            }
        }
        17 => {
            if on {
                let pts = [
                    (-13., -16.),
                    (9., -20.),
                    (12., -6.),
                    (-11., -3.),
                    (-4., -25.),
                    (3., -3.),
                ];
                for n in 0..(k + 2).min(6) {
                    let (x, y) = pts[(n + k) % 6];
                    p.plus(x, y, [GREEN, PINK, BLUE][n % 3]);
                }
            }
        }
        19 => {
            p.r(-9., -2., 18., 1., INK);
            p.r(-8., -5., 16., 3., GOLD);
            p.r(-6., -6., 12., 1., PAPER);
            p.r(-4., -5., 1., 2., 0xB87A51);
            p.r(1., -5., 1., 2., 0xB87A51);
            if on && q >= 4 {
                for n in 0..3 {
                    p.r(
                        -4. + n as f64 * 4.,
                        -13. - ((k + n) % 3) as f64 * 2.,
                        1.,
                        3.,
                        0xACB9BD,
                    );
                }
            }
        }
        21 => {
            let cols = [BLUE, PINK, GREEN, GOLD];
            let count = (k / 2 + 1).min(4);
            for (n, col) in cols.iter().enumerate().take(count) {
                p.cube(12., -4. - n as f64 * 4., *col);
            }
            if on && q < 9 {
                p.cube(r.x + 1., r.y - 3., cols[(k / 2 + 1).min(3)]);
            }
        }
        22 => {
            if on {
                let path = [
                    (-5., -23., 10., 1.),
                    (4., -22., 1., 8.),
                    (-4., -15., 8., 1.),
                    (-5., -21., 1., 6.),
                    (-3., -21., 5., 1.),
                    (1., -20., 1., 4.),
                    (-2., -17., 3., 1.),
                ];
                for &(x, y, w, h) in path.iter().take((k + 1).min(7)) {
                    p.r(x, y, w, h, PURPLE);
                }
                if q >= 8 {
                    p.line(-2., -25., 4., GREEN);
                }
            }
        }
        24 => {
            p.r(9., -1., 15., 1., PURPLE);
            if on {
                let n = (k / 2).min(2);
                for j in 0..=n {
                    p.cube(10. + j as f64 * 4., -5., [GREEN, BLUE, GOLD][j]);
                }
                if q.is_multiple_of(2) {
                    p.cube(r.x + 1., r.y - 2., [GREEN, BLUE, GOLD][n]);
                }
            }
        }
        25 => {
            p.panel(11., -17., 12., 11., 0xD3E4D0);
            if on {
                if q < 7 {
                    let x = 13. + (k % 3) as f64 * 2.;
                    p.r(x, -12., 3., 3., PURPLE);
                    p.line(x - 1., -11., 5., PURPLE);
                    p.r(r.x + 2., r.y - 3., 4., 4., BLUE);
                    p.r(r.x + 3., r.y - 2., 2., 2., 0xD5EFE3);
                    p.r(r.x + 1., r.y, 1., 3., INK);
                } else {
                    p.check(15., -13.);
                }
            }
        }
        26 => {
            p.laptop(on && !exit, BLUE);
            if on && q >= 5 {
                p.panel(9., -25., 14., 7., 0xD8E7DA);
                for n in 0..(k - 2).min(3) {
                    p.line(11., -22. + n as f64, 8. - n as f64, GREEN);
                }
            }
        }
        27 => {
            p.r(20., -12., 4., 7., PAPER);
            p.r(21., -10., 1., 2., INK);
            p.r(23., -10., 1., 2., INK);
            if on {
                p.r(r.x + 1., r.y, 4., 3., BLUE);
                p.r(r.x + 5., r.y, 2., 1., GOLD);
                p.r(r.x + 5., r.y + 2., 2., 1., GOLD);
                p.r(r.x - 1., r.y + 2., 2., 4., PURPLE);
                p.line(8., -3., r.x - 6., PURPLE);
                if (5..9).contains(&q) {
                    p.plus(21., -18., GREEN);
                }
            }
        }
        28 => {
            if on {
                let dx = [-2., 0., 2., 0.][k % 4];
                p.r(-5. + dx, -23., 8., 1., PURPLE);
                p.r(-6. + dx, -22., 1., 5., PURPLE);
                p.r(-5. + dx, -17., 8., 1., PURPLE);
                p.r(3. + dx, -22., 1., 5., PURPLE);
                p.plus(9. - dx, -20., PINK);
                if q >= 7 {
                    p.line(-3., -20., 4., GREEN);
                }
            }
        }
        29 => {
            p.r(r.x + 1., r.y - 5., 8., 9., BLUE);
            p.r(r.x + 2., r.y - 4., 6., 7., PINK);
            if q < 7 {
                p.r(r.x + 4., r.y - 3., 2., 3., PAPER);
                p.r(r.x + 4., r.y + 1., 2., 1., PAPER);
            } else {
                p.check(r.x + 2., r.y - 2.);
            }
        }
        31 => {
            p.panel(10., -19., 12., 11., BLUE);
            if on {
                if q < 6 {
                    p.r(13., -15., 5., 1., PINK);
                    p.r(15., -17., 1., 5., PINK);
                } else {
                    p.line(12., -15., (k - 2) as f64 * 2., GREEN);
                    if q >= 8 {
                        p.check(13., -13.);
                    }
                }
            }
        }
        32 => {
            if on {
                for n in 0..3 {
                    let len = (k + n) % 4 + 2;
                    p.r(
                        -13. - len as f64,
                        -10. + n as f64 * 3.,
                        len as f64,
                        1.,
                        [PURPLE, GOLD, GREEN][n],
                    );
                }
                p.r(r.x + 1., r.y - 3., 3., 5., PAPER);
                p.line(r.x + 2., r.y - 3., 2., GREEN);
            }
        }
        33 => {
            p.r(9., -2., 5., 1., INK);
            p.r(10., -5., 3., 3., PAPER);
            p.r(13., -4., 1., 2., PAPER);
            if on {
                for n in 0..5 {
                    let h = if q < 7 { ((k + n) % 3 + 2) as f64 } else { 1. };
                    p.r(-13. + n as f64 * 3., -h, 2., h, GOLD);
                    p.r(-12. + n as f64 * 3., -h - 2., 1., 2., PINK);
                }
                if q >= 8 {
                    p.plus(13., -17., GREEN);
                }
            }
        }
        34 => {
            if on {
                for n in 0..5 {
                    let x = -12. + n as f64 * 6.;
                    p.plant(x, -2., (k + 1).min(4) as f64);
                    if q >= 5 {
                        p.plant(x, -14., (k - 2).min(5) as f64);
                    }
                }
            }
        }
        35 => {
            if on {
                for n in 0..6 {
                    let side = if n % 2 == 0 { -1. } else { 1. };
                    p.r(
                        side * (10. + (n / 2) as f64 * 3.),
                        -20. + ((k + n) % 5) as f64 * 3.,
                        2.,
                        2.,
                        [GREEN, PINK, GOLD, BLUE][n % 4],
                    );
                }
            }
        }
        39 => {
            if on {
                let pts = [
                    (-10., -20.),
                    (-2., -24.),
                    (8., -21.),
                    (12., -13.),
                    (7., -3.),
                    (-5., -2.),
                    (-13., -9.),
                ];
                for n in 0..7 {
                    let (x, y) = pts[(n + k) % 7];
                    p.r(
                        x,
                        y,
                        if n == 0 { 3. } else { 2. },
                        2.,
                        if n == 0 { GREEN } else { PURPLE },
                    );
                }
                if q >= 8 {
                    p.check(-3., -24.);
                }
            }
        }
        41 => {
            let count = if on { (k / 2 + 1).min(3) } else { 1 };
            for n in 0..count {
                let x = -15. + n as f64 * 10.;
                p.r(x + 3., -15., 1., 4., BLUE);
                p.server(
                    x,
                    -20. - if n == 1 { 3. } else { 0. },
                    [BLUE, GREEN, PURPLE][n],
                    q >= 5,
                );
            }
            if on && q >= 8 {
                p.check(-2., -30.);
            }
        }
        43 => {
            p.flask(l.x - 3., l.y - 4., PURPLE, if q >= 6 { 3. } else { 1. });
            p.flask(r.x, r.y - 5., if q >= 6 { GREEN } else { BLUE }, 3.);
            if (4..7).contains(&q) {
                p.r(r.x - 2., r.y - 3., 2., 1., BLUE);
                p.r(r.x - 3., r.y - 1., 1., 2., BLUE);
                p.r(r.x - 5., r.y, 2., 1., BLUE);
                p.r(r.x - 8., r.y, 3., 1., BLUE);
            }
            if (7..10).contains(&q) {
                p.r(-10., -20., 1., 1., GREEN);
                p.r(-8., -23., 2., 2., GREEN);
                p.plus(-13., -27., GOLD);
            }
        }
        44 => {
            let h = bar_height(q) + 1.;
            p.r(-13., h, 26., 1., 0xBCC4D2);
            for x in [-13., 10.] {
                p.r(x, h - 2., 3., 5., BLUE);
                p.r(x + 1., h - 3., 1., 7., INK);
            }
            if q == 5 || q == 6 {
                p.r(-16., -17., 1., 3., GOLD);
                p.r(16., -17., 1., 3., GOLD);
            }
        }
        46 => {
            p.panel(-23., -21., 15., 16., 0x345C54);
            if on {
                for n in 0..4 {
                    p.line(
                        -21.,
                        -17. + n as f64 * 3.,
                        10. - (n % 2) as f64 * 3.,
                        if n == k % 4 { GOLD } else { GREEN },
                    );
                }
                if q < 7 {
                    p.r(-19., -15., 3., 3., PINK);
                    p.line(-20., -14., 5., PINK);
                } else {
                    p.check(-19., -15.);
                }
            }
        }
        47 => {
            let count = if on { (k / 2 + 1).min(3) } else { 1 };
            for n in 0..count {
                p.server(12., -5. - n as f64 * 6., PURPLE, q >= 4 + n);
            }
            if on && q.is_multiple_of(2) {
                p.r(r.x + 1., r.y, 2., 2., GOLD);
            }
        }
        48 => {
            p.panel(-7., -25., 14., 9., PINK);
            if on {
                if q < 6 {
                    p.r(-3., -21., 6., 1., INK);
                    p.r(-1., -23., 2., 5., INK);
                } else {
                    p.r(-4., -21., 8., 1., PAPER);
                    p.check(-2., -21.);
                }
                p.r(r.x + 1., r.y - 4., 1., 5., GOLD);
            }
        }
        49 => {
            let cards = [(-21., -23., BLUE), (9., -27., PURPLE), (10., -10., GOLD)];
            let count = if on { (k / 2 + 1).min(3) } else { 1 };
            for (n, &(x, y, col)) in cards.iter().take(count).enumerate() {
                p.panel(x, y, 11., 7., col);
                if n == k / 3 {
                    p.line(x + 2., y + 4., 7., PAPER);
                }
            }
        }
        50 => {
            if on {
                for n in 0..2 {
                    let x = if n == 0 { -14. } else { 10. };
                    let h = (k.saturating_sub(n) + 1).min(5) as f64;
                    p.r(x, -1. - h, 2., h, PAPER);
                    p.r(x - 3., -3. - h, 8., 2., PURPLE);
                    p.r(x - 1., -4. - h, 4., 1., BLUE);
                    p.r(x - 2., -3. - h, 1., 1., PAPER);
                    if q >= 7 {
                        p.r(x + 2., -3. - h, 1., 1., PINK);
                    }
                }
            }
        }
        51 => {
            p.r(9., -1., 15., 1., GOLD);
            if on {
                p.plant(13., -2., (k + 1).min(7) as f64);
                if q >= 5 {
                    p.plant(19., -2., (k - 2).min(5) as f64);
                }
                if q < 5 {
                    p.r(r.x + 1., r.y, 2., 2., GREEN);
                }
                if q == 6 || q == 7 {
                    p.r(11., -10., 1., 2., BLUE);
                    p.r(15., -9., 1., 2., BLUE);
                }
            }
        }
        52 => {
            if on {
                let pts = [(-13., -23.), (-15., -8.), (11., -24.), (14., -9.)];
                for (n, &(x, y)) in pts.iter().take((k + 1).min(4)).enumerate() {
                    if n % 2 == 0 {
                        p.plus(x, y, [PINK, BLUE, GOLD, GREEN][n]);
                    } else {
                        p.r(x, y, 1., 5., BLUE);
                        p.line(x, y, 4., BLUE);
                        p.line(x, y + 4., 4., BLUE);
                    }
                }
                if q >= 8 {
                    p.line(-4., -27., 8., GREEN);
                }
            }
        }
        53 => {
            p.r(-13., -17., 9., 13., PAPER);
            for n in 0..4 {
                p.line(-12., -15. + n as f64 * 2., 5. - (n % 2) as f64, GREEN);
            }
            p.r(r.x + 1., r.y - 3., 1., 6., GOLD);
            if on && q >= 5 {
                for n in 0..(k - 2).min(3) {
                    p.r(-5., -15. + n as f64 * 3., 1., 1., GREEN);
                }
            }
            if q >= 8 {
                p.check(5., -23.);
            }
        }
        55 => {
            if on {
                let pts = [
                    (-8., -24.),
                    (0., -24.),
                    (8., -22.),
                    (10., -5.),
                    (0., -2.),
                    (-10., -4.),
                ];
                for &(x, y) in pts.iter().take((k + 2).min(6)) {
                    p.line(x, y, 5., GREEN);
                }
                if q >= 4 {
                    p.arrow(9., -23., GREEN);
                }
                if q >= 7 {
                    p.arrow(-13., -7., GREEN);
                }
            }
        }
        56 => {
            if on {
                p.r(-6., -14., 12., 2., GOLD);
                p.r(-4., -17., 8., 3., GOLD);
            }
            p.r(12., -3., 8., 2., INK);
            p.r(14., -5., 4., 2., BLUE);
            p.r(l.x - 1., l.y - 3., 2., 6., 0xAABDC9);
            p.r(l.x - 2., l.y - 4., 4., 2., 0xAABDC9);
            p.r(l.x - 1., l.y - 4., 2., 1., INK);
            p.r(r.x + 1., r.y, 1., 5., BLUE);
            p.r(r.x - 1., r.y + 4., 5., 2., 0xAABDC9);
            if q == 5 || q == 7 {
                p.plus(17., -9., GOLD);
            }
        }
        57 => {
            let w = if on { 8. + k.min(4) as f64 } else { 5. };
            p.r(-20., -23., w, 18., 0xC3BBDD);
            p.r(-21., -24., w + 2., 2., PURPLE);
            p.r(-21., -6., w + 2., 2., PURPLE);
            if on {
                for n in 0..6 {
                    p.line(
                        -19.,
                        -21. + n as f64 * 2.,
                        if (n + k) % 3 == 0 { w - 3. } else { w - 5. },
                        INK,
                    );
                }
                p.r(-10., -22. + (k % 6) as f64 * 2., 1., 2., GOLD);
            }
        }
        58 => {
            let rise = match q {
                0..=4 => 0.,
                5 => 3.,
                6 => 7.,
                7 => 12.,
                8 => 18.,
                9 => 24.,
                _ => 0.,
            };
            let x = 12.;
            let y = -12. - rise;
            if q < 10 {
                p.r(x, y, 5., 8., PAPER);
                p.r(x + 1., y - 2., 3., 2., PINK);
                p.r(x + 2., y - 4., 1., 2., PINK);
                p.r(x + 1., y + 2., 3., 3., BLUE);
                p.r(x - 2., y + 5., 2., 4., PURPLE);
                p.r(x + 5., y + 5., 2., 4., PURPLE);
                if q >= 5 {
                    p.r(x + 1., y + 8., 3., 4. + (k % 2) as f64, GOLD);
                    p.r(x + 2., y + 11., 1., 2., PINK);
                }
            }
            p.r(10., -1., 10., 1., INK);
            if (3..5).contains(&q) {
                for n in 0..(5 - q) {
                    p.r(20., -16. - n as f64 * 3., 2., 2., GOLD);
                }
            }
        }
        60 => {
            let count = if on { (k / 2 + 1).min(3) } else { 1 };
            for n in 0..count {
                p.server(-21., -5. - n as f64 * 6., BLUE, q >= 3 + n);
            }
            if on && q >= 8 {
                p.check(-20., -26.);
            }
        }
        62 => {
            if on {
                let x = f.body.body_x;
                p.r(-7. + x, -13., 14., 2., GREEN);
                p.r(7. + x, -12., 5., 1., GREEN);
                p.r(10. + x, -11., 4., 1., GOLD);
                if q >= 5 {
                    p.r(-17., -5., 3., 1., 0xBBCBB3);
                    p.r(14., -8., 3., 1., 0xBBCBB3);
                }
            }
        }
        63 => {
            p.r(r.x + 2., r.y - 5., 1., 7., GOLD);
            if on {
                if q < 6 {
                    p.r(r.x, r.y - 3., 5., 2., GOLD);
                } else {
                    p.r(r.x - 1., r.y - 5., 7., 2., GOLD);
                    p.r(r.x + 1., r.y - 7., 3., 6., GOLD);
                    p.check(-4., -24.);
                }
            }
        }
        65 => {
            p.panel(11., -22., 14., 18., INK);
            if on {
                for n in 0..4 {
                    p.line(
                        13.,
                        -18. + n as f64 * 2.,
                        if n == k % 4 { 8. } else { 4. + (n % 2) as f64 },
                        GREEN,
                    );
                }
                for n in 0..3 {
                    p.r(
                        13. + n as f64 * 3.,
                        -7.,
                        2.,
                        1. + ((k + n) % 3) as f64,
                        BLUE,
                    );
                }
            }
        }
        66 => {
            if on {
                for side in [-1., 1.] {
                    let x = side * 10.;
                    let y = -22. + if q < 6 { 0. } else { 2. };
                    p.r(x, y, 1., 6., GREEN);
                    p.r(x + side * 2., y - 2., 1., 2., GREEN);
                    p.r(x + side * 2., y + 6., 1., 2., GREEN);
                }
                if q >= 6 {
                    p.line(-4., -26., 8., BLUE);
                    p.line(-2., -24., 4., GREEN);
                }
            }
        }
        68 => {
            p.r(10., -16., 13., 13., 0xB3D8C0);
            p.line(12., -12., 8., BLUE);
            p.r(16., -14., 1., 9., BLUE);
            if on {
                let pts = [(12., -7.), (18., -11.), (14., -15.)];
                for &(x, y) in pts.iter().take((k / 2 + 1).min(3)) {
                    p.r(x, y - 3., 3., 3., PINK);
                    p.r(x + 1., y, 1., 2., PINK);
                }
                if q >= 8 {
                    p.check(15., -25.);
                }
            }
        }
        69 => {
            if on {
                for n in 0..(k + 1).min(5) {
                    p.r(9. + n as f64 * 2., -8. - n as f64 * 3., 2., 3., GREEN);
                }
                if q >= 6 {
                    p.arrow(17., -24., GREEN);
                    p.plus(-11., -18., BLUE);
                }
            }
        }
        70 => {
            p.panel(-23., -19., 15., 12., GOLD);
            p.r(-21., -15., 4., 5., PURPLE);
            if on {
                p.line(-15., -15., 5., GREEN);
                p.line(-15., -12., 5., BLUE);
                p.r(-15. + (k % 4) as f64, -16., 1., 3., PAPER);
                if q >= 7 {
                    p.r(-21., -15., 4., 5., GREEN);
                    p.check(-19., -24.);
                }
            }
        }
        71 => {
            if on {
                p.r(14., -8., 1., 6., PURPLE);
                if q >= 4 {
                    p.r(10., -13., 9., 1., PURPLE);
                    p.r(10., -13., 1., 4., PURPLE);
                    p.r(18., -13., 1., 4., PURPLE);
                    p.r(9., -10., 3., 3., BLUE);
                    p.r(17., -10., 3., 3., BLUE);
                }
                if q >= 6 {
                    p.r(14., -19., 1., 6., PURPLE);
                    p.r(9., -19., 12., 1., PURPLE);
                    for x in [8., 13., 19.] {
                        p.r(x, -22., 2., 3., GREEN);
                    }
                }
            }
        }
        74 => {
            p.laptop(on && !exit, GREEN);
            if on {
                p.panel(9., -27., 15., 11., INK);
                for n in 0..(k + 1).min(5) {
                    p.line(
                        11. + (n % 2) as f64,
                        -24. + n as f64,
                        7. - (n % 3) as f64,
                        GREEN,
                    );
                }
                if q >= 8 {
                    p.check(16., -31.);
                }
            }
        }
        _ => {}
    }
}

/// Only accessories that genuinely sit in front of the canonical shell.
pub(crate) fn draw_foreground(c: &mut Canvas, id: usize, t: f64, facing: f64, d: f64) {
    let Some(f) = frame(id, t, d) else {
        return;
    };
    let q = phase(t, d);
    if q == 0 || q == 11 || t <= 0. || t >= d {
        return;
    }
    let mut p = Ink {
        c,
        facing,
        a: opacity(t, d, q),
    };
    let x = f.body.body_x;
    let y = f.body.bob;
    if id == 12 {
        p.r(-6. + x, -4. + y, 12., 1., PURPLE);
    }
    if id == 62 {
        p.r(-6. + x, -12. + y, 12., 1., GREEN);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::{FEET_X, FEET_Y, UNIT};
    #[test]
    fn ownership_and_neutral_boundaries() {
        for id in 0..78 {
            assert_eq!(frame(id, 1., 4.).is_some(), IDS.contains(&id));
        }
        for id in IDS {
            for t in [0., 0.05, 3.9, 4., 5.] {
                let f = frame(id, t, 4.).unwrap();
                assert_eq!(f.body.body_x, 0., "{id} at {t}");
                assert_eq!(f.body.bob, 0.);
                assert_eq!(f.body.hands[0].x, -8.);
                assert_eq!(f.body.hands[1].y, -8.);
            }
        }
    }
    #[test]
    fn props_mirror_and_respect_floor() {
        let w = 280;
        let h = 240;
        for id in IDS {
            for q in 0..12 {
                let t = (q as f64 + 0.5) / 3.;
                let mut a = Canvas::new(w, h);
                let mut b = Canvas::new(w, h);
                draw(&mut a, id, t, 1., 4.);
                draw(&mut b, id, t, -1., 4.);
                draw_foreground(&mut a, id, t, 1., 4.);
                draw_foreground(&mut b, id, t, -1., 4.);
                for y in 0..h {
                    for x in 0..w {
                        assert_eq!(
                            a.pixels[y * w + x],
                            b.pixels[y * w + w - 1 - x],
                            "mirror id={id} q={q} x={x} y={y}"
                        );
                        if a.pixels[y * w + x] != 0 {
                            assert!((y as f64) < FEET_Y, "below feet id={id} q={q} y={y}");
                            assert!(
                                (x as f64) >= FEET_X - 28. * UNIT
                                    && (x as f64) < FEET_X + 28. * UNIT
                            );
                            assert!((y as f64) >= FEET_Y - 43. * UNIT);
                        }
                    }
                }
            }
        }
    }
    #[test]
    fn bar_grips_share_the_same_support_height() {
        for q in 2..10 {
            let f = frame(44, (q as f64 + 0.2) / 3., 4.).unwrap();
            assert_eq!(palm(&f, 0).y, bar_height(q));
            assert_eq!(palm(&f, 1).y, bar_height(q));
            assert_eq!(f.body.lifted_pair, None);
        }
    }
    #[test]
    fn every_scene_changes_objects_then_cleans_up() {
        for id in IDS {
            let mut a = Canvas::new(280, 240);
            let mut e = Canvas::new(280, 240);
            // All sampled action stages have full opacity, so a mere entry fade
            // cannot satisfy the test. At least one object state must change.
            draw(&mut a, id, 1.0, 1., 4.);
            let mut changed = false;
            for t in [1.4, 1.8, 2.2, 2.6] {
                let mut b = Canvas::new(280, 240);
                draw(&mut b, id, t, 1., 4.);
                changed |= a.pixels != b.pixels;
            }
            draw(&mut e, id, 3.95, 1., 4.);
            assert!(changed, "static object action {id}");
            assert!(e.pixels.iter().all(|v| *v == 0));
        }
    }

    #[test]
    fn cleanup_preserves_completed_tower_and_map_marks() {
        // Their geometry stays complete while alpha steps down; only the body
        // releases its reach. No early reset to one cube or an unmarked map.
        for id in [21, 68] {
            let mut a = Canvas::new(280, 240);
            let mut b = Canvas::new(280, 240);
            draw(&mut a, id, 3.1, 1., 4.);
            draw(&mut b, id, 3.5, 1., 4.);
            for (a, b) in a.pixels.iter().zip(&b.pixels) {
                assert_eq!(*a != 0, *b != 0, "cleanup geometry reset id={id}");
            }
        }
    }
}
