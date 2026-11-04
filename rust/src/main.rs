#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]
use claude_mascot::{
    engine::{Pet, Skin},
    render::{Atlas, Renderer, FEET_X, FEET_Y, HEIGHT, WIDTH},
    settings::{config_path, Settings},
    Vec2, World,
};
use std::path::PathBuf;

struct Options {
    inspect: bool,
    preview: Option<PathBuf>,
    headless: bool,
    seconds: Option<f64>,
    config: PathBuf,
    trace: Option<PathBuf>,
    skin: Option<Skin>,
}
fn options() -> Result<Options, String> {
    let mut o = Options {
        inspect: false,
        preview: None,
        headless: false,
        seconds: None,
        config: config_path(),
        trace: None,
        skin: None,
    };
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--help" | "-h" => {
                println!("Claude Mascot Rust\n  --skin claude|tagilla\n  --config FILE\n  --preview DIRECTORY   Export both characters' states as PNGs\n  --headless            Run deterministic simulation checks without a window\n  --smoke-test SECONDS  Open native pet, then exit automatically\n  --trace FILE          Write smoke-test metrics as JSON");
                std::process::exit(0)
            }
            "--preview" => {
                o.preview = Some(args.next().ok_or("--preview needs a directory")?.into())
            }
            "--headless" => o.headless = true,
            "--inspect-window" => o.inspect = true,
            "--smoke-test" => {
                let n: f64 = args
                    .next()
                    .ok_or("--smoke-test needs seconds")?
                    .parse()
                    .map_err(|_| "Invalid duration")?;
                if !n.is_finite() || !(0.1..=3600.0).contains(&n) {
                    return Err("Duration must be 0.1..3600 seconds".into());
                }
                o.seconds = Some(n)
            }
            "--config" => o.config = args.next().ok_or("--config needs a file")?.into(),
            "--trace" => o.trace = Some(args.next().ok_or("--trace needs a file")?.into()),
            "--skin" => {
                o.skin = Some(match args.next().as_deref() {
                    Some("claude") => Skin::Claude,
                    Some("tagilla") => Skin::Tagilla,
                    _ => return Err("Skin must be claude or tagilla".into()),
                })
            }
            _ => return Err(format!("Unknown option {a}")),
        }
    }
    Ok(o)
}
fn main() {
    if let Err(e) = run() {
        eprintln!("{e}");
        let p = config_path().with_file_name("last-error.log");
        if let Some(parent) = p.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(&p, &e);
        #[cfg(windows)]
        unsafe {
            use windows_sys::Win32::UI::WindowsAndMessaging::*;
            let msg = format!("{e}\n\nПодробности: {}", p.display())
                .encode_utf16()
                .chain(Some(0))
                .collect::<Vec<_>>();
            let title = "Claude Mascot Rust"
                .encode_utf16()
                .chain(Some(0))
                .collect::<Vec<_>>();
            MessageBoxW(
                std::ptr::null_mut(),
                msg.as_ptr(),
                title.as_ptr(),
                MB_OK | MB_ICONERROR,
            );
        }
        std::process::exit(1);
    }
}
fn run() -> Result<(), String> {
    let o = options()?;
    if let Some(dir) = &o.preview {
        return preview(dir);
    }
    if o.headless {
        return headless(&o);
    }
    #[cfg(windows)]
    {
        native::run(o)
    }
    #[cfg(not(windows))]
    {
        Err("The native desktop host currently supports Windows. Use --headless or --preview on other platforms.".into())
    }
}
fn preview(dir: &std::path::Path) -> Result<(), String> {
    use claude_mascot::engine::State;
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let mut r = Renderer::new()?;
    let states = [
        State::Idle,
        State::Walk,
        State::Sit,
        State::Sleep,
        State::Happy,
        State::Jump,
        State::Climb,
        State::Hold,
        State::Smash,
    ];
    for skin in [Skin::Claude, Skin::Tagilla] {
        for state in states {
            let mut p = Pet::new(1, skin, Vec2::default());
            p.state = state;
            p.time = 0.5;
            p.state_time = 0.5;
            p.bubble = Some("Привет! Что кодим сегодня?".into());
            p.bubble_time = 3.0;
            if state == State::Happy {
                p.hearts = 1.0;
            }
            r.frame(&p, true)
                .save_png(&dir.join(format!("{:?}-{:?}.png", skin, state)))?;
        }
    }
    let crack = claude_mascot::effects::CrackEffect::new(
        1,
        Vec2::new(160.0, 120.0),
        claude_mascot::Rect::new(0.0, 0.0, 320.0, 240.0),
    );
    crack.frame().save_png(&dir.join("Tagilla-Cracks.png"))?;
    Ok(())
}
fn headless(o: &Options) -> Result<(), String> {
    let world = World::default();
    let mut pets = vec![
        Pet::new(1, Skin::Claude, Vec2::new(500.0, 50.0)),
        Pet::new(2, Skin::Tagilla, Vec2::new(700.0, 50.0)),
    ];
    for _ in 0..36000 {
        for p in &mut pets {
            p.update(1.0 / 60.0, &world);
            if !p.pos.x.is_finite() || !p.pos.y.is_finite() {
                return Err("Nonfinite simulation state".into());
            }
        }
        claude_mascot::engine::update_interactions(&mut pets, 1.0 / 60.0, &world);
    }
    let result = serde_json::json!({"headless_steps":36000,"simulated_seconds":600,"pets":pets.len(),"finite":true});
    println!("{result}");
    if let Some(path) = &o.trace {
        std::fs::write(path, serde_json::to_vec_pretty(&result).unwrap())
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg(windows)]
mod native {
    use super::*;
    use claude_mascot::{
        engine::{update_interactions, Command},
        platform::{self, InputEvent, InstanceGuard, MenuState, NativeHost},
        settings,
    };
    use std::time::{Duration, Instant};
    struct Drag {
        offset: Vec2,
        start: Vec2,
        last: Vec2,
        last_at: Instant,
        velocity: Vec2,
        moved: bool,
    }
    struct Entry {
        pet: Pet,
        host: NativeHost,
        drag: Option<Drag>,
        last_attack: u64,
    }
    fn host(settings: &Settings, tray: bool) -> Result<NativeHost, String> {
        NativeHost::new(
            "Claude Mascot Rust",
            (WIDTH as f64 * settings.scale).round() as i32,
            (HEIGHT as f64 * settings.scale).round() as i32,
            tray,
        )
    }
    fn spawn(pet: Pet, settings: &Settings, tray: bool) -> Result<Entry, String> {
        Ok(Entry {
            pet,
            host: host(settings, tray)?,
            drag: None,
            last_attack: 0,
        })
    }
    pub fn run(o: Options) -> Result<(), String> {
        platform::initialize()?;
        let Some(_instance) = InstanceGuard::acquire("Local\\ClaudeMascotRust-v1")? else {
            return Ok(());
        };
        let mut settings = Settings::load(&o.config)?;
        if let Some(s) = o.skin {
            settings.skin = s;
        }
        let mut renderer = Renderer::new()?;
        if let Some(dir) = &settings.custom_atlas {
            renderer.custom = Some(Atlas::from_files(
                &dir.join("tagilla_frames.png"),
                &dir.join("tagilla_frames.json"),
            )?);
        }
        let mut world = platform::snapshot();
        let area = world.area_at(world.cursor);
        let media = claude_mascot::media::MediaMonitor::new();
        let mut entries = vec![spawn(
            Pet::new(
                1,
                settings.skin,
                Vec2::new(area.x + area.w / 2.0, area.y + 60.0),
            ),
            &settings,
            true,
        )?];
        if o.inspect {
            entries[0].host.expose_for_inspection()?;
        }
        let mut next_id = 2;
        let mut effects: Vec<(claude_mascot::effects::CrackEffect, NativeHost)> = Vec::new();
        let mut effect_elapsed = 0.0;
        let start = Instant::now();
        let mut previous = start;
        let mut world_at = start - Duration::from_secs(1);
        let mut render_at = start - Duration::from_secs(1);
        let mut autostart = settings::autostart_enabled();
        let mut accumulator = 0.0;
        let mut frames = 0u64;
        let mut snapshots = 0u64;
        let mut running = true;
        while running && NativeHost::pump() {
            let now = Instant::now();
            if o.seconds
                .is_some_and(|s| now.duration_since(start).as_secs_f64() >= s)
            {
                break;
            }
            let elapsed = now.duration_since(previous).as_secs_f64().min(0.1);
            previous = now;
            accumulator = (accumulator + elapsed).min(0.1);
            let hidden = settings.hide_fullscreen && world.fullscreen_active();
            let interval = if hidden || settings.paused {
                Duration::from_millis(250)
            } else {
                Duration::from_millis(33)
            };
            if now.duration_since(world_at) >= interval {
                world = platform::snapshot();
                media.apply(&mut world);
                world_at = now;
                snapshots += 1;
            }
            let mut commands = Vec::new();
            for (i, e) in entries.iter_mut().enumerate() {
                for input in e.host.take_events() {
                    match input {
                        InputEvent::Quit => commands.push((i, 30)),
                        InputEvent::Command(id) => commands.push((i, id)),
                        InputEvent::DoubleClick => {
                            e.drag = None;
                            e.pet.dragging = false;
                            e.pet.jump();
                        }
                        InputEvent::PointerDown(p) => {
                            e.drag = Some(Drag {
                                offset: Vec2::new(e.pet.pos.x - p.x, e.pet.pos.y - p.y),
                                start: p,
                                last: p,
                                last_at: now,
                                velocity: Vec2::default(),
                                moved: false,
                            })
                        }
                        InputEvent::PointerMove(p) => {
                            if let Some(d) = &mut e.drag {
                                d.moved |= (p.x - d.start.x).hypot(p.y - d.start.y) > 4.0;
                                let dt =
                                    now.duration_since(d.last_at).as_secs_f64().max(1.0 / 240.0);
                                d.velocity = Vec2::new(
                                    ((p.x - d.last.x) / dt).clamp(-1800.0, 1800.0),
                                    ((p.y - d.last.y) / dt).clamp(-1800.0, 1800.0),
                                );
                                d.last = p;
                                d.last_at = now;
                                if d.moved {
                                    e.pet.drag_to(Vec2::new(p.x + d.offset.x, p.y + d.offset.y));
                                }
                            }
                        }
                        InputEvent::PointerUp(_) => {
                            if let Some(d) = e.drag.take() {
                                if d.moved {
                                    let v = if now.duration_since(d.last_at)
                                        < Duration::from_millis(120)
                                    {
                                        d.velocity
                                    } else {
                                        Vec2::default()
                                    };
                                    e.pet.release(v);
                                } else {
                                    e.pet.pet();
                                }
                            }
                        }
                    }
                }
            }
            let mut remove = Vec::new();
            let mut spawn_skins = Vec::new();
            let mut rebuild = false;
            let mut save = false;
            for (i, c) in commands {
                let Some(e) = entries.get_mut(i) else {
                    continue;
                };
                match c {
                    1..=8 => {
                        let command = match c {
                            1 => Command::Wander,
                            2 => Command::Sit,
                            3 => Command::Sleep,
                            4 => Command::Climb,
                            5 => Command::Chase,
                            6 => Command::Smash,
                            7 => Command::Rage,
                            _ => Command::ToggleRage,
                        };
                        e.pet.command(command, &world);
                    }
                    10 | 11 => {
                        e.drag = None;
                        e.pet
                            .set_skin(if c == 10 { Skin::Claude } else { Skin::Tagilla });
                        settings.skin = e.pet.skin;
                        save = true;
                    }
                    20 => spawn_skins.push(Skin::Claude),
                    21 => spawn_skins.push(Skin::Tagilla),
                    30 => remove.push(i),
                    31 => running = false,
                    40 => {
                        settings.paused = !settings.paused;
                        save = true;
                    }
                    41 => {
                        settings.hide_fullscreen = !settings.hide_fullscreen;
                        save = true;
                    }
                    42 => {
                        settings::set_autostart(!autostart)?;
                        autostart = !autostart;
                    }
                    50..=52 => {
                        settings.scale = match c {
                            50 => 0.75,
                            52 => 1.5,
                            _ => 1.0,
                        };
                        rebuild = true;
                        save = true;
                    }
                    _ => {}
                }
            }
            remove.sort_unstable();
            remove.dedup();
            let removed_tray = remove.contains(&0);
            for i in remove.into_iter().rev() {
                entries.remove(i);
            }
            if entries.is_empty() {
                break;
            }
            if removed_tray {
                for (i, e) in entries.iter_mut().enumerate() {
                    e.host = host(&settings, i == 0)?;
                    if o.inspect {
                        e.host.expose_for_inspection()?;
                    }
                    e.drag = None;
                    if e.pet.dragging {
                        e.pet.release(Vec2::default());
                    }
                }
            }
            if rebuild {
                for e in &mut entries {
                    e.host.set_size(
                        (WIDTH as f64 * settings.scale).round() as i32,
                        (HEIGHT as f64 * settings.scale).round() as i32,
                    )?;
                }
            }
            for skin in spawn_skins {
                if entries.len() < 8 {
                    let area = world.area_at(world.cursor);
                    entries.push(spawn(
                        Pet::new(
                            next_id,
                            skin,
                            Vec2::new(
                                (world.cursor.x + 50.0).clamp(area.x + 60.0, area.right() - 60.0),
                                area.y + 60.0,
                            ),
                        ),
                        &settings,
                        false,
                    )?);
                    next_id += 1;
                    if o.inspect {
                        entries.last().unwrap().host.expose_for_inspection()?;
                    }
                }
            }
            if save {
                settings.save(&o.config)?;
            }
            let hidden = settings.hide_fullscreen && world.fullscreen_active();
            if settings.paused || hidden {
                accumulator = 0.0;
            } else {
                while accumulator >= 1.0 / 60.0 {
                    for e in &mut entries {
                        e.pet.update(1.0 / 60.0, &world);
                    }
                    let mut pets = entries.iter().map(|e| e.pet.clone()).collect::<Vec<_>>();
                    update_interactions(&mut pets, 1.0 / 60.0, &world);
                    for (e, p) in entries.iter_mut().zip(pets) {
                        e.pet = p;
                    }
                    accumulator -= 1.0 / 60.0;
                }
            }
            let render_due = now.duration_since(render_at) >= Duration::from_millis(16);
            for e in &mut entries {
                if e.pet.attack_serial != e.last_attack {
                    e.last_attack = e.pet.attack_serial;
                    if let Some(claude_mascot::engine::Attack::Window { window_id, point }) =
                        &e.pet.attack
                    {
                        if let Some(window) = world.windows.iter().find(|w| w.id == *window_id) {
                            let effect = claude_mascot::effects::CrackEffect::new(
                                *window_id,
                                *point,
                                window.rect,
                            );
                            let host = NativeHost::new(
                                "Mascot cosmetic effect",
                                effect.frame().width as i32,
                                effect.frame().height as i32,
                                false,
                            )?;
                            host.set_input_transparent()?;
                            if effects.len() >= 8 {
                                effects.remove(0);
                            }
                            effects.push((effect, host));
                        }
                    }
                }
                e.host.set_menu_state(MenuState {
                    selected_action: match e.pet.state {
                        claude_mascot::engine::State::Sit => 2,
                        claude_mascot::engine::State::Sleep => 3,
                        claude_mascot::engine::State::Climb => 4,
                        _ => 1,
                    },
                    paused: settings.paused,
                    hide_fullscreen: settings.hide_fullscreen,
                    autostart,
                    chase: e.pet.chase,
                    rage: e.pet.rage_forever,
                    tagilla: e.pet.skin == Skin::Tagilla,
                    scale: settings.scale,
                });
                e.host.set_visible(!hidden);
                if !hidden && render_due {
                    let frame = renderer.frame(&e.pet, settings.show_bubbles);
                    let x = (e.pet.pos.x - FEET_X * settings.scale).round() as i32;
                    let y = (e.pet.pos.y - FEET_Y * settings.scale).round() as i32;
                    if settings.scale == 1.0 {
                        e.host.present(&frame.pixels, x, y)?;
                    } else {
                        let scaled = frame.scaled(settings.scale);
                        e.host.present(&scaled.pixels, x, y)?;
                    }
                }
            }
            if !hidden && !settings.paused {
                effect_elapsed += elapsed;
            }
            if render_due {
                effects.retain_mut(|(effect, _)| effect.update(effect_elapsed, &world));
                effect_elapsed = 0.0;
            }
            for (effect, host) in &mut effects {
                host.set_visible(!hidden);
                if !hidden && render_due {
                    let (x, y) = effect.position();
                    host.present(&effect.frame().pixels, x, y)?;
                }
            }
            if render_due {
                frames += 1;
                render_at = now;
            }
            NativeHost::wait(if hidden || settings.paused { 100 } else { 16 });
        }
        if let Some(path) = o.trace {
            let result = serde_json::json!({"native_frames":frames,"world_snapshots":snapshots,"elapsed_seconds":start.elapsed().as_secs_f64(),"pets":entries.len(),"exit":"clean"});
            std::fs::write(path, serde_json::to_vec_pretty(&result).unwrap())
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }
}
