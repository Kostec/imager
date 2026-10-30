//! What `open` refuses, and that damaged images produce errors, not panics.

mod common;

use std::fs::{self, OpenOptions};
use std::io::{Seek, SeekFrom, Write};

use classic_ext4::{ClassicExt4, Ext4Error};
use common::{fixture, fixture_from, pristine, u32_at, BLOCK, PART_OFFSET};

const SB: usize = PART_OFFSET + 1024;

fn or_u32(img: &mut [u8], off: usize, bits: u32) {
    let v = u32_at(img, off) | bits;
    img[off..off + 4].copy_from_slice(&v.to_le_bytes());
}

fn refused_with(mutate: impl FnOnce(&mut Vec<u8>), word: &str) {
    let mut img = pristine();
    mutate(&mut img);
    let fx = fixture_from(&img);
    for result in [
        ClassicExt4::open(&fx.path),
        ClassicExt4::open_read_only(&fx.path),
    ] {
        match result {
            Err(Ext4Error::Unsupported(m)) => assert!(m.contains(word), "{m}"),
            Err(other) => panic!("{word}: wrong error {other:?}"),
            Ok(_) => panic!("{word}: accepted"),
        }
    }
    assert!(
        fs::read(&fx.path).unwrap() == img,
        "{word}: open changed the file"
    );
}

#[test]
fn the_fixture_itself_opens() {
    let fx = fixture();
    ClassicExt4::open(&fx.path).unwrap();
}

#[test]
fn refuses_each_feature_that_would_need_more_than_this_writer_does() {
    let incompat = SB + 0x60;
    let ro_compat = SB + 0x64;
    let compat = SB + 0x5C;
    for (off, bit, word) in [
        (incompat, 0x4, "needs_recovery"),
        (incompat, 0x80, "64bit"),
        (incompat, 0x8000, "inline_data"),
        (incompat, 0x10, "meta_bg"),
        (incompat, 0x10000, "encrypt"),
        (incompat, 0x4000_0000, "unknown"),
        (ro_compat, 0x400, "metadata_csum"),
        (ro_compat, 0x10, "gdt_csum"),
        (ro_compat, 0x200, "bigalloc"),
        (ro_compat, 0x100, "quota"),
        (ro_compat, 0x4000_0000, "unknown"),
        (compat, 0x400, "fast_commit"),
    ] {
        refused_with(|img| or_u32(img, off, bit), word);
    }
}

#[test]
fn refuses_other_geometry() {
    refused_with(|img| img[SB + 0x18] = 1, "block size");
    refused_with(
        |img| img[SB + 0x58..SB + 0x5A].copy_from_slice(&128u16.to_le_bytes()),
        "inode size",
    );
    refused_with(
        |img| img[SB + 0xFE..SB + 0x100].copy_from_slice(&64u16.to_le_bytes()),
        "descriptor",
    );
}

#[test]
fn refuses_a_filesystem_that_was_not_cleanly_unmounted() {
    refused_with(|img| img[SB + 0x3A] = 0, "clean");
    refused_with(|img| img[SB + 0x3A] = 3, "clean");
    refused_with(|img| or_u32(img, SB + 0xE8, 77), "orphan");
}

#[test]
fn no_linux_partition_is_not_found() {
    let mut img = pristine();
    img[446 + 4] = 0x0C; // FAT32
    let fx = fixture_from(&img);
    assert!(matches!(
        ClassicExt4::open(&fx.path),
        Err(Ext4Error::NotFound(_))
    ));

    let mut img = pristine();
    img[510] = 0;
    let fx = fixture_from(&img);
    assert!(matches!(
        ClassicExt4::open(&fx.path),
        Err(Ext4Error::NotFound(_))
    ));
}

#[test]
fn a_linux_partition_without_ext4_is_corrupt() {
    let mut img = pristine();
    img[SB + 0x38] = 0;
    let fx = fixture_from(&img);
    assert!(matches!(
        ClassicExt4::open(&fx.path),
        Err(Ext4Error::Corrupt(_))
    ));
}

/// `mbrman::MBR::read_from` panics on a used entry at LBA 0.
#[test]
fn a_partition_at_lba_zero_is_an_error_not_a_panic() {
    let mut img = pristine();
    img[446 + 8..446 + 12].copy_from_slice(&0u32.to_le_bytes());
    let fx = fixture_from(&img);
    assert!(matches!(
        ClassicExt4::open(&fx.path),
        Err(Ext4Error::Corrupt(_))
    ));
}

#[test]
fn truncated_images_are_errors() {
    let img = pristine();
    let cuts = [
        0,
        100,
        511,
        512,
        PART_OFFSET,
        SB + 100,
        SB + 1024,
        PART_OFFSET + 2 * BLOCK,
        PART_OFFSET + 64 * BLOCK,
        img.len() / 2,
        img.len() - 1,
    ];
    let fx = fixture();
    for cut in cuts {
        fs::write(&fx.path, &img[..cut]).unwrap();
        assert!(ClassicExt4::open(&fx.path).is_err(), "cut at {cut}");
    }
}

#[test]
fn a_zero_rec_len_in_a_directory_is_corrupt_not_a_hang() {
    let mut img = pristine();
    let root = common::inode_offset(&img, 2);
    let block = u32_at(&img, root + 0x28 + 12 + 8) as usize;
    let first = PART_OFFSET + block * BLOCK;
    img[first + 12 + 4..first + 12 + 6].copy_from_slice(&0u16.to_le_bytes());
    let fx = fixture_from(&img);
    let fs4 = ClassicExt4::open_read_only(&fx.path).unwrap();
    assert!(matches!(
        fs4.exists("/hello.txt"),
        Err(Ext4Error::Corrupt(_))
    ));
}

#[test]
fn a_descriptor_counter_that_disagrees_with_the_bitmap_stops_allocation() {
    let mut img = pristine();
    let free = PART_OFFSET + BLOCK + 0x0C;
    img[free] = img[free].wrapping_add(1);
    let fx = fixture_from(&img);
    let mut fs4 = ClassicExt4::open(&fx.path).unwrap();
    assert!(matches!(
        fs4.write_new_file("/x", b"data", 0o644),
        Err(Ext4Error::Corrupt(_))
    ));
}

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, limit: usize) -> usize {
        (self.next() % limit as u64) as usize
    }
}

/// Exercises every entry point; the outcome does not matter, only that it returns.
fn exercise(path: &std::path::Path, commit: bool) {
    if let Ok(mut fs4) = ClassicExt4::open(path) {
        let dir = "/etc/systemd/system/armbian-firstrun.service.d";
        let _ = fs4.exists("/usr/lib/systemd/system/armbian-firstrun.service");
        let _ = fs4.is_regular_file("/htree/a-rather-long-file-name-to-fill-blocks-7");
        let _ = fs4.mkdir(dir, 0o755);
        let _ = fs4.write_new_file(&format!("{dir}/f"), &[7; 9000], 0o644);
        let _ = fs4.write_new_file("/hello2", b"x", 0o600);
        if commit {
            let _ = fs4.commit();
        }
    }
}

/// Without a commit the file only has to be patched back between rounds; with
/// one it is rewritten whole, so those rounds are fewer.
#[test]
fn random_damage_to_metadata_never_panics() {
    let clean = pristine();
    let fx = fixture();
    let table = u32_at(&clean, PART_OFFSET + BLOCK + 8) as usize;
    let root = common::inode_offset(&clean, 2);
    let root_block = u32_at(&clean, root + 0x28 + 12 + 8) as usize;
    // Superblock with descriptors, the start of the inode table, the root directory.
    let regions = [
        (0, 512),
        (SB, 1024),
        (PART_OFFSET + BLOCK, 256),
        (PART_OFFSET + table * BLOCK, 32 * 256),
        (PART_OFFSET + root_block * BLOCK, BLOCK),
    ];
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    let patch = |offset: usize, byte: u8| {
        let mut file = OpenOptions::new().write(true).open(&fx.path).unwrap();
        file.seek(SeekFrom::Start(offset as u64)).unwrap();
        file.write_all(&[byte]).unwrap();
    };
    for round in 0..1500 {
        let mut touched = Vec::new();
        for _ in 0..1 + round % 8 {
            let (start, len) = regions[rng.below(regions.len())];
            let offset = start + rng.below(len);
            patch(offset, clean[offset] ^ (1 << rng.below(8)));
            touched.push(offset);
        }
        let commit = round % 50 == 0;
        exercise(&fx.path, commit);
        if commit {
            fs::write(&fx.path, &clean).unwrap();
        } else {
            for offset in touched {
                patch(offset, clean[offset]);
            }
        }
    }
}
