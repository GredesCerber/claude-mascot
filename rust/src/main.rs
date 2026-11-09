#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]
use claude_mascot::{
    behavior::{BehaviorCoordinator, BehaviorSettings, SpacingPolicy},
    engine::{Pet, Skin},
    render::{Renderer, FEET_X, FEET_Y, HEIGHT, WIDTH},
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
    desktop_probe: bool,
    seconds: Option<f64>,
    config: PathBuf,
    trace: Option<PathBuf>,
    skin: Option<Skin>,
    fps: Option<u32>,
    spacing: Option<(f64, f64)>,
    pets: usize,
    activity: Option<claude_mascot::activities::ActivityId>,
    open_settings: bool,
}
fn options() -> Result<Options, String> {
    let mut o = Options {
        check_updates: false,
        skip_update_check: false,
        inspect: false,
        preview: None,
        headless: false,
        desktop_probe: false,
        seconds: None,
        config: config_path(),
        trace: None,
        skin: None,
        fps: None,
        spacing: None,
        pets: 1,
        activity: None,
        open_settings: false,
    };
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--help" | "-h" => {
                println!("Claude Mascot Rust\n  --skin claude\n  --pets 1..8           Start with several Claudes\n  --activity SLUG       Start a catalog scene after landing\n  --fps 30..240         Set and persist target frame rate\n  --spacing MIN:MAX    Set and persist free edge distance (e.g. 24:96)\n  --config FILE\n  --preview DIRECTORY   Export Claude animation states as PNGs\n  --headless            Run deterministic simulation checks without a window\n  --smoke-test SECONDS  Open native pet, then exit automatically\n  --desktop-probe       Inspect desktop-icon geometry only\n  --trace FILE          Write smoke-test metrics as JSON");
                std::process::exit(0)
            }
            "--preview" => {
                o.preview = Some(args.next().ok_or("--preview needs a directory")?.into())
            }
            "--headless" => o.headless = true,
            "--desktop-probe" => o.desktop_probe = true,
            "--settings" => o.open_settings = true,
            "--pets" => {
                o.pets = args
                    .next()
                    .ok_or("--pets needs a number")?
                    .parse()
                    .map_err(|_| "Pets must be an integer from 1 to 8")?;
                if !(1..=8).contains(&o.pets) {
                    return Err("Pets must be an integer from 1 to 8".into());
                }
            }
            "--activity" => {
                let slug = args.next().ok_or("--activity needs a scene slug")?;
                o.activity = Some(
                    claude_mascot::activities::CATALOG
                        .iter()
                        .find(|spec| spec.slug == slug)
                        .ok_or("Unknown activity slug")?
                        .id,
                );
            }
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
                    _ => return Err("Only Claude is available".into()),
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
            "--spacing" => {
                let value = args
                    .next()
                    .ok_or("--spacing needs MIN:MAX (for example 24:96)")?;
                let (min, max) = value.split_once(':').ok_or("--spacing needs MIN:MAX")?;
                let min = min.parse().map_err(|_| "Invalid minimum spacing")?;
                let max = max.parse().map_err(|_| "Invalid maximum spacing")?;
                if !claude_mascot::settings::valid_personal_space(min, max) {
                    return Err("Spacing needs 8..256 minimum, up to 512 maximum, and a range of at least 8".into());
                }
                o.spacing = Some((min, max));
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
    #[cfg(windows)]
    if o.desktop_probe {
        platform_probe(&o)?;
        return Ok(());
    }
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
    let service = UpdateService::new(s.update_repo.clone(), false);
    service.request_check();
    let start = std::time::Instant::now();
    while start.elapsed() < std::time::Duration::from_secs(45) {
        for event in service.poll() {
            match event {
                UpdateEvent::Checked { release, .. } => {
                    let result = serde_json::json!({"current":env!("CARGO_PKG_VERSION"),"repository":s.update_repo,"available":release.map(|r|r.version)});
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
#[cfg(windows)]
fn platform_probe(o: &Options) -> Result<(), String> {
    claude_mascot::platform::initialize()?;
    let monitor = claude_mascot::desktop::DesktopMonitor::new();
    std::thread::sleep(std::time::Duration::from_millis(1800));
    let icons = monitor.snapshot();
    let result = serde_json::json!({
        "version":env!("CARGO_PKG_VERSION"),
        "icon_count":icons.len(),
        "rectangles":icons.iter().map(|icon|serde_json::json!({"x":icon.rect.x,"y":icon.rect.y,"w":icon.rect.w,"h":icon.rect.h})).collect::<Vec<_>>()
    });
    let bytes = serde_json::to_vec_pretty(&result).map_err(|e| e.to_string())?;
    if let Some(path) = &o.trace {
        std::fs::write(path, &bytes).map_err(|e| e.to_string())?;
    }
    println!("{}", String::from_utf8_lossy(&bytes));
    Ok(())
}

fn preview(dir: &std::path::Path) -> Result<(), String> {
    use claude_mascot::engine::State;
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let events: Vec<_> = claude_mascot::dialogue::ALL_EVENTS
        .iter()
        .map(|event| serde_json::json!({"event":format!("{event:?}"),"phrases":event.lines()}))
        .collect();
    std::fs::write(
        dir.join("dialogue.json"),
        serde_json::to_vec_pretty(&events).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let catalog: Vec<_> = claude_mascot::activities::CATALOG.iter().map(|scene|
        serde_json::json!({"id":scene.id.0,"source":format!("{}.svg",scene.slug),
            "title":scene.title_ru,"category":scene.group.label_ru(),
            "duration_seconds":scene.duration,"phrase":scene.phrase,"phrases":claude_mascot::activity_dialogue::ACTIVITY_LINES[scene.id.index()],"menu_command":100+scene.id.0 as u16})
    ).collect();
    std::fs::write(
        dir.join("catalog.json"),
        serde_json::to_vec_pretty(&catalog).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let mut r = Renderer::new()?;
    let states = [
        State::Idle,
        State::Walk,
        State::Run,
        State::Chase,
        State::Sit,
        State::Sleep,
        State::Happy,
        State::Jump,
        State::Thrown,
        State::Hold,
        State::Dance,
        State::Wave,
        State::Climb,
        State::Laptop,
        State::Stretch,
        State::LookAround,
    ];
    for skin in [Skin::Claude] {
        for state in states {
            let mut p = Pet::new(1, skin, Vec2::default());
            p.state = state;
            p.time = 0.5;
            p.state_time = 0.5;
            p.bubble = Some("Привет! Что кодим сегодня?".into());
            p.bubble_time = 3.0;
            if state == State::Thrown {
                p.vel = Vec2::new(650.0, -180.0);
            }
            if state == State::Happy {
                p.hearts = 1.0;
            }
            r.frame(&p, true)
                .save_png(&dir.join(format!("{:?}-{:?}.png", skin, state)))?;
        }
    }
    for skin in [Skin::Claude] {
        for satisfaction in [10.0, 50.0, 95.0] {
            let mut p = Pet::new(1, skin, Vec2::default());
            p.satisfaction = satisfaction;
            r.frame(&p, false)
                .save_png(&dir.join(format!("{skin:?}-Mood-{}.png", satisfaction as u8)))?;
        }
    }
    for (state, phases) in [
        (State::Laptop, &[0.0, 0.5, 1.5, 3.0, 9.5][..]),
        (State::Stretch, &[0.2, 1.2, 2.3][..]),
        (State::LookAround, &[0.2, 0.8, 1.8][..]),
    ] {
        for &phase in phases {
            let mut p = Pet::new(1, Skin::Claude, Vec2::default());
            p.state = state;
            p.time = phase;
            p.state_time = phase;
            r.frame(&p, false)
                .save_png(&dir.join(format!("Claude-{state:?}-{phase:.1}.png")))?;
        }
    }
    for scene in &claude_mascot::activities::CATALOG {
        for phase in [0.4, scene.duration * 0.5, scene.duration * 0.92] {
            let mut pet = Pet::new(1, Skin::Claude, Vec2::default());
            pet.state = State::Activity;
            pet.activity = Some(scene.id);
            pet.time = phase;
            pet.state_time = phase;
            r.frame(&pet, false)
                .save_png(&dir.join(format!("Scene-{}-{phase:.1}.png", scene.slug)))?;
        }
    }
    use claude_mascot::social::{SocialActivity, SocialKind, SocialRole};
    for kind in [
        SocialKind::Greeting,
        SocialKind::HighFive,
        SocialKind::DanceTogether,
        SocialKind::ShareLaptop,
        SocialKind::Comfort,
    ] {
        for phase in [0.4, 1.2, 2.0] {
            let mut pair = claude_mascot::render::Canvas::new(WIDTH + 90, HEIGHT);
            for (index, role) in [SocialRole::Initiator, SocialRole::Partner]
                .into_iter()
                .enumerate()
            {
                let mut pet = Pet::new(index as u64 + 1, Skin::Claude, Vec2::default());
                pet.state = if kind == SocialKind::ShareLaptop && index == 0 {
                    State::Laptop
                } else {
                    State::Social
                };
                pet.social = Some(SocialActivity {
                    kind,
                    role,
                    partner_id: 2 - index as u64,
                    elapsed: phase,
                    partner_offset: Vec2::new(if index == 0 { 90.0 } else { -90.0 }, 0.0),
                });
                pet.facing = if index == 0 { 1.0 } else { -1.0 };
                pet.time = phase;
                pet.state_time = phase;
                let frame = r.frame(&pet, false);
                for y in 0..HEIGHT {
                    for x in 0..WIDTH {
                        let pixel = frame.pixels[y * WIDTH + x];
                        if pixel >> 24 != 0 {
                            pair.pixels[y * pair.width + x + index * 90] = pixel;
                        }
                    }
                }
            }
            pair.save_png(&dir.join(format!("Pair-{kind:?}-{phase:.1}.png")))?;
        }
    }
    for &(_, _, color) in claude_mascot::palette::PRESETS {
        let mut pet = Pet::new(1, Skin::Claude, Vec2::default());
        pet.state = State::Idle;
        pet.body_color = color;
        r.frame(&pet, false)
            .save_png(&dir.join(format!("Palette-{color:06X}.png")))?;
    }
    preview_social_play(dir)?;
    preview_group_tower(dir)?;
    Ok(())
}

fn preview_group_tower(dir: &std::path::Path) -> Result<(), String> {
    use claude_mascot::{
        engine::State,
        social::{SocialCoordinator, SocialKind, SocialMotion},
        Rect,
    };
    let world = World {
        work_areas: vec![Rect::new(0.0, 0.0, 1800.0, 1000.0)],
        ..World::default()
    };
    let mut pets: Vec<_> = (0..8)
        .map(|i| {
            let mut pet = Pet::new(
                i + 1,
                Skin::Claude,
                Vec2::new(450.0 + i as f64 * 112.0, 1000.0),
            );
            pet.state = State::Idle;
            pet.auto_activities = false;
            pet.auto_reactions = false;
            pet.body_color = claude_mascot::palette::PRESETS[i as usize].2;
            pet
        })
        .collect();
    let mut social = SocialCoordinator::new();
    social.request_for(1, Some(SocialKind::Tower));
    social.update(0.0, pets.iter_mut(), &world);
    let mut renderer = Renderer::new()?;
    let mut snapshots = Vec::new();
    let mut last_perched = usize::MAX;
    let mut max_perched = 0;
    let mut seen_completed = false;
    for step in 0..=7200 {
        if step > 0 {
            for pet in &mut pets {
                pet.update(1.0 / 240.0, &world);
            }
            social.update(1.0 / 240.0, pets.iter_mut(), &world);
        }
        let perched = pets
            .iter()
            .filter(|p| matches!(p.social_motion, Some(SocialMotion::Perch { .. })))
            .count();
        max_perched = max_perched.max(perched);
        let completed_now = social.stats.completed > 0 && !seen_completed;
        let capture = perched != last_perched || completed_now || step == 7200;
        if capture {
            last_perched = perched;
            seen_completed |= completed_now;
            let mut scene = claude_mascot::render::Canvas::new(1800, 1060);
            scene.rect(0.0, 1000.0, 1800.0, 1.0, 0xFF68706B);
            for pet in &pets {
                let frame = renderer.frame(pet, true);
                let ox = (pet.pos.x - FEET_X).round() as i32;
                let oy = (pet.pos.y - FEET_Y).round() as i32;
                for y in 0..HEIGHT {
                    for x in 0..WIDTH {
                        let color = frame.pixels[y * WIDTH + x];
                        let a = color >> 24;
                        if a == 0 {
                            continue;
                        }
                        let channel = |shift: u32| (((color >> shift) & 255u32) * 255 / a).min(255);
                        scene.pixel(
                            ox + x as i32,
                            oy + y as i32,
                            (a << 24) | (channel(16) << 16) | (channel(8) << 8) | channel(0),
                        );
                    }
                }
            }
            let file = format!("Group-Tower-8-{step:04}-{perched}-perched.png");
            scene.save_png(&dir.join(&file))?;
            snapshots.push(serde_json::json!({"step":step,"perched":perched,"file":file,"completed":social.stats.completed,"cancelled":social.stats.cancelled,"positions":pets.iter().map(|p|serde_json::json!({"id":p.id,"x":p.pos.x,"y":p.pos.y})).collect::<Vec<_>>() }));
        }
        if seen_completed && pets.iter().all(|p| p.pos.y >= 999.0 && p.social.is_none()) {
            break;
        }
    }
    std::fs::write(
        dir.join("group-tower.json"),
        serde_json::to_vec_pretty(&snapshots).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    if max_perched != 7 || !seen_completed {
        return Err(format!(
            "Eight-pet tower preview did not complete: {max_perched} levels, {} cancellations",
            social.stats.cancelled
        ));
    }
    Ok(())
}

fn preview_social_play(dir: &std::path::Path) -> Result<(), String> {
    use claude_mascot::{
        engine::State,
        social::{SocialCoordinator, SocialKind},
        Rect,
    };
    let world = World {
        work_areas: vec![Rect::new(0.0, 0.0, 1000.0, 700.0)],
        ..World::default()
    };
    let mut renderer = Renderer::new()?;
    for kind in [SocialKind::Tag, SocialKind::Tower, SocialKind::Leapfrog] {
        let mut pets: Vec<_> = [300.0, 380.0]
            .into_iter()
            .enumerate()
            .map(|(i, x)| {
                let mut pet = Pet::new(i as u64 + 1, Skin::Claude, Vec2::new(x, 700.0));
                pet.state = State::Idle;
                pet.auto_activities = false;
                pet.auto_reactions = false;
                pet
            })
            .collect();
        let mut social = SocialCoordinator::new();
        social.request_for(1, Some(kind));
        social.update(0.0, pets.iter_mut(), &world);
        for step in 1..=2640 {
            for pet in &mut pets {
                pet.update(1.0 / 240.0, &world);
            }
            social.update(1.0 / 240.0, pets.iter_mut(), &world);
            if ![24, 144, 288, 720, 1440, 1680, 2400, 2640].contains(&step) {
                continue;
            }
            let mut scene = claude_mascot::render::Canvas::new(1100, 400);
            scene.rect(0.0, 300.0, 1100.0, 1.0, 0xFF68706B);
            for pet in &pets {
                let frame = renderer.frame(pet, true);
                let ox = (pet.pos.x + 50.0 - FEET_X).round() as i32;
                let oy = (pet.pos.y - 400.0 - FEET_Y).round() as i32;
                for y in 0..HEIGHT {
                    for x in 0..WIDTH {
                        let color = frame.pixels[y * WIDTH + x];
                        let a = color >> 24;
                        if a == 0 {
                            continue;
                        }
                        let channel = |shift: u32| (((color >> shift) & 255u32) * 255 / a).min(255);
                        scene.pixel(
                            ox + x as i32,
                            oy + y as i32,
                            (a << 24) | (channel(16) << 16) | (channel(8) << 8) | channel(0),
                        );
                    }
                }
            }
            scene.save_png(&dir.join(format!("Play-{kind:?}-{:04}.png", step)))?;
        }
    }
    Ok(())
}

fn headless(o: &Options) -> Result<(), String> {
    let world = World::default();
    let mut pets: Vec<_> = (0..o.pets.max(2))
        .map(|i| {
            Pet::new(
                i as u64 + 1,
                Skin::Claude,
                Vec2::new(400.0 + i as f64 * 70.0, 50.0),
            )
        })
        .collect();
    let mut social = claude_mascot::social::SocialCoordinator::new();
    let (min, max) = o.spacing.unwrap_or((24.0, 96.0));
    let mut behavior = BehaviorCoordinator::new(behavior_settings(min, max));
    let mut contacts = claude_mascot::contacts::ContactCoordinator::new();
    for _ in 0..36000 {
        for p in &mut pets {
            p.update(1.0 / 60.0, &world);
            if !p.pos.x.is_finite()
                || !p.pos.y.is_finite()
                || !p.vel.x.is_finite()
                || !p.vel.y.is_finite()
                || !p.gaze.x.is_finite()
                || !p.gaze.y.is_finite()
                || !p.satisfaction.is_finite()
                || !(0.0..=100.0).contains(&p.satisfaction)
            {
                return Err("Nonfinite simulation state".into());
            }
        }
        contacts.update(1.0 / 60.0, pets.iter_mut(), &world);
        behavior.update(1.0 / 60.0, pets.iter_mut(), &world);
        social.update(1.0 / 60.0, pets.iter_mut(), &world);
    }
    let result = serde_json::json!({"headless_steps":36000,"simulated_seconds":600,"pets":pets.len(),"finite":true,"social_started":social.stats.started,"social_completed":social.stats.completed,"social_cancelled":social.stats.cancelled,"behavior_decisions":behavior.decision_count,"spacing_actions":behavior.steering_count});
    println!("{result}");
    if let Some(path) = &o.trace {
        std::fs::write(path, serde_json::to_vec_pretty(&result).unwrap())
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn behavior_settings(min: f64, max: f64) -> BehaviorSettings {
    BehaviorSettings {
        spacing: SpacingPolicy {
            min_free_gap_px: min,
            max_free_gap_px: max,
        },
        ..BehaviorSettings::default()
    }
}

#[cfg(windows)]
mod native {
    use super::*;
    use claude_mascot::update::{UpdateEvent, UpdateService};
    use claude_mascot::{
        engine::Command,
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
    }
    fn panel_state(
        settings: &Settings,
        entries: &[Entry],
        selected: u64,
    ) -> claude_mascot::preferences::PreferencesState {
        let pet = &entries
            .iter()
            .find(|e| e.pet.id == selected)
            .unwrap_or(&entries[0])
            .pet;
        claude_mascot::preferences::PreferencesState {
            selected_pet_id: pet.id,
            pet_ids: entries.iter().map(|e| e.pet.id).collect(),
            body_color: pet.body_color,
            fps: settings.target_fps,
            scale: settings.scale,
            intensity: settings.activity_intensity,
            min_gap: settings.personal_space_min,
            max_gap: settings.personal_space_max,
            show_mood: settings.show_mood,
            show_bubbles: settings.show_bubbles,
            auto_reactions: settings.auto_reactions,
            auto_activities: settings.auto_activities,
            social_enabled: settings.social_enabled,
            desktop_perches: settings.desktop_perches,
            paused: settings.paused,
        }
    }
    fn host(settings: &Settings, tray: bool) -> Result<NativeHost, String> {
        NativeHost::new(
            "Claude Mascot Rust",
            (WIDTH as f64 * settings.scale).round() as i32,
            (HEIGHT as f64 * settings.scale).round() as i32,
            tray,
        )
    }
    fn spawn(mut pet: Pet, settings: &Settings, tray: bool) -> Result<Entry, String> {
        static SESSION_SPEECH: std::sync::OnceLock<u64> = std::sync::OnceLock::new();
        let speech_seed = SESSION_SPEECH.get_or_init(|| {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(1, |d| d.as_nanos() as u64)
        });
        pet.reseed_speech(*speech_seed ^ pet.id);
        pet.body_color = settings.body_color;
        pet.activity_intensity = settings.activity_intensity;
        Ok(Entry {
            pet,
            host: host(settings, tray)?,
            drag: None,
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
        if let Some((min, max)) = o.spacing {
            settings.personal_space_min = min;
            settings.personal_space_max = max;
            settings.save(&o.config)?;
        }
        if let Some(s) = o.skin {
            settings.skin = s;
        }
        let mut renderer = Renderer::new()?;
        let desktop = claude_mascot::desktop::DesktopMonitor::new();
        desktop.set_enabled(settings.desktop_perches);
        let mut world = platform::snapshot();
        desktop.apply(&mut world);
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
        for index in 1..o.pets {
            entries.push(spawn(
                Pet::new(
                    index as u64 + 1,
                    Skin::Claude,
                    Vec2::new(
                        (area.x + area.w / 2.0 + index as f64 * 72.0)
                            .clamp(area.x + 60.0, area.right() - 60.0),
                        area.y + 60.0,
                    ),
                ),
                &settings,
                false,
            )?);
            if o.inspect {
                entries.last().unwrap().host.expose_for_inspection()?;
            }
        }
        for entry in &mut entries {
            entry.pet.auto_activities = settings.auto_activities;
            entry.pet.auto_reactions = settings.auto_reactions;
            entry.pet.display_scale = settings.scale;
            if let Some(activity) = o.activity {
                entry.pet.command(Command::Activity(activity), &world);
            }
        }
        let mut social = claude_mascot::social::SocialCoordinator::new();
        social.enabled = settings.social_enabled;
        let mut behavior = BehaviorCoordinator::new(behavior_settings(
            settings.personal_space_min,
            settings.personal_space_max,
        ));
        let mut preferences: Option<claude_mascot::preferences::PreferencesWindow> = None;
        let mut contacts = claude_mascot::contacts::ContactCoordinator::new();
        let mut selected_pet_id = 1;
        let mut open_preferences = o.open_settings;
        let mut next_id = o.pets as u64 + 1;
        let start = Instant::now();
        let mut preferences_at = start;
        let mut previous = start;
        let mut world_at = start - Duration::from_secs(1);
        let mut pacer = claude_mascot::timing::FramePacer::new(start, settings.target_fps);
        let mut autostart = settings::autostart_enabled();
        let mut accumulator = 0.0;
        let mut frames = 0u64;
        let mut presented_frames = 0u64;
        let mut activity_presented_frames = 0u64;
        let mut social_presented_frames = 0u64;
        let mut drag_presented_frames = 0u64;
        let mut dragging_seconds = 0.0;
        let mut last_drag_frame = None::<Instant>;
        let mut max_drag_frame_interval = 0.0f64;
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
                desktop.apply(&mut world);
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
            if let Some(panel) = preferences.as_mut() {
                use claude_mascot::preferences::PreferencesEvent::*;
                for event in panel.poll() {
                    let selected = entries
                        .iter()
                        .position(|e| e.pet.id == selected_pet_id)
                        .unwrap_or(0);
                    match event {
                        SelectPet(id) => selected_pet_id = id,
                        SetFps(fps) => {
                            settings.target_fps = fps;
                            pacer = claude_mascot::timing::FramePacer::new(Instant::now(), fps);
                            save = true;
                        }
                        SetScale(scale) => {
                            settings.scale = scale;
                            rebuild = true;
                            save = true;
                        }
                        SetIntensity(value) => {
                            settings.activity_intensity = value.min(100);
                            save = true;
                        }
                        SetSpacing(min, max) => {
                            if settings::valid_personal_space(min, max) {
                                settings.personal_space_min = min;
                                settings.personal_space_max = max;
                                save = true;
                            }
                        }
                        ToggleMood(value) => {
                            settings.show_mood = value;
                            save = true;
                        }
                        ToggleBubbles(value) => {
                            settings.show_bubbles = value;
                            save = true;
                        }
                        ToggleReactions(value) => {
                            settings.auto_reactions = value;
                            save = true;
                        }
                        ToggleActivities(value) => {
                            settings.auto_activities = value;
                            save = true;
                        }
                        ToggleSocial(value) => {
                            settings.social_enabled = value;
                            save = true;
                        }
                        TogglePaused(value) => {
                            settings.paused = value;
                            save = true;
                        }
                        ToggleDesktop(value) => {
                            settings.desktop_perches = value;
                            desktop.set_enabled(value);
                            world = platform::snapshot();
                            desktop.apply(&mut world);
                            save = true;
                        }
                        ChooseColor => commands.push((selected, 188)),
                        ResetColor => commands.push((selected, 180)),
                        RunAction(action) => commands.push((selected, action)),
                    }
                }
            }
            let friend_count = entries.len();
            for (i, c) in commands {
                let Some(e) = entries.get_mut(i) else {
                    continue;
                };
                match c {
                    49 => {
                        selected_pet_id = e.pet.id;
                        open_preferences = true;
                    }
                    180..=188 => {
                        let color = if c == 188 {
                            e.host.choose_body_color(e.pet.body_color)
                        } else {
                            claude_mascot::palette::PRESETS
                                .iter()
                                .find(|(id, _, _)| *id == c)
                                .map(|(_, _, rgb)| *rgb)
                        };
                        if let Some(color) = color {
                            e.pet.body_color = color;
                            settings.body_color = color;
                            save = true;
                        }
                    }
                    1..=5 => {
                        let command = match c {
                            1 => Command::Wander,
                            2 => Command::Sit,
                            3 => Command::Sleep,
                            4 => Command::Climb,
                            _ => Command::Chase,
                        };
                        e.pet.command(command, &world);
                    }
                    20 => spawn_skins.push(Skin::Claude),
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
                    43 => {
                        settings.show_bubbles = !settings.show_bubbles;
                        save = true;
                    }
                    44 => {
                        settings.show_mood = !settings.show_mood;
                        save = true;
                    }
                    45 => {
                        settings.social_enabled = !settings.social_enabled;
                        save = true;
                    }
                    46 => {
                        settings.auto_activities = !settings.auto_activities;
                        save = true;
                    }
                    47 => {
                        settings.auto_reactions = !settings.auto_reactions;
                        save = true;
                    }
                    48 => {
                        settings.desktop_perches = !settings.desktop_perches;
                        desktop.set_enabled(settings.desktop_perches);
                        // Refresh immediately so removed icon supports disappear on the next step.
                        world = platform::snapshot();
                        desktop.apply(&mut world);
                        save = true;
                    }
                    85..=87 => {
                        let (min, max) = match c {
                            85 => (12.0, 48.0),
                            87 => (48.0, 144.0),
                            _ => (24.0, 96.0),
                        };
                        settings.personal_space_min = min;
                        settings.personal_space_max = max;
                        save = true;
                    }
                    70 => {
                        e.drag = None;
                        e.pet.dragging = false;
                        e.pet.jump();
                    }
                    71 => e.pet.pet(),
                    72 => e.pet.command(Command::Dance, &world),
                    73 => e.pet.command(Command::Wave, &world),
                    74 => e.pet.command(Command::Laptop, &world),
                    75 => e.pet.command(Command::Stretch, &world),
                    76 => e.pet.command(Command::LookAround, &world),
                    90..=98 => {
                        use claude_mascot::social::SocialKind;
                        let kind = match c {
                            91 => Some(SocialKind::Greeting),
                            92 => Some(SocialKind::HighFive),
                            93 => Some(SocialKind::DanceTogether),
                            94 => Some(SocialKind::ShareLaptop),
                            95 => Some(SocialKind::Comfort),
                            96 => Some(SocialKind::Tag),
                            97 => Some(SocialKind::Tower),
                            98 => Some(SocialKind::Leapfrog),
                            _ => None,
                        };
                        e.pet.command(Command::Wander, &world);
                        let prompt = if !settings.social_enabled {
                            "включи взаимодействия в меню друзей"
                        } else if friend_count < 2 {
                            "позови ещё одного Клода :)"
                        } else {
                            "кто со мной?"
                        };
                        e.pet.bubble = Some(prompt.into());
                        e.pet.bubble_time = 2.0;
                        if settings.social_enabled && friend_count > 1 {
                            e.pet
                                .speak(claude_mascot::dialogue::SpeechEvent::SocialInvite, 2.0);
                            social.request_for(e.pet.id, kind);
                        }
                    }
                    100..=174 => {
                        if let Some(activity) =
                            claude_mascot::activities::ActivityId::new((c - 100) as usize)
                        {
                            e.pet.command(Command::Activity(activity), &world);
                        }
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
            if open_preferences {
                if preferences.is_none() {
                    match claude_mascot::preferences::PreferencesWindow::new() {
                        Ok(panel) => preferences = Some(panel),
                        Err(error) => {
                            update_dialog(&error, false);
                        }
                    }
                }
                if let Some(panel) = preferences.as_mut() {
                    panel.show(panel_state(&settings, &entries, selected_pet_id));
                }
                open_preferences = false;
            }
            if let Some(panel) = preferences.as_mut() {
                if panel.is_visible()
                    && (save || now.duration_since(preferences_at) >= Duration::from_millis(200))
                {
                    panel.sync(panel_state(&settings, &entries, selected_pet_id));
                    preferences_at = now;
                }
            }
            social.enabled = settings.social_enabled;
            behavior.set_settings(behavior_settings(
                settings.personal_space_min,
                settings.personal_space_max,
            ));
            for entry in &mut entries {
                entry.pet.auto_activities = settings.auto_activities;
                entry.pet.auto_reactions = settings.auto_reactions;
                entry.pet.display_scale = settings.scale;
                entry.pet.activity_intensity = settings.activity_intensity;
            }
            social.update(0.0, entries.iter_mut().map(|entry| &mut entry.pet), &world);
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
                    contacts.update(
                        1.0 / claude_mascot::timing::SIMULATION_HZ as f64,
                        entries.iter_mut().map(|entry| &mut entry.pet),
                        &world,
                    );
                    behavior.update(
                        1.0 / claude_mascot::timing::SIMULATION_HZ as f64,
                        entries.iter_mut().map(|entry| &mut entry.pet),
                        &world,
                    );
                    social.update(
                        1.0 / claude_mascot::timing::SIMULATION_HZ as f64,
                        entries.iter_mut().map(|entry| &mut entry.pet),
                        &world,
                    );
                    accumulator -= 1.0 / claude_mascot::timing::SIMULATION_HZ as f64;
                }
            }
            // Input dispatch can consume time, especially with a high-rate mouse.
            // Test the deadline after handling input rather than using a stale time.
            let render_at = Instant::now();
            let render_due = pacer.due(render_at);
            let dragging = entries
                .iter()
                .any(|e| e.drag.as_ref().is_some_and(|d| d.moved));
            if dragging {
                dragging_seconds += elapsed;
            } else {
                last_drag_frame = None;
            }
            for e in &mut entries {
                e.host.set_menu_state(MenuState {
                    auto_updates: settings.auto_updates && !msi_mode,
                    selected_action: match e.pet.state {
                        claude_mascot::engine::State::Sit => 2,
                        claude_mascot::engine::State::Sleep => 3,
                        claude_mascot::engine::State::Climb => 4,
                        claude_mascot::engine::State::Dance => 72,
                        claude_mascot::engine::State::Wave => 73,
                        claude_mascot::engine::State::Laptop => 74,
                        claude_mascot::engine::State::Stretch => 75,
                        claude_mascot::engine::State::LookAround => 76,
                        claude_mascot::engine::State::Activity => {
                            e.pet.activity.map_or(1, |id| 100 + id.0 as u16)
                        }
                        _ if e.pet.chase => 5,
                        _ => 1,
                    },
                    paused: settings.paused,
                    hide_fullscreen: settings.hide_fullscreen,
                    autostart,
                    chase: e.pet.chase,
                    scale: settings.scale,
                    target_fps: settings.target_fps,
                    show_bubbles: settings.show_bubbles,
                    show_mood: settings.show_mood,
                    social_enabled: settings.social_enabled,
                    auto_activities: settings.auto_activities,
                    auto_reactions: settings.auto_reactions,
                    desktop_perches: settings.desktop_perches,
                    personal_space_min: settings.personal_space_min,
                    personal_space_max: settings.personal_space_max,
                    body_color: e.pet.body_color,
                    satisfaction: e.pet.satisfaction.clamp(0.0, 100.0).round() as u8,
                });
                e.host.set_visible(!hidden);
                if !hidden && render_due {
                    renderer.show_mood = settings.show_mood;
                    let frame =
                        renderer.frame_scaled(&e.pet, settings.show_bubbles, settings.scale);
                    let x = (e.pet.pos.x - FEET_X * settings.scale).round() as i32;
                    let y = (e.pet.pos.y - FEET_Y * settings.scale).round() as i32;
                    e.host.present(&frame.pixels, x, y)?;
                    presented_frames += 1;
                    if e.pet.state == claude_mascot::engine::State::Activity {
                        activity_presented_frames += 1;
                    }
                    if e.pet.social.is_some() {
                        social_presented_frames += 1;
                    }
                    if e.drag.as_ref().is_some_and(|d| d.moved) {
                        drag_presented_frames += 1;
                        let presented_at = Instant::now();
                        if let Some(previous) = last_drag_frame {
                            max_drag_frame_interval = max_drag_frame_interval
                                .max(presented_at.duration_since(previous).as_secs_f64());
                        }
                        last_drag_frame = Some(presented_at);
                    }
                }
            }
            if render_due {
                frames += 1;
                pacer.advance(render_at);
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
                "activity_presented_frames": activity_presented_frames,
                "social_presented_frames": social_presented_frames,
                "drag_presented_frames": drag_presented_frames,
                "dragging_seconds": dragging_seconds,
                "max_drag_frame_interval_ms": max_drag_frame_interval * 1000.0,
                "physics_steps": physics_steps,
                "target_fps": settings.target_fps,
                "world_snapshots": snapshots,
                "desktop_icon_count": world.windows.iter().filter(|w| w.is_desktop_icon).count(),
                "desktop_perches": settings.desktop_perches,
                "auto_reactions": settings.auto_reactions,
                "personal_space_min": settings.personal_space_min,
                "personal_space_max": settings.personal_space_max,
                "activity_intensity": settings.activity_intensity,
                "behavior_decisions": behavior.decision_count,
                "spacing_actions": behavior.steering_count,
                "contacts": contacts.contact_count,
                "strong_impacts": contacts.strong_impacts,
                "pet_states": entries.iter().map(|entry|serde_json::json!({"id":entry.pet.id,"state":format!("{:?}",entry.pet.state),"x":entry.pet.pos.x,"y":entry.pet.pos.y,"vx":entry.pet.vel.x,"vy":entry.pet.vel.y})).collect::<Vec<_>>(),
                "elapsed_seconds": start.elapsed().as_secs_f64(),
                "pets": entries.len(),
                "social_started": social.stats.started,
                "social_completed": social.stats.completed,
                "social_cancelled": social.stats.cancelled,
                "exit": "clean"
            });
            std::fs::write(path, serde_json::to_vec_pretty(&result).unwrap())
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }
}
