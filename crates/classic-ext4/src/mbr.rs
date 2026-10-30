// SPDX-License-Identifier: MIT

use std::fs::File;
use std::io::{Cursor, Read, Seek, SeekFrom};

use mbrman::MBRHeader;

use crate::error::{Ext4Error, Result};

const SECTOR_SIZE: u64 = 512;
const LINUX_PARTITION: u8 = 0x83;

/// Where the rootfs lies in the image, in bytes.
pub(crate) struct Partition {
    pub(crate) offset: u64,
    pub(crate) len: u64,
}

/// Finds the first 0x83 primary partition.
///
/// Only the MBR sector goes through mbrman: `MBR::read_from`, which would also
/// walk logical partitions, panics on a used entry at LBA 0, and the rootfs of
/// these images is always primary.
pub(crate) fn find_linux_partition(file: &mut File, image_len: u64) -> Result<Partition> {
    if image_len < SECTOR_SIZE {
        return Err(Ext4Error::Corrupt("image is shorter than an MBR".into()));
    }
    let mut sector = [0; SECTOR_SIZE as usize];
    file.seek(SeekFrom::Start(0))?;
    file.read_exact(&mut sector)?;
    let header = MBRHeader::read_from(&mut Cursor::new(&sector[..]))
        .map_err(|_| Ext4Error::NotFound("no valid MBR".into()))?;
    let (_, entry) = header
        .iter()
        .find(|(_, entry)| entry.sys == LINUX_PARTITION)
        .ok_or_else(|| Ext4Error::NotFound("no Linux (0x83) partition".into()))?;
    if entry.starting_lba == 0 || entry.sectors == 0 {
        return Err(Ext4Error::Corrupt(format!(
            "partition at LBA {} of {} sectors",
            entry.starting_lba, entry.sectors
        )));
    }
    Ok(Partition {
        offset: u64::from(entry.starting_lba) * SECTOR_SIZE,
        len: u64::from(entry.sectors) * SECTOR_SIZE,
    })
}
