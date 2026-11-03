use crate::error::BrArchiveError;
use crate::versions::{EntryDescriptor, Header, ENTRY_NAME_LEN_MAX, MAGIC, VERSIONS};
use byteorder::{ByteOrder, LittleEndian};
use std::ops::Range;

/// Size of the archive header: u64 magic, u32 entry count, u32 version.
pub(crate) const HEADER_LEN: usize = 16;
/// Size of one entry descriptor in the entry table.
pub(crate) const DESCRIPTOR_LEN: usize = 256;
/// Position of the u32 content offset inside a descriptor; the u32 length follows.
const CONTENTS_OFFSET_FIELD: usize = 1 + ENTRY_NAME_LEN_MAX;

pub(crate) fn read_header(data: &[u8]) -> Result<Header, BrArchiveError> {
    let header = data
        .get(..HEADER_LEN)
        .ok_or(BrArchiveError::TruncatedHeader(data.len()))?;
    let magic = LittleEndian::read_u64(&header[0..8]);
    if magic != MAGIC {
        return Err(BrArchiveError::MagicMismatch(magic));
    }
    let entries = LittleEndian::read_u32(&header[8..12]);
    let version = LittleEndian::read_u32(&header[12..16]);
    if !VERSIONS.contains(&version) {
        return Err(BrArchiveError::UnsupportedVersion(version));
    }
    Ok(Header { entries, version })
}

/// The entry table of `data`, checked to fit inside it, and the position where
/// the content block starts.
pub(crate) fn entry_table<'a>(
    data: &'a [u8],
    header: &Header,
) -> Result<(&'a [u8], usize), BrArchiveError> {
    let truncated = || BrArchiveError::TruncatedEntryTable(header.entries);
    let data_start = (header.entries as usize)
        .checked_mul(DESCRIPTOR_LEN)
        .and_then(|table_len| table_len.checked_add(HEADER_LEN))
        .ok_or_else(truncated)?;
    let table = data.get(HEADER_LEN..data_start).ok_or_else(truncated)?;
    Ok((table, data_start))
}

/// Read one descriptor from a `DESCRIPTOR_LEN`-byte slice of the entry table.
pub(crate) fn read_entry_descriptor(
    descriptor: &[u8],
) -> Result<EntryDescriptor<'_>, BrArchiveError> {
    debug_assert_eq!(descriptor.len(), DESCRIPTOR_LEN);
    let name_len = descriptor[0] as usize;
    if name_len > ENTRY_NAME_LEN_MAX {
        return Err(BrArchiveError::EntryNameTooLong(name_len));
    }
    let name = std::str::from_utf8(&descriptor[1..1 + name_len])?;
    let contents_offset = LittleEndian::read_u32(&descriptor[CONTENTS_OFFSET_FIELD..]);
    let contents_len = LittleEndian::read_u32(&descriptor[CONTENTS_OFFSET_FIELD + 4..]);
    Ok(EntryDescriptor {
        name,
        contents_offset,
        contents_len,
    })
}

/// The byte range of `entry`'s contents in an archive of `data_len` bytes whose
/// content block starts at `data_start`.
pub(crate) fn entry_contents_range(
    entry: &EntryDescriptor,
    data_start: usize,
    data_len: usize,
) -> Result<Range<usize>, BrArchiveError> {
    let out_of_bounds = || BrArchiveError::EntryOutOfBounds(entry.name.to_string());
    let start = data_start
        .checked_add(entry.contents_offset as usize)
        .ok_or_else(out_of_bounds)?;
    let end = start
        .checked_add(entry.contents_len as usize)
        .ok_or_else(out_of_bounds)?;
    if end > data_len {
        return Err(out_of_bounds());
    }
    Ok(start..end)
}
