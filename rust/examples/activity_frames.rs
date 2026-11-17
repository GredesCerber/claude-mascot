//! Export actual renderer frames for visual review (12 pose frames/second).
use claude_mascot::{
    activities::CATALOG,
    engine::{Pet, Skin, State},
    render::Renderer,
    Vec2,
};
use std::{fs, path::PathBuf};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let out = PathBuf::from(args.next().ok_or(
        "usage: activity_frames OUTPUT [--facing -1|1] [--scale .75..3] [--samples N] [SLUG ...]",
    )?);
    let mut slugs = Vec::new();
    let mut facing = 1.0;
    let mut scale = 1.0;
    let mut samples = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--facing" => facing = args.next().ok_or("missing facing")?.parse::<f64>()?,
            "--scale" => scale = args.next().ok_or("missing scale")?.parse::<f64>()?,
            "--samples" => samples = Some(args.next().ok_or("missing samples")?.parse::<usize>()?),
            _ => slugs.push(arg),
        }
    }
    if ![-1.0, 1.0].contains(&facing)
        || !claude_mascot::settings::valid_scale(scale)
        || samples.is_some_and(|n| !(2..=120).contains(&n))
    {
        return Err("invalid preview options".into());
    }
    fs::create_dir_all(&out)?;
    let mut renderer = Renderer::new()?;
    renderer.show_mood = false;
    for spec in CATALOG
        .iter()
        .filter(|spec| slugs.is_empty() || slugs.iter().any(|s| s == spec.slug))
    {
        let dir = out.join(spec.slug);
        fs::create_dir_all(&dir)?;
        let mut pet = Pet::new(1, Skin::Claude, Vec2::default());
        pet.state = State::Activity;
        pet.activity = Some(spec.id);
        pet.facing = facing;
        pet.display_scale = scale;
        let count = samples.unwrap_or((spec.duration * 12.0).ceil() as usize + 1);
        for frame in 0..count {
            let t = if samples.is_some() {
                spec.duration * (0.12 + 0.76 * frame as f64 / (count - 1) as f64)
            } else {
                frame as f64 / 12.0
            };
            pet.time = t;
            pet.state_time = t;
            renderer
                .frame_scaled(&pet, false, scale)
                .save_png(&dir.join(format!("{frame:03}.png")))?;
        }
        println!("{}", spec.slug);
    }
    Ok(())
}
