// SPDX-License-Identifier: MIT

//! Linear directories. Reading also works on indexed ones: their index blocks
//! look like blocks with one unused entry, so a scan passes over them.

use crate::error::{Ext4Error, Result};
use crate::inode::{Inode, Now};
use crate::raw::{get_u16, get_u32, put_u16, put_u32, zeroed_block, BLOCK_SIZE};
use crate::ClassicExt4;

pub(crate) const FT_REG_FILE: u8 = 1;
pub(crate) const FT_DIR: u8 = 2;

const OFF_INODE: usize = 0;
const OFF_REC_LEN: usize = 4;
const OFF_NAME_LEN: usize = 6;
const OFF_FILE_TYPE: usize = 7;
const HEADER_SIZE: usize = 8;

fn corrupt(what: &str) -> Ext4Error {
    Ext4Error::Corrupt(format!("directory entry: {what}"))
}

/// Space an entry with a name of this length takes.
fn entry_size(name_len: usize) -> usize {
    (HEADER_SIZE + name_len).next_multiple_of(4)
}

struct Entry<'a> {
    pos: usize,
    ino: u32,
    rec_len: usize,
    name: &'a [u8],
}

/// Walks the entries of one directory block, validating each. The last one
/// must end exactly at the end of the block.
struct Entries<'a> {
    block: &'a [u8],
    pos: usize,
}

fn entries(block: &[u8]) -> Entries<'_> {
    Entries { block, pos: 0 }
}

impl<'a> Entries<'a> {
    fn parse(&self) -> Result<Entry<'a>> {
        let (block, pos) = (self.block, self.pos);
        if pos + HEADER_SIZE > block.len() {
            return Err(corrupt("header crosses the block"));
        }
        let rec_len = usize::from(get_u16(block, pos + OFF_REC_LEN));
        let name_len = usize::from(block[pos + OFF_NAME_LEN]);
        if rec_len < HEADER_SIZE || rec_len % 4 != 0 || pos + rec_len > block.len() {
            return Err(corrupt("bad record length"));
        }
        if HEADER_SIZE + name_len > rec_len {
            return Err(corrupt("name overflows the record"));
        }
        Ok(Entry {
            pos,
            ino: get_u32(block, pos + OFF_INODE),
            rec_len,
            name: &block[pos + HEADER_SIZE..pos + HEADER_SIZE + name_len],
        })
    }
}

impl<'a> Iterator for Entries<'a> {
    type Item = Result<Entry<'a>>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.pos >= self.block.len() {
            return None;
        }
        let parsed = self.parse();
        match &parsed {
            Ok(entry) => self.pos += entry.rec_len,
            Err(_) => self.pos = self.block.len(),
        }
        Some(parsed)
    }
}

fn write_entry(block: &mut [u8], pos: usize, ino: u32, rec_len: usize, name: &[u8], kind: u8) {
    put_u32(block, pos + OFF_INODE, ino);
    put_u16(block, pos + OFF_REC_LEN, rec_len as u16);
    block[pos + OFF_NAME_LEN] = name.len() as u8;
    block[pos + OFF_FILE_TYPE] = kind;
    block[pos + HEADER_SIZE..pos + HEADER_SIZE + name.len()].copy_from_slice(name);
}

/// Lays out the `.` and `..` of a new directory.
pub(crate) fn init_block(block: &mut [u8], ino: u32, parent: u32) {
    let dot = entry_size(1);
    write_entry(block, 0, ino, dot, b".", FT_DIR);
    write_entry(block, dot, parent, BLOCK_SIZE - dot, b"..", FT_DIR);
}

/// Puts an entry into the first gap that fits, splitting the record around it.
/// Returns false if the block has no such gap.
fn insert_in_block(block: &mut [u8], name: &[u8], ino: u32, kind: u8) -> Result<bool> {
    let needed = entry_size(name.len());
    let mut slot = None;
    for entry in entries(block) {
        let entry = entry?;
        let used = if entry.ino == 0 {
            0
        } else {
            entry_size(entry.name.len())
        };
        if entry.rec_len - used >= needed {
            slot = Some((entry.pos, used, entry.rec_len));
            break;
        }
    }
    let Some((pos, used, rec_len)) = slot else {
        return Ok(false);
    };
    if used > 0 {
        put_u16(block, pos + OFF_REC_LEN, used as u16);
    }
    write_entry(block, pos + used, ino, rec_len - used, name, kind);
    Ok(true)
}

impl ClassicExt4 {
    /// Physical blocks of a directory, in order.
    fn dir_blocks(&self, dir: &Inode) -> Result<Vec<u32>> {
        if !dir.uses_extents() {
            return Err(Ext4Error::Unsupported(
                "directory without extents".to_owned(),
            ));
        }
        let size = dir.size();
        let block_size = BLOCK_SIZE as u64;
        if size == 0 || size % block_size != 0 || size / block_size > self.sb.blocks_count.into() {
            return Err(corrupt("bad directory size"));
        }
        let count = (size / block_size) as u32;
        (0..count)
            .map(|logical| {
                self.map_block(dir.i_block(), logical)?
                    .ok_or_else(|| corrupt("hole in a directory"))
            })
            .collect()
    }

    pub(crate) fn find_entry(&self, dir: &Inode, name: &[u8]) -> Result<Option<u32>> {
        for block_no in self.dir_blocks(dir)? {
            let block = self.dev.read(block_no)?;
            for entry in entries(&block) {
                let entry = entry?;
                if entry.ino != 0 && entry.name == name {
                    return Ok(Some(entry.ino));
                }
            }
        }
        Ok(None)
    }

    /// Links `name` to `ino` in `parent`, whose name is free and which has
    /// passed `check_accepts_entries`, adding a block if none has room.
    pub(crate) fn insert_entry(
        &mut self,
        parent: u32,
        name: &str,
        ino: u32,
        kind: u8,
        now: &Now,
    ) -> Result<()> {
        let mut dir = self.read_inode(parent)?;
        let blocks = self.dir_blocks(&dir)?;
        let mut placed = false;
        for &block_no in &blocks {
            let mut block = self.dev.read(block_no)?;
            if insert_in_block(&mut block, name.as_bytes(), ino, kind)? {
                self.dev.put(block_no, block)?;
                placed = true;
                break;
            }
        }
        if !placed {
            self.append_dir_block(&mut dir, blocks.len() as u32, name, ino, kind)?;
        }
        dir.touch(now);
        self.write_inode(&dir)
    }

    fn append_dir_block(
        &mut self,
        dir: &mut Inode,
        logical: u32,
        name: &str,
        ino: u32,
        kind: u8,
    ) -> Result<()> {
        let last = logical
            .checked_sub(1)
            .and_then(|previous| self.map_block(dir.i_block(), previous).transpose())
            .ok_or_else(|| corrupt("directory without blocks"))??;
        let block_no = self.alloc_block_after(last)?;
        let mut block = zeroed_block();
        write_entry(&mut block, 0, ino, BLOCK_SIZE, name.as_bytes(), kind);
        self.dev.put(block_no, block)?;
        self.append_block(dir, logical, block_no)?;
        dir.add_size_block()?;
        dir.add_block_sectors()
    }
}
