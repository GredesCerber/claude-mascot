//! CPU renderer. Pixels are premultiplied ARGB u32 (BGRA in little-endian memory).
//! No native handles or behavior decisions live here.
use crate::engine::{Pet, Skin, State, CURSOR_HOLD_DY};
use fontdue::{Font, FontSettings};
use serde::Deserialize;
use std::{collections::BTreeMap, io::Cursor, path::Path};

pub const WIDTH: usize = 280;
pub const HEIGHT: usize = 240;
pub const FEET_X: f64 = 140.0;
pub const FEET_Y: f64 = 216.0;

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

#[derive(Deserialize)]
struct AtlasMeta {
    frames: BTreeMap<String, [usize; 5]>,
}
pub struct Atlas {
    pixels: Vec<u8>,
    width: usize,
    height: usize,
    frames: BTreeMap<String, [usize; 5]>,
}
impl Atlas {
    fn decode(png_bytes: &[u8], json: &str) -> Result<Self, String> {
        let meta: AtlasMeta =
            serde_json::from_str(json).map_err(|e| format!("Invalid atlas metadata: {e}"))?;
        let mut dec = png::Decoder::new(Cursor::new(png_bytes));
        dec.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
        let mut reader = dec.read_info().map_err(|e| e.to_string())?;
        // Skin packs are local data, but bound allocations before decoding.
        if reader.info().width > 4096 || reader.info().height > 4096 {
            return Err("Atlas exceeds 4096x4096".into());
        }
        let mut buf = vec![0; reader.output_buffer_size()];
        let info = reader.next_frame(&mut buf).map_err(|e| e.to_string())?;
        let width = info.width as usize;
        let height = info.height as usize;
        let channels = info.color_type.samples();
        let mut pixels = Vec::with_capacity(width * height * 4);
        for p in buf[..info.buffer_size()].chunks_exact(channels) {
            let rgba = match info.color_type {
                png::ColorType::Rgba => [p[0], p[1], p[2], p[3]],
                png::ColorType::Rgb => [p[0], p[1], p[2], 255],
                png::ColorType::Grayscale => [p[0], p[0], p[0], 255],
                png::ColorType::GrayscaleAlpha => [p[0], p[0], p[0], p[1]],
                _ => return Err("Unsupported atlas PNG".into()),
            };
            pixels.extend_from_slice(&rgba);
        }
        for &[x, y, w, h, anchor] in meta.frames.values() {
            if w == 0
                || h == 0
                || x.checked_add(w).is_none_or(|v| v > width)
                || y.checked_add(h).is_none_or(|v| v > height)
                || anchor > w
            {
                return Err("Atlas frame outside image".into());
            }
        }
        if !meta.frames.contains_key("idle") {
            return Err("Atlas must contain an idle frame".into());
        }
        Ok(Self {
            pixels,
            width,
            height,
            frames: meta.frames,
        })
    }
    pub fn from_files(png: &Path, json: &Path) -> Result<Self, String> {
        if std::fs::metadata(png).map_err(|e| e.to_string())?.len() > 32 * 1024 * 1024 {
            return Err("Atlas PNG exceeds 32 MiB".into());
        }
        use std::io::Read;
        let mut metadata = String::new();
        std::fs::File::open(json)
            .map_err(|e| e.to_string())?
            .take(1024 * 1024 + 1)
            .read_to_string(&mut metadata)
            .map_err(|e| e.to_string())?;
        if metadata.len() > 1024 * 1024 {
            return Err("Atlas JSON exceeds 1 MiB".into());
        }
        Self::decode(&std::fs::read(png).map_err(|e| e.to_string())?, &metadata)
    }
    fn draw(&self, canvas: &mut Canvas, pet: &Pet) {
        let name = if pet.dragging {
            "dangle".into()
        } else {
            match pet.state {
                State::Walk | State::Run | State::Chase | State::Hunt | State::Flee => {
                    format!("walk{}", (pet.time * 9.0) as usize % 8 + 1)
                }
                State::Sleep => "sleep".into(),
                State::Sit | State::Ledge | State::Dizzy => "sit".into(),
                State::Hold | State::Climb => "hang".into(),
                State::Fall | State::Jump | State::Thrown => "jump".into(),
                State::Smash => if pet.state_time < 0.35 {
                    "windup"
                } else {
                    "strike"
                }
                .into(),
                State::Happy | State::Block => "windup".into(),
                _ => "idle".into(),
            }
        };
        let &[sx, sy, w, h, anchor] = self.frames.get(&name).unwrap_or(&self.frames["idle"]);
        debug_assert!(sy + h <= self.height);
        let climbing = pet.state == State::Climb && !pet.dragging;
        // The source atlas has one hang pose. A tiny alternating lower-body
        // shift and whole-body lift keeps that pose alive without resampling it.
        let climb_phase = (pet.state_time * 4.0).floor() as i32 & 1;
        let bob = if climbing {
            if climb_phase == 0 {
                -1.0
            } else {
                0.0
            }
        } else if matches!(pet.state, State::Hold) {
            (pet.time * 2.5).sin() * 1.25
        } else {
            0.0
        };
        for y in 0..h {
            for x in 0..w {
                let i = ((sy + y) * self.width + sx + x) * 4;
                let p = &self.pixels[i..i + 4];
                let c = ((p[3] as u32) << 24)
                    | ((p[0] as u32) << 16)
                    | ((p[1] as u32) << 8)
                    | p[2] as u32;
                let xx = if pet.facing >= 0.0 {
                    x as f64 - anchor as f64
                } else {
                    anchor as f64 - x as f64
                };
                let lower_shift = if climbing && y > h * 2 / 3 {
                    if climb_phase == 0 {
                        -1.0
                    } else {
                        1.0
                    }
                } else {
                    0.0
                };
                canvas.pixel(
                    (FEET_X + xx + lower_shift * pet.facing.signum()) as i32,
                    (FEET_Y - h as f64 + y as f64 + bob) as i32,
                    c,
                );
            }
        }
    }
}

pub struct Renderer {
    pub canvas: Canvas,
    scaled_canvas: Canvas,
    font: Option<Font>,
    glyphs: BTreeMap<(char, u16), (fontdue::Metrics, Vec<u8>)>,
    tagilla: Atlas,
    pub custom: Option<Atlas>,
    /// Controls the small satisfaction meter below speech bubbles.
    pub show_mood: bool,
}
impl Renderer {
    pub fn new() -> Result<Self, String> {
        let tagilla = Atlas::decode(
            include_bytes!("../../assets/tagilla_frames.png"),
            include_str!("../../assets/tagilla_frames.json"),
        )?;
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
            tagilla,
            custom: None,
            show_mood: true,
        })
    }
    pub fn frame(&mut self, pet: &Pet, show_bubbles: bool) -> &Canvas {
        self.canvas.clear();
        if pet.skin == Skin::Tagilla {
            self.custom
                .as_ref()
                .unwrap_or(&self.tagilla)
                .draw(&mut self.canvas, pet);
        } else {
            draw_claude(&mut self.canvas, pet);
        }
        if self.show_mood {
            let width = 42.0;
            let x = FEET_X - width / 2.0;
            let y = 139.0;
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
                let t = (pet.time * 0.8 + n as f64 * 0.3).fract();
                let x = FEET_X - 32.0 + n as f64 * 30.0 + (pet.time * 3.0 + n as f64).sin() * 7.0;
                let y = FEET_Y - 80.0 - t * 65.0;
                heart(
                    &mut self.canvas,
                    x,
                    y,
                    ((255.0 * (1.0 - t)) as u32) << 24 | 0xF17692,
                );
            }
        }
        if pet.rage && pet.skin == Skin::Tagilla {
            self.text("!", FEET_X - 3.0, FEET_Y - 144.0, 18.0, 0xFFFF493D);
        }
        if pet.state == State::Sleep {
            self.text(
                "z z Z",
                FEET_X + 20.0,
                FEET_Y - 90.0 - (pet.time * 2.0).sin() * 3.0,
                15.0,
                0xFFDDD7CA,
            );
        }
        if pet.state == State::Smash && pet.state_time > 0.35 {
            let x = FEET_X + pet.facing * 54.0;
            let y = FEET_Y - 30.0;
            for n in 0..7 {
                let a = n as f64 * 0.9;
                let x2 = x + a.cos() * 24.0;
                let y2 = y + a.sin() * 24.0;
                self.canvas
                    .line(x as i32, y as i32, x2 as i32, y2 as i32, 0xCCFFF2BD);
            }
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
        if pet.state == State::Dizzy {
            for n in 0..3 {
                let a = pet.time * 4.0 + n as f64 * std::f64::consts::TAU / 3.0;
                let x = FEET_X + a.cos() * 25.0;
                let y = FEET_Y - 132.0 + a.sin() * 8.0;
                self.text("*", x, y, 12.0, 0xFFFFD76A);
            }
        }
        if show_bubbles {
            if let Some(text) = &pet.bubble {
                if pet.bubble_time > 0.0 {
                    self.bubble(text, pet.skin == Skin::Tagilla);
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
                let tall = pet.skin == Skin::Tagilla;
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
    let sitting = matches!(
        p.state,
        State::Sit | State::Sleep | State::Dizzy | State::Ledge
    );
    let moving = matches!(
        p.state,
        State::Walk | State::Run | State::Chase | State::Hunt | State::Flee
    );
    let air = matches!(p.state, State::Fall | State::Jump | State::Thrown);
    let phase = (p.time * 7.0) as usize % 2;
    let climbing = p.state == State::Climb && !p.dragging;
    let climb_phase = (p.state_time * 5.0).floor() as usize % 4;
    let dancing = p.state == State::Dance;
    let waving = p.state == State::Wave;
    let angry = p.satisfaction < 30.0;
    let bob = if climbing {
        if climb_phase < 2 {
            -0.35
        } else {
            0.15
        }
    } else if dancing {
        if ((p.state_time * 5.0).floor() as usize).is_multiple_of(2) {
            -0.7
        } else {
            0.25
        }
    } else if p.state == State::Hold || p.dragging {
        (p.time * 2.7).sin() * 0.25
    } else if moving && phase == 1 {
        -0.5
    } else {
        0.0
    };
    let top = if sitting { -8.0 } else { -11.0 } + bob;
    let mut px = |x: f64, y: f64, w: f64, h: f64, col: u32| {
        let sx = 1.0 + p.squash.clamp(0.0, 1.0) * 0.16;
        let sy = 1.0 - p.squash.clamp(0.0, 1.0) * 0.32;
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
                0xFFB85C3E,
            );
        } else if dancing {
            let pair_phase = (p.state_time * 5.0).floor() as usize % 2;
            let lifted = i / 2 == pair_phase;
            px(
                *lx + if lifted { 0.5 } else { 0.0 },
                -3.0 + if lifted { -0.8 } else { 0.0 },
                1.0,
                if lifted { 2.2 } else { 3.0 },
                0xFFB85C3E,
            );
        } else if p.state == State::Hold || p.dragging {
            px(
                lx + (p.time * 5.0 + i as f64).sin() * 0.5,
                -3.0,
                1.0,
                4.0,
                0xFFB85C3E,
            );
        } else if p.state == State::Ledge {
            px(lx + 1.0, -1.0, 1.0, 3.5, 0xFFB85C3E);
        } else if !sitting {
            let h = if air || moving && i % 2 == phase {
                2.0
            } else {
                3.0
            };
            px(*lx, -3.0 + bob, 1.0, h, 0xFFB85C3E);
        }
    }
    let torso_shift = if climbing {
        (climb_phase as f64 - 1.5) * 0.12
    } else {
        0.0
    };
    px(-6.0 + torso_shift, top, 12.0, 8.0, 0xFFD97757);
    px(-6.0, top + 7.0, 12.0, 1.0, 0xFFB85C3E);
    px(-5.0, top, 3.0, 1.0, 0xFFE8957A);
    let up = climbing
        || waving
        || p.dragging
        || matches!(
            p.state,
            State::Happy | State::Jump | State::Thrown | State::Hold | State::Climb | State::Block
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
        px(lead_arm_x, top - 2.6, 2.0, 3.4, 0xFFD97757);
        px(trail_arm_x, top + 1.0, 2.0, 2.8, 0xFFD97757);
        px(lead_x, top - 2.0, 2.0, 2.0, 0xFFD97757);
        px(trail_x, top + 2.0, 2.0, 2.0, 0xFFD97757);
    } else if dancing {
        let beat = (p.state_time * 5.0).floor() as usize % 4;
        let raised = if beat < 2 { -8.0 } else { 6.0 };
        let lowered = if raised < 0.0 { 6.0 } else { -8.0 };
        px(raised, top - 2.0, 2.0, 2.0, 0xFFD97757);
        px(lowered, top + 2.0, 2.0, 2.0, 0xFFD97757);
    } else if waving {
        let wave = (p.state_time * 9.0).sin() * 1.2;
        px(6.0, top - 2.0 + wave, 2.0, 2.0, 0xFFD97757);
        px(-8.0, top + 3.0, 2.0, 2.0, 0xFFD97757);
    } else {
        let dangling = p.state == State::Hold || p.dragging;
        let swing = if dangling {
            (p.time * 4.0).sin() * 0.45
        } else {
            0.0
        };
        if dangling {
            let hand_y = -CURSOR_HOLD_DY / 5.0;
            // Forearms rise from the shoulders and turn inward toward the
            // grip. Keep the center gap open at the cursor's exact hotspot.
            px(-7.0 + swing, -13.0, 1.5, 5.0, 0xFFD97757);
            px(-7.0 + swing, -13.0, 4.0, 1.5, 0xFFD97757);
            px(-3.5 + swing, hand_y, 1.5, 1.5, 0xFFD97757);
            px(5.5 - swing, -13.0, 1.5, 5.0, 0xFFD97757);
            px(1.5 - swing, -13.0, 4.0, 1.5, 0xFFD97757);
            px(2.0 - swing, hand_y, 1.5, 1.5, 0xFFD97757);
        } else {
            px(-8.0, arm, 2.0, 2.0, 0xFFD97757);
            px(6.0, arm, 2.0, 2.0, 0xFFD97757);
        }
    }
    for eye in [-4.0, 3.0] {
        if p.state == State::Sleep || (p.time * 0.2).fract() > 0.98 {
            px(eye - 0.3, top + 3.2, 1.6, 0.6, 0xFF1B1B1B);
        } else if p.state == State::Happy {
            px(eye - 0.3, top + 2.8, 0.6, 0.6, 0xFF1B1B1B);
            px(eye + 0.3, top + 2.2, 0.6, 0.6, 0xFF1B1B1B);
            px(eye + 0.9, top + 2.8, 0.6, 0.6, 0xFF1B1B1B);
        } else {
            px(eye, top + 2.0, 1.0, 2.0, 0xFF1B1B1B);
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
}

#[cfg(test)]
mod tests {
    use super::*;
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
    fn atlas_embedded_valid() {
        let mut r = Renderer::new().unwrap();
        let p = Pet::new(1, Skin::Tagilla, crate::Vec2::default());
        r.frame(&p, false);
        assert!(r.canvas.pixels.iter().any(|p| *p >> 24 > 0));
        assert_eq!(r.canvas.pixels[0], 0);
    }
    #[test]
    fn invalid_atlas_metadata_is_rejected() {
        assert!(Atlas::decode(
            include_bytes!("../../assets/tagilla_frames.png"),
            r#"{"frames":{"idle":[999999,0,10,10,0]}}"#
        )
        .is_err());
    }
    #[test]
    fn custom_tagilla_does_not_replace_claude() {
        let mut r = Renderer::new().unwrap();
        let p = Pet::new(1, Skin::Claude, crate::Vec2::default());
        let baseline = r.frame(&p, false).pixels.clone();
        r.custom = Some(
            Atlas::decode(
                include_bytes!("../../assets/tagilla_frames.png"),
                include_str!("../../assets/tagilla_frames.json"),
            )
            .unwrap(),
        );
        assert_eq!(baseline, r.frame(&p, false).pixels);
    }
    #[test]
    fn climbing_frames_alternate_and_stay_inside_mascot_bounds() {
        let mut renderer = Renderer::new().unwrap();
        let mut pet = Pet::new(1, Skin::Claude, crate::Vec2::default());
        pet.state = State::Climb;
        pet.facing = -1.0;
        let preview_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("work/climb-preview");
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
        let preview_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("work/climb-preview");
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
