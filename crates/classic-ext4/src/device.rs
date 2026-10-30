// SPDX-License-Identifier: MIT

//! The image file behind an in-memory overlay of changed blocks.

use std::collections::BTreeMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::mem;

use crate::error::{Ext4Error, Result};
use crate::raw::{zeroed_block, BLOCK_SIZE};

pub(crate) type Blocks = BTreeMap<u32, Vec<u8>>;

pub(crate) struct Device {
    file: File,
    base: u64,
    blocks_count: u32,
    writable: bool,
    dirty: Blocks,
    /// What each block touched in the current transaction held in `dirty`.
    undo: BTreeMap<u32, Option<Vec<u8>>>,
}

impl Device {
    pub(crate) fn new(file: File, base: u64, blocks_count: u32, writable: bool) -> Self {
        Self {
            file,
            base,
            blocks_count,
            writable,
            dirty: Blocks::new(),
            undo: BTreeMap::new(),
        }
    }

    pub(crate) fn is_writable(&self) -> bool {
        self.writable
    }

    pub(crate) fn is_clean(&self) -> bool {
        self.dirty.is_empty()
    }

    pub(crate) fn file(&self) -> &File {
        &self.file
    }

    pub(crate) fn base(&self) -> u64 {
        self.base
    }

    fn check_range(&self, block: u32) -> Result<()> {
        if block < self.blocks_count {
            Ok(())
        } else {
            Err(Ext4Error::Corrupt(format!(
                "block {block} is past the end of the filesystem"
            )))
        }
    }

    fn read_from_file(&self, block: u32) -> Result<Vec<u8>> {
        let mut buf = zeroed_block();
        let mut file = &self.file;
        file.seek(SeekFrom::Start(self.offset_of(block)))?;
        file.read_exact(&mut buf)?;
        Ok(buf)
    }

    fn offset_of(&self, block: u32) -> u64 {
        self.base + u64::from(block) * BLOCK_SIZE as u64
    }

    pub(crate) fn read(&self, block: u32) -> Result<Vec<u8>> {
        self.check_range(block)?;
        match self.dirty.get(&block) {
            Some(buf) => Ok(buf.clone()),
            None => self.read_from_file(block),
        }
    }

    fn remember(&mut self, block: u32) {
        if !self.undo.contains_key(&block) {
            let previous = self.dirty.get(&block).cloned();
            self.undo.insert(block, previous);
        }
    }

    /// The block's content, for changing in place.
    pub(crate) fn block_mut(&mut self, block: u32) -> Result<&mut [u8]> {
        self.check_range(block)?;
        self.remember(block);
        if !self.dirty.contains_key(&block) {
            let buf = self.read_from_file(block)?;
            self.dirty.insert(block, buf);
        }
        self.dirty
            .get_mut(&block)
            .map(Vec::as_mut_slice)
            .ok_or_else(|| Ext4Error::Corrupt(format!("block {block} left the overlay")))
    }

    /// Replaces a whole block. `data` is one block long.
    pub(crate) fn put(&mut self, block: u32, data: Vec<u8>) -> Result<()> {
        self.check_range(block)?;
        self.remember(block);
        self.dirty.insert(block, data);
        Ok(())
    }

    pub(crate) fn begin(&mut self) {
        self.undo.clear();
    }

    pub(crate) fn rollback(&mut self) {
        for (block, previous) in mem::take(&mut self.undo) {
            match previous {
                Some(buf) => {
                    self.dirty.insert(block, buf);
                }
                None => {
                    self.dirty.remove(&block);
                }
            }
        }
    }

    pub(crate) fn take_dirty(&mut self) -> Blocks {
        self.undo.clear();
        mem::take(&mut self.dirty)
    }

    pub(crate) fn write_out(&mut self, dirty: &Blocks) -> Result<()> {
        let mut file = &self.file;
        for (block, data) in dirty {
            file.seek(SeekFrom::Start(self.offset_of(*block)))?;
            file.write_all(data)?;
        }
        file.flush()?;
        file.sync_all()?;
        Ok(())
    }
}
