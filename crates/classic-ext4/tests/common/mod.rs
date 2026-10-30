//! Helpers shared by the integration tests. Everything here reads the on-disk
//! format by hand or through `ext4-view`, never through the crate under test.
#![allow(dead_code)]

use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use ext4_view::{Ext4, Ext4Read};
use flate2::read::GzDecoder;
use tempfile::TempDir;

/// The fixture has its single partition at LBA 8192.
pub const PART_OFFSET: usize = 4 * 1024 * 1024;
pub const BLOCK: usize = 4096;

pub fn pristine() -> Vec<u8> {
    let gz = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/classic.img.gz");
    let mut out = Vec::new();
    GzDecoder::new(File::open(gz).expect("fixture"))
        .read_to_end(&mut out)
        .expect("gunzip fixture");
    out
}

pub struct Fixture {
    _dir: TempDir,
    pub path: PathBuf,
}

pub fn fixture() -> Fixture {
    fixture_from(&pristine())
}

pub fn fixture_from(bytes: &[u8]) -> Fixture {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().join("classic.img");
    fs::write(&path, bytes).expect("write fixture");
    Fixture { _dir: dir, path }
}

/// Keeps a result for `scripts/fsck-check.sh`, which runs e2fsck on it.
pub fn keep(image: &Path, name: &str) {
    if let Some(dir) = std::env::var_os("CLASSIC_EXT4_KEEP_DIR") {
        fs::create_dir_all(&dir).expect("keep dir");
        fs::copy(image, Path::new(&dir).join(format!("{name}.img"))).expect("keep image");
    }
}

struct PartReader {
    file: File,
    base: u64,
}

impl Ext4Read for PartReader {
    fn read(
        &mut self,
        start_byte: u64,
        dst: &mut [u8],
    ) -> Result<(), Box<dyn core::error::Error + Send + Sync + 'static>> {
        self.file.seek(SeekFrom::Start(self.base + start_byte))?;
        self.file.read_exact(dst)?;
        Ok(())
    }
}

pub fn view_at(image: &Path, base: u64) -> Ext4 {
    let file = File::open(image).expect("open image");
    Ext4::load(Box::new(PartReader { file, base })).expect("ext4-view load")
}

pub fn view(image: &Path) -> Ext4 {
    view_at(image, PART_OFFSET as u64)
}

pub fn u16_at(buf: &[u8], off: usize) -> u32 {
    u32::from(u16::from_le_bytes([buf[off], buf[off + 1]]))
}

pub fn u32_at(buf: &[u8], off: usize) -> u32 {
    u32::from_le_bytes([buf[off], buf[off + 1], buf[off + 2], buf[off + 3]])
}

/// Counters as stored on disk, read by hand from the superblock and descriptors.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct Counters {
    pub sb_free_blocks: u32,
    pub sb_free_inodes: u32,
    pub gd_free_blocks: u32,
    pub gd_free_inodes: u32,
    pub used_dirs: u32,
}

fn group_count(img: &[u8]) -> usize {
    let sb = &img[PART_OFFSET + 1024..];
    (u32_at(sb, 4) as usize).div_ceil(u32_at(sb, 0x20) as usize)
}

fn descriptor(img: &[u8], group: usize) -> &[u8] {
    &img[PART_OFFSET + BLOCK + group * 32..]
}

pub fn counters(image: &Path) -> Counters {
    let img = fs::read(image).expect("read image");
    let sb = &img[PART_OFFSET + 1024..];
    let mut c = Counters {
        sb_free_blocks: u32_at(sb, 0x0C),
        sb_free_inodes: u32_at(sb, 0x10),
        gd_free_blocks: 0,
        gd_free_inodes: 0,
        used_dirs: 0,
    };
    for g in 0..group_count(&img) {
        let d = descriptor(&img, g);
        c.gd_free_blocks += u16_at(d, 0x0C);
        c.gd_free_inodes += u16_at(d, 0x0E);
        c.used_dirs += u16_at(d, 0x10);
    }
    c
}

/// Byte offset in the image of an inode's on-disk record.
pub fn inode_offset(img: &[u8], ino: usize) -> usize {
    let sb = &img[PART_OFFSET + 1024..];
    let per_group = u32_at(sb, 0x28) as usize;
    let inode_size = u16_at(sb, 0x58) as usize;
    let group = (ino - 1) / per_group;
    let table = u32_at(descriptor(img, group), 8) as usize;
    PART_OFFSET + table * BLOCK + (ino - 1) % per_group * inode_size
}

/// Inode number of a name in the root directory, whose first block is at
/// the start of its only extent.
pub fn root_entry(img: &[u8], name: &str) -> usize {
    let root = inode_offset(img, 2);
    let block = u32_at(img, root + 0x28 + 12 + 8) as usize;
    let dir = &img[PART_OFFSET + block * BLOCK..][..BLOCK];
    let mut pos = 0;
    while pos < BLOCK {
        let rec_len = u16_at(dir, pos + 4) as usize;
        let name_len = dir[pos + 6] as usize;
        if u32_at(dir, pos) != 0 && &dir[pos + 8..pos + 8 + name_len] == name.as_bytes() {
            return u32_at(dir, pos) as usize;
        }
        pos += rec_len;
    }
    panic!("{name} is not in the root directory");
}
