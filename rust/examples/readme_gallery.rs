//! Export README artwork using the application's actual pixel renderer.
//! Run from the repository root:
//! cargo run --manifest-path rust/Cargo.toml --example readme_gallery -- docs/images
use claude_mascot::{
    contacts::ContactCoordinator,
    engine::{Pet, Skin, State},
    palette,
    render::{Canvas, Renderer, FEET_X, FEET_Y},
    social::{SocialCoordinator, SocialMotion},
    Rect, Vec2, World,
};
use std::path::Path;

const BACKGROUND: u32 = 0xFF252220;
const PANEL: u32 = 0xFF302B27;

fn background(width: usize, height: usize) -> Canvas {
    let mut canvas = Canvas::new(width, height);
    canvas.pixels.fill(BACKGROUND);
    canvas
}

fn pet(id: u64, state: State, color_index: usize) -> Pet {
    let mut p = Pet::new(id, Skin::Claude, Vec2::default());
    p.state = state;
    p.time = 0.5;
    p.state_time = if state == State::Laptop { 3.0 } else { 0.5 };
    p.body_color = palette::distinct_color(color_index);
    p
}

fn stamp(canvas: &mut Canvas, renderer: &mut Renderer, pet: &Pet, x: f64, y: f64, scale: f64) {
    let frame = renderer.frame(pet, false).scaled(scale);
    let ox = (x - FEET_X * scale).round() as i32;
    let oy = (y - FEET_Y * scale).round() as i32;
    for py in 0..frame.height {
        for px in 0..frame.width {
            let color = frame.pixels[py * frame.width + px];
            let alpha = color >> 24;
            if alpha != 0 {
                let channel = |shift: u32| (((color >> shift) & 255) * 255 / alpha).min(255);
                canvas.pixel(
                    ox + px as i32,
                    oy + py as i32,
                    (alpha << 24) | (channel(16) << 16) | (channel(8) << 8) | channel(0),
                );
            }
        }
    }
}

fn company(renderer: &mut Renderer, dir: &Path) -> Result<(), String> {
    let mut canvas = background(1400, 270);
    canvas.rect(24.0, 232.0, 1352.0, 2.0, 0xFF5A4940);
    for (index, state) in [
        State::Wave,
        State::Idle,
        State::Laptop,
        State::Dance,
        State::Stretch,
        State::Sit,
        State::Happy,
        State::LookAround,
    ]
    .into_iter()
    .enumerate()
    {
        let p = pet(index as u64 + 1, state, index);
        stamp(
            &mut canvas,
            renderer,
            &p,
            140.0 + index as f64 * 160.0,
            232.0,
            2.0,
        );
    }
    canvas.save_png(&dir.join("claude-company.png"))
}

fn laptop(renderer: &mut Renderer, dir: &Path) -> Result<(), String> {
    let mut canvas = background(640, 520);
    canvas.rect(24.0, 24.0, 592.0, 472.0, PANEL);
    let p = pet(1, State::Laptop, 0);
    stamp(&mut canvas, renderer, &p, 320.0, 320.0, 3.0);
    canvas.save_png(&dir.join("claude-laptop.png"))
}

fn moments(renderer: &mut Renderer, dir: &Path) -> Result<(), String> {
    let mut canvas = background(1280, 300);
    for (index, state) in [State::Wave, State::Sleep, State::Dance, State::Climb]
        .into_iter()
        .enumerate()
    {
        let left = index as f64 * 320.0;
        canvas.rect(left + 12.0, 16.0, 296.0, 268.0, PANEL);
        let mut p = pet(index as u64 + 1, state, [1, 4, 3, 5][index]);
        if state == State::Climb {
            p.facing = -1.0;
            canvas.rect(left + 208.0, 32.0, 2.0, 234.0, 0xFF786252);
        }
        stamp(&mut canvas, renderer, &p, left + 160.0, 244.0, 2.0);
    }
    canvas.save_png(&dir.join("claude-moments.png"))
}

fn tower(renderer: &mut Renderer, dir: &Path) -> Result<(), String> {
    let world = World {
        work_areas: vec![Rect::new(0.0, 0.0, 1920.0, 1040.0)],
        ..World::default()
    };
    let mut pets: Vec<_> = (0..4)
        .map(|index| {
            let mut p = pet(index + 1, State::Idle, [0, 3, 4, 5][index as usize]);
            p.pos = Vec2::new(800.0 + index as f64 * 100.0, 1040.0);
            p.auto_activities = false;
            p.auto_reactions = false;
            p
        })
        .collect();
    let mut social = SocialCoordinator::new();
    social.frequency = 0;
    social.request_tower(Some(1));
    social.update(0.0, pets.iter_mut(), &world);
    let mut contacts = ContactCoordinator::new();
    for _ in 0..7200 {
        for p in &mut pets {
            p.update(1.0 / 240.0, &world);
        }
        contacts.update(1.0 / 240.0, pets.iter_mut(), &world);
        social.update(1.0 / 240.0, pets.iter_mut(), &world);
        if pets
            .iter()
            .filter(|p| matches!(p.social_motion, Some(SocialMotion::Perch { .. })))
            .count()
            == 3
        {
            let mut canvas = background(640, 520);
            canvas.rect(24.0, 24.0, 592.0, 472.0, PANEL);
            let center = pets[0].pos.x;
            for p in &pets {
                stamp(
                    &mut canvas,
                    renderer,
                    p,
                    320.0 + (p.pos.x - center) * 2.0,
                    470.0 + (p.pos.y - 1040.0) * 2.0,
                    2.0,
                );
            }
            return canvas.save_png(&dir.join("claude-tower.png"));
        }
    }
    Err("Four-pet tower did not form; refusing to publish a staged screenshot".into())
}

fn main() -> Result<(), String> {
    let dir = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "docs/images".into());
    let dir = Path::new(&dir);
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let mut renderer = Renderer::new()?;
    renderer.show_mood = false;
    company(&mut renderer, dir)?;
    laptop(&mut renderer, dir)?;
    moments(&mut renderer, dir)?;
    tower(&mut renderer, dir)?;
    println!("README gallery exported to {}", dir.display());
    Ok(())
}
