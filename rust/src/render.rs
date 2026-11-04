//! CPU renderer. Pixels are premultiplied ARGB u32 (BGRA in little-endian memory).
//! No native handles or behavior decisions live here.
use crate::engine::{Pet, Skin, State};
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
        Self::decode(
            &std::fs::read(png).map_err(|e| e.to_string())?,
            &std::fs::read_to_string(json).map_err(|e| e.to_string())?,
        )
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
                canvas.pixel(
                    (FEET_X + xx) as i32,
                    (FEET_Y - h as f64 + y as f64) as i32,
                    c,
                );
            }
        }
    }
}

pub struct Renderer {
    pub canvas: Canvas,
    font: Option<Font>,
    glyphs: BTreeMap<(char, u16), (fontdue::Metrics, Vec<u8>)>,
    tagilla: Atlas,
    pub custom: Option<Atlas>,
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
            font,
            glyphs: BTreeMap::new(),
            tagilla,
            custom: None,
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
        if show_bubbles {
            if let Some(text) = &pet.bubble {
                if pet.bubble_time > 0.0 {
                    self.bubble(text, pet.skin == Skin::Tagilla);
                }
            }
        }
        &self.canvas
    }
    fn text_width(&self, text: &str, size: f32) -> f64 {
        self.font
            .as_ref()
            .map(|f| {
                text.chars()
                    .map(|c| f.metrics(c, size).advance_width as f64)
                    .sum()
            })
            .unwrap_or(0.0)
    }
    fn text(&mut self, text: &str, mut x: f64, y: f64, size: f32, color: u32) {
        let Some(font) = &self.font else {
            return;
        };
        for ch in text.chars() {
            let (m, b) = self
                .glyphs
                .entry((ch, (size * 10.0).round() as u16))
                .or_insert_with(|| font.rasterize(ch, size));
            for yy in 0..m.height {
                for xx in 0..m.width {
                    let a = b[yy * m.width + xx] as u32 * (color >> 24) / 255;
                    self.canvas.pixel(
                        x as i32 + m.xmin + xx as i32,
                        y as i32 - m.ymin - m.height as i32 + yy as i32,
                        (a << 24) | (color & 0xFFFFFF),
                    );
                }
            }
            x += m.advance_width as f64;
        }
    }
    fn bubble(&mut self, text: &str, tall: bool) {
        if self.font.is_none() {
            return;
        }
        // Cap speech to the fixed canvas rather than drawing over unrelated windows.
        let mut text = text.chars().take(40).collect::<String>();
        while self.text_width(&text, 13.0) > 248.0 {
            text.pop();
        }
        let w = self.text_width(&text, 13.0) + 20.0;
        let x = (WIDTH as f64 - w) / 2.0;
        let y = if tall { 28.0 } else { 95.0 };
        self.canvas.rect(x, y, w, 27.0, 0xF52D2929);
        self.canvas.rect(x + 1.0, y + 1.0, w - 2.0, 1.0, 0xFF776A62);
        self.text(&text, x + 10.0, y + 18.0, 13.0, 0xFFF7EDE2);
        for n in 0..5 {
            self.canvas.rect(
                FEET_X - 5.0 + n as f64,
                y + 27.0 + n as f64,
                10.0 - 2.0 * n as f64,
                1.0,
                0xF52D2929,
            );
        }
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
    let bob = if moving && phase == 1 { -0.5 } else { 0.0 };
    let top = if sitting { -8.0 } else { -11.0 } + bob;
    let mut px = |x: f64, y: f64, w: f64, h: f64, col: u32| {
        let xx = if p.facing >= 0.0 { x } else { -x - w };
        c.rect(FEET_X + xx * 5.0, FEET_Y + y * 5.0, w * 5.0, h * 5.0, col);
    };
    for (i, lx) in [-5.0, -3.0, 2.0, 4.0].iter().enumerate() {
        if p.state == State::Hold || p.dragging {
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
    px(-6.0, top, 12.0, 8.0, 0xFFD97757);
    px(-6.0, top + 7.0, 12.0, 1.0, 0xFFB85C3E);
    px(-5.0, top, 3.0, 1.0, 0xFFE8957A);
    let up = p.dragging
        || matches!(
            p.state,
            State::Happy | State::Jump | State::Thrown | State::Hold | State::Climb | State::Block
        );
    let arm = top + if up { 0.0 } else { 3.0 };
    px(-8.0, arm, 2.0, 2.0, 0xFFD97757);
    px(6.0, arm, 2.0, 2.0, 0xFFD97757);
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
}
