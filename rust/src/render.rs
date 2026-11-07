//! CPU renderer. Pixels are premultiplied ARGB u32 (BGRA in little-endian memory).
//! No native handles or behavior decisions live here.
use crate::activities::ActivityGroup;
use crate::engine::{Pet, State, CURSOR_HOLD_DY};
use crate::social::{SocialKind, SocialRole};
use fontdue::{Font, FontSettings};
use std::{collections::BTreeMap, path::Path};

pub const WIDTH: usize = 280;
pub const HEIGHT: usize = 240;
pub const FEET_X: f64 = 140.0;
pub const FEET_Y: f64 = 216.0;

/// Sprite poses share one stepped clock; world-space physics stays at 240 Hz.
pub(crate) fn pose_time(time: f64) -> f64 {
    if time.is_finite() {
        (time.max(0.0) * 12.0 + 1e-8).floor() / 12.0
    } else {
        0.0
    }
}

pub struct Canvas {
    pub pixels: Vec<u32>,
    pub width: usize,
    pub height: usize,
}
impl Canvas {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            pixels: vec![0; width * height],
            width,
            height,
        }
    }
    pub fn clear(&mut self) {
        self.pixels.fill(0);
    }
    pub fn pixel(&mut self, x: i32, y: i32, color: u32) {
        if x < 0 || y < 0 || x as usize >= self.width || y as usize >= self.height {
            return;
        }
        let a = color >> 24;
        let source = ((((color >> 16) & 255) * a / 255) << 16)
            | ((((color >> 8) & 255) * a / 255) << 8)
            | ((color & 255) * a / 255);
        let d = &mut self.pixels[y as usize * self.width + x as usize];
        let inv = 255 - a;
        let blend =
            |shift: u32| ((source >> shift) & 255u32) + (((*d >> shift) & 255u32) * inv / 255u32);
        *d = ((a + (*d >> 24) * inv / 255) << 24) | (blend(16) << 16) | (blend(8) << 8) | blend(0);
    }
    pub fn rect(&mut self, x: f64, y: f64, w: f64, h: f64, color: u32) {
        let x0 = (x.round() as i32).max(0);
        let y0 = (y.round() as i32).max(0);
        let x1 = ((x + w).round() as i32).min(self.width as i32);
        let y1 = ((y + h).round() as i32).min(self.height as i32);
        for py in y0..y1 {
            for px in x0..x1 {
                self.pixel(px, py, color);
            }
        }
    }
    pub fn line(&mut self, mut x: i32, mut y: i32, x1: i32, y1: i32, color: u32) {
        let dx = (x1 - x).abs();
        let sx = if x < x1 { 1 } else { -1 };
        let dy = -(y1 - y).abs();
        let sy = if y < y1 { 1 } else { -1 };
        let mut e = dx + dy;
        loop {
            self.pixel(x, y, color);
            if x == x1 && y == y1 {
                break;
            }
            let e2 = 2 * e;
            if e2 >= dy {
                e += dy;
                x += sx;
            }
            if e2 <= dx {
                e += dx;
                y += sy;
            }
        }
    }
    pub fn scaled(&self, scale: f64) -> Self {
        let w = (self.width as f64 * scale).round() as usize;
        let h = (self.height as f64 * scale).round() as usize;
        let mut out = Self::new(w, h);
        for y in 0..h {
            for x in 0..w {
                out.pixels[y * w + x] =
                    self.pixels[((y as f64 / scale) as usize).min(self.height - 1) * self.width
                        + ((x as f64 / scale) as usize).min(self.width - 1)];
            }
        }
        out
    }
    pub fn save_png(&self, path: &Path) -> Result<(), String> {
        let file = std::fs::File::create(path).map_err(|e| e.to_string())?;
        let mut enc = png::Encoder::new(file, self.width as u32, self.height as u32);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        let mut writer = enc.write_header().map_err(|e| e.to_string())?;
        let mut bytes = Vec::with_capacity(self.pixels.len() * 4);
        for &p in &self.pixels {
            let a = p >> 24;
            let un = |s: u32| {
                (((p >> s) & 255) * 255)
                    .checked_div(a)
                    .unwrap_or(0)
                    .min(255) as u8
            };
            bytes.extend_from_slice(&[un(16), un(8), un(0), a as u8]);
        }
        writer.write_image_data(&bytes).map_err(|e| e.to_string())
    }
}

pub struct Renderer {
    pub canvas: Canvas,
    scaled_canvas: Canvas,
    font: Option<Font>,
    glyphs: BTreeMap<(char, u16), (fontdue::Metrics, Vec<u8>)>,
    /// Controls the small satisfaction meter below speech bubbles.
    pub show_mood: bool,
}
impl Renderer {
    pub fn new() -> Result<Self, String> {
        let font = [
            "C:/Windows/Fonts/segoeui.ttf",
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        ]
        .iter()
        .find_map(|p| {
            std::fs::read(p)
                .ok()
                .and_then(|b| Font::from_bytes(b, FontSettings::default()).ok())
        });
        Ok(Self {
            canvas: Canvas::new(WIDTH, HEIGHT),
            scaled_canvas: Canvas::new(WIDTH, HEIGHT),
            font,
            glyphs: BTreeMap::new(),
            show_mood: true,
        })
    }
    pub fn frame(&mut self, pet: &Pet, show_bubbles: bool) -> &Canvas {
        self.canvas.clear();
        let animation_time = pose_time(pet.time);
        if pet.state == State::Activity && !pet.dragging {
            if let Some(activity) = pet.activity {
                crate::activities::draw_activity(
                    &mut self.canvas,
                    activity,
                    pose_time(pet.state_time),
                    pet.facing,
                    pet.gaze,
                );
            }
        }
        // Activities contribute only their props. Claude's recognizable body,
        // palette, eyes, and proportions always come from this renderer.
        draw_claude(&mut self.canvas, pet);
        if self.show_mood {
            let width = 42.0;
            let in_tower = pet.social.is_some_and(|s| s.kind == SocialKind::Tower);
            let x = FEET_X - width / 2.0 - if in_tower { 72.0 } else { 0.0 };
            let y = if in_tower { FEET_Y - 30.0 } else { 139.0 };
            self.canvas.rect(x, y, width, 4.0, 0xCC211B1B);
            self.canvas
                .rect(x + 1.0, y + 1.0, width - 2.0, 2.0, 0xFF493B35);
            let satisfaction = pet.satisfaction.clamp(0.0, 100.0);
            let color = if satisfaction < 30.0 {
                0xFFE86B59
            } else if satisfaction < 70.0 {
                0xFFE5B85C
            } else {
                0xFF77C88A
            };
            self.canvas.rect(
                x + 1.0,
                y + 1.0,
                (width - 2.0) * satisfaction / 100.0,
                2.0,
                color,
            );
        }
        if pet.hearts > 0.0 {
            for n in 0..3 {
                let t = (animation_time * 0.8 + n as f64 * 0.3).fract();
                let x =
                    FEET_X - 32.0 + n as f64 * 30.0 + (animation_time * 3.0 + n as f64).sin() * 7.0;
                let y = FEET_Y - 80.0 - t * 65.0;
                heart(
                    &mut self.canvas,
                    x,
                    y,
                    ((255.0 * (1.0 - t)) as u32) << 24 | 0xF17692,
                );
            }
        }
        if pet.state == State::Sleep {
            self.text(
                "z z Z",
                FEET_X + 20.0,
                FEET_Y - 90.0 - (animation_time * 2.0).sin() * 3.0,
                15.0,
                0xFFDDD7CA,
            );
        }
        let speed = pet.vel.x.hypot(pet.vel.y);
        if !pet.dragging
            && speed > 110.0
            && matches!(pet.state, State::Thrown | State::Fall | State::Jump)
        {
            // Three short, tapered trails follow velocity, so they read as
            // motion and disappear when the pet is held or nearly still.
            let dx = -pet.vel.x / speed;
            let dy = -pet.vel.y / speed;
            let alpha = ((speed - 110.0) * 0.45).clamp(38.0, 112.0) as u32;
            for n in 0..3 {
                let back = 24.0 + n as f64 * 9.0;
                let x = FEET_X + dx * back;
                let y = FEET_Y - 58.0 + dy * back;
                let length = 7.0 - n as f64 * 1.4;
                let color = ((alpha.saturating_sub(n as u32 * 20)) << 24) | 0x00F0C5A6;
                self.canvas.line(
                    x as i32,
                    y as i32,
                    (x + dx * length + dy * 1.2) as i32,
                    (y + dy * length - dx * 1.2) as i32,
                    color,
                );
            }
        }
        if show_bubbles {
            if let Some(text) = &pet.bubble {
                if pet.bubble_time > 0.0 {
                    self.bubble(text, false);
                }
            }
        }
        &self.canvas
    }
    /// Render the pixel-art scene at its logical size, then draw speech at the
    /// final output resolution so small fonts are never reduced by nearest-neighbor scaling.
    pub fn frame_scaled(&mut self, pet: &Pet, show_bubbles: bool, scale: f64) -> &Canvas {
        let scale = if scale.is_finite() {
            scale.clamp(0.5, 2.0)
        } else {
            1.0
        };
        if scale == 1.0 {
            return self.frame(pet, show_bubbles);
        }
        self.frame(pet, false);
        self.scaled_canvas = self.canvas.scaled(scale);
        if show_bubbles && pet.bubble_time > 0.0 {
            if let Some(text) = &pet.bubble {
                let tall = false;
                draw_bubble(
                    &mut self.scaled_canvas,
                    self.font.as_ref(),
                    &mut self.glyphs,
                    text,
                    tall,
                    scale,
                );
            }
        }
        &self.scaled_canvas
    }
    fn text(&mut self, text: &str, x: f64, y: f64, size: f32, color: u32) {
        draw_text(
            &mut self.canvas,
            self.font.as_ref(),
            &mut self.glyphs,
            text,
            (x, y),
            size,
            color,
        );
    }
    fn bubble(&mut self, text: &str, tall: bool) {
        draw_bubble(
            &mut self.canvas,
            self.font.as_ref(),
            &mut self.glyphs,
            text,
            tall,
            1.0,
        );
    }
}

fn font_text_width(font: Option<&Font>, text: &str, size: f32) -> f64 {
    font.map(|font| {
        text.chars()
            .map(|ch| font.metrics(ch, size).advance_width as f64)
            .sum()
    })
    .unwrap_or(0.0)
}

fn draw_text(
    canvas: &mut Canvas,
    font: Option<&Font>,
    glyphs: &mut BTreeMap<(char, u16), (fontdue::Metrics, Vec<u8>)>,
    text: &str,
    origin: (f64, f64),
    size: f32,
    color: u32,
) {
    let Some(font) = font else {
        return;
    };
    let (mut x, y) = origin;
    for ch in text.chars() {
        let (metrics, coverage) = glyphs
            .entry((ch, (size * 10.0).round() as u16))
            .or_insert_with(|| font.rasterize(ch, size));
        for yy in 0..metrics.height {
            for xx in 0..metrics.width {
                // fontdue supplies grayscale coverage. Keep every coverage
                // value and let Canvas::pixel premultiply and composite it.
                let alpha = coverage[yy * metrics.width + xx] as u32 * (color >> 24) / 255;
                canvas.pixel(
                    x.round() as i32 + metrics.xmin + xx as i32,
                    y.round() as i32 - metrics.ymin - metrics.height as i32 + yy as i32,
                    (alpha << 24) | (color & 0x00FF_FFFF),
                );
            }
        }
        x += metrics.advance_width as f64;
    }
}

fn draw_bubble(
    canvas: &mut Canvas,
    font: Option<&Font>,
    glyphs: &mut BTreeMap<(char, u16), (fontdue::Metrics, Vec<u8>)>,
    text: &str,
    tall: bool,
    scale: f64,
) {
    if font.is_none() {
        return;
    }
    let scale = scale.clamp(0.5, 2.0);
    // At small UI scales preserve a readable minimum physical font size.
    let font_size = (16.0 * scale as f32).clamp(13.0, 40.0);
    let margin_x = (12.0 * scale).max(6.0);
    let max_text_width = (canvas.width as f64 - margin_x * 2.0).max(0.0);
    let mut text = text.chars().take(40).collect::<String>();
    while font_text_width(font, &text, font_size) > max_text_width && !text.is_empty() {
        text.pop();
    }
    let w = (font_text_width(font, &text, font_size) + margin_x * 2.0).min(canvas.width as f64);
    let x = (canvas.width as f64 - w) / 2.0;
    let y = if tall { 28.0 } else { 95.0 } * scale;
    let height = font_size as f64 + 18.0 * scale;
    let baseline = y + font_size as f64 * 1.38;

    // Opaque dark fill and a warm, bright edge keep the antialiased glyphs
    // readable against both the pet and desktop regardless of what is behind it.
    canvas.rect(x, y, w, height, 0xFF241B1B);
    canvas.rect(
        x + 1.0 * scale,
        y + 1.0 * scale,
        (w - 2.0 * scale).max(0.0),
        1.0 * scale,
        0xFFE8957A,
    );
    draw_text(
        canvas,
        font,
        glyphs,
        &text,
        (x + margin_x, baseline),
        font_size,
        0xFFFFF8F2,
    );
    let tail_step = scale.max(1.0);
    for n in 0..5 {
        let inset = n as f64 * tail_step;
        canvas.rect(
            canvas.width as f64 / 2.0 - 5.0 * scale + inset,
            y + height + n as f64 * tail_step,
            (10.0 * scale - 2.0 * inset).max(tail_step),
            tail_step,
            0xFF241B1B,
        );
    }
}
fn heart(c: &mut Canvas, x: f64, y: f64, color: u32) {
    c.rect(x, y, 5.0, 4.0, color);
    c.rect(x + 7.0, y, 5.0, 4.0, color);
    c.rect(x - 1.0, y + 3.0, 14.0, 4.0, color);
    c.rect(x + 1.0, y + 7.0, 10.0, 3.0, color);
    c.rect(x + 4.0, y + 10.0, 4.0, 3.0, color);
}
fn draw_claude(c: &mut Canvas, p: &Pet) {
    let animation_time = pose_time(p.time);
    let state_time = pose_time(p.state_time);
    let palette = crate::palette::Palette::from_rgb(p.body_color);
    let sitting = matches!(p.state, State::Sit | State::Sleep | State::Ledge)
        || (!p.dragging
            && p.state == State::Social
            && p.social
                .is_some_and(|s| s.kind == SocialKind::Leapfrog && s.role == SocialRole::Partner));
    let moving = matches!(p.state, State::Walk | State::Run | State::Chase);
    let air = matches!(p.state, State::Fall | State::Jump | State::Thrown);
    let phase = (animation_time * 7.0) as usize % 2;
    let climbing = p.state == State::Climb && !p.dragging;
    let climb_phase = (state_time * 5.0).floor() as usize % 4;
    let social = if !p.dragging { p.social.as_ref() } else { None };
    let social_kind = social.map(|activity| activity.kind);
    let social_greeting = social_kind == Some(SocialKind::Greeting);
    let social_high_five = social_kind == Some(SocialKind::HighFive);
    let social_dance = social_kind == Some(SocialKind::DanceTogether);
    let social_comfort = social_kind == Some(SocialKind::Comfort);
    let social_watching_laptop = social_kind == Some(SocialKind::ShareLaptop)
        && social.is_some_and(|activity| activity.role == SocialRole::Partner);
    let shared_laptop_owner = social_kind == Some(SocialKind::ShareLaptop)
        && social.is_some_and(|activity| activity.role == SocialRole::Initiator);
    let scene_scale = if p.display_scale.is_finite() {
        p.display_scale.clamp(0.75, 1.5)
    } else {
        1.0
    };
    let peer_dx = social.map_or(0.0, |activity| activity.partner_offset.x / scene_scale);
    let activity = if p.state == State::Activity && !p.dragging {
        p.activity.map(|id| id.spec())
    } else {
        None
    };
    let activity_group = activity.map(|spec| spec.group);
    let activity_slug = activity.map_or("", |spec| spec.slug);
    let activity_food = activity_group == Some(ActivityGroup::Food);
    let activity_strength = activity_group == Some(ActivityGroup::Strength);
    let activity_nature = activity_group == Some(ActivityGroup::Nature);
    let activity_reflection = activity_group == Some(ActivityGroup::Reflection);
    let activity_celebration = activity_group == Some(ActivityGroup::Celebration);
    let activity_coding = activity_group == Some(ActivityGroup::Coding);
    let activity_laptop = matches!(
        activity_slug,
        "angry-at-laptop" | "writing-software-code" | "developer-pair-programming-scene"
    );
    let activity_angry = matches!(
        activity_slug,
        "angry-at-laptop" | "angry-pixel-coffee" | "angry-powerful-lightning"
    );
    let activity_sad = matches!(activity_slug, "crying-sad-reaction" | "broken-heart-sad");
    let activity_sleepy = matches!(activity_slug, "sleeping-soundly" | "artboard-24cla2");
    let activity_lightning = activity_slug == "angry-powerful-lightning";
    let activity_wizard = activity_slug == "artboard-2cla3";
    let activity_document = activity_slug == "reading-data-document";
    let activity_malware = activity_slug == "malware-detection-window";
    let activity_ninja = activity_slug == "stealth-ninja-security";
    let dancing = p.state == State::Dance || social_dance;
    let waving = p.state == State::Wave || social_greeting;
    let typing = p.state == State::Laptop && !p.dragging;
    let typing_pose = typing || activity_coding;
    let laptop_front = typing || activity_laptop;
    let stretching = p.state == State::Stretch && !p.dragging;
    let looking = p.state == State::LookAround && !p.dragging;
    let laptop_phase = if activity_laptop {
        let duration = activity.map_or(3.8, |spec| spec.duration).max(0.1);
        (state_time / duration * 10.0).clamp(0.0, 10.0)
    } else {
        state_time.clamp(0.0, 10.0)
    };
    let angry = p.satisfaction < 30.0 || activity_angry;
    let bob = if climbing {
        if climb_phase < 2 {
            -0.35
        } else {
            0.15
        }
    } else if dancing {
        if ((state_time * 5.0).floor() as usize).is_multiple_of(2) {
            -0.7
        } else {
            0.25
        }
    } else if p.state == State::Hold || p.dragging {
        (animation_time * 2.7).sin() * 0.25
    } else if moving && phase == 1 {
        -0.5
    } else {
        0.0
    };
    let top = if sitting || typing || social_watching_laptop {
        -8.0
    } else {
        -11.0
    } + bob;
    let mut px = |x: f64, y: f64, w: f64, h: f64, col: u32| {
        let sx = 1.0 + ((p.squash.clamp(0.0, 1.0) * 4.0).round() / 4.0) * 0.16;
        let sy = 1.0 - ((p.squash.clamp(0.0, 1.0) * 4.0).round() / 4.0) * 0.32;
        let xx = if p.facing >= 0.0 { x } else { -x - w };
        c.rect(
            FEET_X + xx * 5.0 * sx,
            FEET_Y + y * 5.0 * sy,
            w * 5.0 * sx,
            h * 5.0 * sy,
            col,
        );
    };
    for (i, lx) in [-5.0, -3.0, 2.0, 4.0].iter().enumerate() {
        if climbing {
            let pair_phase = climb_phase % 2;
            let pair = i / 2;
            let lifted = pair == pair_phase;
            let swing = if lifted { 0.45 } else { -0.15 };
            px(
                *lx + swing,
                -3.0 + if lifted { -0.8 } else { 0.0 },
                1.0,
                if lifted { 2.4 } else { 3.2 },
                palette.shadow,
            );
        } else if typing_pose {
            px(*lx, -3.0, 1.0, 3.0, palette.shadow);
        } else if stretching {
            px(
                *lx + if i % 2 == 0 { -0.5 } else { 0.5 },
                -3.0,
                1.0,
                3.0,
                palette.shadow,
            );
        } else if dancing {
            let pair_phase = (state_time * 5.0).floor() as usize % 2;
            let lifted = i / 2 == pair_phase;
            px(
                *lx + if lifted { 0.5 } else { 0.0 },
                -3.0 + if lifted { -0.8 } else { 0.0 },
                1.0,
                if lifted { 2.2 } else { 3.0 },
                palette.shadow,
            );
        } else if p.state == State::Hold || p.dragging {
            px(
                lx + (animation_time * 5.0 + i as f64).sin() * 0.5,
                -3.0,
                1.0,
                4.0,
                palette.shadow,
            );
        } else if p.state == State::Ledge {
            px(lx + 1.0, -1.0, 1.0, 3.5, palette.shadow);
        } else if !sitting {
            let h = if air || moving && i % 2 == phase {
                2.0
            } else {
                3.0
            };
            px(*lx, -3.0 + bob, 1.0, h, palette.shadow);
        }
    }
    let torso_shift = if social_high_five || social_watching_laptop || activity_nature {
        0.3
    } else if climbing {
        (climb_phase as f64 - 1.5) * 0.12
    } else {
        0.0
    };
    px(-6.0 + torso_shift, top, 12.0, 8.0, palette.body);
    px(-6.0, top + 7.0, 12.0, 1.0, palette.shadow);
    px(-5.0, top, 3.0, 1.0, palette.highlight);
    drop(px);
    if laptop_front {
        draw_laptop(
            c,
            laptop_phase,
            FEET_X
                + if shared_laptop_owner {
                    peer_dx.clamp(-110.0, 110.0) * 0.5
                } else {
                    0.0
                },
        );
    }
    let mut px = |x: f64, y: f64, w: f64, h: f64, col: u32| {
        let sx = 1.0 + ((p.squash.clamp(0.0, 1.0) * 4.0).round() / 4.0) * 0.16;
        let sy = 1.0 - ((p.squash.clamp(0.0, 1.0) * 4.0).round() / 4.0) * 0.32;
        let xx = if p.facing >= 0.0 { x } else { -x - w };
        c.rect(
            FEET_X + xx * 5.0 * sx,
            FEET_Y + y * 5.0 * sy,
            w * 5.0 * sx,
            h * 5.0 * sy,
            col,
        );
    };
    let up = climbing
        || waving
        || stretching
        || p.dragging
        || matches!(
            p.state,
            State::Happy | State::Jump | State::Thrown | State::Hold | State::Climb
        );
    let arm = top + if up { 0.0 } else { 3.0 };
    if climbing {
        // Reach alternates sides; mirroring is handled by px(), so the leading
        // hand stays in front of the direction Claude is climbing.
        let reach = if climb_phase < 2 { 1.0 } else { -1.0 };
        let lead_x = if reach > 0.0 { 6.0 } else { -8.0 };
        let trail_x = if reach > 0.0 { -8.0 } else { 6.0 };
        let lead_arm_x = if reach > 0.0 { 5.0 } else { -7.0 };
        let trail_arm_x = if reach > 0.0 { -7.0 } else { 5.0 };
        px(lead_arm_x, top - 2.6, 2.0, 3.4, palette.body);
        px(trail_arm_x, top + 1.0, 2.0, 2.8, palette.body);
        px(lead_x, top - 2.0, 2.0, 2.0, palette.body);
        px(trail_x, top + 2.0, 2.0, 2.0, palette.body);
    } else if social_high_five {
        // `px` already mirrors local coordinates for left-facing Claude.
        let elapsed = social.map_or(state_time, |activity| pose_time(activity.elapsed));
        let rise = if elapsed < 0.55 {
            let t = (elapsed / 0.55).clamp(0.0, 1.0);
            1.0 - (1.0 - t).powi(3)
        } else if elapsed < 0.9 {
            1.0
        } else {
            (1.0 - (elapsed - 0.9) / 0.95).clamp(0.0, 1.0)
        };
        let recoil = if (0.7..0.95).contains(&elapsed) {
            ((elapsed - 0.7) / 0.25 * std::f64::consts::PI).sin() * 0.3
        } else {
            0.0
        };
        let lead_x = 6.0;
        let trailing_x = -8.0;
        px(
            lead_x,
            top + 3.0 - rise * 4.0 + recoil,
            2.0,
            2.0 + rise,
            palette.body,
        );
        px(trailing_x, top + 3.0, 2.0, 2.0, palette.body);
        let contact_x = (peer_dx.abs() / 10.0 - 1.0).clamp(6.0, 8.6);
        px(
            lead_x + (contact_x - lead_x) * rise,
            top + 3.0 - rise * 6.0 + recoil,
            2.0,
            2.0,
            palette.highlight,
        );
    } else if social_comfort {
        let open = (state_time * 2.0).sin().abs() * 0.35;
        px(-9.0, top + 1.5 + open, 3.0, 2.0, palette.body);
        px(-11.0, top + 0.5 + open, 2.0, 2.0, palette.highlight);
        px(6.0, top + 1.5 - open, 3.0, 2.0, palette.body);
        px(9.0, top + 0.5 - open, 2.0, 2.0, palette.highlight);
    } else if social_watching_laptop || shared_laptop_owner {
        // Both poses reach toward the same world-space laptop, not two
        // unrelated centers. Only the owner draws the shared prop.
        let target = (peer_dx.abs() / 10.0).clamp(6.0, 11.0);
        let tap = if shared_laptop_owner {
            (state_time * 12.0).sin() * 0.2
        } else {
            0.0
        };
        px(5.0, top + 3.0, 2.0, 2.0, palette.body);
        px(6.0, -2.6, (target - 6.0).max(1.0), 1.0, palette.body);
        px(target - 1.0, -2.8 + tap, 2.0, 1.4, palette.highlight);
        px(-8.0, top + 3.0, 2.0, 2.0, palette.body);
    } else if activity_strength {
        // Lift on the effort beat, lower into a short breathing pause.
        let beat = (state_time * 1.9).fract();
        if beat > 0.19 && beat < 0.59 {
            px(-8.0, top - 1.0, 2.0, 3.0, palette.body);
            px(6.0, top - 1.0, 2.0, 3.0, palette.body);
            px(-9.0, top - 3.0, 2.0, 2.0, palette.highlight);
            px(7.0, top - 3.0, 2.0, 2.0, palette.highlight);
        } else {
            px(-8.0, top + 3.0, 2.0, 2.0, palette.body);
            px(6.0, top + 3.0, 2.0, 2.0, palette.body);
        }
    } else if activity_food {
        // A small centered hold pose supports the food prop without changing
        // Claude's torso size or replacing the canonical hands.
        px(-7.0, top + 2.0, 2.0, 2.0, palette.body);
        px(5.0, top + 2.0, 2.0, 2.0, palette.body);
        px(-3.0, top + 4.0, 2.0, 2.0, palette.highlight);
        px(1.0, top + 4.0, 2.0, 2.0, palette.highlight);
    } else if activity_nature {
        // Claude reaches down gently toward the growing prop beside him.
        px(-7.0, top + 3.0, 2.0, 2.0, palette.body);
        px(5.0, top + 3.0, 2.0, 2.0, palette.body);
        px(7.0, top + 5.0, 2.0, 2.0, palette.highlight);
    } else if activity_reflection {
        // A tiny hand-to-chin gesture reads as thought while keeping the face
        // and its eye grid in the same canonical position.
        px(5.0, top + 2.0, 2.0, 2.0, palette.body);
        px(4.0, top + 1.0, 2.0, 2.0, palette.highlight);
        px(-8.0, top + 3.0, 2.0, 2.0, palette.body);
    } else if activity_celebration {
        let beat = (state_time * 3.0).fract();
        let lift = if beat < 0.65 { -2.0 } else { -1.0 };
        px(-9.0, top + lift, 2.0, 3.0, palette.body);
        px(-11.0, top + lift - 1.0, 2.0, 2.0, palette.highlight);
        px(7.0, top + lift, 2.0, 3.0, palette.body);
        px(9.0, top + lift - 1.0, 2.0, 2.0, palette.highlight);
    } else if typing_pose {
        // Arms angle inward from Claude's shoulders to the laptop keyboard.
        let tapping = ((laptop_phase * 3.0).floor() as usize).is_multiple_of(2);
        px(-7.0, top + 2.0, 2.0, 2.0, palette.body);
        px(5.0, top + 2.0, 2.0, 2.0, palette.body);
        px(-5.0, top + 4.0, 2.0, 2.0, palette.body);
        px(3.0, top + 4.0, 2.0, 2.0, palette.body);
        px(
            -3.0,
            top + 5.0 + if tapping { -0.3 } else { 0.3 },
            2.0,
            2.0,
            palette.body,
        );
        px(
            1.0,
            top + 5.0 + if tapping { 0.3 } else { -0.3 },
            2.0,
            2.0,
            palette.body,
        );
    } else if stretching {
        let reach = (state_time * 2.5).sin() * 0.5;
        px(-9.0, top - 2.0 + reach, 3.0, 2.0, palette.body);
        px(-11.0, top - 4.0 + reach, 2.0, 2.0, palette.body);
        px(6.0, top - 2.0 - reach, 3.0, 2.0, palette.body);
        px(9.0, top - 4.0 - reach, 2.0, 2.0, palette.body);
    } else if dancing {
        let beat = (state_time * 5.0).floor() as usize % 4;
        let raised = if beat < 2 { -8.0 } else { 6.0 };
        let lowered = if raised < 0.0 { 6.0 } else { -8.0 };
        px(raised, top - 2.0, 2.0, 2.0, palette.body);
        px(lowered, top + 2.0, 2.0, 2.0, palette.body);
    } else if waving {
        let wave = (state_time * 9.0).sin() * 1.2;
        px(6.0, top - 2.0 + wave, 2.0, 2.0, palette.body);
        px(-8.0, top + 3.0, 2.0, 2.0, palette.body);
    } else {
        let dangling = p.state == State::Hold || p.dragging;
        let swing = if dangling {
            (animation_time * 4.0).sin() * 0.45
        } else {
            0.0
        };
        if dangling {
            let hand_y = -CURSOR_HOLD_DY / 5.0;
            // Forearms rise from the shoulders and turn inward toward the
            // grip. Keep the center gap open at the cursor's exact hotspot.
            px(-7.0 + swing, -13.0, 1.5, 5.0, palette.body);
            px(-7.0 + swing, -13.0, 4.0, 1.5, palette.body);
            px(-3.5 + swing, hand_y, 1.5, 1.5, palette.body);
            px(5.5 - swing, -13.0, 1.5, 5.0, palette.body);
            px(1.5 - swing, -13.0, 4.0, 1.5, palette.body);
            px(2.0 - swing, hand_y, 1.5, 1.5, palette.body);
        } else {
            px(-8.0, arm, 2.0, 2.0, palette.body);
            px(6.0, arm, 2.0, 2.0, palette.body);
        }
    }
    let blink = (animation_time * 0.2).fract() > 0.98;
    let activity_closed_eyes = activity_sleepy && (state_time * 1.5).fract() > 0.35;
    let eyes_closed =
        p.state == State::Sleep || p.state == State::Happy || activity_closed_eyes || blink;
    let facing_sign = if p.facing < 0.0 { -1.0 } else { 1.0 };
    let cursor_gaze_x = if p.gaze.x.is_finite() {
        p.gaze.x.clamp(-1.0, 1.0)
    } else {
        0.0
    };
    let cursor_gaze_y = if p.gaze.y.is_finite() {
        p.gaze.y.clamp(-1.0, 1.0)
    } else {
        0.0
    };
    let look_around_x = if looking {
        if ((state_time * 2.0).floor() as usize).is_multiple_of(2) {
            1.0
        } else {
            -1.0
        }
    } else {
        0.0
    };
    let social_look = if social_watching_laptop || social_greeting || social_high_five {
        facing_sign * 0.8
    } else {
        0.0
    };
    let gaze_x = if eyes_closed {
        0.0
    } else {
        (cursor_gaze_x + look_around_x + social_look).clamp(-1.0, 1.0)
    };
    let gaze_y = if eyes_closed { 0.0 } else { cursor_gaze_y };
    // Keep a visible glance at 100% scale while staying within the face.
    // The local pixel unit is 5 physical pixels.
    let eye_offset_x = gaze_x * facing_sign * 0.5;
    let eye_offset_y = gaze_y * 0.3;
    for eye in [-4.0, 3.0] {
        if p.state == State::Sleep || activity_closed_eyes || blink {
            px(eye - 0.3, top + 3.2, 1.6, 0.6, palette.eyes);
        } else if p.state == State::Happy {
            px(eye - 0.3, top + 2.8, 0.6, 0.6, palette.eyes);
            px(eye + 0.3, top + 2.2, 0.6, 0.6, palette.eyes);
            px(eye + 0.9, top + 2.8, 0.6, 0.6, palette.eyes);
        } else {
            px(
                eye + eye_offset_x,
                top + 2.0 + eye_offset_y,
                1.0,
                2.0,
                palette.eyes,
            );
        }
    }
    if angry && !matches!(p.state, State::Happy | State::Sleep) {
        px(-4.5, top + 1.2, 2.0, 0.6, 0xFF6E3029);
        px(2.5, top + 1.2, 2.0, 0.6, 0xFF6E3029);
    }
    if matches!(
        p.state,
        State::Happy | State::Sit | State::Ledge | State::Peek
    ) {
        px(-5.5, top + 4.6, 1.5, 0.8, 0xAAFF8C96);
        px(4.0, top + 4.6, 1.5, 0.8, 0xAAFF8C96);
    }
    if social_greeting {
        px(-0.4, top + 5.1, 1.8, 1.1, 0xFF6E3029);
    }
    if activity_sad {
        // Keep the canonical eye locations and mark sadness with a small
        // downturned mouth and restrained tear pixels.
        px(-0.5, top + 5.1, 1.0, 0.6, 0xFF6E3029);
        px(-2.5, top + 4.7, 0.6, 1.0, 0xFF8BC9D2);
        px(2.0, top + 4.7, 0.6, 1.0, 0xFF8BC9D2);
    }
    if social_comfort {
        let elapsed = social.map_or(0.0, |activity| pose_time(activity.elapsed));
        let pulse = (elapsed * 2.0).sin() * 1.5;
        heart(c, FEET_X + 17.0, FEET_Y - 102.0 - pulse, 0xBFFF9AAE);
    }
    if social_high_five
        && social.is_some_and(|activity| (0.55..0.95).contains(&pose_time(activity.elapsed)))
    {
        // A small contact sparkle marks the raised hand without adding another
        // character or desktop object to the renderer.
        let x = FEET_X
            + if peer_dx.abs() > 1.0 {
                peer_dx * 0.5
            } else {
                p.facing.signum() * 40.0
            };
        let y = FEET_Y - 73.0;
        c.rect(x - 1.0, y - 4.0, 2.0, 8.0, 0xFFD9C88C);
        c.rect(x - 4.0, y - 1.0, 8.0, 2.0, 0xFFD9C88C);
        if social.is_some_and(|activity| (pose_time(activity.elapsed) * 8.0).fract() < 0.5) {
            // Pixel-center rounding is half-open at the canvas center; this
            // asymmetric pair mirrors exactly when facing is reversed.
            c.pixel(x.round() as i32 - 4, y.round() as i32 - 3, 0xFFD9C88C);
            c.pixel(x.round() as i32 + 3, y.round() as i32 + 3, 0xFFD9C88C);
        }
    }
    if activity_lightning {
        // The source bolt was masked with its imported body; this small
        // green bolt sits above Claude's head and stays clear of the eyes.
        let x = FEET_X + 17.0;
        let y = FEET_Y + top * 5.0 - 16.0;
        c.rect(x + 3.0, y, 5.0, 4.0, 0xFF78C56A);
        c.rect(x + 1.0, y + 4.0, 5.0, 4.0, 0xFF78C56A);
        c.rect(x - 1.0, y + 8.0, 5.0, 4.0, 0xFF78C56A);
        c.rect(x - 3.0, y + 12.0, 5.0, 4.0, 0xFF78C56A);
    }
    if activity_wizard {
        // A blue pointed hat preserves the activity cue without importing a
        // differently proportioned face or body.
        let x = FEET_X;
        let y = FEET_Y + top * 5.0 - 18.0;
        c.rect(x - 9.0, y + 12.0, 18.0, 3.0, 0xFF527CC2);
        c.rect(x - 6.0, y + 8.0, 12.0, 4.0, 0xFF527CC2);
        c.rect(x - 3.0, y + 4.0, 6.0, 4.0, 0xFF527CC2);
        c.rect(x - 1.0, y, 3.0, 4.0, 0xFF527CC2);
        c.rect(x - 7.0, y + 13.0, 3.0, 1.0, 0xFFE7C766);
    }
    if activity_ninja {
        // Headband crosses the forehead above the canonical eye row.
        let x = FEET_X;
        let y = FEET_Y + (top + 1.0) * 5.0;
        c.rect(x - 29.0, y, 58.0, 3.0, 0xFF4A9B54);
        c.rect(x + 27.0, y + 1.0, 9.0, 2.0, 0xFF4A9B54);
        c.rect(x + 33.0, y + 3.0, 2.0, 5.0, 0xFF4A9B54);
    }
    if activity_document {
        // A page held beside the torso remains visible without covering
        // Claude's canonical face or changing its proportions.
        let x = if p.facing >= 0.0 {
            FEET_X + 31.0
        } else {
            FEET_X - 51.0
        };
        let y = FEET_Y - 42.0;
        c.rect(x, y, 20.0, 28.0, 0xFFE6E1D4);
        c.rect(x + 3.0, y + 4.0, 13.0, 1.0, 0xFF8A8790);
        c.rect(x + 3.0, y + 8.0, 11.0, 1.0, 0xFF8A8790);
        c.rect(x + 3.0, y + 12.0, 13.0, 1.0, 0xFF8A8790);
        c.rect(x + 3.0, y + 16.0, 9.0, 1.0, 0xFF8A8790);
        c.rect(x + 3.0, y + 21.0, 13.0, 1.0, 0xFF8A8790);
        c.rect(x - 2.0, y + 22.0, 4.0, 3.0, palette.body);
    }
    if activity_malware {
        // Keep the security window small, green, and grounded beside Claude.
        let x = FEET_X + 46.0;
        let y = FEET_Y - 28.0;
        c.rect(x, y, 29.0, 24.0, 0xFF56A95B);
        c.rect(x + 2.0, y + 2.0, 25.0, 20.0, 0xFF263C32);
        c.rect(x + 4.0, y + 5.0, 3.0, 3.0, 0xFF80D577);
        c.rect(x + 10.0, y + 5.0, 3.0, 3.0, 0xFF80D577);
        c.rect(x + 16.0, y + 5.0, 3.0, 3.0, 0xFF80D577);
        c.rect(x + 4.0, y + 12.0, 18.0, 1.0, 0xFF79B978);
        c.rect(x + 4.0, y + 16.0, 13.0, 1.0, 0xFF79B978);
    }
    if activity_document {
        // Thin frames follow the fixed eye grid; the eyes remain visible in
        // the open center of each lens.
        let y = FEET_Y + (top + 1.7) * 5.0;
        for x in [FEET_X - 26.0, FEET_X + 9.0] {
            c.rect(x, y, 17.0, 1.0, 0xFF77528E);
            c.rect(x, y + 1.0, 1.0, 8.0, 0xFF77528E);
            c.rect(x + 16.0, y + 1.0, 1.0, 8.0, 0xFF77528E);
            c.rect(x + 1.0, y + 8.0, 15.0, 1.0, 0xFF77528E);
        }
        c.rect(FEET_X - 9.0, y + 3.0, 18.0, 1.0, 0xFF77528E);
    }
}

/// Draw the rear of a compact laptop lid with its keyboard deck facing Claude.
/// The open amount maps to the ten second sequence: open, type, then close.
fn draw_laptop(c: &mut Canvas, phase: f64, center: f64) {
    let open = if phase < 1.0 {
        phase
    } else if phase < 9.0 {
        1.0
    } else {
        10.0 - phase
    }
    .clamp(0.0, 1.0);
    let base_y = FEET_Y - 11.0;
    let screen_h = 2.0 + open * 14.0;
    let screen_top = base_y - screen_h;

    // The matte rear lid faces the viewer. A restrained four-pixel mark keeps
    // it from reading as a lit screen; no display content faces outward.
    c.rect(center - 19.0, base_y - 1.0, 38.0, 2.0, 0xFF514947);
    c.rect(center - 17.0, screen_top, 34.0, screen_h, 0xFF393638);
    c.rect(
        center - 15.0,
        screen_top + 1.0,
        30.0,
        (screen_h - 2.0).max(0.0),
        0xFF5B5653,
    );
    if open > 0.2 {
        c.rect(
            center - 1.0,
            screen_top + screen_h * 0.48,
            2.0,
            2.0,
            0xFFB7A99A,
        );
    }

    // The thin deck is seen from behind the lid. Its keyboard and wrists sit
    // on the Claude-facing side; small alternating key highlights imply taps.
    c.rect(center - 22.0, base_y, 44.0, 3.0, 0xFFB9ADA0);
    c.rect(center - 21.0, base_y, 42.0, 1.0, 0xFFE0D5C8);
    let key_flash =
        if (1.0..9.0).contains(&phase) && ((phase * 3.0).floor() as usize).is_multiple_of(2) {
            0xFF736B66
        } else {
            0xFF625B57
        };
    c.rect(center - 17.0, base_y + 1.0, 34.0, 1.0, key_flash);
    c.rect(center - 3.0, base_y + 1.0, 7.0, 1.0, 0xFF9C9188);
}

#[cfg(test)]
mod tests {
    #[test]
    fn pixel_poses_hold_between_common_animation_ticks() {
        use super::*;
        let mut pet = Pet::new(1, crate::engine::Skin::Claude, crate::Vec2::default());
        let mut renderer = Renderer::new().unwrap();
        renderer.show_mood = false;
        for state in [
            State::Walk,
            State::Dance,
            State::Hold,
            State::Laptop,
            State::Stretch,
            State::LookAround,
        ] {
            pet.state = state;
            pet.time = 2.01;
            pet.state_time = 2.01;
            let first = renderer.frame(&pet, false).pixels.clone();
            pet.time = 2.06;
            pet.state_time = 2.06;
            assert_eq!(
                first,
                renderer.frame(&pet, false).pixels,
                "{state:?} must hold its pose during a shared animation tick"
            );
        }
        assert_eq!(pose_time(2.06), 2.0);
        assert!(pose_time(2.09) > 2.0);
    }
    #[test]
    fn recoloring_preserves_claude_geometry_and_laptop_colors() {
        use super::*;
        use crate::engine::Skin;
        let mut pet = Pet::new(1, Skin::Claude, crate::Vec2::default());
        pet.state = State::Idle;
        let mut renderer = Renderer::new().unwrap();
        let original = renderer.frame(&pet, false).pixels.clone();
        pet.body_color = 0x609BD5;
        let colored = renderer.frame(&pet, false).pixels.clone();
        assert!(colored.iter().filter(|&&c| c == 0xFF609BD5).count() > 500);
        assert!(original
            .iter()
            .zip(&colored)
            .all(|(a, b)| a >> 24 == b >> 24));
        assert!(!colored.contains(&0xFFD97757));
        pet.state = State::Laptop;
        pet.state_time = 4.0;
        assert!(renderer.frame(&pet, false).pixels.contains(&0xFF5B5653));
    }
    use super::*;
    use crate::engine::Skin;
    #[test]
    fn alpha_is_premultiplied() {
        let mut c = Canvas::new(2, 2);
        c.pixel(0, 0, 0x80FF0000);
        assert_eq!(c.pixels[0], 0x80800000);
        c.pixel(0, 0, 0x800000FF);
        assert_eq!(c.pixels[0] >> 24, 191);
        assert!(c.pixels[0] & 255 <= c.pixels[0] >> 24);
    }
    #[test]
    fn laptop_open_type_close_has_three_distinct_phases() {
        let mut renderer = Renderer::new().unwrap();
        let mut pet = Pet::new(1, Skin::Claude, crate::Vec2::default());
        pet.state = State::Laptop;
        let preview_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("work/qa-0.5.0/previews");
        std::fs::create_dir_all(&preview_dir).unwrap();
        let frames = [0.0, 0.5, 1.5, 3.0, 9.5];
        let mut snapshots = Vec::new();
        for time in frames {
            pet.state_time = time;
            snapshots.push(renderer.frame(&pet, false).pixels.clone());
            renderer
                .canvas
                .save_png(&preview_dir.join(format!("laptop-{time:.1}.png")))
                .unwrap();
        }
        assert_ne!(snapshots[0], snapshots[1], "lid should visibly open");
        assert_ne!(
            snapshots[1], snapshots[2],
            "lid should finish opening before typing"
        );
        assert_ne!(snapshots[2], snapshots[3], "typing should animate");
        assert_ne!(snapshots[3], snapshots[4], "lid should visibly close");
        let closed_laptop = snapshots[0]
            .iter()
            .filter(|pixel| **pixel == 0xFF393638)
            .count();
        let open_laptop = snapshots[2]
            .iter()
            .filter(|pixel| **pixel == 0xFF393638)
            .count();
        assert!(
            open_laptop > closed_laptop,
            "open rear lid should have more visible area"
        );
        assert!(
            snapshots[2].contains(&0xFFB7A99A),
            "rear mark should be visible"
        );
        assert!(
            !snapshots[2].contains(&0xFF9DC6C1),
            "display should face Claude"
        );
    }
    #[test]
    fn stretch_and_look_around_have_distinct_poses() {
        let mut renderer = Renderer::new().unwrap();
        let mut pet = Pet::new(1, Skin::Claude, crate::Vec2::default());
        pet.state = State::Stretch;
        pet.state_time = 0.2;
        let stretch = renderer.frame(&pet, false).pixels.clone();
        pet.state = State::LookAround;
        pet.state_time = 0.2;
        let look_a = renderer.frame(&pet, false).pixels.clone();
        pet.state_time = 0.8;
        let look_b = renderer.frame(&pet, false).pixels.clone();
        assert_ne!(
            stretch, look_a,
            "stretch should use a relaxed extended pose"
        );
        assert_ne!(
            look_a, look_b,
            "look-around should shift the eyes over time"
        );
    }
    #[test]
    fn cursor_gaze_moves_open_eyes_and_stays_clamped() {
        let mut renderer = Renderer::new().unwrap();
        let mut pet = Pet::new(1, Skin::Claude, crate::Vec2::default());
        pet.time = 1.0;
        let neutral = renderer.frame(&pet, false).pixels.clone();

        pet.gaze = crate::Vec2::new(-1.0, -1.0);
        let left_up = renderer.frame(&pet, false).pixels.clone();
        assert_ne!(neutral, left_up, "open eyes should follow cursor direction");
        let preview_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("work/qa-0.5.0/previews");
        std::fs::create_dir_all(&preview_dir).unwrap();
        renderer
            .canvas
            .save_png(&preview_dir.join("gaze-left-up.png"))
            .unwrap();

        pet.gaze = crate::Vec2::new(1.0, 1.0);
        let right_down = renderer.frame(&pet, false).pixels.clone();
        assert_ne!(left_up, right_down, "eyes should move in both axes");
        renderer
            .canvas
            .save_png(&preview_dir.join("gaze-right-down.png"))
            .unwrap();

        pet.gaze = crate::Vec2::new(50.0, -50.0);
        let extreme = renderer.frame(&pet, false).pixels.clone();
        pet.gaze = crate::Vec2::new(1.0, -1.0);
        let bounded = renderer.frame(&pet, false).pixels.clone();
        assert_eq!(extreme, bounded, "gaze should clamp to normalized bounds");

        for state in [State::Sleep, State::Happy] {
            pet.state = state;
            pet.gaze = crate::Vec2::default();
            let closed_neutral = renderer.frame(&pet, false).pixels.clone();
            pet.gaze = crate::Vec2::new(-1.0, 1.0);
            assert_eq!(
                closed_neutral,
                renderer.frame(&pet, false).pixels,
                "closed eyes in {state:?} should ignore gaze"
            );
        }
        pet.state = State::Idle;
        pet.time = 4.95;
        pet.gaze = crate::Vec2::default();
        let blink = renderer.frame(&pet, false).pixels.clone();
        pet.gaze = crate::Vec2::new(-1.0, 1.0);
        assert_eq!(
            blink,
            renderer.frame(&pet, false).pixels,
            "blink should ignore gaze"
        );
    }

    #[test]
    fn cursor_glance_visibly_tracks_world_direction_when_facing_either_way() {
        let mut renderer = Renderer::new().unwrap();
        renderer.show_mood = false;
        let mut pet = Pet::new(1, Skin::Claude, crate::Vec2::default());
        pet.state = State::Idle;
        pet.time = 1.0;
        let centroid = |pixels: &[u32]| {
            let mut x_sum = 0.0;
            let mut y_sum = 0.0;
            let mut count = 0.0;
            for y in 165..188 {
                for x in 105..176 {
                    if pixels[y * WIDTH + x] == 0xFF1B1B1B {
                        x_sum += x as f64;
                        y_sum += y as f64;
                        count += 1.0;
                    }
                }
            }
            assert!(count >= 80.0, "both canonical pupils must remain visible");
            (x_sum / count, y_sum / count)
        };
        for facing in [-1.0, 1.0] {
            pet.facing = facing;
            pet.gaze = crate::Vec2::new(-1.0, -1.0);
            let left_up = centroid(&renderer.frame(&pet, false).pixels);
            pet.gaze = crate::Vec2::new(1.0, 1.0);
            let right_down = centroid(&renderer.frame(&pet, false).pixels);
            assert!(
                right_down.0 - left_up.0 >= 4.0,
                "nearby left/right gaze must visibly differ"
            );
            assert!(
                right_down.1 - left_up.1 >= 2.0,
                "nearby up/down gaze must visibly differ"
            );
        }
    }
    #[test]
    fn social_interactions_have_distinct_rendered_poses() {
        let mut renderer = Renderer::new().unwrap();
        let mut pet = Pet::new(1, Skin::Claude, crate::Vec2::default());
        pet.time = 1.0;
        let idle = renderer.frame(&pet, false).pixels.clone();
        let preview_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("work/qa-0.5.0/previews");
        std::fs::create_dir_all(&preview_dir).unwrap();
        let cases = [
            (SocialKind::Greeting, SocialRole::Initiator, 0.2),
            (SocialKind::HighFive, SocialRole::Initiator, 0.2),
            (SocialKind::DanceTogether, SocialRole::Partner, 0.2),
            (SocialKind::ShareLaptop, SocialRole::Partner, 0.2),
            (SocialKind::Comfort, SocialRole::Partner, 0.2),
        ];
        for (kind, role, elapsed) in cases {
            pet.state = State::Social;
            pet.state_time = elapsed;
            pet.social = Some(crate::social::SocialActivity {
                kind,
                role,
                partner_id: 2,
                elapsed,
                partner_offset: crate::Vec2::new(80.0, 0.0),
            });
            let pose = renderer.frame(&pet, false).pixels.clone();
            assert_ne!(idle, pose, "{kind:?} should have a distinct social pose");
            renderer
                .canvas
                .save_png(&preview_dir.join(format!("social-{kind:?}.png").to_lowercase()))
                .unwrap();
            if matches!(
                kind,
                SocialKind::Greeting
                    | SocialKind::HighFive
                    | SocialKind::DanceTogether
                    | SocialKind::Comfort
            ) {
                pet.state_time = 0.8;
                pet.social.as_mut().unwrap().elapsed = 0.8;
                let later = renderer.frame(&pet, false).pixels.clone();
                assert_ne!(
                    pose, later,
                    "{kind:?} should animate through its interaction"
                );
            }
        }
    }
    #[test]
    fn high_five_reaches_toward_friend_when_facing_left_or_right() {
        let mut renderer = Renderer::new().unwrap();
        renderer.show_mood = false;
        let mut pet = Pet::new(1, Skin::Claude, crate::Vec2::default());
        pet.time = 1.0;
        pet.state = State::Social;
        pet.state_time = 0.65;
        pet.social = Some(crate::social::SocialActivity {
            kind: SocialKind::HighFive,
            role: SocialRole::Initiator,
            partner_id: 2,
            elapsed: 0.65,
            partner_offset: crate::Vec2::new(80.0, 0.0),
        });
        pet.facing = 1.0;
        let right_facing = renderer.frame(&pet, false).pixels.clone();
        pet.facing = -1.0;
        let left_facing = renderer.frame(&pet, false).pixels.clone();
        let hand_bounds = |pixels: &[u32]| {
            let mut xs = (WIDTH, 0);
            for y in 135..160 {
                for x in 0..WIDTH {
                    if pixels[y * WIDTH + x] == 0xFFE8957A {
                        xs.0 = xs.0.min(x);
                        xs.1 = xs.1.max(x);
                    }
                }
            }
            xs
        };
        let right = hand_bounds(&right_facing);
        let left = hand_bounds(&left_facing);
        assert!(
            right.0 > FEET_X as usize,
            "right-facing lead hand should reach right"
        );
        assert!(
            left.1 < FEET_X as usize,
            "left-facing lead hand should reach left"
        );
        let right_offset = (right.0 + right.1) as f64 * 0.5 - FEET_X;
        let left_offset = FEET_X - (left.0 + left.1) as f64 * 0.5;
        assert!((right_offset - 40.0).abs() <= 1.0);
        assert!((left_offset - 40.0).abs() <= 1.0);
    }

    #[test]
    fn high_five_lifts_and_returns_the_hand_instead_of_only_blinking_a_spark() {
        let mut renderer = Renderer::new().unwrap();
        renderer.show_mood = false;
        let mut pet = Pet::new(1, Skin::Claude, crate::Vec2::default());
        pet.state = State::Social;
        pet.time = 1.0;
        let count_raised = |pixels: &[u32]| {
            (135..160)
                .flat_map(|y| (165..190).map(move |x| y * WIDTH + x))
                .filter(|&i| pixels[i] == 0xFFE8957A)
                .count()
        };
        let mut samples = Vec::new();
        for elapsed in [0.05, 0.65, 1.95] {
            pet.state_time = elapsed;
            pet.social = Some(crate::social::SocialActivity {
                kind: SocialKind::HighFive,
                role: SocialRole::Initiator,
                partner_id: 2,
                elapsed,
                partner_offset: crate::Vec2::new(80.0, 0.0),
            });
            samples.push(count_raised(&renderer.frame(&pet, false).pixels));
        }
        assert!(
            samples[1] > samples[0] + 20,
            "hand must visibly lift toward contact"
        );
        assert!(
            samples[1] > samples[2] + 20,
            "hand must return after contact"
        );
    }

    #[test]
    fn a_shared_laptop_has_one_owner_and_sits_between_the_two_pets() {
        let mut renderer = Renderer::new().unwrap();
        renderer.show_mood = false;
        let mut pet = Pet::new(1, Skin::Claude, crate::Vec2::default());
        pet.state = State::Laptop;
        pet.time = 1.0;
        pet.state_time = 2.0;
        pet.social = Some(crate::social::SocialActivity {
            kind: SocialKind::ShareLaptop,
            role: SocialRole::Initiator,
            partner_id: 2,
            elapsed: 2.0,
            partner_offset: crate::Vec2::new(90.0, 0.0),
        });
        let frame = renderer.frame(&pet, false);
        let xs: Vec<_> = frame
            .pixels
            .iter()
            .enumerate()
            .filter(|(_, p)| **p == 0xFF5B5653)
            .map(|(i, _)| (i % WIDTH) as f64)
            .collect();
        assert!(xs.len() > 50, "shared owner must draw the rear lid");
        let center = (xs.iter().copied().fold(f64::INFINITY, f64::min)
            + xs.iter().copied().fold(f64::NEG_INFINITY, f64::max))
            * 0.5;
        assert!(
            (center - (FEET_X + 45.0)).abs() < 3.0,
            "shared prop must be halfway to the partner"
        );
        pet.state = State::Social;
        pet.facing = -1.0;
        pet.social.as_mut().unwrap().role = SocialRole::Partner;
        pet.social.as_mut().unwrap().partner_offset.x = -90.0;
        assert!(
            !renderer.frame(&pet, false).pixels.contains(&0xFF5B5653),
            "partner must not draw a duplicate laptop"
        );
    }
    #[test]
    fn activity_state_uses_pack_drawing_path() {
        let mut renderer = Renderer::new().unwrap();
        let mut pet = Pet::new(1, Skin::Claude, crate::Vec2::default());
        renderer.show_mood = false;
        pet.state = State::Activity;
        for (index, spec) in crate::activities::CATALOG.iter().enumerate() {
            pet.activity = Some(crate::activities::ActivityId::new(index).unwrap());
            pet.state_time = spec.duration * 0.5;
            let activity = &renderer.frame(&pet, false).pixels;
            assert_eq!(
                activity[184 * WIDTH + 140],
                0xFFD97757,
                "{} torso",
                spec.slug
            );
            for (eye_x, label) in [(120, "left"), (155, "right")] {
                let eye_present = (170..184)
                    .any(|y| (eye_x..eye_x + 12).any(|x| activity[y * WIDTH + x] == 0xFF1B1B1B));
                assert!(eye_present, "{} canonical {label} eye", spec.slug);
            }
            assert_eq!(activity[0], 0, "{} transparent margin", spec.slug);
        }
        let preview_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("work/qa-0.5.0/previews");
        std::fs::create_dir_all(&preview_dir).unwrap();
        renderer
            .canvas
            .save_png(&preview_dir.join("activity-sample.png"))
            .unwrap();
    }

    #[test]
    fn pair_programming_activity_keeps_only_the_current_claude_and_its_laptop() {
        let mut renderer = Renderer::new().unwrap();
        renderer.show_mood = false;
        let mut pet = Pet::new(1, Skin::Claude, crate::Vec2::default());
        pet.state = State::Activity;
        pet.activity = Some(crate::activities::ActivityId::new(26).unwrap());
        pet.state_time = 2.0;
        pet.time = 1.0;
        let frame = renderer.frame(&pet, false);
        assert_eq!(frame.pixels[184 * WIDTH + 140], 0xFFD97757);
        assert!(
            (130..220).all(|y| (185..260).all(|x| frame.pixels[y * WIDTH + x] == 0)),
            "the imported human beside Claude must not reappear"
        );
    }
    #[test]
    fn climbing_frames_alternate_and_stay_inside_mascot_bounds() {
        let mut renderer = Renderer::new().unwrap();
        let mut pet = Pet::new(1, Skin::Claude, crate::Vec2::default());
        pet.state = State::Climb;
        pet.facing = -1.0;
        let preview_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("work/claude-only-preview");
        std::fs::create_dir_all(&preview_dir).unwrap();
        let mut first = None;
        for (index, state_time) in [0.05, 0.25, 0.45, 0.65].into_iter().enumerate() {
            pet.state_time = state_time;
            let pixels = renderer.frame(&pet, false).pixels.clone();
            if let Some(previous) = &first {
                assert_ne!(
                    &pixels, previous,
                    "climbing phase {index} should change the pose"
                );
            } else {
                first = Some(pixels.clone());
            }
            let occupied = pixels
                .iter()
                .enumerate()
                .filter(|(_, px)| **px >> 24 > 0)
                .map(|(i, _)| (i % WIDTH, i / WIDTH));
            let (mut min_x, mut min_y, mut max_x, mut max_y) = (WIDTH, HEIGHT, 0, 0);
            for (x, y) in occupied {
                min_x = min_x.min(x);
                min_y = min_y.min(y);
                max_x = max_x.max(x);
                max_y = max_y.max(y);
            }
            assert!(
                min_x >= 90 && max_x < 190 && min_y >= 130 && max_y < 220,
                "climbing frame escaped expected bounds: ({min_x},{min_y})..({max_x},{max_y})"
            );
            renderer
                .canvas
                .save_png(&preview_dir.join(format!("claude-climb-{}.png", index + 1)))
                .unwrap();
        }
    }
    #[test]
    fn dance_wave_and_velocity_streaks_animate_only_in_their_states() {
        let mut renderer = Renderer::new().unwrap();
        let mut pet = Pet::new(1, Skin::Claude, crate::Vec2::default());
        let preview_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("work/claude-only-preview");
        std::fs::create_dir_all(&preview_dir).unwrap();
        renderer.frame(&pet, false);
        renderer
            .canvas
            .save_png(&preview_dir.join("claude-idle.png"))
            .unwrap();
        pet.satisfaction = 18.0;
        renderer.frame(&pet, false);
        renderer
            .canvas
            .save_png(&preview_dir.join("claude-mood-low.png"))
            .unwrap();
        pet.satisfaction = 82.0;
        renderer.frame(&pet, false);
        renderer
            .canvas
            .save_png(&preview_dir.join("claude-mood-high.png"))
            .unwrap();
        pet.satisfaction = 75.0;
        pet.state = State::Hold;
        pet.time = 0.4;
        renderer.frame(&pet, false);
        renderer
            .canvas
            .save_png(&preview_dir.join("claude-hold.png"))
            .unwrap();
        pet.state = State::Dance;
        pet.state_time = 0.1;
        let dance_a = renderer.frame(&pet, false).pixels.clone();
        renderer
            .canvas
            .save_png(&preview_dir.join("claude-dance.png"))
            .unwrap();
        pet.state_time = 0.5;
        let dance_b = renderer.frame(&pet, false).pixels.clone();
        assert_ne!(dance_a, dance_b, "dance beats should change Claude's pose");
        pet.state = State::Wave;
        pet.state_time = 0.1;
        let wave_a = renderer.frame(&pet, false).pixels.clone();
        renderer
            .canvas
            .save_png(&preview_dir.join("claude-wave.png"))
            .unwrap();
        pet.state_time = 0.3;
        let wave_b = renderer.frame(&pet, false).pixels.clone();
        assert_ne!(wave_a, wave_b, "the raised hand should wave");

        pet.state = State::Thrown;
        pet.state_time = 0.4;
        pet.dragging = false;
        pet.vel = crate::Vec2::default();
        let still = renderer.frame(&pet, false).pixels.clone();
        renderer
            .canvas
            .save_png(&preview_dir.join("claude-thrown-still.png"))
            .unwrap();
        pet.vel = crate::Vec2::new(900.0, -300.0);
        let moving = renderer.frame(&pet, false).pixels.clone();
        renderer
            .canvas
            .save_png(&preview_dir.join("claude-speed-streaks.png"))
            .unwrap();
        assert_ne!(
            still, moving,
            "fast airborne movement should add velocity-aligned trails"
        );

        pet.state = State::Hold;
        pet.dragging = true;
        let held_fast = renderer.frame(&pet, false).pixels.clone();
        renderer
            .canvas
            .save_png(&preview_dir.join("claude-hold-fast.png"))
            .unwrap();
        pet.vel = crate::Vec2::default();
        let held_still = renderer.frame(&pet, false).pixels.clone();
        assert_eq!(
            held_fast, held_still,
            "held mascots must not show speed streaks"
        );
    }
    #[test]
    fn bubble_is_drawn_at_output_scale_with_coverage() {
        let mut renderer = Renderer::new().unwrap();
        if renderer.font.is_none() {
            return;
        }
        let mut pet = Pet::new(1, Skin::Claude, crate::Vec2::default());
        pet.bubble = Some("приятно :)".into());
        pet.bubble_time = 2.0;
        let regular = renderer.frame(&pet, true).pixels.clone();
        assert_eq!(regular, renderer.frame_scaled(&pet, true, 1.0).pixels);
        let preview_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("work/text-preview");
        std::fs::create_dir_all(&preview_dir).unwrap();

        for (scale, suffix) in [(0.5, "0_5x"), (1.0, "1x"), (1.5, "1_5x"), (2.0, "2x")] {
            let frame = renderer.frame_scaled(&pet, true, scale);
            assert_eq!(frame.width, (WIDTH as f64 * scale).round() as usize);
            assert_eq!(frame.height, (HEIGHT as f64 * scale).round() as usize);
            let bubble_top = (95.0 * scale).round() as usize;
            let bubble_bottom = (bubble_top + (54.0 * scale).round() as usize).min(frame.height);
            let near_white = frame
                .pixels
                .iter()
                .enumerate()
                .filter(|(index, pixel)| {
                    let y = index / frame.width;
                    let red = (**pixel >> 16) & 255;
                    let green = (**pixel >> 8) & 255;
                    let blue = **pixel & 255;
                    (bubble_top..bubble_bottom).contains(&y)
                        && red > 210
                        && green > 200
                        && blue > 180
                })
                .count();
            assert!(
                near_white > 30,
                "bubble text should remain readable at {scale}x"
            );
            frame
                .save_png(&preview_dir.join(format!("pleasant-{suffix}.png")))
                .unwrap();
        }
    }
}
