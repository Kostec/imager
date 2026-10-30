// SPDX-License-Identifier: MIT

//! Little-endian field access. Offsets are either constants into a buffer of
//! known size or have been bounds-checked by the caller.

pub(crate) const BLOCK_SIZE: usize = 4096;

pub(crate) fn zeroed_block() -> Vec<u8> {
    vec![0; BLOCK_SIZE]
}

pub(crate) fn get_u16(buf: &[u8], off: usize) -> u16 {
    u16::from_le_bytes([buf[off], buf[off + 1]])
}

pub(crate) fn get_u32(buf: &[u8], off: usize) -> u32 {
    u32::from_le_bytes([buf[off], buf[off + 1], buf[off + 2], buf[off + 3]])
}

pub(crate) fn put_u16(buf: &mut [u8], off: usize, value: u16) {
    buf[off..off + 2].copy_from_slice(&value.to_le_bytes());
}

pub(crate) fn put_u32(buf: &mut [u8], off: usize, value: u32) {
    buf[off..off + 4].copy_from_slice(&value.to_le_bytes());
}
