// SPDX-License-Identifier: MIT

use std::error::Error;
use std::fmt;
use std::io;

pub(crate) type Result<T> = std::result::Result<T, Ext4Error>;

#[derive(Debug)]
pub enum Ext4Error {
    Io(io::Error),
    Unsupported(String),
    NotFound(String),
    AlreadyExists(String),
    NoSpace,
    Corrupt(String),
}

impl fmt::Display for Ext4Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "I/O error: {e}"),
            Self::Unsupported(what) => write!(f, "unsupported: {what}"),
            Self::NotFound(what) => write!(f, "not found: {what}"),
            Self::AlreadyExists(what) => write!(f, "already exists: {what}"),
            Self::NoSpace => write!(f, "no space left in the filesystem"),
            Self::Corrupt(what) => write!(f, "corrupt image: {what}"),
        }
    }
}

impl Error for Ext4Error {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<io::Error> for Ext4Error {
    fn from(e: io::Error) -> Self {
        if e.kind() == io::ErrorKind::UnexpectedEof {
            Self::Corrupt("image is truncated".into())
        } else {
            Self::Io(e)
        }
    }
}
