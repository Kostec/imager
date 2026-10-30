//! The drop-in insertion on a copy of the real Wukong image. Skipped without
//! `WUKONG_TEST_IMAGE`; that file is only ever read. The copy goes to
//! `CLASSIC_EXT4_KEEP_DIR` when set, for `scripts/fsck-check.sh`, and otherwise
//! to a temporary directory.

mod common;

use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use classic_ext4::{ClassicExt4, Ext4Error};
use common::{u16_at, u32_at, view_at, BLOCK, PART_OFFSET};
use tempfile::TempDir;

const DROPIN_DIR: &str = "/etc/systemd/system/armbian-firstrun.service.d";
const SERVICE: &str = "/usr/lib/systemd/system/armbian-firstrun.service";
const WANTS: &str = "/etc/systemd/system/multi-user.target.wants/armbian-firstrun.service";

fn source() -> Option<PathBuf> {
    match std::env::var_os("WUKONG_TEST_IMAGE") {
        Some(path) => Some(PathBuf::from(path)),
        None => {
            eprintln!("skipped: WUKONG_TEST_IMAGE is not set");
            None
        }
    }
}

/// Blocks that differ between two images of the same size.
fn changed_blocks(a: &Path, b: &Path) -> Vec<u64> {
    let (mut a, mut b) = (File::open(a).unwrap(), File::open(b).unwrap());
    let (mut left, mut right) = (vec![0; 1 << 20], vec![0; 1 << 20]);
    let mut changed = Vec::new();
    let mut offset = 0u64;
    loop {
        let len = a.read(&mut left).unwrap();
        if len == 0 {
            return changed;
        }
        b.read_exact(&mut right[..len]).unwrap();
        if left[..len] != right[..len] {
            for (i, (x, y)) in left[..len]
                .chunks(BLOCK)
                .zip(right[..len].chunks(BLOCK))
                .enumerate()
            {
                if x != y {
                    changed.push(offset / BLOCK as u64 + i as u64);
                }
            }
        }
        offset += len as u64;
    }
}

fn read_at(path: &Path, offset: usize, len: usize) -> Vec<u8> {
    let mut file = File::open(path).unwrap();
    file.seek(SeekFrom::Start(offset as u64)).unwrap();
    let mut buf = vec![0; len];
    file.read_exact(&mut buf).unwrap();
    buf
}

/// Sorts changed image blocks by what the original bitmaps say they were:
/// returns how many were free, and how many were in use but are neither
/// superblock, descriptors, bitmaps nor inode tables. Those could be someone's
/// file, so the test pins their number.
fn sort_changes(original: &Path, changed: &[u64]) -> (usize, usize) {
    let sb = read_at(original, PART_OFFSET + 1024, 1024);
    let (blocks, per_group) = (u32_at(&sb, 4) as usize, u32_at(&sb, 0x20) as usize);
    let table_blocks = (u32_at(&sb, 0x28) as usize * u16_at(&sb, 0x58) as usize).div_ceil(BLOCK);
    let groups = blocks.div_ceil(per_group);
    let descriptors = read_at(original, PART_OFFSET + BLOCK, groups * 32);
    let field = |group: usize, off: usize| u32_at(&descriptors, group * 32 + off) as usize;
    let is_metadata = |block: usize| {
        block <= groups * 32 / BLOCK + 1
            || (0..groups).any(|g| {
                let table = field(g, 8);
                block == field(g, 0)
                    || block == field(g, 4)
                    || (table..table + table_blocks).contains(&block)
            })
    };
    let (mut fresh, mut other) = (0, 0);
    for &block in changed {
        let block = block as usize - PART_OFFSET / BLOCK;
        let bitmap = read_at(
            original,
            PART_OFFSET + field(block / per_group, 0) * BLOCK,
            BLOCK,
        );
        let bit = block % per_group;
        if bitmap[bit / 8] & (1 << (bit % 8)) == 0 {
            fresh += 1;
        } else if !is_metadata(block) {
            other += 1;
        }
    }
    (fresh, other)
}

#[test]
fn firstrun_service_is_where_the_design_expects_it() {
    let Some(image) = source() else { return };
    let fs4 = ClassicExt4::open_read_only(&image).unwrap();
    assert!(fs4.is_regular_file(SERVICE).unwrap());
    assert!(fs4.exists(WANTS).unwrap());
    assert!(!fs4.is_regular_file(WANTS).unwrap(), "it is a symlink");
    assert!(!fs4.exists(DROPIN_DIR).unwrap());
    assert!(matches!(
        fs4.exists("/lib/systemd/system/armbian-firstrun.service"),
        Err(Ext4Error::Unsupported(_))
    ));
}

#[test]
fn dropin_goes_in_and_touches_nothing_else() {
    let Some(image) = source() else { return };
    let scratch = TempDir::new().unwrap();
    let dir = std::env::var_os("CLASSIC_EXT4_KEEP_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| scratch.path().to_owned());
    fs::create_dir_all(&dir).unwrap();
    let copy = dir.join("real.img");
    fs::copy(&image, &copy).unwrap();

    let script = vec![b'#'; 70_000];
    let files: [(&str, u16, &[u8]); 5] = [
        ("10-wukong.conf", 0o644, b"[Unit]\nBefore=ssh.service\n"),
        ("firstboot.sh", 0o755, b"#!/bin/bash\n"),
        ("post-network.sh", 0o755, b"#!/bin/bash\n"),
        ("firstboot.conf", 0o600, b"WK_HOSTNAME='wukong'\n"),
        ("user-script.sh", 0o700, &script),
    ];
    let mut fs4 = ClassicExt4::open(&copy).unwrap();
    fs4.mkdir(DROPIN_DIR, 0o755).unwrap();
    for (name, mode, data) in files {
        fs4.write_new_file(&format!("{DROPIN_DIR}/{name}"), data, mode)
            .unwrap();
    }
    fs4.commit().unwrap();

    let ext = view_at(&copy, PART_OFFSET as u64);
    assert_eq!(ext.metadata(DROPIN_DIR).unwrap().mode() & 0o7777, 0o755);
    for (name, mode, data) in files {
        let path = format!("{DROPIN_DIR}/{name}");
        assert_eq!(ext.metadata(path.as_str()).unwrap().mode() & 0o7777, mode);
        assert_eq!(ext.read(path.as_str()).unwrap(), data);
    }

    let changed = changed_blocks(&image, &copy);
    let (fresh, other) = sort_changes(&image, &changed);
    // 1 directory block, 4 small files, 18 blocks of the script.
    assert_eq!(fresh, 23, "blocks that were free in the original");
    assert_eq!(other, 1, "used blocks that are not metadata: the parent");
}
