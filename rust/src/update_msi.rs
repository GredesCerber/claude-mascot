//! Read-only package inspection and per-user Windows Installer upgrades.
use super::{hash_file, launch, validate_digest, wide, ApplyPlan};
use std::os::windows::{fs::OpenOptionsExt, process::CommandExt};
use std::{
    ffi::{OsStr, OsString},
    fs::OpenOptions,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::Duration,
};
use windows_sys::Win32::{
    Storage::FileSystem::FILE_SHARE_READ,
    System::{ApplicationInstallationAndServicing::*, SystemInformation::GetSystemDirectoryW},
};

const UPGRADE_CODE: &str = "{E6C58A32-82A8-4D49-9B7C-55F97B948E1A}";

struct MsiHandle(u32);
impl Drop for MsiHandle {
    fn drop(&mut self) {
        unsafe {
            MsiCloseHandle(self.0);
        }
    }
}

fn query(db: u32, sql: &str) -> Result<Option<String>, String> {
    let query = wide(OsStr::new(sql));
    let mut handle = 0;
    let code = unsafe { MsiDatabaseOpenViewW(db, query.as_ptr(), &mut handle) };
    if code != 0 {
        return Err(format!("Cannot inspect MSI database ({code})"));
    }
    let view = MsiHandle(handle);
    let code = unsafe { MsiViewExecute(view.0, 0) };
    if code != 0 {
        return Err(format!("Cannot execute MSI metadata query ({code})"));
    }
    let mut record = 0;
    match unsafe { MsiViewFetch(view.0, &mut record) } {
        259 => return Ok(None), // ERROR_NO_MORE_ITEMS
        0 => {}
        code => return Err(format!("Cannot fetch MSI metadata ({code})")),
    }
    let record = MsiHandle(record);
    let mut buf = [0u16; 4096];
    let mut len = buf.len() as u32;
    let code = unsafe { MsiRecordGetStringW(record.0, 1, buf.as_mut_ptr(), &mut len) };
    if code != 0 || len as usize >= buf.len() {
        return Err("Invalid or oversized MSI metadata".into());
    }
    String::from_utf16(&buf[..len as usize])
        .map(Some)
        .map_err(|_| "Invalid MSI text".into())
}

pub(super) fn validate_package(path: &Path, version: &str) -> Result<(), String> {
    let parsed = semver::Version::parse(version).map_err(|_| "Invalid MSI version")?;
    if !parsed.pre.is_empty()
        || !parsed.build.is_empty()
        || parsed.major > 255
        || parsed.minor > 255
        || parsed.patch > 65535
    {
        return Err("Invalid Windows Installer version".into());
    }
    let path = wide(path.as_os_str());
    let mut handle = 0;
    // MSIDBOPEN_READONLY is a null pointer. Never open/execute an installation
    // session just to inspect untrusted downloaded metadata.
    let code = unsafe { MsiOpenDatabaseW(path.as_ptr(), std::ptr::null(), &mut handle) };
    if code != 0 {
        return Err(format!("Cannot open downloaded MSI database ({code})"));
    }
    let db = MsiHandle(handle);
    let property = |key: &str| {
        query(
            db.0,
            &format!("SELECT `Value` FROM `Property` WHERE `Property`='{key}'"),
        )
    };
    validate_properties(
        property("ProductName")?.as_deref(),
        property("UpgradeCode")?.as_deref(),
        property("ProductVersion")?.as_deref(),
        property("ALLUSERS")?.as_deref(),
        version,
    )?;
    let folder = query(
        db.0,
        "SELECT `DefaultDir` FROM `Directory` WHERE `Directory`='INSTALLFOLDER'",
    )?;
    let parent = query(
        db.0,
        "SELECT `Directory_Parent` FROM `Directory` WHERE `Directory`='INSTALLFOLDER'",
    )?;
    if folder
        .as_deref()
        .and_then(|s| s.split(':').next())
        .and_then(|s| s.rsplit('|').next())
        != Some("claude-mascot-rust")
        || parent.as_deref() != Some("LocalAppDataFolder")
    {
        return Err("MSI installation directory does not match this application".into());
    }
    Ok(())
}

fn validate_properties(
    name: Option<&str>,
    upgrade: Option<&str>,
    version: Option<&str>,
    all_users: Option<&str>,
    expected: &str,
) -> Result<(), String> {
    if name != Some("Claude Mascot Rust")
        || !upgrade.is_some_and(|v| v.eq_ignore_ascii_case(UPGRADE_CODE))
    {
        return Err("MSI belongs to a different product family".into());
    }
    if version != Some(expected) {
        return Err("MSI version differs from verified release metadata".into());
    }
    if all_users.is_some_and(|s| !s.is_empty()) {
        return Err("Automatic MSI updates must remain per-user".into());
    }
    Ok(())
}

pub(super) fn validate_target(target: &Path) -> Result<(), String> {
    let expected = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .ok_or("LOCALAPPDATA is unavailable")?
        .join("claude-mascot-rust")
        .join("claude-mascot.exe");
    if !super::same_path(target, &expected)
        || !target
            .parent()
            .is_some_and(|p| p.join("install-mode-msi.marker").is_file())
    {
        return Err("MSI update target is not this user's installed application".into());
    }
    Ok(())
}

fn system_installer() -> Result<PathBuf, String> {
    let mut buf = [0u16; 32768];
    let len = unsafe { GetSystemDirectoryW(buf.as_mut_ptr(), buf.len() as u32) } as usize;
    if len == 0 || len >= buf.len() {
        return Err("Cannot locate Windows Installer".into());
    }
    Ok(PathBuf::from(String::from_utf16_lossy(&buf[..len])).join("msiexec.exe"))
}

/// Rust canonical paths use the extended Win32 prefix, which msiexec's command
/// line parser rejects (1619), even though read-only MSI database APIs accept it.
fn installer_argument(path: &Path) -> OsString {
    use std::os::windows::ffi::{OsStrExt, OsStringExt};
    let units: Vec<u16> = path.as_os_str().encode_wide().collect();
    let unc: Vec<u16> = "\\\\?\\UNC\\".encode_utf16().collect();
    let prefix: Vec<u16> = "\\\\?\\".encode_utf16().collect();
    if units.starts_with(&unc) {
        let mut normal: Vec<u16> = "\\\\".encode_utf16().collect();
        normal.extend_from_slice(&units[unc.len()..]);
        OsString::from_wide(&normal)
    } else if units.starts_with(&prefix) {
        OsString::from_wide(&units[prefix.len()..])
    } else {
        path.as_os_str().to_owned()
    }
}

pub(super) fn install_and_restart(
    plan: &ApplyPlan,
    package: &Path,
    target: &Path,
    old_hash: &str,
) -> Result<(), String> {
    let expected_exe = plan
        .executable_sha256
        .as_deref()
        .ok_or("Missing installed executable digest")?;
    validate_digest(expected_exe)?;
    // Deny writes/deletes to the verified package until Windows Installer exits.
    let log = package
        .parent()
        .ok_or("Missing staging directory")?
        .join("installer.log");
    let result = (|| {
        let _package_lock = OpenOptions::new()
            .read(true)
            .share_mode(FILE_SHARE_READ)
            .open(package)
            .map_err(|e| format!("Cannot lock MSI package: {e}"))?;
        super::verify_payload(
            package,
            &plan.sha256,
            plan.kind,
            &plan.version,
            Some(expected_exe),
        )?;
        let installer = system_installer()?;
        let package_arg = installer_argument(package);
        let log_arg = installer_argument(&log);
        let mut exit = 1618;
        for attempt in 0..4 {
            let status = Command::new(&installer)
                .args([
                    OsStr::new("/i"),
                    package_arg.as_os_str(),
                    OsStr::new("/qn"),
                    OsStr::new("/norestart"),
                    OsStr::new("/L*v"),
                    log_arg.as_os_str(),
                ])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .creation_flags(0x0800_0000)
                .status()
                .map_err(|e| format!("Cannot start Windows Installer: {e}"))?;
            exit = status
                .code()
                .ok_or("Windows Installer exited without a status")?;
            if exit != 1618 || attempt == 3 {
                break;
            }
            thread::sleep(Duration::from_secs(5));
        }
        if !matches!(exit, 0 | 3010) {
            return Err(format!(
                "Windows Installer failed ({exit}); log: {}",
                log.display()
            ));
        }
        if !hash_file(target)?.eq_ignore_ascii_case(expected_exe) {
            return Err("Installed EXE differs from the verified release asset".into());
        }
        Ok(())
    })();
    let status = match &result {
        Ok(()) => "Обновление установлено".to_string(),
        Err(e) => format!("Не удалось установить обновление: {e}"),
    };
    if let Some(parent) = target.parent() {
        let _ = std::fs::write(
            parent.join("update-result.json"),
            serde_json::to_vec(
                &serde_json::json!({"ok":result.is_ok(),"version":plan.version,"message":status}),
            )
            .unwrap_or_default(),
        );
    }
    // Installer rollback owns recovery. Never replace MSI files ourselves.
    // Relaunch only either the verified new EXE or the exact old executable.
    if hash_file(target).is_ok_and(|h| h.eq_ignore_ascii_case(expected_exe) || h == old_hash) {
        launch(target, &plan.restart_args)?;
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn installer_command_accepts_drive_unc_spaces_and_unicode_paths() {
        assert_eq!(
            installer_argument(Path::new(r"\\?\C:\Users\Имя с пробелом\new.msi")),
            OsStr::new(r"C:\Users\Имя с пробелом\new.msi")
        );
        assert_eq!(
            installer_argument(Path::new(r"\\?\UNC\server\share\new.msi")),
            OsStr::new(r"\\server\share\new.msi")
        );
        assert_eq!(
            installer_argument(Path::new(r"C:\normal\new.msi")),
            OsStr::new(r"C:\normal\new.msi")
        );
    }
    #[test]
    fn refuses_wrong_product_version_and_machine_scope() {
        let good = (
            Some("Claude Mascot Rust"),
            Some(UPGRADE_CODE),
            Some("1.2.3"),
            None,
            "1.2.3",
        );
        assert!(validate_properties(good.0, good.1, good.2, good.3, good.4).is_ok());
        assert!(validate_properties(Some("Other"), good.1, good.2, None, good.4).is_err());
        assert!(validate_properties(good.0, Some("{OTHER}"), good.2, None, good.4).is_err());
        assert!(validate_properties(good.0, good.1, Some("1.2.2"), None, good.4).is_err());
        assert!(validate_properties(good.0, good.1, good.2, Some("1"), good.4).is_err());
    }
    #[test]
    #[ignore = "requires a built release MSI in MASCOT_TEST_MSI_PATH"]
    fn validates_real_release_package() {
        let path = std::env::var_os("MASCOT_TEST_MSI_PATH").expect("Set MASCOT_TEST_MSI_PATH");
        let version = std::env::var("MASCOT_TEST_MSI_VERSION")
            .unwrap_or_else(|_| env!("CARGO_PKG_VERSION").into());
        let path = std::fs::canonicalize(path).unwrap();
        validate_package(&path, &version).unwrap();
        assert!(validate_package(&path, "99.99.99").is_err());
    }
}
