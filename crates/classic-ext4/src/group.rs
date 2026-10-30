// SPDX-License-Identifier: MIT

//! Block group descriptors and the superblock's totals.

use crate::error::{Ext4Error, Result};
use crate::raw::{get_u16, get_u32, put_u16, put_u32, BLOCK_SIZE};
use crate::superblock::{DESC_SIZE, OFF_FREE_BLOCKS, OFF_FREE_INODES, SUPERBLOCK_OFFSET};
use crate::ClassicExt4;

const OFF_BLOCK_BITMAP: usize = 0x00;
const OFF_INODE_BITMAP: usize = 0x04;
const OFF_INODE_TABLE: usize = 0x08;
const OFF_FREE_BLOCKS_COUNT: usize = 0x0C;
const OFF_FREE_INODES_COUNT: usize = 0x0E;
const OFF_USED_DIRS: usize = 0x10;
const OFF_FLAGS: usize = 0x12;

const FLAG_INODE_UNINIT: u16 = 0x1;
const FLAG_BLOCK_UNINIT: u16 = 0x2;

pub(crate) struct GroupDesc {
    pub(crate) block_bitmap: u32,
    pub(crate) inode_bitmap: u32,
    pub(crate) inode_table: u32,
    pub(crate) free_blocks: u16,
    pub(crate) free_inodes: u16,
    pub(crate) used_dirs: u16,
    flags: u16,
}

impl ClassicExt4 {
    fn desc_location(&self, group: u32) -> (u32, usize) {
        let byte = group as usize * DESC_SIZE;
        (1 + (byte / BLOCK_SIZE) as u32, byte % BLOCK_SIZE)
    }

    pub(crate) fn read_desc(&self, group: u32) -> Result<GroupDesc> {
        let (block, off) = self.desc_location(group);
        let buf = self.dev.read(block)?;
        let raw = &buf[off..off + DESC_SIZE];
        Ok(GroupDesc {
            block_bitmap: get_u32(raw, OFF_BLOCK_BITMAP),
            inode_bitmap: get_u32(raw, OFF_INODE_BITMAP),
            inode_table: get_u32(raw, OFF_INODE_TABLE),
            free_blocks: get_u16(raw, OFF_FREE_BLOCKS_COUNT),
            free_inodes: get_u16(raw, OFF_FREE_INODES_COUNT),
            used_dirs: get_u16(raw, OFF_USED_DIRS),
            flags: get_u16(raw, OFF_FLAGS),
        })
    }

    /// Stores the three counters of `desc`; nothing else in it ever changes.
    pub(crate) fn write_desc_counters(&mut self, group: u32, desc: &GroupDesc) -> Result<()> {
        let (block, off) = self.desc_location(group);
        let raw = &mut self.dev.block_mut(block)?[off..off + DESC_SIZE];
        put_u16(raw, OFF_FREE_BLOCKS_COUNT, desc.free_blocks);
        put_u16(raw, OFF_FREE_INODES_COUNT, desc.free_inodes);
        put_u16(raw, OFF_USED_DIRS, desc.used_dirs);
        Ok(())
    }

    /// Validates where every group keeps its metadata, once, at open.
    pub(crate) fn check_groups(&self) -> Result<()> {
        let first_free = self.sb.first_block_after_descriptors();
        let table_blocks = self.sb.inode_table_blocks();
        let end = self.sb.blocks_count;
        for group in 0..self.sb.group_count {
            let desc = self.read_desc(group)?;
            let in_range = |block: u32, len: u32| {
                block >= first_free && block.checked_add(len).is_some_and(|last| last <= end)
            };
            if !in_range(desc.block_bitmap, 1)
                || !in_range(desc.inode_bitmap, 1)
                || !in_range(desc.inode_table, table_blocks)
            {
                return Err(Ext4Error::Corrupt(format!(
                    "group {group}: metadata out of range"
                )));
            }
            if desc.flags & (FLAG_INODE_UNINIT | FLAG_BLOCK_UNINIT) != 0 {
                return Err(Ext4Error::Unsupported(format!(
                    "group {group} has uninitialized bitmaps"
                )));
            }
        }
        Ok(())
    }

    /// Sets the superblock's free counts to the sum of the descriptors, which
    /// is what e2fsck compares them with.
    pub(crate) fn sync_superblock_counters(&mut self) -> Result<()> {
        let mut free_blocks = 0u32;
        let mut free_inodes = 0u32;
        for group in 0..self.sb.group_count {
            let desc = self.read_desc(group)?;
            free_blocks = free_blocks
                .checked_add(u32::from(desc.free_blocks))
                .ok_or_else(|| Ext4Error::Corrupt("free block count overflows".into()))?;
            free_inodes = free_inodes
                .checked_add(u32::from(desc.free_inodes))
                .ok_or_else(|| Ext4Error::Corrupt("free inode count overflows".into()))?;
        }
        let block0 = self.dev.block_mut(0)?;
        put_u32(block0, SUPERBLOCK_OFFSET + OFF_FREE_BLOCKS, free_blocks);
        put_u32(block0, SUPERBLOCK_OFFSET + OFF_FREE_INODES, free_inodes);
        Ok(())
    }
}
