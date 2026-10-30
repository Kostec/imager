// SPDX-License-Identifier: MIT

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};

use crate::error::{Ext4Error, Result};
use crate::raw::{get_u16, get_u32, BLOCK_SIZE};

pub(crate) const SUPERBLOCK_OFFSET: usize = 1024;
const SUPERBLOCK_SIZE: usize = 1024;
const MAGIC: u16 = 0xEF53;

const OFF_INODES_COUNT: usize = 0x00;
const OFF_BLOCKS_COUNT: usize = 0x04;
pub(crate) const OFF_FREE_BLOCKS: usize = 0x0C;
pub(crate) const OFF_FREE_INODES: usize = 0x10;
const OFF_FIRST_DATA_BLOCK: usize = 0x14;
const OFF_LOG_BLOCK_SIZE: usize = 0x18;
const OFF_LOG_CLUSTER_SIZE: usize = 0x1C;
const OFF_BLOCKS_PER_GROUP: usize = 0x20;
const OFF_CLUSTERS_PER_GROUP: usize = 0x24;
const OFF_INODES_PER_GROUP: usize = 0x28;
const OFF_MAGIC: usize = 0x38;
const OFF_STATE: usize = 0x3A;
const OFF_REV_LEVEL: usize = 0x4C;
const OFF_FIRST_INO: usize = 0x54;
const OFF_INODE_SIZE: usize = 0x58;
const OFF_FEATURE_COMPAT: usize = 0x5C;
const OFF_FEATURE_INCOMPAT: usize = 0x60;
const OFF_FEATURE_RO_COMPAT: usize = 0x64;
const OFF_LAST_ORPHAN: usize = 0xE8;
const OFF_DESC_SIZE: usize = 0xFE;
const OFF_WANT_EXTRA_ISIZE: usize = 0x15E;

const STATE_CLEAN: u16 = 1;
const LOG_BLOCK_SIZE_4K: u32 = 2;
const MIN_INODE_SIZE: u32 = 256;
const GOOD_OLD_INODE_SIZE: u32 = 128;
pub(crate) const DESC_SIZE: usize = 32;
/// Room for every field up to and including `i_crtime_extra`.
const MIN_EXTRA_ISIZE: u16 = 32;
const MAX_BITS_PER_GROUP: u32 = (BLOCK_SIZE * 8) as u32;
const FIRST_NON_RESERVED_INO: u32 = 11;

const INCOMPAT_FILETYPE: u32 = 0x2;
const INCOMPAT_EXTENTS: u32 = 0x40;
const INCOMPAT_FLEX_BG: u32 = 0x200;
const INCOMPAT_ALLOWED: u32 = INCOMPAT_FILETYPE | INCOMPAT_EXTENTS | INCOMPAT_FLEX_BG;
const INCOMPAT_REQUIRED: u32 = INCOMPAT_FILETYPE | INCOMPAT_EXTENTS;

// sparse_super, large_file, huge_file, dir_nlink, extra_isize.
const RO_COMPAT_ALLOWED: u32 = 0x1 | 0x2 | 0x8 | 0x20 | 0x40;

const COMPAT_FAST_COMMIT: u32 = 0x400;
const COMPAT_ORPHAN_FILE: u32 = 0x1000;

const INCOMPAT_NAMES: &[(u32, &str)] = &[
    (0x1, "compression"),
    (0x4, "needs_recovery"),
    (0x8, "journal_dev"),
    (0x10, "meta_bg"),
    (0x80, "64bit"),
    (0x100, "mmp"),
    (0x400, "ea_inode"),
    (0x1000, "dirdata"),
    (0x2000, "metadata_csum_seed"),
    (0x4000, "large_dir"),
    (0x8000, "inline_data"),
    (0x10000, "encrypt"),
    (0x20000, "casefold"),
];

const RO_COMPAT_NAMES: &[(u32, &str)] = &[
    (0x10, "gdt_csum"),
    (0x100, "quota"),
    (0x200, "bigalloc"),
    (0x400, "metadata_csum"),
    (0x1000, "readonly"),
    (0x2000, "project"),
    (0x8000, "verity"),
    (0x10000, "orphan_present"),
];

/// The fields this crate uses, validated so the rest of it can do arithmetic
/// on them.
pub(crate) struct Superblock {
    pub(crate) blocks_count: u32,
    pub(crate) inodes_count: u32,
    pub(crate) blocks_per_group: u32,
    pub(crate) inodes_per_group: u32,
    pub(crate) inode_size: u32,
    pub(crate) first_ino: u32,
    pub(crate) group_count: u32,
    /// `i_extra_isize` for new inodes.
    pub(crate) extra_isize: u16,
}

impl Superblock {
    pub(crate) fn read(file: &mut File, partition_offset: u64) -> Result<Self> {
        let mut buf = [0; SUPERBLOCK_SIZE];
        file.seek(SeekFrom::Start(partition_offset + SUPERBLOCK_OFFSET as u64))?;
        file.read_exact(&mut buf)?;
        Self::parse(&buf)
    }

    fn parse(sb: &[u8]) -> Result<Self> {
        if get_u16(sb, OFF_MAGIC) != MAGIC {
            return Err(Ext4Error::Corrupt("no ext4 superblock".into()));
        }
        check_features(sb)?;
        check_clean(sb)?;

        let log_block_size = get_u32(sb, OFF_LOG_BLOCK_SIZE);
        if log_block_size != LOG_BLOCK_SIZE_4K {
            return Err(Ext4Error::Unsupported(format!(
                "block size is not 4096 (log {log_block_size})"
            )));
        }
        if get_u32(sb, OFF_LOG_CLUSTER_SIZE) != log_block_size {
            return Err(Ext4Error::Corrupt("cluster size without bigalloc".into()));
        }
        let inode_size = u32::from(get_u16(sb, OFF_INODE_SIZE));
        if inode_size < MIN_INODE_SIZE {
            return Err(Ext4Error::Unsupported(format!(
                "inode size {inode_size} is below {MIN_INODE_SIZE}"
            )));
        }
        if !inode_size.is_power_of_two() || inode_size as usize > BLOCK_SIZE {
            return Err(Ext4Error::Corrupt(format!("inode size {inode_size}")));
        }
        let desc_size = get_u16(sb, OFF_DESC_SIZE);
        if desc_size != 0 && usize::from(desc_size) != DESC_SIZE {
            return Err(Ext4Error::Unsupported(format!(
                "group descriptor size {desc_size}"
            )));
        }
        Self::geometry(sb, inode_size)
    }

    fn geometry(sb: &[u8], inode_size: u32) -> Result<Self> {
        let corrupt = |what: &str| Ext4Error::Corrupt(format!("superblock: {what}"));
        let blocks_count = get_u32(sb, OFF_BLOCKS_COUNT);
        let inodes_count = get_u32(sb, OFF_INODES_COUNT);
        let blocks_per_group = get_u32(sb, OFF_BLOCKS_PER_GROUP);
        let inodes_per_group = get_u32(sb, OFF_INODES_PER_GROUP);
        let first_ino = get_u32(sb, OFF_FIRST_INO);

        if get_u32(sb, OFF_FIRST_DATA_BLOCK) != 0 {
            return Err(corrupt("first data block with 4 KiB blocks"));
        }
        if blocks_count == 0 {
            return Err(corrupt("no blocks"));
        }
        if blocks_per_group == 0 || blocks_per_group > MAX_BITS_PER_GROUP {
            return Err(corrupt("blocks per group"));
        }
        if get_u32(sb, OFF_CLUSTERS_PER_GROUP) != blocks_per_group {
            return Err(corrupt("clusters per group"));
        }
        if inodes_per_group == 0 || inodes_per_group > MAX_BITS_PER_GROUP {
            return Err(corrupt("inodes per group"));
        }
        let group_count = blocks_count.div_ceil(blocks_per_group);
        if u64::from(group_count) * u64::from(inodes_per_group) != u64::from(inodes_count) {
            return Err(corrupt("inode count does not match the groups"));
        }
        if first_ino < FIRST_NON_RESERVED_INO || first_ino > inodes_count {
            return Err(corrupt("first inode"));
        }
        let want = get_u16(sb, OFF_WANT_EXTRA_ISIZE).max(MIN_EXTRA_ISIZE);
        let room = (inode_size - GOOD_OLD_INODE_SIZE) as u16;
        let extra_isize = if want <= room && want % 4 == 0 {
            want
        } else {
            MIN_EXTRA_ISIZE
        };
        Ok(Self {
            blocks_count,
            inodes_count,
            blocks_per_group,
            inodes_per_group,
            inode_size,
            first_ino,
            group_count,
            extra_isize,
        })
    }

    pub(crate) fn fs_bytes(&self) -> u64 {
        u64::from(self.blocks_count) * BLOCK_SIZE as u64
    }

    /// First block after the superblock and the group descriptors.
    pub(crate) fn first_block_after_descriptors(&self) -> u32 {
        let bytes = u64::from(self.group_count) * DESC_SIZE as u64;
        1 + bytes.div_ceil(BLOCK_SIZE as u64) as u32
    }

    /// Blocks an inode table takes.
    pub(crate) fn inode_table_blocks(&self) -> u32 {
        let bytes = u64::from(self.inodes_per_group) * u64::from(self.inode_size);
        bytes.div_ceil(BLOCK_SIZE as u64) as u32
    }

    /// Blocks in a group: the last one may be short.
    pub(crate) fn blocks_in_group(&self, group: u32) -> u32 {
        let start = group * self.blocks_per_group;
        (self.blocks_count - start).min(self.blocks_per_group)
    }
}

fn check_features(sb: &[u8]) -> Result<()> {
    let incompat = get_u32(sb, OFF_FEATURE_INCOMPAT);
    let ro_compat = get_u32(sb, OFF_FEATURE_RO_COMPAT);
    let compat = get_u32(sb, OFF_FEATURE_COMPAT);

    let mut refused = feature_names(incompat & !INCOMPAT_ALLOWED, INCOMPAT_NAMES);
    refused.extend(feature_names(
        ro_compat & !RO_COMPAT_ALLOWED,
        RO_COMPAT_NAMES,
    ));
    if compat & COMPAT_FAST_COMMIT != 0 {
        refused.push("fast_commit".into());
    }
    if compat & COMPAT_ORPHAN_FILE != 0 {
        refused.push("orphan_file".into());
    }
    if !refused.is_empty() {
        return Err(Ext4Error::Unsupported(format!(
            "ext4 features: {}",
            refused.join(", ")
        )));
    }
    if incompat & INCOMPAT_REQUIRED != INCOMPAT_REQUIRED {
        return Err(Ext4Error::Unsupported(
            "ext4 without filetype or extents".into(),
        ));
    }
    if get_u32(sb, OFF_REV_LEVEL) == 0 {
        return Err(Ext4Error::Unsupported("revision 0 filesystem".into()));
    }
    Ok(())
}

fn feature_names(mut bits: u32, table: &[(u32, &str)]) -> Vec<String> {
    let mut names = Vec::new();
    for (bit, name) in table {
        if bits & bit != 0 {
            names.push((*name).to_owned());
            bits &= !bit;
        }
    }
    if bits != 0 {
        names.push(format!("unknown 0x{bits:x}"));
    }
    names
}

fn check_clean(sb: &[u8]) -> Result<()> {
    if get_u16(sb, OFF_STATE) != STATE_CLEAN {
        return Err(Ext4Error::Unsupported(
            "filesystem was not unmounted clean".into(),
        ));
    }
    if get_u32(sb, OFF_LAST_ORPHAN) != 0 {
        return Err(Ext4Error::Unsupported(
            "filesystem has a pending orphan list".into(),
        ));
    }
    Ok(())
}
