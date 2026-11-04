//! Cosmetic effects drawn in transparent native overlay windows.
//! Effects are tied to a foreign window ID and never modify that window.

use crate::render::Canvas;
use crate::{Rect, Vec2, World};

const WIDTH: usize = 320;
const HEIGHT: usize = 240;
const LIFETIME: f64 = 6.5;

#[derive(Clone, Debug)]
struct Particle {
    x: f64,
    y: f64,
    vx: f64,
    vy: f64,
    size: i32,
    color: u32,
}

/// A short-lived, transparent crack and glass-dust overlay attached to one window.
pub struct CrackEffect {
    window_id: usize,
    window_rect: Rect,
    impact_in_window: Vec2,
    age: f64,
    cracks: Vec<Vec<(i32, i32)>>,
    particles: Vec<Particle>,
    canvas: Canvas,
}

impl CrackEffect {
    pub fn new(window_id: usize, world_point: Vec2, window_rect: Rect) -> Self {
        let mut rng = 0x9E37_79B9_u32 ^ window_id as u32;
        rng ^= world_point.x.to_bits() as u32;
        rng = xorshift(rng);
        rng ^= world_point.y.to_bits() as u32;
        let center = (WIDTH as i32 / 2, HEIGHT as i32 / 2);
        let mut cracks = Vec::new();

        // Irregular radial fractures with small, deterministic branches.
        for ray in 0..14 {
            let angle = std::f64::consts::TAU * ray as f64 / 14.0 + random_signed(&mut rng) * 0.10;
            let length = 52.0 + random_unit(&mut rng) * 72.0;
            let segments = (length / 11.0).ceil() as usize;
            let mut path = vec![center];
            for step in 1..=segments {
                let distance = length * step as f64 / segments as f64;
                let jitter = if step == segments {
                    0.0
                } else {
                    random_signed(&mut rng) * 5.0
                };
                path.push((
                    (center.0 as f64 + angle.cos() * distance + jitter).round() as i32,
                    (center.1 as f64 + angle.sin() * distance + jitter).round() as i32,
                ));
            }
            cracks.push(path.clone());

            if ray % 2 == 0 && path.len() > 4 {
                let at = path.len() / 2;
                let branch_angle = angle + if ray % 4 == 0 { 0.72 } else { -0.72 };
                let start = path[at];
                let branch_len = 22.0 + random_unit(&mut rng) * 30.0;
                let end = (
                    (start.0 as f64 + branch_angle.cos() * branch_len).round() as i32,
                    (start.1 as f64 + branch_angle.sin() * branch_len).round() as i32,
                );
                cracks.push(vec![start, end]);
            }
        }

        let mut particles = Vec::with_capacity(24);
        for _ in 0..24 {
            let angle = random_unit(&mut rng) * std::f64::consts::TAU;
            let speed = 20.0 + random_unit(&mut rng) * 95.0;
            particles.push(Particle {
                x: center.0 as f64 + random_signed(&mut rng) * 13.0,
                y: center.1 as f64 + random_signed(&mut rng) * 13.0,
                vx: angle.cos() * speed,
                vy: angle.sin() * speed * 0.4 - random_unit(&mut rng) * 38.0,
                size: 1 + (random_unit(&mut rng) * 3.0) as i32,
                color: if random_unit(&mut rng) < 0.5 {
                    0xC7DDE8F0
                } else {
                    0xAAB4CBD5
                },
            });
        }

        let mut effect = Self {
            window_id,
            window_rect,
            impact_in_window: Vec2::new(
                world_point.x - window_rect.x,
                world_point.y - window_rect.y,
            ),
            age: 0.0,
            cracks,
            particles,
            canvas: Canvas::new(WIDTH, HEIGHT),
        };
        effect.render();
        effect
    }

    /// Advance the animation and follow the associated window. Returns false when
    /// the effect expired or its window is no longer in the current snapshot.
    pub fn update(&mut self, dt: f64, world: &World) -> bool {
        let Some(window) = world.windows.iter().find(|w| w.id == self.window_id) else {
            return false;
        };
        if !valid_rect(window.rect) {
            return false;
        }

        self.age += if dt.is_finite() { dt.max(0.0) } else { 0.0 };
        if self.age >= LIFETIME {
            return false;
        }
        self.window_rect = window.rect;
        self.render();
        true
    }

    pub fn frame(&self) -> &Canvas {
        &self.canvas
    }

    /// Top-left physical desktop coordinates for the overlay host.
    pub fn position(&self) -> (i32, i32) {
        let anchor_x = self.window_rect.x + self.impact_in_window.x;
        let anchor_y = self.window_rect.y + self.impact_in_window.y;
        (
            (anchor_x - WIDTH as f64 / 2.0).round() as i32,
            (anchor_y - HEIGHT as f64 / 2.0).round() as i32,
        )
    }

    fn render(&mut self) {
        self.canvas.clear();
        let (origin_x, origin_y) = self.position();
        let clip = clip_to_window(self.window_rect, origin_x, origin_y);
        if clip.empty() {
            return;
        }
        let fade = if self.age <= 4.5 {
            1.0
        } else {
            ((LIFETIME - self.age) / (LIFETIME - 4.5)).clamp(0.0, 1.0)
        };

        for path in &self.cracks {
            for pair in path.windows(2) {
                draw_clipped_line(
                    &mut self.canvas,
                    pair[0],
                    pair[1],
                    clip,
                    alpha(0xB0121B25, fade),
                );
                draw_clipped_line(
                    &mut self.canvas,
                    (pair[0].0, pair[0].1 - 1),
                    (pair[1].0, pair[1].1 - 1),
                    clip,
                    alpha(0xD8DCEAF0, fade),
                );
            }
        }

        let age = self.age;
        for particle in &self.particles {
            let x = (particle.x + particle.vx * age).round() as i32;
            let y = (particle.y + particle.vy * age + 48.0 * age * age).round() as i32;
            draw_clipped_rect(
                &mut self.canvas,
                x,
                y,
                particle.size,
                alpha(particle.color, fade * (1.0 - age / LIFETIME)),
                clip,
            );
        }
    }
}

#[derive(Clone, Copy)]
struct Clip {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}
impl Clip {
    fn contains(self, x: i32, y: i32) -> bool {
        x >= self.left && x < self.right && y >= self.top && y < self.bottom
    }
    fn empty(self) -> bool {
        self.left >= self.right || self.top >= self.bottom
    }
}

fn clip_to_window(rect: Rect, origin_x: i32, origin_y: i32) -> Clip {
    Clip {
        left: (rect.x - origin_x as f64).ceil().max(0.0) as i32,
        top: (rect.y - origin_y as f64).ceil().max(0.0) as i32,
        right: (rect.right() - origin_x as f64).ceil().min(WIDTH as f64) as i32,
        bottom: (rect.bottom() - origin_y as f64).ceil().min(HEIGHT as f64) as i32,
    }
}

fn draw_clipped_line(
    canvas: &mut Canvas,
    from: (i32, i32),
    to: (i32, i32),
    clip: Clip,
    color: u32,
) {
    let (mut x, mut y) = from;
    let dx = (to.0 - x).abs();
    let sx = if x < to.0 { 1 } else { -1 };
    let dy = -(to.1 - y).abs();
    let sy = if y < to.1 { 1 } else { -1 };
    let mut error = dx + dy;
    loop {
        if clip.contains(x, y) {
            canvas.pixel(x, y, color);
        }
        if x == to.0 && y == to.1 {
            break;
        }
        let twice = 2 * error;
        if twice >= dy {
            error += dy;
            x += sx;
        }
        if twice <= dx {
            error += dx;
            y += sy;
        }
    }
}

fn draw_clipped_rect(canvas: &mut Canvas, x: i32, y: i32, size: i32, color: u32, clip: Clip) {
    for py in y..y + size {
        for px in x..x + size {
            if clip.contains(px, py) {
                canvas.pixel(px, py, color);
            }
        }
    }
}

fn alpha(color: u32, amount: f64) -> u32 {
    let a = ((color >> 24) as f64 * amount.clamp(0.0, 1.0)).round() as u32;
    (color & 0x00FF_FFFF) | (a << 24)
}

fn valid_rect(rect: Rect) -> bool {
    rect.x.is_finite()
        && rect.y.is_finite()
        && rect.w.is_finite()
        && rect.h.is_finite()
        && rect.w > 0.0
        && rect.h > 0.0
}

fn xorshift(state: u32) -> u32 {
    let mut value = state;
    value ^= value << 13;
    value ^= value >> 17;
    value ^= value << 5;
    value
}

fn random_unit(state: &mut u32) -> f64 {
    *state = xorshift(*state);
    *state as f64 / u32::MAX as f64
}

fn random_signed(state: &mut u32) -> f64 {
    random_unit(state) * 2.0 - 1.0
}

#[cfg(test)]
mod tests {
    use super::CrackEffect;
    use crate::{Rect, Vec2, WindowInfo, World};

    fn world_with(id: usize, rect: Rect) -> World {
        let mut world = World::default();
        world.windows.push(WindowInfo {
            id,
            rect,
            title: String::new(),
            active: false,
            fullscreen: false,
        });
        world
    }

    #[test]
    fn follows_window_movement() {
        let start = Rect::new(100.0, 200.0, 600.0, 400.0);
        let mut effect = CrackEffect::new(7, Vec2::new(180.0, 270.0), start);
        let before = effect.position();
        let world = world_with(7, Rect::new(175.0, 245.0, 600.0, 400.0));
        assert!(effect.update(0.1, &world));
        assert_eq!(effect.position(), (before.0 + 75, before.1 + 45));
    }

    #[test]
    fn expires_at_six_and_a_half_seconds() {
        let rect = Rect::new(0.0, 0.0, 500.0, 400.0);
        let mut effect = CrackEffect::new(1, Vec2::new(200.0, 160.0), rect);
        let world = world_with(1, rect);
        assert!(effect.update(6.49, &world));
        assert!(!effect.update(0.01, &world));
    }

    #[test]
    fn ends_when_associated_window_closes() {
        let rect = Rect::new(0.0, 0.0, 500.0, 400.0);
        let mut effect = CrackEffect::new(1, Vec2::new(200.0, 160.0), rect);
        assert!(!effect.update(0.1, &World::default()));
    }

    #[test]
    fn draws_only_inside_the_associated_window() {
        let rect = Rect::new(160.0, 100.0, 20.0, 20.0);
        let effect = CrackEffect::new(1, Vec2::new(170.0, 110.0), rect);
        let (x, y) = effect.position();
        let frame = effect.frame();
        let nonzero = frame.pixels.iter().enumerate().filter(|(_, p)| **p != 0);
        let mut count = 0;
        for (index, _) in nonzero {
            let px = x + index as i32 % frame.width as i32;
            let py = y + index as i32 / frame.width as i32;
            assert!(rect.contains(Vec2::new(px as f64, py as f64)));
            count += 1;
        }
        assert!(
            count > 0,
            "the clipped effect should still draw in the window"
        );
    }
}
