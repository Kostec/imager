// SPDX-License-Identifier: MIT

//! Inodes: the first 160 bytes of each record, which hold every field used.

use std::time::{SystemTime, UNIX_EPOCH};

use crate::alloc::Run;
use crate::error::{Ext4Error, Result};
use crate::extent;
use crate::raw::{get_u16, get_u32, put_u16, put_u32, BLOCK_SIZE};
use crate::ClassicExt4;

const HEAD_SIZE: usize = 160;
const SECTOR_SIZE: u64 = 512;

const S_IFMT: u16 = 0o170000;
pub(crate) const S_IFREG: u16 = 0o100000;
pub(crate) const S_IFDIR: u16 = 0o040000;
const S_IFLNK: u16 = 0o120000;

const OFF_MODE: usize = 0x00;
const OFF_SIZE_LO: usize = 0x04;
const OFF_ATIME: usize = 0x08;
const OFF_CTIME: usize = 0x0C;
const OFF_MTIME: usize = 0x10;
const OFF_LINKS: usize = 0x1A;
const OFF_BLOCKS_LO: usize = 0x1C;
const OFF_FLAGS: usize = 0x20;
const OFF_I_BLOCK: usize = 0x28;
pub(crate) const I_BLOCK_LEN: usize = 60;
const OFF_GENERATION: usize = 0x64;
const OFF_SIZE_HI: usize = 0x6C;
const OFF_BLOCKS_HI: usize = 0x74;
const OFF_EXTRA_ISIZE: usize = 0x80;
const OFF_CTIME_EXTRA: usize = 0x84;
const OFF_MTIME_EXTRA: usize = 0x88;
const OFF_ATIME_EXTRA: usize = 0x8C;
const OFF_CRTIME: usize = 0x90;
const OFF_CRTIME_EXTRA: usize = 0x94;

const FLAG_IMMUTABLE: u32 = 0x10;
const FLAG_APPEND: u32 = 0x20;
const FLAG_ENCRYPT: u32 = 0x800;
const FLAG_INDEX: u32 = 0x1000;
const FLAG_HUGE_FILE: u32 = 0x40000;
const FLAG_EXTENTS: u32 = 0x80000;
const FLAG_INLINE_DATA: u32 = 0x1000_0000;
const FLAG_CASEFOLD: u32 = 0x4000_0000;

/// A link count of 1 on a directory means "too many to count" with `dir_nlink`.
const NLINK_UNCOUNTED: u16 = 1;
const MAX_DIR_LINKS: u16 = 64999;

/// A timestamp as ext4 stores it: seconds, then nanoseconds with the epoch bits.
pub(crate) struct Now {
    seconds: u32,
    extra: u32,
    generation: u32,
}

impl Now {
    pub(crate) fn current() -> Self {
        let since_epoch = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        let seconds = since_epoch.as_secs() as i64;
        let nanos = since_epoch.subsec_nanos();
        let epoch = ((seconds - i64::from(seconds as i32)) >> 32) as u32 & 3;
        Self {
            seconds: seconds as u32,
            extra: (nanos << 2) | epoch,
            generation: nanos ^ (seconds as u32),
        }
    }
}

pub(crate) struct Inode {
    ino: u32,
    raw: [u8; HEAD_SIZE],
}

impl Inode {
    /// A fresh inode of root:root, mapping `runs` from logical block 0.
    pub(crate) fn new(
        ino: u32,
        mode: u16,
        size: u64,
        links: u16,
        runs: &[Run],
        now: &Now,
        extra_isize: u16,
    ) -> Result<Self> {
        let mut raw = [0; HEAD_SIZE];
        put_u16(&mut raw, OFF_MODE, mode);
        put_u32(&mut raw, OFF_SIZE_LO, size as u32);
        put_u32(&mut raw, OFF_SIZE_HI, (size >> 32) as u32);
        put_u16(&mut raw, OFF_LINKS, links);
        put_u32(&mut raw, OFF_FLAGS, FLAG_EXTENTS);
        put_u32(&mut raw, OFF_GENERATION, now.generation ^ ino);
        put_u16(&mut raw, OFF_EXTRA_ISIZE, extra_isize);
        let blocks: u64 = runs.iter().map(|run| u64::from(run.len)).sum();
        let sectors = blocks * (BLOCK_SIZE as u64 / SECTOR_SIZE);
        put_u32(&mut raw, OFF_BLOCKS_LO, sectors as u32);
        put_u16(&mut raw, OFF_BLOCKS_HI, (sectors >> 32) as u16);
        put_u32(&mut raw, OFF_CRTIME, now.seconds);
        put_u32(&mut raw, OFF_CRTIME_EXTRA, now.extra);
        put_u32(&mut raw, OFF_ATIME, now.seconds);
        put_u32(&mut raw, OFF_ATIME_EXTRA, now.extra);
        let mut inode = Self { ino, raw };
        inode.touch(now);
        extent::init_inline(inode.i_block_mut(), runs)?;
        Ok(inode)
    }

    pub(crate) fn mode(&self) -> u16 {
        get_u16(&self.raw, OFF_MODE)
    }

    pub(crate) fn is_dir(&self) -> bool {
        self.mode() & S_IFMT == S_IFDIR
    }

    pub(crate) fn is_regular(&self) -> bool {
        self.mode() & S_IFMT == S_IFREG
    }

    pub(crate) fn is_symlink(&self) -> bool {
        self.mode() & S_IFMT == S_IFLNK
    }

    fn flags(&self) -> u32 {
        get_u32(&self.raw, OFF_FLAGS)
    }

    pub(crate) fn size(&self) -> u64 {
        u64::from(get_u32(&self.raw, OFF_SIZE_LO))
            | (u64::from(get_u32(&self.raw, OFF_SIZE_HI)) << 32)
    }

    pub(crate) fn i_block(&self) -> &[u8] {
        &self.raw[OFF_I_BLOCK..OFF_I_BLOCK + I_BLOCK_LEN]
    }

    pub(crate) fn i_block_mut(&mut self) -> &mut [u8] {
        &mut self.raw[OFF_I_BLOCK..OFF_I_BLOCK + I_BLOCK_LEN]
    }

    /// Directories are only ever read through extents.
    pub(crate) fn uses_extents(&self) -> bool {
        self.flags() & FLAG_EXTENTS != 0
    }

    /// Refuses a directory that cannot take a linear insertion. `path` only
    /// names it in the message.
    pub(crate) fn check_accepts_entries(&self, path: &str) -> Result<()> {
        let flags = self.flags();
        if flags & FLAG_INDEX != 0 {
            return Err(Ext4Error::Unsupported(format!(
                "{path} has a hash index (EXT4_INDEX_FL)"
            )));
        }
        let refused = FLAG_IMMUTABLE
            | FLAG_APPEND
            | FLAG_ENCRYPT
            | FLAG_HUGE_FILE
            | FLAG_INLINE_DATA
            | FLAG_CASEFOLD;
        if flags & refused != 0 || !self.uses_extents() {
            return Err(Ext4Error::Unsupported(format!(
                "{path} has inode flags 0x{flags:x}"
            )));
        }
        Ok(())
    }

    /// Sets ctime and mtime, with nanoseconds if the record has room for them.
    pub(crate) fn touch(&mut self, now: &Now) {
        put_u32(&mut self.raw, OFF_CTIME, now.seconds);
        put_u32(&mut self.raw, OFF_MTIME, now.seconds);
        let extra_isize = usize::from(get_u16(&self.raw, OFF_EXTRA_ISIZE));
        let end_of = |off: usize| off + 4 - OFF_EXTRA_ISIZE;
        if extra_isize >= end_of(OFF_CTIME_EXTRA) {
            put_u32(&mut self.raw, OFF_CTIME_EXTRA, now.extra);
        }
        if extra_isize >= end_of(OFF_MTIME_EXTRA) {
            put_u32(&mut self.raw, OFF_MTIME_EXTRA, now.extra);
        }
    }

    fn overflow(&self) -> Ext4Error {
        Ext4Error::Corrupt(format!("inode {}: size overflows", self.ino))
    }

    /// Accounts for one more block of the directory's content.
    pub(crate) fn add_size_block(&mut self) -> Result<()> {
        let size = self
            .size()
            .checked_add(BLOCK_SIZE as u64)
            .ok_or_else(|| self.overflow())?;
        put_u32(&mut self.raw, OFF_SIZE_LO, size as u32);
        put_u32(&mut self.raw, OFF_SIZE_HI, (size >> 32) as u32);
        Ok(())
    }

    /// Accounts for one more block held by the inode, content or extent tree.
    pub(crate) fn add_block_sectors(&mut self) -> Result<()> {
        let sectors = get_u32(&self.raw, OFF_BLOCKS_LO)
            .checked_add((BLOCK_SIZE as u64 / SECTOR_SIZE) as u32)
            .ok_or_else(|| self.overflow())?;
        put_u32(&mut self.raw, OFF_BLOCKS_LO, sectors);
        Ok(())
    }

    /// Counts the `..` of a new subdirectory.
    fn add_link(&mut self) -> Result<()> {
        let links = get_u16(&self.raw, OFF_LINKS);
        if links == NLINK_UNCOUNTED || links >= MAX_DIR_LINKS {
            return Err(Ext4Error::Unsupported(format!(
                "directory inode {} has {links} links",
                self.ino
            )));
        }
        put_u16(&mut self.raw, OFF_LINKS, links + 1);
        Ok(())
    }
}

impl ClassicExt4 {
    fn inode_location(&self, ino: u32) -> Result<(u32, usize)> {
        if ino == 0 || ino > self.sb.inodes_count {
            return Err(Ext4Error::Corrupt(format!("inode number {ino}")));
        }
        let group = self.group_of_inode(ino);
        let index = (ino - 1) % self.sb.inodes_per_group;
        let table = self.read_desc(group)?.inode_table;
        // At most 2^15 * 2^12 bytes, and `check_groups` bounded the table.
        let byte = index as usize * self.sb.inode_size as usize;
        Ok((table + (byte / BLOCK_SIZE) as u32, byte % BLOCK_SIZE))
    }

    pub(crate) fn read_inode(&self, ino: u32) -> Result<Inode> {
        let (block, off) = self.inode_location(ino)?;
        let buf = self.dev.read(block)?;
        let mut raw = [0; HEAD_SIZE];
        raw.copy_from_slice(&buf[off..off + HEAD_SIZE]);
        Ok(Inode { ino, raw })
    }

    /// Stores an inode whose record already holds it.
    pub(crate) fn write_inode(&mut self, inode: &Inode) -> Result<()> {
        let (block, off) = self.inode_location(inode.ino)?;
        self.dev.block_mut(block)?[off..off + HEAD_SIZE].copy_from_slice(&inode.raw);
        Ok(())
    }

    /// Stores a new inode, clearing what the previous owner left in its record.
    pub(crate) fn create_inode(&mut self, inode: &Inode) -> Result<()> {
        let (block, off) = self.inode_location(inode.ino)?;
        let size = self.sb.inode_size as usize;
        let buf = self.dev.block_mut(block)?;
        buf[off..off + size].fill(0);
        buf[off..off + HEAD_SIZE].copy_from_slice(&inode.raw);
        Ok(())
    }

    pub(crate) fn add_parent_link(&mut self, parent: u32) -> Result<()> {
        let mut inode = self.read_inode(parent)?;
        inode.add_link()?;
        self.write_inode(&inode)
    }
}
