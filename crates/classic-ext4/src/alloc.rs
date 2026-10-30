// SPDX-License-Identifier: MIT

//! Inode and block allocation from the group bitmaps.

use crate::error::{Ext4Error, Result};
use crate::extent::{MAX_EXTENT_LEN, MAX_INLINE_EXTENTS};
use crate::ClassicExt4;

/// A run of contiguous blocks.
#[derive(Clone, Copy)]
pub(crate) struct Run {
    pub(crate) start: u32,
    pub(crate) len: u32,
}

impl Run {
    pub(crate) fn end(&self) -> u32 {
        self.start + self.len
    }
}

fn bit_is_set(bitmap: &[u8], bit: u32) -> bool {
    bitmap[(bit / 8) as usize] & (1 << (bit % 8)) != 0
}

fn set_bit(bitmap: &mut [u8], bit: u32) {
    bitmap[(bit / 8) as usize] |= 1 << (bit % 8);
}

/// Clear bits among the first `bits`. Bits past those are padding set to one.
fn count_clear(bitmap: &[u8], bits: u32) -> u32 {
    let whole = (bits / 8) as usize;
    let mut clear: u32 = bitmap[..whole].iter().map(|b| b.count_zeros()).sum();
    for bit in whole as u32 * 8..bits {
        clear += u32::from(!bit_is_set(bitmap, bit));
    }
    clear
}

/// The first clear run of at least `want` bits, cut to `want`, or else the
/// longest one.
fn best_run(bitmap: &[u8], bits: u32, want: u32) -> Option<Run> {
    let mut best: Option<Run> = None;
    let mut start = 0;
    while start < bits {
        if bit_is_set(bitmap, start) {
            start += 1;
            continue;
        }
        let mut end = start + 1;
        while end < bits && end - start < want && !bit_is_set(bitmap, end) {
            end += 1;
        }
        let len = end - start;
        if len >= want {
            return Some(Run { start, len });
        }
        if best.is_none_or(|b| len > b.len) {
            best = Some(Run { start, len });
        }
        start = end;
    }
    best
}

fn mismatch(group: u32, what: &str) -> Ext4Error {
    Ext4Error::Corrupt(format!(
        "group {group}: {what} count disagrees with the bitmap"
    ))
}

impl ClassicExt4 {
    pub(crate) fn group_of_inode(&self, ino: u32) -> u32 {
        (ino - 1) / self.sb.inodes_per_group
    }

    /// Groups in the order to try them, `first` first.
    fn groups_from(&self, first: u32) -> impl Iterator<Item = u32> {
        let count = self.sb.group_count;
        (0..count).map(move |i| (first + i) % count)
    }

    /// Takes a free inode, preferring `preferred`, and counts a new directory.
    pub(crate) fn alloc_inode(&mut self, preferred: u32, is_dir: bool) -> Result<u32> {
        let per_group = self.sb.inodes_per_group;
        for group in self.groups_from(preferred) {
            let mut desc = self.read_desc(group)?;
            if desc.free_inodes == 0 {
                continue;
            }
            let mut bitmap = self.dev.read(desc.inode_bitmap)?;
            if count_clear(&bitmap, per_group) != u32::from(desc.free_inodes) {
                return Err(mismatch(group, "free inode"));
            }
            let first_ino = self.sb.first_ino;
            let ino_of = |bit: u32| group * per_group + bit + 1;
            let Some(bit) =
                (0..per_group).find(|&bit| ino_of(bit) >= first_ino && !bit_is_set(&bitmap, bit))
            else {
                continue;
            };
            set_bit(&mut bitmap, bit);
            self.dev.put(desc.inode_bitmap, bitmap)?;
            desc.free_inodes -= 1;
            if is_dir {
                desc.used_dirs = desc
                    .used_dirs
                    .checked_add(1)
                    .ok_or_else(|| mismatch(group, "used directory"))?;
            }
            self.write_desc_counters(group, &desc)?;
            return Ok(ino_of(bit));
        }
        Err(Ext4Error::NoSpace)
    }

    /// Takes `count` blocks in as many runs as a new inode can map itself.
    pub(crate) fn alloc_blocks(&mut self, count: u32, goal_group: u32) -> Result<Vec<Run>> {
        let mut free = 0u64;
        for group in 0..self.sb.group_count {
            free += u64::from(self.read_desc(group)?.free_blocks);
        }
        if free < u64::from(count) {
            return Err(Ext4Error::NoSpace);
        }
        let mut runs = Vec::new();
        let mut remaining = count;
        while remaining > 0 {
            if runs.len() == MAX_INLINE_EXTENTS {
                return Err(Ext4Error::NoSpace);
            }
            let run = self
                .find_run(goal_group, remaining.min(MAX_EXTENT_LEN))?
                .ok_or(Ext4Error::NoSpace)?;
            self.take_run(run)?;
            runs.push(run);
            remaining -= run.len;
        }
        Ok(runs)
    }

    /// Takes a block right after `after` if it is free, so that a directory
    /// stays in one extent, or else any block near it.
    pub(crate) fn alloc_block_after(&mut self, after: u32) -> Result<u32> {
        let per_group = self.sb.blocks_per_group;
        let group = after / per_group;
        if let Some(next) = after.checked_add(1) {
            if next < self.sb.blocks_count && next / per_group == group {
                let desc = self.read_desc(group)?;
                let bitmap = self.dev.read(desc.block_bitmap)?;
                if !bit_is_set(&bitmap, next % per_group) {
                    let run = Run {
                        start: next,
                        len: 1,
                    };
                    self.take_run(run)?;
                    return Ok(next);
                }
            }
        }
        let runs = self.alloc_blocks(1, group)?;
        runs.first()
            .map(|run| run.start)
            .ok_or_else(|| Ext4Error::Corrupt("allocation returned nothing".into()))
    }

    /// The first run of `want` blocks, or the longest in any group.
    fn find_run(&self, goal_group: u32, want: u32) -> Result<Option<Run>> {
        let mut best: Option<Run> = None;
        for group in self.groups_from(goal_group) {
            let desc = self.read_desc(group)?;
            if desc.free_blocks == 0 {
                continue;
            }
            let bits = self.sb.blocks_in_group(group);
            let bitmap = self.dev.read(desc.block_bitmap)?;
            if count_clear(&bitmap, bits) != u32::from(desc.free_blocks) {
                return Err(mismatch(group, "free block"));
            }
            let Some(run) = best_run(&bitmap, bits, want) else {
                continue;
            };
            let run = Run {
                start: group * self.sb.blocks_per_group + run.start,
                len: run.len,
            };
            if run.len >= want {
                return Ok(Some(run));
            }
            if best.is_none_or(|b| run.len > b.len) {
                best = Some(run);
            }
        }
        Ok(best)
    }

    /// Marks a run used. It lies inside one group and its bits are clear.
    fn take_run(&mut self, run: Run) -> Result<()> {
        let group = run.start / self.sb.blocks_per_group;
        let first_bit = run.start % self.sb.blocks_per_group;
        let mut desc = self.read_desc(group)?;
        let mut bitmap = self.dev.read(desc.block_bitmap)?;
        for bit in first_bit..first_bit + run.len {
            set_bit(&mut bitmap, bit);
        }
        self.dev.put(desc.block_bitmap, bitmap)?;
        desc.free_blocks = u32::from(desc.free_blocks)
            .checked_sub(run.len)
            .and_then(|left| u16::try_from(left).ok())
            .ok_or_else(|| mismatch(group, "free block"))?;
        self.write_desc_counters(group, &desc)
    }
}
