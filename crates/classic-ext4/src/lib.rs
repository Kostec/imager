// SPDX-License-Identifier: MIT

//! Creates directories and files in the ext4 rootfs of a RAW disk image, in
//! userspace and without mounting, for the classic layout Armbian images use:
//! 4 KiB blocks, 32-byte group descriptors, no metadata checksums, extents.
//!
//! Anything else is refused at `open`: a clean refusal is the whole safety net
//! here, since this code writes past the journal. The journal stays valid
//! because `needs_recovery` is refused, so it is empty, and nothing here
//! records a transaction in it.
//!
//! Every change stays in memory until [`ClassicExt4::commit`], which first
//! reads the result back through `ext4-view` and only then touches the file.
//!
//! How a refusal maps onto what a caller needs to tell apart:
//! - no valid MBR or no 0x83 partition: [`Ext4Error::NotFound`];
//! - a 0x83 partition that is not ext4, or a damaged image:
//!   [`Ext4Error::Corrupt`];
//! - a valid ext4 this writer cannot handle: [`Ext4Error::Unsupported`].

mod alloc;
mod device;
mod dir;
mod error;
mod extent;
mod group;
mod inode;
mod mbr;
mod raw;
mod superblock;
mod validate;

use std::fs::File;
use std::path::Path;

use device::Device;
pub use error::Ext4Error;
use error::Result;
use inode::{Inode, Now};
use superblock::Superblock;

const ROOT_INODE: u32 = 2;
const MAX_NAME_LEN: usize = 255;
const PERMISSION_BITS: u16 = 0o7777;

/// A file created in this session, re-read through `ext4-view` before commit.
struct CreatedFile {
    path: String,
    mode: u16,
    data: Vec<u8>,
}

/// A directory created in this session.
struct CreatedDir {
    path: String,
    mode: u16,
}

/// The rootfs of a disk image, with its pending changes.
pub struct ClassicExt4 {
    dev: Device,
    sb: Superblock,
    created_files: Vec<CreatedFile>,
    created_dirs: Vec<CreatedDir>,
}

impl ClassicExt4 {
    /// Opens the first 0x83 partition of the MBR for reading and writing.
    pub fn open(image: &Path) -> Result<Self> {
        Self::open_file(File::options().read(true).write(true).open(image)?, true)
    }

    /// Opens the same way, for queries only; `mkdir` and `write_new_file` are
    /// refused and `commit` writes nothing.
    pub fn open_read_only(image: &Path) -> Result<Self> {
        Self::open_file(File::open(image)?, false)
    }

    fn open_file(mut file: File, writable: bool) -> Result<Self> {
        let image_len = file.metadata()?.len();
        let partition = mbr::find_linux_partition(&mut file, image_len)?;
        let sb = Superblock::read(&mut file, partition.offset)?;
        let fs_len = sb.fs_bytes();
        if fs_len > partition.len || partition.offset + fs_len > image_len {
            return Err(Ext4Error::Corrupt(format!(
                "filesystem of {fs_len} bytes does not fit its partition"
            )));
        }
        let dev = Device::new(file, partition.offset, sb.blocks_count, writable);
        let fs = Self {
            dev,
            sb,
            created_files: Vec::new(),
            created_dirs: Vec::new(),
        };
        fs.check_groups()?;
        if !fs.read_inode(ROOT_INODE)?.is_dir() {
            return Err(Ext4Error::Corrupt("root inode is not a directory".into()));
        }
        Ok(fs)
    }

    /// True if `path` is a regular file. Like `lstat`, the last component is
    /// not followed; see [`Self::exists`] for symlinks on the way.
    pub fn is_regular_file(&self, path: &str) -> Result<bool> {
        match self.lookup(&components(path)?)? {
            Some(ino) => Ok(self.read_inode(ino)?.is_regular()),
            None => Ok(false),
        }
    }

    /// True if `path` names anything, a symlink included. A symlink in the
    /// middle of the path is `Unsupported`, never followed: on this image
    /// `/lib` is one, so use `/usr/lib/...`.
    pub fn exists(&self, path: &str) -> Result<bool> {
        Ok(self.lookup(&components(path)?)?.is_some())
    }

    /// Creates a directory owned by root. The parent must exist.
    pub fn mkdir(&mut self, path: &str, mode: u16) -> Result<()> {
        self.ensure_writable()?;
        check_mode(mode)?;
        self.transaction(|fs| fs.create_dir(path, mode))?;
        self.created_dirs.push(CreatedDir {
            path: path.to_owned(),
            mode,
        });
        Ok(())
    }

    /// Creates a regular file owned by root. It must not exist yet.
    pub fn write_new_file(&mut self, path: &str, data: &[u8], mode: u16) -> Result<()> {
        self.ensure_writable()?;
        check_mode(mode)?;
        self.transaction(|fs| fs.create_file(path, data, mode))?;
        self.created_files.push(CreatedFile {
            path: path.to_owned(),
            mode,
            data: data.to_vec(),
        });
        Ok(())
    }

    /// Updates the superblock counters, checks the result through `ext4-view`
    /// and writes it out. Dropping the handle instead discards every change.
    ///
    /// A failed write leaves the file half updated: callers work on a private
    /// copy of the image and throw it away on any error.
    pub fn commit(mut self) -> Result<()> {
        if self.dev.is_clean() {
            return Ok(());
        }
        self.sync_superblock_counters()?;
        let dirty = self.dev.take_dirty();
        validate::read_back(&self, &dirty)?;
        self.dev.write_out(&dirty)
    }

    fn ensure_writable(&self) -> Result<()> {
        if self.dev.is_writable() {
            Ok(())
        } else {
            Err(Ext4Error::Unsupported("image opened read-only".into()))
        }
    }

    /// Runs `op` so that a failure leaves the pending changes as they were.
    fn transaction<T>(&mut self, op: impl FnOnce(&mut Self) -> Result<T>) -> Result<T> {
        self.dev.begin();
        let result = op(self);
        if result.is_err() {
            self.dev.rollback();
        }
        result
    }

    fn create_dir(&mut self, path: &str, mode: u16) -> Result<()> {
        let (parent, name) = self.prepare_new(path)?;
        let now = Now::current();
        let group = self.group_of_inode(parent);
        let ino = self.alloc_inode(group, true)?;
        let runs = self.alloc_blocks(1, group)?;
        let mut block = raw::zeroed_block();
        dir::init_block(&mut block, ino, parent);
        for run in &runs {
            self.dev.put(run.start, block.clone())?;
        }
        let inode = Inode::new(
            ino,
            inode::S_IFDIR | mode,
            raw::BLOCK_SIZE as u64,
            2,
            &runs,
            &now,
            self.sb.extra_isize,
        )?;
        self.create_inode(&inode)?;
        self.insert_entry(parent, name, ino, dir::FT_DIR, &now)?;
        self.add_parent_link(parent)
    }

    fn create_file(&mut self, path: &str, data: &[u8], mode: u16) -> Result<()> {
        let (parent, name) = self.prepare_new(path)?;
        let now = Now::current();
        let group = self.group_of_inode(parent);
        let ino = self.alloc_inode(group, false)?;
        let blocks =
            u32::try_from(data.len().div_ceil(raw::BLOCK_SIZE)).map_err(|_| Ext4Error::NoSpace)?;
        let runs = self.alloc_blocks(blocks, group)?;
        let mut chunks = data.chunks(raw::BLOCK_SIZE);
        for run in &runs {
            for block in run.start..run.end() {
                let mut buf = raw::zeroed_block();
                if let Some(chunk) = chunks.next() {
                    buf[..chunk.len()].copy_from_slice(chunk);
                }
                self.dev.put(block, buf)?;
            }
        }
        let inode = Inode::new(
            ino,
            inode::S_IFREG | mode,
            data.len() as u64,
            1,
            &runs,
            &now,
            self.sb.extra_isize,
        )?;
        self.create_inode(&inode)?;
        self.insert_entry(parent, name, ino, dir::FT_REG_FILE, &now)
    }

    /// Resolves the parent of a path to create and checks it can take a new
    /// entry of that name.
    fn prepare_new<'p>(&self, path: &'p str) -> Result<(u32, &'p str)> {
        let parts = components(path)?;
        let Some((name, parent_parts)) = parts.split_last() else {
            return Err(Ext4Error::AlreadyExists(path.to_owned()));
        };
        let parent_path = || format!("/{}", parent_parts.join("/"));
        let parent = self
            .lookup(parent_parts)?
            .ok_or_else(|| Ext4Error::NotFound(parent_path()))?;
        let parent_inode = self.read_inode(parent)?;
        if parent_inode.is_symlink() {
            return Err(Ext4Error::Unsupported(format!(
                "{} is a symlink",
                parent_path()
            )));
        }
        if !parent_inode.is_dir() {
            return Err(Ext4Error::NotFound(format!(
                "{} is not a directory",
                parent_path()
            )));
        }
        parent_inode.check_accepts_entries(&parent_path())?;
        if self.find_entry(&parent_inode, name.as_bytes())?.is_some() {
            return Err(Ext4Error::AlreadyExists(path.to_owned()));
        }
        Ok((parent, name))
    }

    /// Walks from the root. `None` means a component is missing, or a parent
    /// is a file, which is the same to a caller asking what exists.
    fn lookup(&self, parts: &[&str]) -> Result<Option<u32>> {
        let mut ino = ROOT_INODE;
        for part in parts {
            let inode = self.read_inode(ino)?;
            if inode.is_symlink() {
                return Err(Ext4Error::Unsupported(format!(
                    "symlink before {part}, not followed"
                )));
            }
            if !inode.is_dir() {
                return Ok(None);
            }
            match self.find_entry(&inode, part.as_bytes())? {
                Some(next) => ino = next,
                None => return Ok(None),
            }
        }
        Ok(Some(ino))
    }
}

/// Splits an absolute path. `/` gives no components.
fn components(path: &str) -> Result<Vec<&str>> {
    let bad = |why: &str| Ext4Error::Unsupported(format!("path {path:?}: {why}"));
    let rest = path.strip_prefix('/').ok_or_else(|| bad("not absolute"))?;
    if rest.is_empty() {
        return Ok(Vec::new());
    }
    let parts: Vec<&str> = rest.split('/').collect();
    for part in &parts {
        match *part {
            "" => return Err(bad("empty component")),
            "." | ".." => return Err(bad("relative component")),
            _ if part.len() > MAX_NAME_LEN => return Err(bad("name too long")),
            _ if part.contains('\0') => return Err(bad("NUL in name")),
            _ => {}
        }
    }
    Ok(parts)
}

fn check_mode(mode: u16) -> Result<()> {
    if mode & !PERMISSION_BITS == 0 {
        Ok(())
    } else {
        Err(Ext4Error::Unsupported(format!(
            "mode {mode:o} has bits beyond the permissions"
        )))
    }
}
