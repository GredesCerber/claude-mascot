//! Isolated development preview: no desktop pets, settings files or updates.
#[cfg(windows)]
fn main() -> Result<(), String> {
    use claude_mascot::{
        platform::NativeHost,
        preferences::{PreferencesEvent, PreferencesState, PreferencesWindow},
    };
    let mut panel = PreferencesWindow::new()?;
    let mut state = PreferencesState {
        pet_ids: vec![1, 2],
        pet_names: vec!["Рыжик (превью)".into(), "Мятный (превью)".into()],
        ..PreferencesState::default()
    };
    panel.show(state.clone());
    let start = std::time::Instant::now();
    while start.elapsed().as_secs() < 600 && NativeHost::pump() && panel.is_visible() {
        for event in panel.poll() {
            println!("{event:?}");
            if let PreferencesEvent::SelectPet(id) = event {
                state.selected_pet_id = id;
                state.body_color = if id == 2 { 0x63B69A } else { 0xD97757 };
                panel.sync(state.clone());
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(8));
    }
    Ok(())
}
#[cfg(not(windows))]
fn main() {
    println!("Native preferences preview requires Windows.");
}
