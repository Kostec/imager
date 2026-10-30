// SPDX-License-Identifier: GPL-2.0-or-later

//! What an image can take, and laying the bundle onto a working copy of it.

use std::path::Path;

use classic_ext4::{ClassicExt4, Ext4Error};
use serde::Serialize;

use super::bundle::{render_bundle, DROPIN_DIR};
use super::config::AutoconfigConfig;
use super::ProvisionError;

/// Whether settings can go onto an image; anything but `Supported` means write it as it is, or not at all.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", content = "reason", rename_all = "camelCase")]
pub enum ImageSupport {
    Supported,
    NotExt4,
    UnsupportedFs(String),
    NoFirstrunService,
    AlreadyBooted,
}

/// `/lib` is a symlink on these images, and `ClassicExt4` does not follow one in the middle of a path.
const FIRSTRUN_UNIT: &str = "/usr/lib/systemd/system/armbian-firstrun.service";
/// armbian-firstrun disables itself when it finishes: without this link the image has booted, and the drop-in would never run.
const FIRSTRUN_ENABLED: &str =
    "/etc/systemd/system/multi-user.target.wants/armbian-firstrun.service";
const DROPIN_DIR_MODE: u16 = 0o755;

/// Reads `image` only. Its answer stands for an uncompressed image: a compressed one is `NotExt4`.
pub fn probe_image(image: &Path) -> Result<ImageSupport, ProvisionError> {
    if !std::fs::metadata(image)
        .map_err(Ext4Error::Io)?
        .file_type()
        .is_file()
    {
        return Err(ProvisionError::Image("not a regular file".to_string()));
    }
    let fs = match ClassicExt4::open_read_only(image) {
        Ok(fs) => fs,
        Err(Ext4Error::NotFound(_) | Ext4Error::Corrupt(_)) => return Ok(ImageSupport::NotExt4),
        Err(Ext4Error::Unsupported(why)) => return Ok(ImageSupport::UnsupportedFs(why)),
        Err(e) => return Err(e.into()),
    };
    if !fs.is_regular_file(FIRSTRUN_UNIT)? {
        return Ok(ImageSupport::NoFirstrunService);
    }
    if !fs.exists(FIRSTRUN_ENABLED)? {
        return Ok(ImageSupport::AlreadyBooted);
    }
    Ok(ImageSupport::Supported)
}

/// Lays the bundle onto `image`, which must be a private working copy: all of it lands at `commit`, or nothing does.
pub fn provision_image(image: &Path, cfg: &AutoconfigConfig) -> Result<(), ProvisionError> {
    let files = render_bundle(cfg)?;
    if files.is_empty() {
        return Ok(());
    }
    let mut fs = ClassicExt4::open(image)?;
    fs.mkdir(DROPIN_DIR, DROPIN_DIR_MODE)?;
    for file in &files {
        fs.write_new_file(&file.path, &file.data, file.mode)?;
    }
    fs.commit()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::io::Read;
    use std::path::PathBuf;

    use tempfile::TempDir;

    use super::*;

    const FIXTURE: &[u8] =
        include_bytes!("../../../crates/classic-ext4/tests/fixtures/classic.img.gz");
    const PART_OFFSET: usize = 4 * 1024 * 1024;
    const SUPERBLOCK_RO_COMPAT: usize = 1024 + 0x64;
    const METADATA_CSUM: u8 = 0x04; // 0x400, high byte

    fn pristine() -> Vec<u8> {
        let mut out = Vec::new();
        flate2::read::GzDecoder::new(FIXTURE)
            .read_to_end(&mut out)
            .unwrap();
        out
    }

    fn image(bytes: &[u8]) -> (TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("image.img");
        std::fs::write(&path, bytes).unwrap();
        (dir, path)
    }

    // By hand: the fixture is 68 MiB and these tests run unoptimized.
    fn hits_of(bytes: &[u8], needle: &[u8]) -> Vec<usize> {
        let last = bytes.len() - needle.len();
        (0..last)
            .filter(|&at| bytes[at] == needle[0] && bytes[at..at + needle.len()] == *needle)
            .collect()
    }

    fn replace_all(bytes: &mut [u8], from: &[u8], to: &[u8]) {
        assert_eq!(from.len(), to.len());
        let hits = hits_of(bytes, from);
        assert!(!hits.is_empty(), "the fixture changed");
        for at in hits {
            bytes[at..at + to.len()].copy_from_slice(to);
        }
    }

    fn profile() -> AutoconfigConfig {
        AutoconfigConfig {
            hostname: Some("wukong".to_string()),
            user_script: Some("echo from-the-user-script\n".to_string()),
            ..AutoconfigConfig::default()
        }
    }

    #[test]
    fn a_classic_image_with_firstrun_pending_is_supported() {
        let (_dir, path) = image(&pristine());
        assert_eq!(probe_image(&path).unwrap(), ImageSupport::Supported);
    }

    #[test]
    fn no_mbr_or_no_ext4_is_not_ext4() {
        let (_dir, path) = image(&vec![0u8; 1024 * 1024]);
        assert_eq!(probe_image(&path).unwrap(), ImageSupport::NotExt4);

        let mut no_superblock = pristine();
        no_superblock[PART_OFFSET..PART_OFFSET + 4096].fill(0);
        let (_dir, path) = image(&no_superblock);
        assert_eq!(probe_image(&path).unwrap(), ImageSupport::NotExt4);
    }

    #[test]
    fn an_ext4_with_unsupported_features_says_why() {
        let mut bytes = pristine();
        bytes[PART_OFFSET + SUPERBLOCK_RO_COMPAT + 1] |= METADATA_CSUM;
        let (_dir, path) = image(&bytes);
        match probe_image(&path).unwrap() {
            ImageSupport::UnsupportedFs(why) => assert!(!why.is_empty()),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn no_firstrun_unit_is_told_apart() {
        let mut bytes = pristine();
        replace_all(
            &mut bytes,
            b"armbian-firstrun.service",
            b"armbian-firstrum.service",
        );
        let (_dir, path) = image(&bytes);
        assert_eq!(probe_image(&path).unwrap(), ImageSupport::NoFirstrunService);
    }

    #[test]
    fn a_disabled_firstrun_means_the_image_has_booted() {
        let mut bytes = pristine();
        replace_all(
            &mut bytes,
            b"multi-user.target.wants",
            b"multi-user.target.wantz",
        );
        let (_dir, path) = image(&bytes);
        assert_eq!(probe_image(&path).unwrap(), ImageSupport::AlreadyBooted);
    }

    #[test]
    fn something_that_is_no_file_is_an_error_not_a_verdict() {
        let dir = tempfile::tempdir().unwrap();
        assert!(probe_image(dir.path()).is_err());
        assert!(probe_image(&dir.path().join("missing.img")).is_err());
    }

    #[test]
    fn the_verdict_serializes_for_the_frontend() {
        assert_eq!(
            serde_json::to_string(&ImageSupport::Supported).unwrap(),
            r#"{"kind":"supported"}"#
        );
        assert_eq!(
            serde_json::to_string(&ImageSupport::UnsupportedFs("64bit".to_string())).unwrap(),
            r#"{"kind":"unsupportedFs","reason":"64bit"}"#
        );
    }

    #[test]
    fn provisioning_lays_every_file_of_the_bundle_on_the_rootfs() {
        let (_dir, path) = image(&pristine());
        provision_image(&path, &profile()).unwrap();

        let fs = ClassicExt4::open_read_only(&path).unwrap();
        for file in render_bundle(&profile()).unwrap() {
            assert!(fs.is_regular_file(&file.path).unwrap(), "{}", file.path);
        }
        assert!(fs.exists(DROPIN_DIR).unwrap());
        let bytes = std::fs::read(&path).unwrap();
        let script = b"echo from-the-user-script\n";
        assert!(!hits_of(&bytes, script).is_empty());
    }

    #[test]
    fn empty_settings_leave_the_image_alone() {
        let bytes = pristine();
        let (_dir, path) = image(&bytes);
        provision_image(&path, &AutoconfigConfig::default()).unwrap();
        assert!(std::fs::read(&path).unwrap() == bytes);
    }

    #[test]
    fn a_failure_leaves_no_trace_in_the_image() {
        let bytes = pristine();
        let (_dir, path) = image(&bytes);
        let bad = AutoconfigConfig {
            hostname: Some("not a hostname".to_string()),
            ..AutoconfigConfig::default()
        };
        assert!(provision_image(&path, &bad).is_err());
        assert!(std::fs::read(&path).unwrap() == bytes);

        // A card that was already provisioned: the second bundle must not half-land on top.
        provision_image(&path, &profile()).unwrap();
        let once = std::fs::read(&path).unwrap();
        assert!(provision_image(&path, &profile()).is_err());
        assert!(std::fs::read(&path).unwrap() == once);
    }
}
