//! Individually authored small-prop scenes from the original PNG illustrations.
use super::{alpha, cell, Frame};
use crate::{render::Canvas, Vec2};

const IDS: [usize; 23] = [
    1, 3, 5, 6, 11, 16, 18, 20, 23, 30, 36, 37, 38, 40, 42, 45, 54, 59, 61, 64, 67, 72, 73,
];
type Track = [(f64, f64); 8];
const REST_L: (f64, f64) = (-8.0, -8.0);
const REST_R: (f64, f64) = (6.0, -8.0);

// Stage zero and seven are genuinely neutral. All seven boundaries belong to
// this particular scene, including the lengths of its main holds and recovery.
fn boundaries(id: usize) -> [f64; 7] {
    match id {
        1 => [0.06, 0.18, 0.31, 0.47, 0.64, 0.76, 0.92],
        3 => [0.08, 0.20, 0.32, 0.44, 0.61, 0.78, 0.93],
        5 => [0.05, 0.17, 0.30, 0.48, 0.65, 0.80, 0.94],
        6 => [0.07, 0.19, 0.33, 0.46, 0.62, 0.79, 0.94],
        11 => [0.06, 0.16, 0.28, 0.40, 0.54, 0.75, 0.92],
        16 => [0.07, 0.22, 0.35, 0.49, 0.63, 0.78, 0.94],
        18 => [0.05, 0.17, 0.29, 0.42, 0.64, 0.80, 0.94],
        20 => [0.06, 0.20, 0.34, 0.45, 0.59, 0.77, 0.94],
        23 => [0.05, 0.15, 0.28, 0.43, 0.62, 0.79, 0.93],
        30 => [0.08, 0.21, 0.34, 0.46, 0.60, 0.76, 0.92],
        36 => [0.06, 0.19, 0.33, 0.48, 0.64, 0.79, 0.93],
        37 => [0.05, 0.20, 0.32, 0.44, 0.57, 0.75, 0.93],
        38 => [0.07, 0.23, 0.37, 0.49, 0.61, 0.78, 0.94],
        40 => [0.05, 0.18, 0.31, 0.47, 0.66, 0.81, 0.94],
        42 => [0.07, 0.18, 0.30, 0.45, 0.63, 0.80, 0.93],
        45 => [0.06, 0.21, 0.34, 0.47, 0.61, 0.79, 0.94],
        54 => [0.05, 0.18, 0.31, 0.47, 0.66, 0.82, 0.94],
        59 => [0.06, 0.18, 0.32, 0.46, 0.60, 0.77, 0.93],
        61 => [0.04, 0.16, 0.28, 0.46, 0.68, 0.83, 0.95],
        64 => [0.07, 0.20, 0.33, 0.45, 0.59, 0.78, 0.94],
        67 => [0.06, 0.19, 0.35, 0.48, 0.62, 0.79, 0.94],
        72 => [0.05, 0.18, 0.30, 0.43, 0.61, 0.80, 0.94],
        _ => [0.07, 0.20, 0.34, 0.48, 0.62, 0.77, 0.93],
    }
}
fn cue(id: usize, t: f64, d: f64) -> usize {
    boundaries(id).iter().filter(|&&cut| t >= cut * d).count()
}

fn tracks(id: usize) -> (Track, Track) {
    let l = [REST_L; 8];
    let r = [REST_R; 8];
    match id {
        1 => (
            l,
            [
                REST_R,
                (8., -5.),
                (7., -7.),
                (4., -8.),
                (4., -8.),
                (7., -7.),
                (8., -5.),
                REST_R,
            ],
        ),
        3 => (
            [
                REST_L,
                (-9., -10.),
                (-9., -14.),
                (-9., -14.),
                (-9., -14.),
                (-9., -14.),
                (-9., -10.),
                REST_L,
            ],
            [
                REST_R,
                (7., -9.),
                (8., -11.),
                (8., -14.),
                (8., -14.),
                (8., -14.),
                (7., -9.),
                REST_R,
            ],
        ),
        5 => (
            [
                REST_L,
                (-6., -9.),
                (-4., -10.),
                (-4., -10.),
                (-5., -9.),
                (-5., -8.),
                (-7., -8.),
                REST_L,
            ],
            [
                REST_R,
                (5., -9.),
                (3., -10.),
                (4., -10.),
                (4., -9.),
                (3., -8.),
                (5., -8.),
                REST_R,
            ],
        ),
        6 => (
            l,
            [
                REST_R,
                (8., -7.),
                (8., -10.),
                (9., -11.),
                (9., -11.),
                (8., -10.),
                (8., -7.),
                REST_R,
            ],
        ),
        11 => (
            [
                REST_L,
                (-9., -9.),
                (-9., -13.),
                (-9., -13.),
                (-8., -12.),
                (-9., -13.),
                (-9., -9.),
                REST_L,
            ],
            r,
        ),
        16 => (
            [
                REST_L,
                (-6., -8.),
                (-4., -11.),
                (-4., -15.),
                (-4., -15.),
                (-4., -13.),
                (-6., -9.),
                REST_L,
            ],
            [
                REST_R,
                (4., -8.),
                (2., -11.),
                (2., -15.),
                (2., -15.),
                (2., -13.),
                (4., -9.),
                REST_R,
            ],
        ),
        18 => (
            l,
            [
                REST_R,
                (8., -8.),
                (8., -12.),
                (8., -12.),
                (8., -12.),
                (8., -12.),
                (8., -8.),
                REST_R,
            ],
        ),
        20 => (
            [
                REST_L,
                (-4., -13.),
                (-2., -15.),
                (-5., -16.),
                (-5., -16.),
                (-2., -15.),
                (-4., -13.),
                REST_L,
            ],
            [
                REST_R,
                (2., -13.),
                (0., -15.),
                (3., -16.),
                (3., -16.),
                (0., -15.),
                (2., -13.),
                REST_R,
            ],
        ),
        23 => (
            [
                REST_L,
                (-6., -9.),
                (-5., -10.),
                (-5., -10.),
                (-6., -11.),
                (-6., -10.),
                (-7., -9.),
                REST_L,
            ],
            [
                REST_R,
                (4., -9.),
                (3., -10.),
                (3., -10.),
                (4., -11.),
                (4., -10.),
                (5., -9.),
                REST_R,
            ],
        ),
        30 => (
            [
                REST_L,
                (-10., -8.),
                (-10., -11.),
                (-11., -12.),
                (-11., -12.),
                (-10., -11.),
                (-10., -8.),
                REST_L,
            ],
            r,
        ),
        36 => (
            l,
            [
                REST_R,
                (8., -7.),
                (8., -10.),
                (8., -10.),
                (8., -10.),
                (8., -10.),
                (8., -7.),
                REST_R,
            ],
        ),
        37 => (
            [
                REST_L,
                (-9., -8.),
                (-9., -11.),
                (-10., -12.),
                (-8., -12.),
                (-10., -12.),
                (-9., -8.),
                REST_L,
            ],
            r,
        ),
        38 => (
            l,
            [
                REST_R,
                (7., -9.),
                (7., -13.),
                (7., -16.),
                (7., -16.),
                (7., -13.),
                (7., -9.),
                REST_R,
            ],
        ),
        40 => (
            [
                REST_L,
                (-10., -8.),
                (-11., -11.),
                (-12., -13.),
                (-12., -13.),
                (-9., -10.),
                (-8., -8.),
                REST_L,
            ],
            [
                REST_R,
                (8., -8.),
                (9., -11.),
                (10., -13.),
                (10., -13.),
                (7., -10.),
                (6., -8.),
                REST_R,
            ],
        ),
        42 => (
            [
                REST_L,
                (-8., -10.),
                (-8., -14.),
                (-8., -16.),
                (-8., -16.),
                (-8., -16.),
                (-8., -10.),
                REST_L,
            ],
            [
                REST_R,
                (6., -10.),
                (6., -14.),
                (6., -16.),
                (6., -16.),
                (6., -16.),
                (6., -10.),
                REST_R,
            ],
        ),
        45 => (
            [
                REST_L,
                (-6., -9.),
                (-4., -12.),
                (-4., -13.),
                (-6., -13.),
                (-8., -12.),
                (-8., -9.),
                REST_L,
            ],
            [
                REST_R,
                (4., -9.),
                (2., -12.),
                (2., -13.),
                (4., -13.),
                (6., -12.),
                (6., -9.),
                REST_R,
            ],
        ),
        54 => (
            [
                REST_L,
                (-6., -8.),
                (13., -9.),
                (13., -12.),
                (9., -12.),
                (5., -6.),
                (-6., -8.),
                REST_L,
            ],
            [
                REST_R,
                (8., -7.),
                (8., -10.),
                (8., -10.),
                (8., -10.),
                (8., -10.),
                (8., -7.),
                REST_R,
            ],
        ),
        59 => (
            [
                REST_L,
                (-6., -8.),
                (3., -12.),
                (8., -12.),
                (8., -12.),
                (3., -12.),
                (-6., -8.),
                REST_L,
            ],
            [
                REST_R,
                (9., -7.),
                (9., -10.),
                (9., -10.),
                (9., -10.),
                (9., -9.),
                (9., -7.),
                REST_R,
            ],
        ),
        61 => (
            [
                REST_L,
                (-6., -8.),
                (-4., -9.),
                (-3., -9.),
                (-3., -9.),
                (-4., -9.),
                (-6., -8.),
                REST_L,
            ],
            [
                REST_R,
                (4., -8.),
                (2., -9.),
                (1., -9.),
                (1., -9.),
                (2., -9.),
                (4., -8.),
                REST_R,
            ],
        ),
        64 => (
            [
                REST_L,
                (-9., -9.),
                (-9., -13.),
                (-9., -13.),
                (-9., -13.),
                (-9., -13.),
                (-9., -9.),
                REST_L,
            ],
            r,
        ),
        67 => (
            l,
            [
                REST_R,
                (5., -9.),
                (3., -11.),
                (3., -11.),
                (3., -11.),
                (5., -10.),
                (6., -9.),
                REST_R,
            ],
        ),
        72 => (
            [
                REST_L,
                (-8., -10.),
                (-8., -14.),
                (-8., -17.),
                (-8., -17.),
                (-8., -15.),
                (-8., -10.),
                REST_L,
            ],
            [
                REST_R,
                (6., -10.),
                (6., -14.),
                (6., -17.),
                (6., -17.),
                (6., -15.),
                (6., -10.),
                REST_R,
            ],
        ),
        73 => (
            l,
            [
                REST_R,
                (8., -8.),
                (8., -12.),
                (8., -14.),
                (8., -14.),
                (8., -12.),
                (8., -8.),
                REST_R,
            ],
        ),
        _ => (l, r),
    }
}

pub(crate) fn frame(id: usize, t: f64, duration: f64) -> Option<Frame> {
    if !IDS.contains(&id) {
        return None;
    }
    let mut f = Frame::neutral();
    if t <= 0. || t >= duration || duration <= 0. {
        return Some(f);
    }
    let s = cue(id, t, duration);
    let (l, r) = tracks(id);
    f.body.hands = [Vec2::new(l[s].0, l[s].1), Vec2::new(r[s].0, r[s].1)];
    if s > 0 && s < 7 {
        // Hearts must clear the forehead in preparation and recovery too,
        // rather than becoming a pair of partially hidden pink protrusions.
        let hand_raise = match id {
            16 => 2.,
            45 => 3.,
            _ => 0.,
        };
        for palm in &mut f.body.hands {
            palm.y -= hand_raise;
        }
        match id {
            1 => {
                f.gaze = Vec2::new(1., 0.);
                if s == 3 || s == 4 {
                    f.body.body_x = -1.;
                }
            }
            5 => {
                if (2..=5).contains(&s) {
                    f.body.bob = 1.;
                }
                f.gaze = Vec2::new(0., 1.);
            }
            6 => {
                if s == 3 {
                    f.body.body_x = -1.;
                }
                f.gaze = Vec2::new(1., 0.);
            }
            11 => {
                if s == 3 {
                    f.body.body_x = 1.;
                }
                f.gaze = Vec2::new(-1., -1.);
            }
            3 if s == 4 => f.body.bob = -1.,
            72 if s == 4 => {
                f.body.bob = -1.;
                f.gaze.y = -1.;
            }
            16 | 38 | 42 | 72 => {
                f.gaze = Vec2::new(0., -1.);
            }
            18 => {
                f.gaze = Vec2::new(1., -1.);
            }
            20 | 23 => {
                if (2..=5).contains(&s) {
                    f.body.bob = 1.;
                }
                f.gaze = Vec2::new(0., 1.);
            }
            30 | 37 => {
                f.gaze = Vec2::new(-1., -1.);
            }
            36 | 54 => {
                f.gaze = Vec2::new(1., if s == 3 { -1. } else { 0. });
            }
            40 => {
                if s == 3 || s == 4 {
                    f.body.bob = -1.;
                }
            }
            45 => {
                f.gaze = Vec2::new(0., -1.);
                if s == 4 {
                    f.body.bob = -1.;
                }
            }
            59 => {
                f.gaze = Vec2::new(1., 0.);
                if s == 5 {
                    f.body.body_x = -1.;
                }
            }
            61 => {
                if (2..=5).contains(&s) {
                    f.body.bob = 2.;
                }
                f.gaze = Vec2::new(0., 1.);
            }
            64 => {
                f.gaze = Vec2::new(-1., -1.);
            }
            67 => {
                f.gaze = Vec2::new(1., -1.);
            }
            73 => {
                if s == 4 {
                    f.body.bob = -1.;
                }
                f.gaze = Vec2::new(1., -1.);
            }
            _ => {}
        }
    }
    Some(f)
}

struct Pen<'a> {
    c: &'a mut Canvas,
    f: f64,
    a: u32,
}
impl Pen<'_> {
    fn r(&mut self, x: f64, y: f64, w: f64, h: f64, color: u32) {
        cell(self.c, self.f, x, y, w, h, color, self.a);
    }
    fn dot(&mut self, x: f64, y: f64, color: u32) {
        self.r(x, y, 1., 1., color);
    }
    fn check(&mut self, x: f64, y: f64, color: u32) {
        self.dot(x, y + 2., color);
        self.dot(x + 1., y + 3., color);
        self.dot(x + 2., y + 2., color);
        self.dot(x + 3., y + 1., color);
        self.dot(x + 4., y, color);
    }
    fn heart(&mut self, x: f64, y: f64, color: u32) {
        self.r(x + 1., y, 2., 1., color);
        self.r(x + 4., y, 2., 1., color);
        self.r(x, y + 1., 7., 2., color);
        self.r(x + 1., y + 3., 5., 1., color);
        self.r(x + 2., y + 4., 3., 1., color);
        self.dot(x + 3., y + 5., color);
    }
    fn z(&mut self, x: f64, y: f64, color: u32) {
        self.r(x, y, 3., 1., color);
        self.dot(x + 1., y + 1., color);
        self.r(x, y + 2., 3., 1., color);
    }
    fn text(&mut self, x: f64, y: f64, text: &str, color: u32) {
        // The board mirrors as a prop, but its letters remain readable.
        let width = text.chars().count() as f64 * 4. - 1.;
        let origin = if self.f < 0. { -x - width } else { x };
        for (i, ch) in text.chars().enumerate() {
            let rows = match ch {
                'A' => [2, 5, 7, 5, 5],
                'P' => [6, 5, 6, 4, 4],
                'I' => [7, 2, 2, 2, 7],
                'T' => [7, 2, 2, 2, 2],
                'D' => [6, 5, 5, 5, 6],
                'E' => [7, 4, 6, 4, 7],
                'N' => [5, 7, 7, 7, 5],
                'S' => [3, 4, 2, 1, 6],
                'W' => [5, 5, 7, 7, 5],
                'L' => [4, 4, 4, 4, 7],
                'C' => [3, 4, 4, 4, 3],
                'O' => [2, 5, 5, 5, 2],
                'M' => [5, 7, 7, 5, 5],
                _ => [0; 5],
            };
            for (row, bits) in rows.iter().enumerate() {
                for col in 0..3 {
                    if bits & (1 << (2 - col)) != 0 {
                        cell(
                            self.c,
                            1.,
                            origin + i as f64 * 4. + col as f64,
                            y + row as f64,
                            1.,
                            1.,
                            color,
                            self.a,
                        );
                    }
                }
            }
        }
    }
}
fn hand(f: &Frame, n: usize) -> Vec2 {
    Vec2::new(
        f.body.hands[n].x + f.body.body_x,
        f.body.hands[n].y + f.body.bob,
    )
}

pub(crate) fn draw(c: &mut Canvas, id: usize, t: f64, facing: f64, duration: f64) {
    draw_layer(c, id, t, facing, duration, false);
}
pub(crate) fn draw_foreground(c: &mut Canvas, id: usize, t: f64, facing: f64, duration: f64) {
    draw_layer(c, id, t, facing, duration, true);
}

fn draw_layer(c: &mut Canvas, id: usize, t: f64, facing: f64, duration: f64, front: bool) {
    let Some(f) = frame(id, t, duration) else {
        return;
    };
    let bounds = boundaries(id);
    let appear = bounds[0] * duration;
    let retire = bounds[6] * duration;
    let a = alpha(t, duration).min(alpha(t - appear, retire - appear));
    if a == 0 {
        return;
    }
    let s = cue(id, t, duration);
    if s == 0 || s == 7 {
        return;
    }
    let mut p = Pen { c, f: facing, a };
    let l = hand(&f, 0);
    let r = hand(&f, 1);
    // These objects cross in front of the body. Drawing them in exactly one
    // pass prevents alpha from being composited twice during appearance.
    if front {
        if id == 54 && (2..=5).contains(&s) {
            let x = f.body.body_x;
            let y = f.body.bob - 11.;
            // Open frames preserve the common renderer's eyes underneath.
            for left in [-6., 1.] {
                p.r(x + left, y, 5., 1., 0x6A45AA);
                p.r(x + left, y + 5., 5., 1., 0x6A45AA);
                p.r(x + left, y + 1., 1., 4., 0x6A45AA);
                p.r(x + left + 4., y + 1., 1., 4., 0x6A45AA);
            }
            p.r(x - 1., y + 1., 2., 1., 0x6A45AA);
        }
        if id != 1 && id != 23 {
            return;
        }
    } else if id == 1 || id == 23 {
        return;
    }
    match id {
        1 => {
            // Hold by the handle; keep the rim below the eyes during the sip.
            let x = r.x + 1.;
            let y = (r.y + 1.).min(-5.);
            p.r(x, y, 5., 4., 0xA5C7AB);
            p.r(x, y + 4., 4., 1., 0x526C64);
            p.r(x - 2., y + 1., 2., 1., 0x526C64);
            p.dot(x - 2., y + 2., 0x526C64);
            p.r(x - 2., y + 3., 2., 1., 0x526C64);
            p.r(x, y, 5., 1., 0xF2D4A2);
            p.r(x + 1., y, 3., 1., 0x704C36);
            if s == 2 || s == 5 {
                p.dot(x + 2., y - 2., 0xD4DAD4);
                p.dot(x + 3., y - 3., 0xD4DAD4);
            }
        }
        3 => {
            // Raise API placard, then reveal a checked response card.
            p.r(l.x - 10., l.y - 6., 12., 7., 0xC2B7DF);
            p.text(l.x - 9., l.y - 5., "API", 0x5D43AA);
            if (2..=5).contains(&s) {
                p.r(r.x + 1., r.y - 5., 7., 7., 0xA6D9CB);
                p.r(r.x + 1., r.y - 5., 7., 1., 0x34BFA6);
                if s >= 3 {
                    p.check(r.x + 2., r.y - 3., 0x20A78A);
                }
            }
        }
        5 => {
            // Rub each eye, settle both hands and release the sleepy gesture.
            if s == 3 {
                p.dot(-2., -17., 0xB7AEC7);
                p.dot(-1., -18., 0xB7AEC7);
            }
            if s == 4 || s == 5 {
                p.z(3., -19., 0xB7AEC7);
            }
        }
        6 => {
            // Lift edge-first, brace, receive one small edge impact.
            let x = r.x + 1.;
            let y = r.y - 6.;
            let w = if s == 1 || s == 6 { 3. } else { 7. };
            p.r(x, y, w, 7., 0x17AFA4);
            p.r(x + 1., y + 7., w - 2., 1., 0x17AFA4);
            p.r(x + 1., y + 1., w - 2., 5., 0x128E80);
            p.dot(x + 1., y - 1., 0x57D5BC);
            if w == 7. {
                p.check(x + 1., y + 2., 0x57D5BC);
            }
            if s == 4 {
                p.r(x + w + 1., y + 1., 2., 1., 0xC0E6BD);
                p.dot(x + w + 2., y + 3., 0xC0E6BD);
            }
        }
        11 => {
            // Raise and inspect an error paddle before lowering it.
            let x = l.x - 5.;
            let y = l.y - 8.;
            p.r(x, y, 7., 7., 0xC4237D);
            p.r(x + 2., y + 1., 2., 3., 0xFFF1DE);
            p.r(x + 2., y + 5., 2., 1., 0xFFF1DE);
            p.r(l.x, l.y - 1., 1., 2., 0x8F638A);
            if s == 5 {
                p.dot(x - 2., y + 2., 0xBB9EB4);
                p.dot(x - 2., y + 4., 0xBB9EB4);
            }
        }
        16 => {
            // Lift a heart medallion between both hands.
            let x = l.x + 1.;
            let y = l.y - 6.;
            p.r(x, y, 7., 7., 0xFFC34C);
            p.r(x - 1., y + 1., 9., 5., 0xFFC34C);
            p.heart(x, y + 1., 0xF0757D);
            if s == 4 {
                for (dx, dy) in [(-3., 2.), (10., 2.), (3., -3.), (3., 10.)] {
                    p.dot(x + dx, y + dy, 0xEFB44A);
                }
            }
        }
        18 => {
            // Turn the hourglass, let the sand run, restore its upright pose.
            let x = r.x + 1.;
            let y = r.y - 8.;
            if s == 2 {
                p.r(x, y + 2., 1., 6., 0x7764D7);
                p.r(x + 6., y + 2., 1., 6., 0x7764D7);
                p.r(x + 1., y + 3., 5., 1., 0xC0CBEA);
                p.r(x + 1., y + 6., 5., 1., 0xC0CBEA);
                p.r(x + 2., y + 4., 3., 2., 0xF1D16E);
            } else {
                p.r(x, y, 7., 1., 0x7764D7);
                p.r(x, y + 8., 7., 1., 0x7764D7);
                for row in 0..3 {
                    p.r(
                        x + 1. + row as f64,
                        y + 1. + row as f64,
                        5. - 2. * row as f64,
                        1.,
                        0xC0CBEA,
                    );
                    p.r(
                        x + 1. + row as f64,
                        y + 7. - row as f64,
                        5. - 2. * row as f64,
                        1.,
                        0xC0CBEA,
                    );
                }
                if s == 3 {
                    p.r(x + 2., y + 2., 3., 1., 0xF1D16E);
                    p.dot(x + 3., y + 3., 0xF1D16E);
                }
                if s == 3 || s == 4 {
                    p.r(x + 3., y + 4., 1., 2., 0xF1D16E);
                }
                if s == 1 || s >= 4 {
                    p.r(x + 2., y + 7., 3., 1., 0xF1D16E);
                    p.dot(x + 3., y + 6., 0xF1D16E);
                }
            }
        }
        20 => {
            // Separate two jagged heart halves, then bring them together.
            let lx = l.x - 3.;
            let rx = r.x + 1.;
            let y = l.y - 7.;
            p.r(lx + 1., y, 3., 1., 0xDA438F);
            p.r(lx, y + 1., 5., 3., 0xDA438F);
            p.r(rx + 1., y, 3., 1., 0xDA438F);
            p.r(rx, y + 1., 5., 3., 0xDA438F);
            for row in 0..4 {
                let inset = row as f64 + 1.;
                p.r(
                    lx + inset,
                    y + 4. + row as f64,
                    4. - row as f64,
                    1.,
                    0xDA438F,
                );
                p.r(rx, y + 4. + row as f64, 4. - row as f64, 1., 0xDA438F);
            }
            p.r(lx + 1., y + 1., 2., 1., 0xEF83B1);
            p.r(rx + 2., y + 1., 2., 1., 0xEF83B1);
            if s == 2 || s == 5 {
                p.dot((l.x + r.x) / 2. + 1., y + 8., 0xDA438F);
            }
        }
        23 => {
            // Tears descend once; a tissue then follows each wiping palm.
            if s <= 3 {
                let dy = if s == 1 {
                    0.
                } else if s == 2 {
                    2.
                } else {
                    5.
                };
                for x in [-5., 4.] {
                    p.r(x, -12. + dy, 1., 2., 0x66BCF4);
                    p.r(x - 1., -10. + dy, 3., 2., 0x66BCF4);
                }
            }
            if s >= 3 {
                for h in [l, r] {
                    p.r(h.x, h.y - 1., 2., 2., 0xECE5DB);
                    p.dot(h.x + 1., h.y - 2., 0xECE5DB);
                }
            }
        }
        30 => {
            // Handle and globe travel together; the holding wrist turns the globe.
            let x = l.x - 7.;
            let y = l.y - 8.;
            p.r(l.x - 2., l.y - 2., 3., 1., 0x6E5BD2);
            p.dot(l.x - 1., l.y - 1., 0x6E5BD2);
            p.r(x + 1., y, 5., 1., 0x6EDB89);
            p.r(x, y + 1., 7., 5., 0x6EDB89);
            p.r(x + 1., y + 6., 5., 1., 0x6EDB89);
            p.r(x + 1., y + 1., 5., 5., 0x27B8A5);
            let shift = if s == 4 { 1. } else { 0. };
            p.r(x + 1. + shift, y + 2., 2., 2., 0x7DE18A);
            p.r(x + 4. - shift, y + 4., 1., 2., 0x7DE18A);
            p.r(x + 2., y + 3., 3., 1., 0x83E6B0);
        }
        36 => {
            // Unroll parchment, read rows, roll it back onto its left rod.
            let x = r.x + 1.;
            let y = r.y - 8.;
            let w = match s {
                1 | 6 => 2.,
                2 | 5 => 5.,
                _ => 8.,
            };
            p.r(x, y, w, 9., 0xF3D17C);
            p.r(x, y, 2., 9., 0xDDB75A);
            p.r(x - 1., y, 3., 1., 0xFFE3A0);
            p.r(x - 1., y + 8., 3., 1., 0xFFE3A0);
            if w > 2. {
                for row in 0..3 {
                    p.r(
                        x + 3.,
                        y + 2. + row as f64 * 2.,
                        w - 4.,
                        1.,
                        if s == 4 && row == 2 {
                            0xAF8540
                        } else {
                            0xD6AD57
                        },
                    );
                }
            }
        }
        37 => {
            // Pinch the same stem through two deliberate waves.
            let x = l.x - 7.;
            let y = l.y - 5.;
            let tilt = if s == 4 { 1. } else { 0. };
            p.r(x, y + tilt, 4., 1., 0x4CA74D);
            p.r(x + 1., y + 1. + tilt, 5., 2., 0x4CA74D);
            p.r(x + 3., y + 3. + tilt, 4., 1., 0x4CA74D);
            for n in 0..5 {
                p.dot(x + 2. + n as f64, y + 1. + tilt + (n / 2) as f64, 0x96CE65);
            }
            p.r(l.x - 1., l.y - 1., 2., 1., 0x78A34C);
        }
        38 => {
            // Raise an unlit bulb, click on, lower the extinguished bulb.
            let x = r.x - 1.;
            let y = r.y - 9.;
            let lit = s == 3 || s == 4;
            let glass = if lit { 0xFFCF38 } else { 0xC1BEC0 };
            p.r(x + 1., y, 5., 1., glass);
            p.r(x, y + 1., 7., 4., glass);
            p.r(x + 1., y + 5., 5., 1., glass);
            p.r(x + 2., y + 6., 3., 2., 0x8373A4);
            p.r(x + 2., y + 8., 3., 1., 0x53476F);
            if lit {
                p.r(x + 2., y + 2., 2., 2., 0xFFE78B);
                for (dx, dy) in [(-3., 2.), (9., 2.), (3., -3.)] {
                    p.dot(x + dx, y + dy, 0xEABC4F);
                }
            }
        }
        40 => {
            // Gather a light ring: arcs open, hold, and contract into palms.
            let radius = match s {
                1 | 6 => 9.,
                2 | 5 => 12.,
                _ => 15.,
            };
            let color = if s == 4 { 0xE6DFD0 } else { 0xD3CFCD };
            for (x, y) in [
                (-radius, -12.),
                (radius - 1., -12.),
                (-radius + 3., -19.),
                (radius - 4., -19.),
                (-4., -24.),
                (3., -24.),
            ] {
                p.r(x, y, 2., 1., color);
            }
            if s == 3 || s == 4 {
                p.r(l.x - 1., l.y - 2., 3., 1., 0xEEE9D9);
                p.r(r.x, r.y - 2., 3., 1., 0xEEE9D9);
            }
        }
        42 => {
            // Two hands raise a sign, present its message and lower it.
            let y = l.y - 7.;
            p.r(-20., y, 40., 7., 0xE9459B);
            p.text(-19., y + 1., "IT DEPENDS", 0xE5F05F);
            p.r(l.x + 1., l.y - 1., 1., 2., 0xAC875D);
            p.r(r.x + 1., r.y - 1., 1., 2., 0xAC875D);
            if s == 4 {
                p.dot(22., y + 2., 0xE9459B);
                p.dot(23., y + 3., 0xE9459B);
            }
        }
        45 => {
            // Form a heart between palms and release it into a warm cloud.
            let rise = match s {
                1 => 0.,
                2 => 1.,
                3 => 3.,
                4 => 5.,
                5 => 7.,
                _ => 9.,
            };
            p.heart(-3., -19. - rise, 0xDE4D82);
            if (3..=5).contains(&s) {
                p.r(-10., -6., 20., 1., 0xF5EC70);
                p.r(-12., -5., 24., 2., 0xF5EC70);
                p.r(-10., -3., 20., 1., 0xF5EC70);
            }
        }
        54 => {
            // Read rows before turning a page around its fixed spine.
            let x = r.x + 1.;
            let y = r.y - 7.;
            p.r(x, y, 7., 9., 0xE3DC9A);
            p.r(x, y, 1., 9., 0xAFA956);
            for row in 0..3 {
                p.r(x + 2., y + 2. + row as f64 * 2., 4., 1., 0xB7B66C);
            }
            if s == 3 {
                p.r(x + 5., y, 2., 8., 0xFFF1C2);
                p.r(x + 4., y + 1., 1., 6., 0xC9C086);
            }
            if s == 4 {
                p.r(x + 1., y, 2., 8., 0xFFF1C2);
                p.r(x - 1., y + 1., 2., 6., 0xFFF1C2);
            }
            if s == 5 {
                p.r(x + 2., y + 2., 3., 1., 0xACAF61);
                p.r(x + 2., y + 6., 2., 1., 0xACAF61);
            }
        }
        59 => {
            // Insert key, close shackle, turn key, tug-test the locked lock.
            let x = r.x + 1.;
            let y = r.y - 4.;
            p.r(x, y, 7., 6., 0xBE1689);
            p.r(x + 1., y + 1., 5., 4., 0xD542A1);
            let open = s < 3;
            let lift = if open { 2. } else { 0. };
            p.r(x + 1., y - 4. - lift, 5., 1., 0xB4CFCA);
            p.r(x + 1., y - 3. - lift, 1., 3. + lift, 0xB4CFCA);
            p.r(
                x + 5.,
                y - 3. - lift,
                1.,
                if open { 2. } else { 3. },
                0xB4CFCA,
            );
            p.r(x + 3., y + 2., 1., 2., 0xF9E1DF);
            if s <= 4 {
                let kx = l.x + 1.;
                let ky = l.y;
                p.r(kx - 2., ky - 1., 2., 2., 0xE9C76E);
                p.r(kx, ky, if s == 3 || s == 4 { 4. } else { 3. }, 1., 0xE9C76E);
                p.dot(kx + 2., ky + if s == 4 { -1. } else { 1. }, 0xE9C76E);
            }
            if s == 5 {
                p.dot(x + 1., y + 2., 0xF9E1DF);
            }
        }
        61 => {
            // Settle into sleep, one rising Z trail, then release the pose.
            if (2..=5).contains(&s) {
                let dy = match s {
                    2 => 0.,
                    3 => -2.,
                    4 => -4.,
                    _ => -6.,
                };
                p.z(1., -18. + dy, 0x7564C7);
                if s >= 3 {
                    p.z(6., -22. + dy, 0xD8AD68);
                }
                if s == 4 {
                    p.z(-6., -18., 0x51B569);
                }
            }
        }
        64 => {
            // Raise alert window, flash the alert, fold the window away.
            let w = if s == 6 { 7. } else { 13. };
            let x = l.x + 1. - w;
            let y = l.y - 8.;
            p.r(x, y, w, 10., 0xE94CC8);
            p.r(x, y, w, 2., 0xB937A7);
            p.dot(x + 1., y + 1., 0xF6B4DE);
            p.dot(x + 3., y + 1., 0xF6B4DE);
            if s <= 4 {
                p.r(x + 5., y + 3., 2., 3., 0xFFF1ED);
                p.r(x + 5., y + 7., 2., 1., 0xFFF1ED);
            }
            if s == 4 {
                p.r(x + w - 2., y, 2., 2., 0xF9D3E9);
            }
            if s == 5 {
                p.r(x + 2., y + 5., 9., 1., 0xF6B4DE);
            }
        }
        67 => {
            // Touch chin, grow dots into a thought bubble, dismiss it.
            if s >= 2 {
                p.dot(7., -15., 0x70CAB0);
            }
            if (3..=5).contains(&s) {
                p.r(9., -18., 2., 2., 0x70CAB0);
            }
            if s == 4 || s == 5 {
                p.r(11., -27., 7., 1., 0x59CCA4);
                p.r(10., -26., 9., 4., 0x59CCA4);
                p.r(11., -22., 7., 1., 0x59CCA4);
                for x in [12., 14., 16.] {
                    p.dot(x, -24., 0x278D76);
                }
            }
            if s == 6 {
                p.dot(12., -26., 0x70CAB0);
                p.dot(17., -23., 0x70CAB0);
            }
        }
        72 => {
            // Unfurl a welcome banner between both palms, hold, roll down.
            let y = l.y - 8.;
            let w = match s {
                1 | 6 => 14.,
                2 => 20.,
                _ => 30.,
            };
            let x = -w / 2.;
            p.r(x, y, w, 7., 0xF1C5DC);
            p.r(x, y, 1., 7., 0xE996C2);
            p.r(x + w - 1., y, 1., 7., 0xE996C2);
            if w == 30. {
                p.text(-13., y + 1., "WELCOME", 0x95406D);
            }
            p.r(l.x + 1., l.y - 1., 1., 2., 0xD49E7A);
            p.r(r.x + 1., r.y - 1., 1., 2., 0xD49E7A);
        }
        73 => {
            // Raise a badge, draw the check in two strokes, present, lower.
            let x = r.x + 1.;
            let y = r.y - 8.;
            p.r(x + 1., y, 7., 1., 0x159F65);
            p.r(x, y + 1., 9., 6., 0x159F65);
            p.r(x + 1., y + 7., 7., 1., 0x159F65);
            // Keep the badge symbol readable when the holding palm is mirrored.
            let column = |n: f64| if facing < 0. { x + 8. - n } else { x + n };
            if s >= 2 {
                p.dot(column(2.), y + 3., 0x50DE9C);
                p.dot(column(3.), y + 4., 0x50DE9C);
            }
            if s >= 3 {
                p.dot(column(4.), y + 3., 0x50DE9C);
                p.dot(column(5.), y + 2., 0x50DE9C);
                p.dot(column(6.), y + 1., 0x50DE9C);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::{FEET_X, FEET_Y, UNIT};
    fn paint(id: usize, t: f64, f: f64) -> Canvas {
        let mut c = Canvas::new(280, 240);
        draw(&mut c, id, t, f, 4.);
        draw_foreground(&mut c, id, t, f, 4.);
        c
    }
    #[test]
    fn only_owned_ids_have_frames() {
        for id in 0..78 {
            assert_eq!(frame(id, 1., 4.).is_some(), IDS.contains(&id));
        }
    }
    #[test]
    fn beginnings_and_ends_are_neutral_and_empty() {
        for id in IDS {
            for t in [-1., 0., 3.99, 4., 5.] {
                let f = frame(id, t, 4.).unwrap();
                assert_eq!(f.body.hands[0].x, -8., "id {id}");
                assert_eq!(f.body.hands[1].y, -8., "id {id}");
                assert_eq!(f.body.body_x, 0.);
                assert_eq!(f.body.bob, 0.);
                assert!(
                    paint(id, t, 1.).pixels.iter().all(|&v| v == 0),
                    "id {id}, t {t}"
                );
            }
        }
    }
    #[test]
    fn held_objects_remain_in_reach_of_the_rendered_palms() {
        for id in [1, 6, 18, 36, 38, 54, 59, 73] {
            for t in [0.8, 1.5, 2.5, 3.4] {
                let f = frame(id, t, 4.).unwrap();
                let h = hand(&f, 1);
                let c = paint(id, t, 1.);
                let mut grip = false;
                for dx in 0..4 {
                    for dy in -2..2 {
                        let x = (FEET_X + (h.x + dx as f64) * UNIT) as usize;
                        let y = (FEET_Y + (h.y + dy as f64) * UNIT) as usize;
                        grip |= c.pixels[y * c.width + x] >> 24 > 0;
                    }
                }
                assert!(grip, "id {id}, t {t}: palm-side grip lost");
            }
        }
    }
    #[test]
    fn geometry_mirrors_and_every_scene_stays_above_the_floor() {
        for id in IDS {
            for t in [0.8, 1.5, 2., 2.7, 3.4] {
                let a = paint(id, t, 1.);
                let b = paint(id, t, -1.);
                for y in 0..a.height {
                    for x in 0..a.width {
                        let v = a.pixels[y * a.width + x];
                        let mirrored = b.pixels[y * b.width + (a.width - 1 - x)];
                        // Glyphs stay readable when the card changes sides. The
                        // card's silhouette still mirrors exactly around FEET_X.
                        if [3, 42, 72, 73].contains(&id) {
                            assert_eq!(v >> 24, mirrored >> 24, "id {id}");
                        } else {
                            assert_eq!(v, mirrored, "id {id}, t {t}, x {x}, y {y}");
                        }
                        if y >= FEET_Y as usize {
                            assert_eq!(v, 0, "id {id} below floor");
                        }
                    }
                }
            }
        }
    }
    #[test]
    fn actions_have_individual_keyframes() {
        let signatures: Vec<_> = IDS
            .iter()
            .map(|&id| {
                (1..=12)
                    .map(|n| {
                        let f = frame(id, n as f64 / 4., 4.).unwrap();
                        (
                            f.body.hands[0].x as i32,
                            f.body.hands[0].y as i32,
                            f.body.hands[1].x as i32,
                            f.body.hands[1].y as i32,
                            f.body.bob as i32,
                        )
                    })
                    .collect::<Vec<_>>()
            })
            .collect();
        for i in 0..signatures.len() {
            for j in i + 1..signatures.len() {
                assert_ne!(
                    signatures[i], signatures[j],
                    "IDs {} and {} share a timeline",
                    IDS[i], IDS[j]
                );
            }
        }
    }
    #[test]
    fn key_reaches_the_keyhole_before_the_turn() {
        let f = frame(59, 2., 4.).unwrap();
        let key = hand(&f, 0);
        let lock = hand(&f, 1);
        assert_eq!(key.x + 5., lock.x + 4.);
        assert_eq!(key.y, lock.y - 2.);
    }
    #[test]
    fn a_leaf_has_two_waves_but_document_has_one_page_turn() {
        let leaf: Vec<_> = [1.4, 1.9, 2.4]
            .iter()
            .map(|&t| hand(&frame(37, t, 4.).unwrap(), 0).x)
            .collect();
        assert!(leaf[0] < leaf[1] && leaf[2] < leaf[1]);
        let doc = frame(54, 2., 4.).unwrap();
        assert_eq!(hand(&doc, 1).x, 8.);
        assert_eq!(hand(&doc, 0).x, 9.);
    }
}
