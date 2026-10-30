// SPDX-License-Identifier: MIT

//! Reads the pending result back through `ext4-view`, an implementation that
//! shares no code with this writer, while the file is still untouched.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};

use ext4_view::{Ext4, Ext4Read};

use crate::device::Blocks;
use crate::error::{Ext4Error, Result};
use crate::raw::BLOCK_SIZE;
use crate::ClassicExt4;

const PERMISSION_MASK: u16 = 0o7777;

type ReadError = Box<dyn core::error::Error + Send + Sync + 'static>;

/// The partition as it will be after the write.
struct Overlay {
    file: File,
    base: u64,
    dirty: Blocks,
}

impl Ext4Read for Overlay {
    fn read(&mut self, start_byte: u64, dst: &mut [u8]) -> std::result::Result<(), ReadError> {
        let block_size = BLOCK_SIZE as u64;
        let mut position = start_byte;
        let mut rest = dst;
        while !rest.is_empty() {
            let offset = (position % block_size) as usize;
            let len = rest.len().min(BLOCK_SIZE - offset);
            let (chunk, tail) = rest.split_at_mut(len);
            let block = u32::try_from(position / block_size)?;
            match self.dirty.get(&block) {
                Some(data) => chunk.copy_from_slice(&data[offset..offset + len]),
                None => {
                    self.file.seek(SeekFrom::Start(self.base + position))?;
                    self.file.read_exact(chunk)?;
                }
            }
            position += len as u64;
            rest = tail;
        }
        Ok(())
    }
}

fn failed(path: &str, what: impl std::fmt::Display) -> Ext4Error {
    Ext4Error::Corrupt(format!("read-back of {path}: {what}"))
}

/// Checks that everything created in this session reads back as asked.
pub(crate) fn read_back(fs: &ClassicExt4, dirty: &Blocks) -> Result<()> {
    let overlay = Overlay {
        file: fs.dev.file().try_clone()?,
        base: fs.dev.base(),
        dirty: dirty.clone(),
    };
    let view = Ext4::load(Box::new(overlay)).map_err(|e| failed("/", e))?;
    for dir in &fs.created_dirs {
        let path = dir.path.as_str();
        let meta = view.metadata(path).map_err(|e| failed(path, e))?;
        if !meta.is_dir() || meta.mode() & PERMISSION_MASK != dir.mode {
            return Err(failed(path, "wrong type or mode"));
        }
    }
    for file in &fs.created_files {
        let path = file.path.as_str();
        let meta = view.metadata(path).map_err(|e| failed(path, e))?;
        if meta.mode() & PERMISSION_MASK != file.mode {
            return Err(failed(path, "wrong mode"));
        }
        let data = view.read(path).map_err(|e| failed(path, e))?;
        if data != file.data {
            return Err(failed(path, "content differs"));
        }
    }
    Ok(())
}
