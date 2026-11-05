#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]
use claude_mascot::{
    engine::{Pet, Skin},
    render::{Atlas, Renderer, FEET_X, FEET_Y, HEIGHT, WIDTH},
    settings::{config_path, Settings},
    Vec2, World,
};
use std::path::PathBuf;

#[cfg(any(windows, test))]
const MSI_INSTALL_MARKER: &str = "install-mode-msi.marker";
#[cfg(windows)]
const MSI_UPDATE_MESSAGE: &str =
    "Эта версия установлена через MSI. Обновите установочным пакетом из Releases.";

#[cfg(any(windows, test))]
fn has_msi_marker(executable: &std::path::Path) -> bool {
    executable
        .parent()
        .is_some_and(|directory| directory.join(MSI_INSTALL_MARKER).is_file())
}

#[cfg(windows)]
fn running_from_msi() -> bool {
    std::env::current_exe()
        .ok()
        .is_some_and(|executable| has_msi_marker(&executable))
}

#[cfg(windows)]
fn show_msi_update_message() {
    use windows_sys::Win32::UI::WindowsAndMessaging::*;
    let message = MSI_UPDATE_MESSAGE
        .encode_utf16()
        .chain(Some(0))
        .collect::<Vec<_>>();
    let title = "Claude Mascot Rust"
        .encode_utf16()
        .chain(Some(0))
        .collect::<Vec<_>>();
    // SAFETY: both nul-terminated buffers live for this synchronous call.
    unsafe {
        MessageBoxW(
            std::ptr::null_mut(),
            message.as_ptr(),
            title.as_ptr(),
            MB_OK | MB_ICONINFORMATION,
        );
    }
}

struct Options {
    check_updates: bool,
    skip_update_check: bool,
    inspect: bool,
    preview: Option<PathBuf>,
    headless: bool,
    seconds: Option<f64>,
    config: PathBuf,
    trace: Option<PathBuf>,
    skin: Option<Skin>,
    fps: Option<u32>,
}
fn options() -> Result<Options, String> {
    let mut o = Options {
        check_updates: false,
        skip_update_check: false,
        inspect: false,
        preview: None,
        headless: false,
        seconds: None,
        config: config_path(),
        trace: None,
        skin: None,
        fps: None,
    };
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--help" | "-h" => {
                println!("Claude Mascot Rust\n  --skin claude|tagilla\n  --fps 30..240         Set and persist target frame rate\n  --config FILE\n  --preview DIRECTORY   Export both characters' states as PNGs\n  --headless            Run deterministic simulation checks without a window\n  --smoke-test SECONDS  Open native pet, then exit automatically\n  --trace FILE          Write smoke-test metrics as JSON");
                std::process::exit(0)
            }
            "--preview" => {
                o.preview = Some(args.next().ok_or("--preview needs a directory")?.into())
            }
            "--headless" => o.headless = true,
            "--check-updates" => o.check_updates = true,
            "--skip-update-check" => o.skip_update_check = true,
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
            "--fps" => {
                let fps: u32 = args
                    .next()
                    .ok_or("--fps needs an integer")?
                    .parse()
                    .map_err(|_| "FPS must be an integer from 30 to 240")?;
                if !claude_mascot::timing::valid_fps(fps) {
                    return Err("FPS must be an integer from 30 to 240".into());
                }
                o.fps = Some(fps);
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
    #[cfg(windows)]
    if std::env::args_os()
        .nth(1)
        .is_some_and(|a| a == "--uninstall-autostart")
    {
        if std::env::args_os().count() != 2 {
            return Err("--uninstall-autostart accepts no additional arguments".into());
        }
        // Windows Installer calls this before removing our EXE. Do not start
        // the desktop host, acquire the singleton, or perform update checks.
        return match claude_mascot::settings::clear_own_autostart() {
            Ok(()) => Ok(()),
            Err(error) => {
                // An uninstall helper must never block MSI on an error dialog.
                eprintln!("{error}");
                std::process::exit(1);
            }
        };
    }
    #[cfg(windows)]
    let msi_mode = running_from_msi();
    #[cfg(windows)]
    {
        let args = std::env::args_os().skip(1).collect::<Vec<_>>();
        if args.first().is_some_and(|a| a == "--apply-update") {
            if msi_mode {
                return Err(MSI_UPDATE_MESSAGE.into());
            }
            if args.len() != 2 {
                return Err("--apply-update requires one plan file".into());
            }
            return claude_mascot::update_apply::apply(std::path::Path::new(&args[1]));
        }
    }
    let o = options()?;
    if let Some(dir) = &o.preview {
        return preview(dir);
    }
    if o.headless {
        return headless(&o);
    }
    #[cfg(windows)]
    if o.check_updates {
        return check_updates(&o, msi_mode);
    }
    #[cfg(windows)]
    {
        native::run(o, msi_mode)
    }
    #[cfg(not(windows))]
    {
        Err("The native desktop host currently supports Windows. Use --headless or --preview on other platforms.".into())
    }
}

#[cfg(test)]
mod msi_marker_tests {
    use super::*;

    #[test]
    fn detects_marker_adjacent_to_executable_only() {
        let directory = std::env::temp_dir().join(format!(
            "claude-mascot-msi-marker-test-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        let executable = directory.join("claude-mascot.exe");
        assert!(!has_msi_marker(&executable));
        std::fs::write(directory.join(MSI_INSTALL_MARKER), "msi").unwrap();
        assert!(has_msi_marker(&executable));
        std::fs::remove_file(directory.join(MSI_INSTALL_MARKER)).unwrap();
        std::fs::remove_dir(directory).unwrap();
    }
}
#[cfg(windows)]
fn check_updates(o: &Options, msi_mode: bool) -> Result<(), String> {
    use claude_mascot::update::{UpdateEvent, UpdateService};
    if msi_mode {
        show_msi_update_message();
        return Ok(());
    }
    let s = Settings::load(&o.config)?;
    let service = UpdateService::new(s.update_repo, false);
    service.request_check();
    let start = std::time::Instant::now();
    while start.elapsed() < std::time::Duration::from_secs(45) {
        for event in service.poll() {
            match event {
                UpdateEvent::Checked { release, .. } => {
                    let result = serde_json::json!({"current":env!("CARGO_PKG_VERSION"),"available":release.map(|r|r.version)});
                    println!("{result}");
                    if let Some(p) = &o.trace {
                        std::fs::write(p, serde_json::to_vec_pretty(&result).unwrap())
                            .map_err(|e| e.to_string())?;
                    }
                    return Ok(());
                }
                UpdateEvent::Error { message, .. } => return Err(message),
                _ => {}
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(30));
    }
    Err("Update check timed out".into())
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
    use claude_mascot::update::{UpdateEvent, UpdateService};
    use claude_mascot::{
        engine::{update_interactions, Command},
        platform::{self, InputEvent, InstanceGuard, MenuState, NativeHost},
        settings,
    };
    use std::time::{Duration, Instant};
    fn update_dialog(text: &str, question: bool) -> bool {
        use windows_sys::Win32::UI::WindowsAndMessaging::*;
        let text = text.encode_utf16().chain(Some(0)).collect::<Vec<_>>();
        let title = "Claude Mascot — обновления"
            .encode_utf16()
            .chain(Some(0))
            .collect::<Vec<_>>();
        // SAFETY: both nul-terminated buffers live for this synchronous call.
        unsafe {
            MessageBoxW(
                std::ptr::null_mut(),
                text.as_ptr(),
                title.as_ptr(),
                if question {
                    MB_YESNO | MB_ICONQUESTION
                } else {
                    MB_OK | MB_ICONINFORMATION
                },
            ) == IDYES
        }
    }
    struct Drag {
        offset: Vec2,
        start: Vec2,
        samples: claude_mascot::timing::DragVelocity,
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
    pub fn run(o: Options, msi_mode: bool) -> Result<(), String> {
        platform::initialize()?;
        let instance_name = if o.seconds.is_some() {
            format!("Local\\ClaudeMascotRust-Smoke-{}", std::process::id())
        } else {
            "Local\\ClaudeMascotRust-v1".to_owned()
        };
        let Some(_instance) = InstanceGuard::acquire(&instance_name)? else {
            return Ok(());
        };
        let mut settings = Settings::load(&o.config)?;
        if let Some(fps) = o.fps {
            settings.target_fps = fps;
            settings.save(&o.config)?;
        }
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
        let updates = UpdateService::new(
            settings.update_repo.clone(),
            settings.auto_updates && !msi_mode && !o.skip_update_check && o.seconds.is_none(),
        );
        let mut update_in_progress = false;
        let mut declined_update: Option<String> = None;
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
        let mut pacer = claude_mascot::timing::FramePacer::new(start, settings.target_fps);
        let mut autostart = settings::autostart_enabled();
        let mut accumulator = 0.0;
        let mut frames = 0u64;
        let mut presented_frames = 0u64;
        let mut physics_steps = 0u64;
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
            accumulator =
                (accumulator + elapsed).min(claude_mascot::timing::MAX_CATCH_UP.as_secs_f64());
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
            for event in updates.poll() {
                match event {
                    UpdateEvent::Checked {
                        release: Some(release),
                        manual,
                    } => {
                        if !update_in_progress
                            && (manual || declined_update.as_deref() != Some(&release.version))
                        {
                            if update_dialog(&format!("Доступна версия {}. Сейчас установлена {}.\n\nСкачать и перезапустить приложение? Настройки сохранятся.",release.version,env!("CARGO_PKG_VERSION")),true){update_in_progress=true;updates.request_download(release);}else{declined_update=Some(release.version);}
                        }
                    }
                    UpdateEvent::Checked {
                        release: None,
                        manual: true,
                    } => {
                        update_dialog(
                            "Более новой версии с готовым Windows EXE не найдено.",
                            false,
                        );
                    }
                    UpdateEvent::Downloaded(staged) => {
                        if msi_mode {
                            update_in_progress = false;
                            update_dialog(MSI_UPDATE_MESSAGE, false);
                            continue;
                        }
                        if update_in_progress {
                            settings.save(&o.config)?;
                            let config =
                                std::fs::canonicalize(&o.config).map_err(|e| e.to_string())?;
                            let args = vec![
                                "--skip-update-check".into(),
                                "--config".into(),
                                config.to_string_lossy().into_owned(),
                            ];
                            match claude_mascot::update_apply::schedule(
                                &staged,
                                &std::env::current_exe().map_err(|e| e.to_string())?,
                                &args,
                            ) {
                                Ok(()) => return Ok(()),
                                Err(e) => {
                                    update_in_progress = false;
                                    update_dialog(
                                        &format!("Не удалось запустить обновление: {e}"),
                                        false,
                                    );
                                }
                            }
                        }
                    }
                    UpdateEvent::Error { message, manual } => {
                        update_in_progress = false;
                        if manual {
                            update_dialog(
                                &format!("Не удалось обновить приложение:\n{message}"),
                                false,
                            );
                        }
                    }
                    _ => {}
                }
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
                                samples: {
                                    let mut samples =
                                        claude_mascot::timing::DragVelocity::default();
                                    samples.begin(now, p);
                                    samples
                                },
                                moved: false,
                            })
                        }
                        InputEvent::PointerMove(p) => {
                            if let Some(d) = &mut e.drag {
                                d.moved |= (p.x - d.start.x).hypot(p.y - d.start.y) > 4.0;
                                d.samples.push(now, p);
                                if d.moved {
                                    e.pet.drag_to(Vec2::new(p.x + d.offset.x, p.y + d.offset.y));
                                }
                            }
                        }
                        InputEvent::PointerUp(p) => {
                            if let Some(mut d) = e.drag.take() {
                                if d.moved {
                                    // Include the release position and any pause before release.
                                    // Reusing the last nonzero move would fling a stationary pet.
                                    let v = d.samples.push(now, p);
                                    e.pet.drag_to(Vec2::new(p.x + d.offset.x, p.y + d.offset.y));
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
                    80..=84 => {
                        settings.target_fps = match c {
                            80 => 30,
                            81 => 60,
                            82 => 120,
                            83 => 144,
                            _ => 240,
                        };
                        pacer.set_fps(now, settings.target_fps);
                        save = true;
                    }
                    60 => {
                        if msi_mode {
                            update_dialog(MSI_UPDATE_MESSAGE, false);
                        } else if !update_in_progress {
                            updates.request_check();
                        }
                    }
                    61 => {
                        if msi_mode {
                            update_dialog(MSI_UPDATE_MESSAGE, false);
                            continue;
                        }
                        settings.auto_updates = !settings.auto_updates;
                        updates
                            .set_enabled(settings.auto_updates && !msi_mode && o.seconds.is_none());
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
                while accumulator >= 1.0 / claude_mascot::timing::SIMULATION_HZ as f64 {
                    physics_steps += 1;
                    for e in &mut entries {
                        e.pet
                            .update(1.0 / claude_mascot::timing::SIMULATION_HZ as f64, &world);
                    }
                    let mut pets = entries.iter().map(|e| e.pet.clone()).collect::<Vec<_>>();
                    update_interactions(
                        &mut pets,
                        1.0 / claude_mascot::timing::SIMULATION_HZ as f64,
                        &world,
                    );
                    for (e, p) in entries.iter_mut().zip(pets) {
                        e.pet = p;
                    }
                    accumulator -= 1.0 / claude_mascot::timing::SIMULATION_HZ as f64;
                }
            }
            let render_due = pacer.due(now);
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
                    auto_updates: settings.auto_updates && !msi_mode,
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
                    target_fps: settings.target_fps,
                });
                e.host.set_visible(!hidden);
                if !hidden && render_due {
                    let frame =
                        renderer.frame_scaled(&e.pet, settings.show_bubbles, settings.scale);
                    let x = (e.pet.pos.x - FEET_X * settings.scale).round() as i32;
                    let y = (e.pet.pos.y - FEET_Y * settings.scale).round() as i32;
                    e.host.present(&frame.pixels, x, y)?;
                    presented_frames += 1;
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
                pacer.advance(now);
            }
            if hidden || settings.paused {
                NativeHost::wait(100);
            } else {
                NativeHost::wait_precise(pacer.time_until_due(Instant::now()));
            }
        }
        if let Some(path) = o.trace {
            let result = serde_json::json!({
                "version": env!("CARGO_PKG_VERSION"),
                "native_frames": frames,
                "presented_frames": presented_frames,
                "physics_steps": physics_steps,
                "target_fps": settings.target_fps,
                "world_snapshots": snapshots,
                "elapsed_seconds": start.elapsed().as_secs_f64(),
                "pets": entries.len(),
                "exit": "clean"
            });
            std::fs::write(path, serde_json::to_vec_pretty(&result).unwrap())
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }
}
