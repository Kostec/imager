// SPDX-License-Identifier: MIT

//! Extent trees: lookup at any depth, and the few changes that new inodes and
//! growing directories need.

use crate::alloc::Run;
use crate::error::{Ext4Error, Result};
use crate::inode::Inode;
use crate::raw::{get_u16, get_u32, put_u16, put_u32, zeroed_block};
use crate::ClassicExt4;

const MAGIC: u16 = 0xF30A;
const ENTRY_SIZE: usize = 12;
const MAX_DEPTH: u16 = 5;
pub(crate) const MAX_INLINE_EXTENTS: usize = 4;
const MAX_LEAF_EXTENTS: u16 = 340;
/// An initialized extent; a longer `ee_len` means an unwritten one.
pub(crate) const MAX_EXTENT_LEN: u32 = 32768;

const OFF_ENTRIES: usize = 2;
const OFF_MAX: usize = 4;
const OFF_DEPTH: usize = 6;

const OFF_EE_BLOCK: usize = 0;
const OFF_EE_LEN: usize = 4;
const OFF_EE_START_HI: usize = 6;
const OFF_EE_START_LO: usize = 8;

const OFF_EI_BLOCK: usize = 0;
const OFF_EI_LEAF_LO: usize = 4;
const OFF_EI_LEAF_HI: usize = 8;

fn corrupt(what: &str) -> Ext4Error {
    Ext4Error::Corrupt(format!("extent tree: {what}"))
}

struct Header {
    entries: usize,
    depth: u16,
}

fn parse_header(node: &[u8]) -> Result<Header> {
    if get_u16(node, 0) != MAGIC {
        return Err(corrupt("bad magic"));
    }
    let entries = usize::from(get_u16(node, OFF_ENTRIES));
    let max = usize::from(get_u16(node, OFF_MAX));
    let depth = get_u16(node, OFF_DEPTH);
    if entries > max || ENTRY_SIZE + entries * ENTRY_SIZE > node.len() || depth > MAX_DEPTH {
        return Err(corrupt("bad header"));
    }
    Ok(Header { entries, depth })
}

fn entry(node: &[u8], index: usize) -> &[u8] {
    let start = ENTRY_SIZE + index * ENTRY_SIZE;
    &node[start..start + ENTRY_SIZE]
}

fn entry_mut(node: &mut [u8], index: usize) -> &mut [u8] {
    let start = ENTRY_SIZE + index * ENTRY_SIZE;
    &mut node[start..start + ENTRY_SIZE]
}

/// Start of a mapped extent; 48-bit addresses have no place without 64bit.
fn physical(hi: u16, lo: u32) -> Result<u32> {
    if hi == 0 {
        Ok(lo)
    } else {
        Err(corrupt("block address above 32 bits"))
    }
}

/// Finds the block a leaf maps `logical` to.
fn find_in_leaf(node: &[u8], entries: usize, logical: u32) -> Result<Option<u32>> {
    for index in 0..entries {
        let e = entry(node, index);
        let first = get_u32(e, OFF_EE_BLOCK);
        let len = u32::from(get_u16(e, OFF_EE_LEN));
        let Some(offset) = logical.checked_sub(first) else {
            continue;
        };
        if offset >= len {
            continue;
        }
        if len > MAX_EXTENT_LEN {
            return Err(corrupt("unwritten extent in a directory"));
        }
        let start = physical(get_u16(e, OFF_EE_START_HI), get_u32(e, OFF_EE_START_LO))?;
        let block = start
            .checked_add(offset)
            .ok_or_else(|| corrupt("extent overflows"))?;
        return Ok(Some(block));
    }
    Ok(None)
}

/// The child to descend into: the last index that starts at or before `logical`.
fn find_in_index(node: &[u8], entries: usize, logical: u32) -> Result<Option<u32>> {
    let mut child = None;
    for index in 0..entries {
        let e = entry(node, index);
        if get_u32(e, OFF_EI_BLOCK) <= logical {
            child = Some(physical(
                get_u16(e, OFF_EI_LEAF_HI),
                get_u32(e, OFF_EI_LEAF_LO),
            )?);
        }
    }
    Ok(child)
}

impl ClassicExt4 {
    /// Maps a logical block of a file through its tree. Holes are `None`.
    pub(crate) fn map_block(&self, i_block: &[u8], logical: u32) -> Result<Option<u32>> {
        let mut node = i_block.to_vec();
        let mut parent_depth = None;
        loop {
            let header = parse_header(&node)?;
            if parent_depth.is_some_and(|parent: u16| header.depth + 1 != parent) {
                return Err(corrupt("depth does not descend"));
            }
            if header.depth == 0 {
                return find_in_leaf(&node, header.entries, logical);
            }
            let Some(child) = find_in_index(&node, header.entries, logical)? else {
                return Ok(None);
            };
            parent_depth = Some(header.depth);
            node = self.dev.read(child)?;
        }
    }
}

/// Fills `i_block` of a new inode with up to four extents.
pub(crate) fn init_inline(i_block: &mut [u8], runs: &[Run]) -> Result<()> {
    if runs.len() > MAX_INLINE_EXTENTS {
        return Err(Ext4Error::NoSpace);
    }
    put_u16(i_block, 0, MAGIC);
    put_u16(i_block, OFF_ENTRIES, runs.len() as u16);
    put_u16(i_block, OFF_MAX, MAX_INLINE_EXTENTS as u16);
    put_u16(i_block, OFF_DEPTH, 0);
    let mut logical = 0;
    for (index, run) in runs.iter().enumerate() {
        write_leaf_entry(entry_mut(i_block, index), logical, run);
        logical += run.len;
    }
    Ok(())
}

fn write_leaf_entry(e: &mut [u8], logical: u32, run: &Run) {
    put_u32(e, OFF_EE_BLOCK, logical);
    // `Run::len` never exceeds MAX_EXTENT_LEN, which fits in 16 bits.
    put_u16(e, OFF_EE_LEN, run.len as u16);
    put_u16(e, OFF_EE_START_HI, 0);
    put_u32(e, OFF_EE_START_LO, run.start);
}

/// Adds `block` at the end of a node with room, extending its last extent when
/// they touch. Returns false if the node is full.
fn try_append(node: &mut [u8], logical: u32, block: u32) -> Result<bool> {
    let header = parse_header(node)?;
    if let Some(index) = header.entries.checked_sub(1) {
        let e = entry_mut(node, index);
        let first = get_u32(e, OFF_EE_BLOCK);
        let len = u32::from(get_u16(e, OFF_EE_LEN));
        let start = physical(get_u16(e, OFF_EE_START_HI), get_u32(e, OFF_EE_START_LO))?;
        let touches = first.checked_add(len) == Some(logical)
            && start.checked_add(len) == Some(block)
            && len < MAX_EXTENT_LEN;
        if touches {
            put_u16(e, OFF_EE_LEN, (len + 1) as u16);
            return Ok(true);
        }
    }
    let max = usize::from(get_u16(node, OFF_MAX));
    if header.entries >= max || ENTRY_SIZE * (header.entries + 2) > node.len() {
        return Ok(false);
    }
    let run = Run {
        start: block,
        len: 1,
    };
    write_leaf_entry(entry_mut(node, header.entries), logical, &run);
    put_u16(node, OFF_ENTRIES, (header.entries + 1) as u16);
    Ok(true)
}

impl ClassicExt4 {
    /// Maps the next logical block of a directory to `block`. The tree grows
    /// to depth 1 when the inode is full; a leaf then holds 340 extents, and a
    /// directory filled past that is refused.
    pub(crate) fn append_block(&mut self, dir: &mut Inode, logical: u32, block: u32) -> Result<()> {
        if parse_header(dir.i_block())?.depth == 0 {
            if try_append(dir.i_block_mut(), logical, block)? {
                return Ok(());
            }
            self.deepen(dir, block)?;
        }
        let header = parse_header(dir.i_block())?;
        if header.depth != 1 {
            return Err(Ext4Error::Unsupported(format!(
                "growing a directory with an extent tree of depth {}",
                header.depth
            )));
        }
        let index = header
            .entries
            .checked_sub(1)
            .ok_or_else(|| corrupt("index without entries"))?;
        let e = entry(dir.i_block(), index);
        let leaf = physical(get_u16(e, OFF_EI_LEAF_HI), get_u32(e, OFF_EI_LEAF_LO))?;
        let mut node = self.dev.read(leaf)?;
        if parse_header(&node)?.depth != 0 {
            return Err(corrupt("depth does not descend"));
        }
        if !try_append(&mut node, logical, block)? {
            return Err(Ext4Error::Unsupported(
                "directory has too many extents".into(),
            ));
        }
        self.dev.put(leaf, node)
    }

    /// Moves the inode's extents to a new leaf block and points the inode at it.
    fn deepen(&mut self, dir: &mut Inode, near: u32) -> Result<()> {
        let header = parse_header(dir.i_block())?;
        let used = ENTRY_SIZE * (header.entries + 1);
        let mut node = zeroed_block();
        node[..used].copy_from_slice(&dir.i_block()[..used]);
        put_u16(&mut node, OFF_MAX, MAX_LEAF_EXTENTS);
        let leaf = self.alloc_block_after(near)?;
        self.dev.put(leaf, node)?;

        let i_block = dir.i_block_mut();
        i_block[ENTRY_SIZE..].fill(0);
        put_u16(i_block, OFF_ENTRIES, 1);
        put_u16(i_block, OFF_DEPTH, 1);
        let index = entry_mut(i_block, 0);
        put_u32(index, OFF_EI_BLOCK, 0);
        put_u32(index, OFF_EI_LEAF_LO, leaf);
        put_u16(index, OFF_EI_LEAF_HI, 0);
        dir.add_block_sectors()
    }
}
