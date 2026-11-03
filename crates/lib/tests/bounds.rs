//! Truncated or lying archives must come back as errors, never as a panic or an
//! out-of-bounds read.

use brarchive::error::BrArchiveError;
use brarchive::Entry;
use std::collections::BTreeMap;

/// The u32 entry count lives at bytes 8..12 of the header.
const COUNT_FIELD: usize = 8;
/// The first descriptor starts right after the 16-byte header.
const FIRST_DESCRIPTOR: usize = 16;
/// Inside a descriptor, the u32 content offset is at 0xF8 and the u32 length at 0xFC.
const OFFSET_FIELD: usize = 0xF8;
const LEN_FIELD: usize = 0xFC;

fn one_entry_archive() -> Vec<u8> {
    brarchive::serialize([("k", "v")]).unwrap()
}

fn deserialize(bytes: &[u8]) -> Result<BTreeMap<String, Vec<u8>>, BrArchiveError> {
    brarchive::deserialize(bytes)
}

/// The first item `entries` yields for `bytes`, whose header and table are sound.
fn first_entry(bytes: &[u8]) -> Result<Entry<'_>, BrArchiveError> {
    brarchive::entries(bytes).unwrap().next().unwrap()
}

#[test]
fn entries_truncated_header_is_an_error() {
    let bytes = one_entry_archive();
    assert!(matches!(
        brarchive::entries(&bytes[..10]),
        Err(BrArchiveError::TruncatedHeader(10))
    ));
}

#[test]
fn deserialize_truncated_header_is_an_error() {
    let bytes = one_entry_archive();
    assert!(matches!(
        deserialize(&bytes[..10]),
        Err(BrArchiveError::TruncatedHeader(10))
    ));
}

#[test]
fn entries_count_past_buffer_is_an_error() {
    let mut bytes = one_entry_archive();
    bytes[COUNT_FIELD..COUNT_FIELD + 4].copy_from_slice(&1000u32.to_le_bytes());
    assert!(matches!(
        brarchive::entries(&bytes),
        Err(BrArchiveError::TruncatedEntryTable(1000))
    ));
}

#[test]
fn deserialize_count_past_buffer_is_an_error() {
    let mut bytes = one_entry_archive();
    bytes[COUNT_FIELD..COUNT_FIELD + 4].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(matches!(
        deserialize(&bytes),
        Err(BrArchiveError::TruncatedEntryTable(u32::MAX))
    ));
}

#[test]
fn entries_descriptor_cut_inside_name_is_an_error() {
    let bytes = one_entry_archive();
    assert!(matches!(
        brarchive::entries(&bytes[..FIRST_DESCRIPTOR + 1]),
        Err(BrArchiveError::TruncatedEntryTable(1))
    ));
}

#[test]
fn entries_name_length_past_maximum_is_an_error() {
    let mut bytes = one_entry_archive();
    bytes[FIRST_DESCRIPTOR] = 248;
    assert!(matches!(
        first_entry(&bytes),
        Err(BrArchiveError::EntryNameTooLong(248))
    ));
}

#[test]
fn entries_non_utf8_name_is_an_error() {
    let mut bytes = one_entry_archive();
    bytes[FIRST_DESCRIPTOR + 1] = 0xFF;
    assert!(matches!(
        first_entry(&bytes),
        Err(BrArchiveError::Utf8Error(_))
    ));
}

#[test]
fn entries_range_past_buffer_is_an_error_and_iteration_goes_on() {
    let mut bytes = brarchive::serialize([("k", "v"), ("ok", "fine")]).unwrap();
    let len = FIRST_DESCRIPTOR + LEN_FIELD;
    bytes[len..len + 4].copy_from_slice(&1000u32.to_le_bytes());
    let items: Vec<_> = brarchive::entries(&bytes).unwrap().collect();
    assert_eq!(
        items[0],
        Err(BrArchiveError::EntryOutOfBounds("k".to_string()))
    );
    assert!(matches!(&items[1], Ok(Entry { name: "ok", .. })));
}

#[test]
fn deserialize_entry_len_past_buffer_is_an_error() {
    let mut bytes = one_entry_archive();
    let field = FIRST_DESCRIPTOR + LEN_FIELD;
    bytes[field..field + 4].copy_from_slice(&1000u32.to_le_bytes());
    assert_eq!(
        deserialize(&bytes),
        Err(BrArchiveError::EntryOutOfBounds("k".to_string()))
    );
}

#[test]
fn deserialize_entry_offset_past_buffer_is_an_error() {
    let mut bytes = one_entry_archive();
    let offset = FIRST_DESCRIPTOR + OFFSET_FIELD;
    bytes[offset..offset + 4].copy_from_slice(&u32::MAX.to_le_bytes());
    let len = FIRST_DESCRIPTOR + LEN_FIELD;
    bytes[len..len + 4].copy_from_slice(&u32::MAX.to_le_bytes());
    assert_eq!(
        deserialize(&bytes),
        Err(BrArchiveError::EntryOutOfBounds("k".to_string()))
    );
}
