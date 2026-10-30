// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (c) 2026 Daniele Briguglio, superkali@armbian.com

//! First-boot settings: per-flash working copies of an image, with the bundle of [`crate::provision`] laid onto them.
//! QDL images are the exception: they still take a preset at `/root/.not_logged_in_yet`, see [`inject_into_bare_ext4_image`].

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use armbian_write_conf::{write_file_into_bare_ext4_image, WriteConfError};

use crate::config;
use crate::flash::FlashState;
use crate::provision::{self, ImageSupport, ProvisionError};
use crate::utils::{leftover_is_stale, sweep_dir, unique_suffix};
use crate::{log_error, log_info, log_warn};

pub use crate::provision::AutoconfigConfig;

/// A profile was requested for an image without Armbian's first-run service (twin in errorUtils.ts).
pub const TAG_NOT_ARMBIAN: &str = "[AUTOCONFIG_NOT_ARMBIAN]";

/// A profile was requested for an image that has already booted, so the first-run service will not start again.
pub const TAG_ALREADY_BOOTED: &str = "[AUTOCONFIG_ALREADY_BOOTED]";

// Upstream's marker probe is gone (see provision::probe_image); the constant stays for a smaller diff in config.
const _: &str = config::autoconfig::FIRSTLOGIN_MARKER;

/// Quote a value for a bash-sourced file: wrap in double quotes and escape the
/// chars that are special inside double quotes (backslash, quote, dollar, backtick).
fn shell_quote(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for ch in value.chars() {
        match ch {
            '\\' | '"' | '$' | '`' => {
                out.push('\\');
                out.push(ch);
            }
            _ => out.push(ch),
        }
    }
    out.push('"');
    out
}

fn push_trimmed(out: &mut String, key: &str, value: &Option<String>) {
    if let Some(v) = value.as_deref().map(str::trim).filter(|v| !v.is_empty()) {
        out.push_str(key);
        out.push('=');
        out.push_str(&shell_quote(v));
        out.push('\n');
    }
}

/// The preset for a QDL image. Profiles no longer hold a password or a Wi-Fi key in the clear, and a hash in a
/// `PRESET_*` password field would become the password itself, so only the plain settings can go there.
fn render_qdl_preset(config: &AutoconfigConfig) -> String {
    let mut out = String::new();
    push_trimmed(&mut out, "PRESET_LOCALE", &config.locale);
    push_trimmed(&mut out, "PRESET_TIMEZONE", &config.timezone);
    push_trimmed(&mut out, "PRESET_USER_NAME", &config.user_name);
    if let Some(shell) = &config.user_shell {
        out.push_str("PRESET_USER_SHELL=");
        out.push_str(&shell_quote(shell.as_str()));
        out.push('\n');
    }
    push_trimmed(&mut out, "PRESET_DEFAULT_REALNAME", &config.user_real_name);

    // Decline "Connect via wireless? [Y/n]": unattended first login would hang on it offline (#193).
    if !out.is_empty() {
        out.push_str("PRESET_CONNECT_WIRELESS=\"n\"\n");
    }

    out
}

/// Render the preset and write it into a BARE ext4 image (no partition table; e.g. Armbian QDL `disk-sdcard.img.root`);
/// `image_path` must be a flat ext4 filesystem that will be mutated. Never logs secret values (password/wifi key).
pub fn inject_into_bare_ext4_image(
    image_path: &Path,
    config: &AutoconfigConfig,
) -> Result<(), WriteConfError> {
    restrict_to_owner(image_path, config::autoconfig::FILE_MODE).map_err(WriteConfError::Io)?;
    let preset = render_qdl_preset(config);
    log_info!(
        "autoconfig",
        "Injecting first-boot preset ({} bytes) into bare ext4 image {}",
        preset.len(),
        image_path.display()
    );

    match write_file_into_bare_ext4_image(
        image_path,
        config::autoconfig::PRESET_PATH,
        preset.as_bytes(),
    ) {
        Ok(report) => {
            log_info!(
                "autoconfig",
                "Preset written to {} ({} scheme, {} bytes, validated: {})",
                report.dest_path,
                report.scheme,
                report.bytes_written,
                report.validated
            );
            Ok(())
        }
        Err(e) => {
            log_error!("autoconfig", "Failed to inject preset: {}", e);
            Err(e)
        }
    }
}

#[cfg(unix)]
fn restrict_to_owner(path: &Path, mode: u32) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))
}

// Elsewhere the per-user cache directory's ACL applies.
#[cfg(not(unix))]
fn restrict_to_owner(_path: &Path, _mode: u32) -> std::io::Result<()> {
    Ok(())
}

/// A per-flash copy of an image with the settings on it; the file is removed on drop.
pub struct WorkingCopy {
    path: PathBuf,
}

impl WorkingCopy {
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for WorkingCopy {
    fn drop(&mut self) {
        if let Err(e) = std::fs::remove_file(&self.path) {
            log_warn!(
                "autoconfig",
                "Failed to remove autoconfig working copy {}: {}",
                self.path.display(),
                e
            );
        }
    }
}

/// The profile a flash applies: one with nothing set is no profile, so the image goes out byte for byte.
pub fn effective_profile(profile: Option<AutoconfigConfig>) -> Option<AutoconfigConfig> {
    profile.filter(|p| !provision::is_empty(p))
}

fn no_ext4_rootfs(detail: &str) -> String {
    format!(
        "This image does not have a writable ext4 root filesystem, so the selected autoconfig profile cannot be applied: {}",
        detail
    )
}

fn support_verdict(probe: Result<ImageSupport, ProvisionError>) -> Result<(), String> {
    match probe {
        Ok(ImageSupport::Supported) => Ok(()),
        Ok(ImageSupport::NotExt4) => Err(no_ext4_rootfs("no ext4 partition found")),
        Ok(ImageSupport::UnsupportedFs(why)) => Err(no_ext4_rootfs(&why)),
        Ok(ImageSupport::NoFirstrunService) => Err(format!(
            "{TAG_NOT_ARMBIAN} This image has no Armbian first-run service, so the selected autoconfig profile cannot be applied"
        )),
        Ok(ImageSupport::AlreadyBooted) => Err(format!(
            "{TAG_ALREADY_BOOTED} This image has already been booted, so the selected autoconfig profile cannot be applied"
        )),
        Err(e) => Err(format!("Failed to read the image for autoconfig: {}", e)),
    }
}

/// Refuse a profile for an image that could not apply it, before any copy or write.
pub fn require_supported_image(image_path: &Path) -> Result<(), String> {
    let probe = provision::probe_image(image_path);
    match &probe {
        Ok(support) => log_info!(
            "autoconfig",
            "Settings support of {}: {:?}",
            image_path.display(),
            support
        ),
        Err(e) => log_warn!(
            "autoconfig",
            "Settings support check failed for {}: {}",
            image_path.display(),
            e
        ),
    }
    support_verdict(probe)
}

fn create_private_copy(source: &Path, temp_dir: &Path) -> Result<WorkingCopy, String> {
    // set_permissions follows links, so a linked dir would hand the chmod to its target.
    if std::fs::symlink_metadata(temp_dir).is_ok_and(|m| m.file_type().is_symlink()) {
        return Err("The autoconfig temp directory is a link; refusing to use it".to_string());
    }
    std::fs::create_dir_all(temp_dir)
        .map_err(|e| format!("Failed to create autoconfig temp directory: {}", e))?;
    restrict_to_owner(temp_dir, config::autoconfig::DIR_MODE)
        .map_err(|e| format!("Failed to protect autoconfig temp directory: {}", e))?;

    let stem = source.file_name().map_or_else(
        || config::autoconfig::COPY_FALLBACK_NAME.into(),
        |n| n.to_string_lossy(),
    );
    let copy = WorkingCopy {
        path: temp_dir.join(format!("{}.{}", unique_suffix(), stem)),
    };

    log_info!(
        "autoconfig",
        "Copying image for autoconfig injection: {} -> {}",
        source.display(),
        copy.path.display()
    );
    // fs::copy carries the source's permission bits over.
    std::fs::copy(source, &copy.path)
        .map_err(|e| format!("Failed to copy image for autoconfig: {}", e))?;
    restrict_to_owner(&copy.path, config::autoconfig::FILE_MODE)
        .map_err(|e| format!("Failed to protect the autoconfig working copy: {}", e))?;
    Ok(copy)
}

fn inject_into_copy(copy: &WorkingCopy, config: &AutoconfigConfig) -> Result<(), String> {
    log_info!(
        "autoconfig",
        "Laying the first-boot bundle onto {}",
        copy.path.display()
    );
    provision::provision_image(&copy.path, config).map_err(|e| {
        log_error!("autoconfig", "Failed to apply the profile: {}", e);
        match e {
            ProvisionError::Image(why) => no_ext4_rootfs(&why),
            other => format!("Failed to apply autoconfig profile: {}", other),
        }
    })
}

/// Why a flash-time preparation produced no copy.
#[derive(Debug, PartialEq, Eq)]
pub enum PrepError {
    Cancelled,
    Failed(String),
}

impl PrepError {
    /// The error a block-device flash returns for this outcome.
    pub fn into_flash_error(self) -> String {
        match self {
            PrepError::Cancelled => crate::flash::cancelled_err(),
            PrepError::Failed(message) => message,
        }
    }
}

struct PrepStageGuard<'a>(&'a FlashState);

impl Drop for PrepStageGuard<'_> {
    fn drop(&mut self) {
        self.0.set_prep_stage(None);
    }
}

fn prepare_tracked(
    source: &Path,
    temp_dir: &Path,
    config: &AutoconfigConfig,
    check_image: bool,
    state: &FlashState,
) -> Result<WorkingCopy, PrepError> {
    let _stage = PrepStageGuard(state);
    let cancelled = || {
        state
            .ensure_not_cancelled()
            .map_err(|_| PrepError::Cancelled)
    };

    cancelled()?;
    if check_image {
        require_supported_image(source).map_err(PrepError::Failed)?;
        cancelled()?;
    }
    state.set_prep_stage(Some(config::flash::PREP_STAGE_COPYING));
    let copy = create_private_copy(source, temp_dir).map_err(PrepError::Failed)?;
    cancelled()?;
    state.set_prep_stage(Some(config::flash::PREP_STAGE_APPLYING_PROFILE));
    inject_into_copy(&copy, config).map_err(PrepError::Failed)?;
    cancelled()?;
    Ok(copy)
}

/// Copy `source` into `temp_dir` and lay the bundle onto the copy, on a blocking thread, publishing `prep_stage` and stopping at a cancel between steps; `check_image` probes the source read-only first.
pub async fn prepare_flash_copy(
    source: PathBuf,
    temp_dir: PathBuf,
    config: AutoconfigConfig,
    check_image: bool,
    state: Arc<FlashState>,
) -> Result<WorkingCopy, PrepError> {
    log_info!(
        "autoconfig",
        "Preparing autoconfig copy of {}",
        source.display()
    );
    let started = Instant::now();
    let result = tokio::task::spawn_blocking(move || {
        prepare_tracked(&source, &temp_dir, &config, check_image, &state)
    })
    .await
    .unwrap_or_else(|e| {
        let why = if e.is_panic() {
            "panicked"
        } else {
            "was aborted"
        };
        Err(PrepError::Failed(format!("Autoconfig preparation {why}")))
    });
    let secs = started.elapsed().as_secs_f64();
    match &result {
        Ok(_) => log_info!("autoconfig", "Autoconfig prep done in {:.1} s", secs),
        Err(PrepError::Cancelled) => {
            log_info!("autoconfig", "Flash cancelled during autoconfig prep")
        }
        Err(PrepError::Failed(e)) => {
            log_error!(
                "autoconfig",
                "Autoconfig prep failed after {:.1} s: {}",
                secs,
                e
            )
        }
    }
    result
}

/// Remove the working copies a crashed or killed session left in `temp_dir`; subdirectories are kept.
pub fn sweep_stale_working_copies(temp_dir: &Path) -> usize {
    sweep_dir("autoconfig", temp_dir, |name, meta| {
        !meta.is_dir() && leftover_is_stale(name, meta.modified().ok())
    })
}

#[cfg(test)]
mod tests {
    use std::io::Read;

    use super::*;
    use crate::utils::test_scratch_dir;

    const FIXTURE: &[u8] =
        include_bytes!("../../crates/classic-ext4/tests/fixtures/classic.img.gz");

    fn classic_image() -> Vec<u8> {
        let mut out = Vec::new();
        flate2::read::GzDecoder::new(FIXTURE)
            .read_to_end(&mut out)
            .unwrap();
        out
    }

    #[test]
    fn a_profile_with_nothing_set_is_no_profile() {
        assert!(effective_profile(None).is_none());
        assert!(effective_profile(Some(AutoconfigConfig::default())).is_none());
        let blank = AutoconfigConfig {
            ethernet_enabled: Some(true),
            hostname: Some("  ".to_string()),
            ..AutoconfigConfig::default()
        };
        assert!(effective_profile(Some(blank)).is_none());
        assert!(effective_profile(Some(profile())).is_some());
    }

    #[test]
    fn the_copy_carries_the_bundle_and_the_source_stays_as_it_was() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source.img");
        let original = classic_image();
        std::fs::write(&source, &original).unwrap();
        let temp_dir = dir.path().join("work");

        let copy = prepare_tracked(&source, &temp_dir, &profile(), true, &FlashState::new())
            .map_err(|e| format!("{e:?}"))
            .unwrap();

        let fs = classic_ext4::ClassicExt4::open_read_only(copy.path()).unwrap();
        for file in provision::render_bundle(&profile()).unwrap() {
            assert!(fs.is_regular_file(&file.path).unwrap(), "{}", file.path);
        }
        assert!(std::fs::read(&source).unwrap() == original);
        drop(copy);
        assert_eq!(std::fs::read_dir(&temp_dir).unwrap().count(), 0);
    }

    #[test]
    fn the_qdl_preset_never_holds_a_secret_field() {
        let config = AutoconfigConfig {
            locale: Some(" en_US.UTF-8 ".to_string()),
            user_name: Some("anna".to_string()),
            user_password_hash: Some("$6$SECRET$x".to_string()),
            root_password_hash: Some("$6$SECRET$y".to_string()),
            wifi_psk: Some("SECRET".to_string()),
            ..AutoconfigConfig::default()
        };
        assert_eq!(
            render_qdl_preset(&config),
            "PRESET_LOCALE=\"en_US.UTF-8\"\n\
             PRESET_USER_NAME=\"anna\"\n\
             PRESET_CONNECT_WIRELESS=\"n\"\n"
        );
        assert_eq!(render_qdl_preset(&AutoconfigConfig::default()), "");
    }

    #[test]
    fn preset_values_are_shell_quoted() {
        assert_eq!(shell_quote("a\"b$c`d\\e"), "\"a\\\"b\\$c\\`d\\\\e\"");
    }

    #[test]
    fn working_copy_never_touches_the_source() {
        let dir = test_scratch_dir("wc-test");
        let temp_dir = dir.join("work");
        let source = dir.join("source.img");
        let original = vec![0x5au8; 64 * 1024];
        std::fs::write(&source, &original).unwrap();

        let result = prepare_tracked(&source, &temp_dir, &profile(), false, &FlashState::new());

        assert!(result.is_err(), "a non-ext4 image must be refused");
        assert_eq!(std::fs::read(&source).unwrap(), original);
        assert_eq!(std::fs::read_dir(&temp_dir).unwrap().count(), 0);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    fn profile() -> AutoconfigConfig {
        AutoconfigConfig {
            locale: Some("en_US.UTF-8".to_string()),
            ..AutoconfigConfig::default()
        }
    }

    #[test]
    fn a_cancel_before_prep_makes_no_copy() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source.img");
        std::fs::write(&source, b"image").unwrap();
        let temp_dir = dir.path().join("work");
        let state = FlashState::new();
        state
            .is_cancelled
            .store(true, std::sync::atomic::Ordering::SeqCst);

        let result = prepare_tracked(&source, &temp_dir, &profile(), false, &state);

        assert_eq!(result.err(), Some(PrepError::Cancelled));
        assert!(!temp_dir.exists());
        assert_eq!(state.prep_stage(), None);
    }

    #[tokio::test]
    async fn a_failed_prep_clears_the_stage_and_the_copy() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source.img");
        std::fs::write(&source, vec![0x5au8; 64 * 1024]).unwrap();
        let temp_dir = dir.path().join("work");
        let state = Arc::new(FlashState::new());

        let result =
            prepare_flash_copy(source, temp_dir.clone(), profile(), false, state.clone()).await;

        assert!(matches!(result, Err(PrepError::Failed(_))));
        assert_eq!(state.prep_stage(), None);
        assert_eq!(std::fs::read_dir(&temp_dir).unwrap().count(), 0);
    }

    #[cfg(unix)]
    #[test]
    fn working_copy_is_owner_only_before_the_preset_lands() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source.img");
        std::fs::write(&source, b"image").unwrap();
        std::fs::set_permissions(&source, std::fs::Permissions::from_mode(0o644)).unwrap();
        let temp_dir = dir.path().join("work");

        let copy = create_private_copy(&source, &temp_dir).unwrap();
        let mode = |p: &Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(copy.path()), config::autoconfig::FILE_MODE);
        assert_eq!(mode(&temp_dir), config::autoconfig::DIR_MODE);
        assert_eq!(mode(&source), 0o644, "the source keeps its own mode");
        drop(copy);
        assert_eq!(std::fs::read_dir(&temp_dir).unwrap().count(), 0);
    }

    #[cfg(unix)]
    #[test]
    fn a_linked_temp_dir_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source.img");
        std::fs::write(&source, b"image").unwrap();
        let elsewhere = dir.path().join("elsewhere");
        std::fs::create_dir_all(&elsewhere).unwrap();
        let link = dir.path().join("work");
        std::os::unix::fs::symlink(&elsewhere, &link).unwrap();
        assert!(create_private_copy(&source, &link).is_err());
        assert_eq!(std::fs::read_dir(&elsewhere).unwrap().count(), 0);
    }

    #[test]
    fn the_startup_sweep_removes_only_stale_copies_in_its_dir() {
        let dir = tempfile::tempdir().unwrap();
        let temp_dir = dir.path().join("work");
        std::fs::create_dir_all(temp_dir.join("nested")).unwrap();
        let ours = temp_dir.join(format!("{}.1.img", std::process::id()));
        let junk = temp_dir.join("leftover.img");
        let fresh = temp_dir.join("fresh.img");
        let nested = temp_dir.join("nested").join("1.1.img");
        let outside = dir.path().join("1.1.img");
        for path in [&ours, &junk, &fresh, &nested, &outside] {
            std::fs::write(path, b"PRESET_USER_PASSWORD=x").unwrap();
        }
        filetime::set_file_mtime(&junk, filetime::FileTime::zero()).unwrap();
        #[cfg(unix)]
        let dead = {
            let dead = temp_dir.join(format!("{}.1.img", i32::MAX));
            std::fs::write(&dead, b"x").unwrap();
            dead
        };

        let removed = sweep_stale_working_copies(&temp_dir);
        assert!(ours.exists(), "a copy of this process is in use");
        assert!(!junk.exists());
        assert!(fresh.exists(), "no owner pid and still recent");
        assert!(
            nested.exists() && outside.exists(),
            "nothing outside the dir itself"
        );
        #[cfg(unix)]
        {
            assert!(!dead.exists(), "the process that made it is gone");
            assert_eq!(removed, 2);
        }
        #[cfg(not(unix))]
        assert_eq!(removed, 1);
    }

    #[test]
    fn a_custom_image_is_checked_before_any_copy() {
        let dir = test_scratch_dir("wc-marker");
        let temp_dir = dir.join("work");
        let source = dir.join("custom.img");
        let original = vec![0x5au8; 64 * 1024];
        std::fs::write(&source, &original).unwrap();
        let config = profile();

        let err = match prepare_tracked(&source, &temp_dir, &config, true, &FlashState::new()) {
            Err(PrepError::Failed(e)) => e,
            other => panic!("expected a refusal, got {:?}", other.map(|_| ())),
        };

        assert!(err.contains("ext4"), "{err}");
        assert!(!temp_dir.exists(), "the refusal comes before the copy");
        assert_eq!(std::fs::read(&source).unwrap(), original);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_directory_is_never_probed_as_an_image() {
        let dir = test_scratch_dir("wc-marker-dir");
        assert!(require_supported_image(&dir).is_err());
        assert!(require_supported_image(&dir.join("missing.img")).is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn every_refusal_keeps_its_own_message() {
        assert!(support_verdict(Ok(ImageSupport::Supported)).is_ok());
        let verdict = |support| support_verdict(Ok(support)).unwrap_err();

        let missing = verdict(ImageSupport::NoFirstrunService);
        assert!(missing.starts_with(TAG_NOT_ARMBIAN), "{missing}");
        let booted = verdict(ImageSupport::AlreadyBooted);
        assert!(booted.starts_with(TAG_ALREADY_BOOTED), "{booted}");
        assert!(verdict(ImageSupport::NotExt4).contains("ext4"));
        let unsupported = verdict(ImageSupport::UnsupportedFs("metadata_csum".to_string()));
        assert!(unsupported.contains("metadata_csum"), "{unsupported}");

        let io = support_verdict(Err(ProvisionError::Image("boom".to_string()))).unwrap_err();
        assert!(!io.contains(TAG_NOT_ARMBIAN), "{io}");
        assert!(io.contains("Failed to read"), "{io}");
    }
}
