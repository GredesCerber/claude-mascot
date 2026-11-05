//! Minimal Windows-side updater helper. The old executable never replaces itself:
//! it copies this helper to the private staging directory, starts it, and exits.
#![cfg(windows)]

use crate::update::StagedUpdate;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    ffi::OsStr,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::{Duration, Instant},
};
use windows_sys::Win32::{
    Foundation::{
        CloseHandle, GetLastError, ERROR_INVALID_PARAMETER, HANDLE, WAIT_OBJECT_0, WAIT_TIMEOUT,
    },
    Storage::FileSystem::{MoveFileExW, MOVEFILE_WRITE_THROUGH, SYNCHRONIZE},
    System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, WaitForSingleObject,
        PROCESS_QUERY_LIMITED_INFORMATION,
    },
};

const WAIT_LIMIT: Duration = Duration::from_secs(30);
const COPY_BUFFER: usize = 64 * 1024;
const MAX_PLAN_SIZE: u64 = 64 * 1024;
const MAX_EXECUTABLE_SIZE: u64 = 64 * 1024 * 1024;
static NEXT_TEMP: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ApplyPlan {
    pub target: PathBuf,
    pub staged: PathBuf,
    pub sha256: String,
    pub version: String,
    pub parent_pid: u32,
    pub restart_args: Vec<String>,
}

/// Start a copied helper, then let the caller close its windows and exit.
pub fn schedule(
    staged: &StagedUpdate,
    target: &Path,
    restart_args: &[String],
) -> Result<(), String> {
    validate_digest(&staged.sha256)?;
    let stage_file = fs::canonicalize(&staged.executable)
        .map_err(|e| format!("Cannot resolve staged update: {e}"))?;
    let stage_dir = stage_file
        .parent()
        .ok_or("Staged update has no parent directory")?
        .to_path_buf();
    let target = absolute_existing_file(target)?;
    ensure_disjoint(&stage_dir, &stage_file, &target)?;
    let running = fs::canonicalize(std::env::current_exe().map_err(|e| e.to_string())?)
        .map_err(|e| format!("Cannot resolve current executable: {e}"))?;
    if !same_path(&running, &target) {
        return Err("Update target must be the currently running executable".into());
    }

    verify_staged(&stage_file, &staged.sha256)?;

    let helper = stage_dir.join("updater.exe");
    copy_create_new(
        &std::env::current_exe().map_err(|e| e.to_string())?,
        &helper,
        MAX_EXECUTABLE_SIZE,
    )?;
    let plan = ApplyPlan {
        target,
        staged: stage_file,
        sha256: staged.sha256.to_ascii_lowercase(),
        version: staged.version.clone(),
        parent_pid: std::process::id(),
        restart_args: restart_args.to_vec(),
    };
    let plan_path = unique_path(&stage_dir, "update-plan", "json");
    if let Err(e) = write_plan_create_new(&plan_path, &plan) {
        let _ = fs::remove_file(&helper);
        return Err(e);
    }

    let mut command = Command::new(&helper);
    command
        .arg("--apply-update")
        .arg(&plan_path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    command.creation_flags(CREATE_NO_WINDOW);
    if let Err(e) = command.spawn() {
        let _ = fs::remove_file(&plan_path);
        let _ = fs::remove_file(&helper);
        return Err(format!("Cannot start update helper: {e}"));
    }
    Ok(())
}

/// Validate and apply a plan. Called by the copied helper before normal startup.
pub fn apply(plan_path: &Path) -> Result<(), String> {
    let result = apply_inner(plan_path);
    if let Err(error) = &result {
        if let Ok(helper) = std::env::current_exe() {
            if let Some(stage_dir) = helper.parent() {
                let log = stage_dir.join("update-error.log");
                if let Ok(mut file) = OpenOptions::new().write(true).create_new(true).open(log) {
                    let _ = writeln!(file, "{error}");
                    let _ = file.sync_all();
                }
            }
        }
    }
    result
}

fn apply_inner(plan_path: &Path) -> Result<(), String> {
    let helper = fs::canonicalize(std::env::current_exe().map_err(|e| e.to_string())?)
        .map_err(|e| format!("Cannot resolve update helper: {e}"))?;
    let plan_path =
        fs::canonicalize(plan_path).map_err(|e| format!("Cannot resolve update plan: {e}"))?;
    let stage_dir = helper
        .parent()
        .ok_or("Update helper has no parent directory")?
        .to_path_buf();
    let plan_len = fs::metadata(&plan_path)
        .map_err(|e| format!("Cannot inspect update plan: {e}"))?
        .len();
    if plan_len > MAX_PLAN_SIZE {
        return Err("Update plan exceeds 64 KiB".into());
    }
    let mut plan_file =
        File::open(&plan_path).map_err(|e| format!("Cannot read update plan: {e}"))?;
    let mut plan_bytes = Vec::with_capacity(plan_len as usize);
    std::io::Read::by_ref(&mut plan_file)
        .take(MAX_PLAN_SIZE + 1)
        .read_to_end(&mut plan_bytes)
        .map_err(|e| format!("Cannot read update plan: {e}"))?;
    if plan_bytes.len() as u64 > MAX_PLAN_SIZE {
        return Err("Update plan exceeds 64 KiB".into());
    }
    let plan: ApplyPlan =
        serde_json::from_slice(&plan_bytes).map_err(|e| format!("Invalid update plan: {e}"))?;
    validate_digest(&plan.sha256)?;
    if plan.version.trim().is_empty() || plan.version.len() > 128 {
        return Err("Update version metadata is invalid".into());
    }
    let target = absolute_existing_file(&plan.target)?;
    let staged =
        fs::canonicalize(&plan.staged).map_err(|e| format!("Cannot resolve staged update: {e}"))?;
    validate_layout(&helper, &plan_path, &stage_dir, &staged, &target)?;
    verify_staged(&staged, &plan.sha256)?;

    wait_for_parent_exit(plan.parent_pid, &target)?;
    let old_sha256 = hash_file(&target)?;
    let replacement = match replace_transaction(&staged, &target, &plan.sha256) {
        Ok(replacement) => replacement,
        Err(error) => {
            let resumed = if hash_file(&target).is_ok_and(|hash| hash == old_sha256) {
                launch(&target, &plan.restart_args)
                    .map_err(|e| format!("; could not relaunch previous executable: {e}"))
            } else {
                Err("; previous executable is unavailable at its original path".into())
            };
            return match resumed {
                Ok(()) => Err(format!("{error}; previous executable restarted")),
                Err(resume_error) => Err(format!("{error}{resume_error}")),
            };
        }
    };
    match launch(&target, &plan.restart_args) {
        Ok(()) => Ok(()),
        Err(start_error) => match rollback(&target, &replacement.backup, &plan.sha256) {
            Ok(()) => {
                let _ = launch(&target, &plan.restart_args);
                Err(format!("Cannot start updated app ({start_error}); restored previous executable"))
            }
            Err(rollback_error) => Err(format!(
                "Cannot start updated app ({start_error}); rollback failed ({rollback_error}); previous executable remains at {}",
                replacement.backup.display()
            )),
        },
    }
}

struct Replacement {
    backup: PathBuf,
}

/// Copy beside the target, then rename the old and new files without ever
/// replacing a pre-existing backup or temporary file.
fn replace_transaction(
    staged: &Path,
    target: &Path,
    expected_sha256: &str,
) -> Result<Replacement, String> {
    let parent = target
        .parent()
        .ok_or("Target executable has no parent directory")?;
    let temp = unique_path(parent, "mascot-new", "exe");
    copy_create_new(staged, &temp, MAX_EXECUTABLE_SIZE)?;
    if let Err(error) = verify_staged(&temp, expected_sha256) {
        let _ = fs::remove_file(&temp);
        return Err(format!("Copied update failed verification: {error}"));
    }

    let backup = unique_path(parent, "mascot-backup", "exe");
    if let Err(e) = move_no_replace(target, &backup) {
        let _ = fs::remove_file(&temp);
        return Err(format!("Cannot preserve current executable: {e}"));
    }
    if let Err(install_error) = move_no_replace(&temp, target) {
        let rollback = move_no_replace(&backup, target);
        let _ = fs::remove_file(&temp);
        return match rollback {
            Ok(()) => Err(format!("Cannot install staged executable ({install_error}); previous executable restored")),
            Err(e) => Err(format!("Cannot install staged executable ({install_error}); automatic rollback failed ({e}); previous executable remains at {}", backup.display())),
        };
    }
    Ok(Replacement { backup })
}

fn rollback(target: &Path, backup: &Path, installed_sha256: &str) -> Result<(), String> {
    if hash_file(target)? != installed_sha256.to_ascii_lowercase() {
        return Err("target changed after replacement; refusing to remove it".into());
    }
    fs::remove_file(target).map_err(|e| format!("Cannot remove failed new executable: {e}"))?;
    move_no_replace(backup, target)
}

fn wait_for_parent_exit(pid: u32, target: &Path) -> Result<(), String> {
    if pid == 0 {
        return Err("Update plan has an invalid parent process ID".into());
    }
    let handle = unsafe { OpenProcess(SYNCHRONIZE | PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if handle.is_null() {
        let error = unsafe { GetLastError() };
        if error == ERROR_INVALID_PARAMETER {
            return Ok(()); // Parent already exited; the PID no longer exists.
        }
        return Err(format!("Cannot inspect update parent process ({error})"));
    }
    let _handle = OwnedHandle(handle);
    let image = process_image(handle)?;
    if !same_path(&image, target) {
        return Err("Update parent PID belongs to a different executable".into());
    }
    let start = Instant::now();
    loop {
        let elapsed = start.elapsed();
        if elapsed >= WAIT_LIMIT {
            return Err("Timed out waiting for the old application to exit".into());
        }
        let remaining = WAIT_LIMIT - elapsed;
        let wait_ms = remaining.as_millis().clamp(1, 1000) as u32;
        match unsafe { WaitForSingleObject(handle, wait_ms) } {
            WAIT_OBJECT_0 => return Ok(()),
            WAIT_TIMEOUT => thread::sleep(Duration::from_millis(25)),
            other => return Err(format!("Waiting for the old application failed ({other})")),
        }
    }
}

struct OwnedHandle(HANDLE);
impl Drop for OwnedHandle {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { CloseHandle(self.0) };
        }
    }
}

fn process_image(process: HANDLE) -> Result<PathBuf, String> {
    let mut buf = vec![0u16; 32768];
    let mut len = buf.len() as u32;
    if unsafe { QueryFullProcessImageNameW(process, 0, buf.as_mut_ptr(), &mut len) } == 0 {
        return Err(format!(
            "Cannot read update parent image path ({})",
            unsafe { GetLastError() }
        ));
    }
    let path = PathBuf::from(String::from_utf16_lossy(&buf[..len as usize]));
    fs::canonicalize(path).map_err(|e| format!("Cannot resolve update parent image path: {e}"))
}

fn launch(target: &Path, args: &[String]) -> Result<(), String> {
    let mut command = Command::new(target);
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    command.creation_flags(CREATE_NO_WINDOW);
    command
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("process launch failed: {e}"))
}

fn absolute_existing_file(path: &Path) -> Result<PathBuf, String> {
    if !path.is_absolute() {
        return Err("Executable path must be absolute".into());
    }
    let resolved =
        fs::canonicalize(path).map_err(|e| format!("Cannot resolve executable path: {e}"))?;
    if !resolved.is_file()
        || !resolved
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("exe"))
    {
        return Err("Executable target must be an existing .exe file".into());
    }
    Ok(resolved)
}

fn ensure_disjoint(stage_dir: &Path, staged: &Path, target: &Path) -> Result<(), String> {
    if same_path(stage_dir, target)
        || same_path(staged, target)
        || same_path(&stage_dir.join("updater.exe"), target)
    {
        return Err("Update target must be separate from the staging files".into());
    }
    Ok(())
}

fn validate_layout(
    helper: &Path,
    plan: &Path,
    stage_dir: &Path,
    staged: &Path,
    target: &Path,
) -> Result<(), String> {
    if !same_path(
        plan.parent().ok_or("Update plan has no parent directory")?,
        stage_dir,
    ) {
        return Err("Update plan must be inside the helper's staging directory".into());
    }
    if !same_path(helper, &stage_dir.join("updater.exe")) {
        return Err("Update helper must be updater.exe in the staging directory".into());
    }
    if !same_path(
        staged
            .parent()
            .ok_or("Staged update has no parent directory")?,
        stage_dir,
    ) {
        return Err("Staged update must be in the helper's staging directory".into());
    }
    ensure_disjoint(stage_dir, staged, target)
}

fn same_path(a: &Path, b: &Path) -> bool {
    let left = fs::canonicalize(a).unwrap_or_else(|_| a.to_path_buf());
    let right = fs::canonicalize(b).unwrap_or_else(|_| b.to_path_buf());
    left.to_string_lossy()
        .eq_ignore_ascii_case(&right.to_string_lossy())
}

fn validate_digest(digest: &str) -> Result<(), String> {
    if digest.len() != 64 || !digest.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("Update SHA-256 must be 64 hexadecimal characters".into());
    }
    Ok(())
}

fn hash_file(path: &Path) -> Result<String, String> {
    let mut file = File::open(path).map_err(|e| format!("Cannot open {}: {e}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; COPY_BUFFER];
    loop {
        let count = file.read(&mut buf).map_err(|e| e.to_string())?;
        if count == 0 {
            break;
        }
        hasher.update(&buf[..count]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn verify_pe(path: &Path) -> Result<(), String> {
    use std::io::{Seek, SeekFrom};
    let mut file = File::open(path).map_err(|e| e.to_string())?;
    let file_len = file.metadata().map_err(|e| e.to_string())?.len();
    let mut dos = [0u8; 64];
    file.read_exact(&mut dos)
        .map_err(|_| "Staged file is not a valid Windows executable".to_string())?;
    if &dos[..2] != b"MZ" {
        return Err("Staged file is not a Windows PE executable".into());
    }
    let pe_offset = u32::from_le_bytes(dos[60..64].try_into().unwrap()) as u64;
    if pe_offset < 64 || pe_offset.checked_add(24).is_none_or(|end| end > file_len) {
        return Err("Staged file has an invalid PE header offset".into());
    }
    file.seek(SeekFrom::Start(pe_offset))
        .map_err(|e| e.to_string())?;
    let mut coff = [0u8; 6];
    file.read_exact(&mut coff)
        .map_err(|_| "Staged file has a truncated PE header".to_string())?;
    if &coff[..4] != b"PE\0\0" {
        return Err("Staged file has an invalid PE signature".into());
    }
    if u16::from_le_bytes([coff[4], coff[5]]) != 0x8664 {
        return Err("Staged executable must target Windows x64".into());
    }
    Ok(())
}

fn verify_staged(path: &Path, expected_sha256: &str) -> Result<(), String> {
    validate_digest(expected_sha256)?;
    let size = fs::metadata(path)
        .map_err(|e| format!("Cannot inspect staged update: {e}"))?
        .len();
    if size == 0 || size > MAX_EXECUTABLE_SIZE {
        return Err("Staged executable size must be between 1 byte and 64 MiB".into());
    }
    if hash_file(path)? != expected_sha256.to_ascii_lowercase() {
        return Err("Staged update SHA-256 does not match its plan".into());
    }
    verify_pe(path)
}

fn unique_path(dir: &Path, prefix: &str, suffix: &str) -> PathBuf {
    loop {
        let n = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
        let name = format!("{prefix}-{}-{n}.{suffix}", std::process::id());
        let candidate = dir.join(name);
        if !candidate.exists() {
            return candidate;
        }
    }
}

fn copy_create_new(source: &Path, destination: &Path, max_bytes: u64) -> Result<(), String> {
    let mut input =
        File::open(source).map_err(|e| format!("Cannot open {}: {e}", source.display()))?;
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)
        .map_err(|e| {
            format!(
                "Cannot create {} without overwriting: {e}",
                destination.display()
            )
        })?;
    let result = (|| {
        let mut buf = [0u8; COPY_BUFFER];
        let mut copied = 0u64;
        loop {
            let count = input.read(&mut buf).map_err(|e| e.to_string())?;
            if count == 0 {
                break;
            }
            copied = copied
                .checked_add(count as u64)
                .ok_or_else(|| "File size overflow while copying".to_string())?;
            if copied > max_bytes {
                return Err(format!("File exceeds {max_bytes} byte copy limit"));
            }
            output.write_all(&buf[..count]).map_err(|e| e.to_string())?;
        }
        output.sync_all().map_err(|e| e.to_string())
    })();
    if result.is_err() {
        let _ = fs::remove_file(destination);
    }
    result.map_err(|e| format!("Cannot copy staged executable: {e}"))
}

fn write_plan_create_new(path: &Path, plan: &ApplyPlan) -> Result<(), String> {
    let data = serde_json::to_vec(plan).map_err(|e| e.to_string())?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|e| e.to_string())?;
    file.write_all(&data)
        .and_then(|_| file.sync_all())
        .map_err(|e| format!("Cannot write update plan: {e}"))
}

fn move_no_replace(from: &Path, to: &Path) -> Result<(), String> {
    let from_w = wide(from.as_os_str());
    let to_w = wide(to.as_os_str());
    // SAFETY: both nul-terminated UTF-16 buffers remain alive for the call.
    let ok = unsafe { MoveFileExW(from_w.as_ptr(), to_w.as_ptr(), MOVEFILE_WRITE_THROUGH) };
    if ok == 0 {
        Err(format!(
            "{} -> {} failed ({})",
            from.display(),
            to.display(),
            unsafe { GetLastError() }
        ))
    } else {
        Ok(())
    }
}

fn wide(value: &OsStr) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    value.encode_wide().chain(std::iter::once(0)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(1);

    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let dir = std::env::temp_dir().join(format!(
                "mascot-update-test-{}-{}",
                std::process::id(),
                NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }
        fn file(&self, name: &str, bytes: &[u8]) -> PathBuf {
            let path = self.0.join(name);
            let mut f = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
                .unwrap();
            f.write_all(bytes).unwrap();
            f.sync_all().unwrap();
            path
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn digest_shape_and_pe_are_checked() {
        assert!(validate_digest(&"a".repeat(64)).is_ok());
        assert!(validate_digest("not-a-hash").is_err());
        let f = Fixture::new();
        let pe = f.file("test.exe", &minimal_pe());
        assert!(verify_pe(&pe).is_ok());
        let not_pe = f.file("not.exe", b"not an executable");
        assert!(verify_pe(&not_pe).is_err());
        let mut x86 = minimal_pe();
        x86[68..70].copy_from_slice(&0x014cu16.to_le_bytes());
        let x86 = f.file("x86.exe", &x86);
        assert!(verify_pe(&x86).is_err());
        let correct_hash = hash_file(&pe).unwrap();
        assert!(verify_staged(&pe, &correct_hash).is_ok());
        assert!(verify_staged(&pe, &"0".repeat(64)).is_err());
    }

    #[test]
    fn install_keeps_old_executable_as_unique_backup() {
        let f = Fixture::new();
        let stage = f.file("staged.exe", &minimal_pe());
        let target = f.file("app.exe", b"old executable");
        let before_hash = hash_file(&stage).unwrap();
        let installed = replace_transaction(&stage, &target, &before_hash).unwrap();
        assert_eq!(hash_file(&target).unwrap(), before_hash);
        assert_eq!(fs::read(&installed.backup).unwrap(), b"old executable");
        assert!(installed.backup.exists());
    }

    #[test]
    fn rename_never_overwrites_existing_destination() {
        let f = Fixture::new();
        let source = f.file("source.bin", b"new");
        let destination = f.file("existing.bin", b"user data");
        assert!(move_no_replace(&source, &destination).is_err());
        assert_eq!(fs::read(&destination).unwrap(), b"user data");
        assert_eq!(fs::read(&source).unwrap(), b"new");
    }

    #[test]
    fn staging_and_target_paths_must_be_separate() {
        let f = Fixture::new();
        let stage_dir = f.0.join("stage");
        fs::create_dir(&stage_dir).unwrap();
        let staged = f.file("stage/staged.exe", &minimal_pe());
        let target = f.file("app.exe", b"old executable");
        assert!(ensure_disjoint(&stage_dir, &staged, &target).is_ok());
        assert!(ensure_disjoint(&stage_dir, &staged, &staged).is_err());
        let helper = f.file("stage/updater.exe", b"helper");
        let in_stage_plan = f.file("stage/plan.json", b"{}");
        assert!(validate_layout(&helper, &in_stage_plan, &stage_dir, &staged, &target).is_ok());
        let outside_plan = f.file("outside-plan.json", b"{}");
        assert!(validate_layout(&helper, &outside_plan, &stage_dir, &staged, &target).is_err());
        let outside_helper = f.file("other-updater.exe", b"helper");
        assert!(validate_layout(
            &outside_helper,
            &in_stage_plan,
            &stage_dir,
            &staged,
            &target
        )
        .is_err());
    }

    #[test]
    fn rollback_restores_previous_executable() {
        let f = Fixture::new();
        let stage = f.file("staged.exe", &minimal_pe());
        let target = f.file("app.exe", b"old executable");
        let staged_hash = hash_file(&stage).unwrap();
        let installed = replace_transaction(&stage, &target, &staged_hash).unwrap();
        rollback(&target, &installed.backup, &staged_hash).unwrap();
        assert_eq!(fs::read(&target).unwrap(), b"old executable");
        assert!(!installed.backup.exists());
    }

    fn minimal_pe() -> Vec<u8> {
        let mut b = vec![0u8; 88];
        b[..2].copy_from_slice(b"MZ");
        b[60..64].copy_from_slice(&64u32.to_le_bytes());
        b[64..68].copy_from_slice(b"PE\0\0");
        b[68..70].copy_from_slice(&0x8664u16.to_le_bytes());
        b
    }
}
