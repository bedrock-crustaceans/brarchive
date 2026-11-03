use crate::versions::{ENTRY_NAME_LEN_MAX, MAGIC};
use std::str::Utf8Error;
use thiserror::Error;

#[derive(Error, Debug)]
#[non_exhaustive]
pub enum BrArchiveError {
    #[error("Magic Sequence Mismatch (expected {MAGIC}, got {0})")]
    MagicMismatch(u64),
    #[error("Unsupported Archive Version {0}")]
    UnsupportedVersion(u32),
    #[error("Entry Name too long! Got {0} bytes, maximum {ENTRY_NAME_LEN_MAX}")]
    EntryNameTooLong(usize),
    #[error("Too many entries: {0} exceeds the u32 maximum")]
    TooManyEntries(usize),
    #[error("Content block exceeds 4 GiB limit")]
    ContentTooLarge,
    #[error("Archive is {0} bytes, shorter than the 16-byte header")]
    TruncatedHeader(usize),
    #[error("Entry table of {0} entries runs past the end of the archive")]
    TruncatedEntryTable(u32),
    #[error("Contents of entry \"{0}\" run past the end of the archive")]
    EntryOutOfBounds(String),
    #[error(transparent)]
    Utf8Error(#[from] Utf8Error),
    #[error(transparent)]
    IOError(#[from] std::io::Error),
}

impl PartialEq for BrArchiveError {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::MagicMismatch(a), Self::MagicMismatch(b)) => a == b,
            (Self::UnsupportedVersion(a), Self::UnsupportedVersion(b)) => a == b,
            (Self::Utf8Error(a), Self::Utf8Error(b)) => a == b,
            (Self::EntryNameTooLong(a), Self::EntryNameTooLong(b)) => a == b,
            (Self::TooManyEntries(a), Self::TooManyEntries(b)) => a == b,
            (Self::ContentTooLarge, Self::ContentTooLarge) => true,
            (Self::TruncatedHeader(a), Self::TruncatedHeader(b)) => a == b,
            (Self::TruncatedEntryTable(a), Self::TruncatedEntryTable(b)) => a == b,
            (Self::EntryOutOfBounds(a), Self::EntryOutOfBounds(b)) => a == b,
            (Self::IOError(_), Self::IOError(_)) => false,
            _ => false,
        }
    }
}
