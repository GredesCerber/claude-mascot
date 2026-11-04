#![cfg(windows)]
use claude_mascot::platform::{initialize, snapshot, InstanceGuard, NativeHost};
use windows_sys::Win32::System::Threading::{
    GetCurrentProcess, GetGuiResources, GR_GDIOBJECTS, GR_USEROBJECTS,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetWindowLongPtrW, GWL_EXSTYLE, WS_EX_NOACTIVATE, WS_EX_TRANSPARENT,
};

#[test]
fn native_surfaces_release_gdi_and_user_handles() {
    initialize().unwrap();
    // Warm up the window class, process graphics state and first GDI surface.
    {
        let mut h = NativeHost::new("Mascot native test", 16, 16, false).unwrap();
        h.present(&vec![0xFFAA4422; 256], 0, 0).unwrap();
    }
    let counts = || unsafe {
        (
            GetGuiResources(GetCurrentProcess(), GR_GDIOBJECTS),
            GetGuiResources(GetCurrentProcess(), GR_USEROBJECTS),
        )
    };
    let before = counts();
    for i in 0..50 {
        let mut h = NativeHost::new("Mascot native test", 16, 16, false).unwrap();
        assert!(h.present(&[], 0, 0).is_err());
        h.present(&vec![0x80804020; 256], 0, 0).unwrap();
        if i % 2 == 0 {
            h.set_size(24, 24).unwrap();
            assert!(h.present(&[0; 256], 0, 0).is_err());
            h.present(&[0; 576], 0, 0).unwrap();
        }
        h.set_input_transparent().unwrap();
        let styles = unsafe { GetWindowLongPtrW(h.hwnd(), GWL_EXSTYLE) } as u32;
        assert_eq!(
            styles & (WS_EX_TRANSPARENT | WS_EX_NOACTIVATE),
            WS_EX_TRANSPARENT | WS_EX_NOACTIVATE
        );
    }
    assert!(NativeHost::pump());
    let after = counts();
    assert!(
        after.0 <= before.0 + 2,
        "GDI object leak: {before:?} -> {after:?}"
    );
    assert!(
        after.1 <= before.1 + 2,
        "USER object leak: {before:?} -> {after:?}"
    );
    let world = snapshot();
    assert!(!world.work_areas.is_empty());
    assert!(world.idle_seconds.is_finite());
    assert!(world
        .windows
        .iter()
        .all(|w| w.rect.w > 0.0 && w.rect.h > 0.0));
    let name = format!("Local\\MascotTest-{}", std::process::id());
    let guard = InstanceGuard::acquire(&name).unwrap().unwrap();
    assert!(InstanceGuard::acquire(&name).unwrap().is_none());
    drop(guard);
    assert!(InstanceGuard::acquire(&name).unwrap().is_some());
}
