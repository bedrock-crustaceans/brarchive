use crate::error::BrArchiveError;
use crate::versions::v1;
use crate::versions::{ENTRY_NAME_LEN_MAX, MAGIC, VERSIONS};
use byteorder::{LittleEndian, WriteBytesExt};
use std::io::Write;
use std::iter::FusedIterator;
use std::ops::Range;
use std::slice::ChunksExact;

pub mod error;
pub(crate) mod versions;

/// Options controlling serialization behavior.
#[derive(Debug, Clone, Default)]
pub struct SerializeOptions {
    /// Deduplicate entries with identical content by reusing their content_offset/len.
    pub dedup: bool,
}

/// Serialize any iterable of key/value pairs into .brarchive bytes.
///
/// Keys are entry names (must be valid UTF-8, max 247 bytes); values are raw
/// content bytes and may hold arbitrary binary data (e.g. Mojang's compiled
/// `MCB` entries), not just UTF-8 text.
pub fn serialize<I, K, V>(data: I) -> Result<Vec<u8>, BrArchiveError>
where
    I: IntoIterator<Item = (K, V)>,
    K: AsRef<str>,
    V: AsRef<[u8]>,
{
    serialize_with(data, SerializeOptions::default())
}

/// Serialize with explicit options.
pub fn serialize_with<I, K, V>(
    data: I,
    options: SerializeOptions,
) -> Result<Vec<u8>, BrArchiveError>
where
    I: IntoIterator<Item = (K, V)>,
    K: AsRef<str>,
    V: AsRef<[u8]>,
{
    let data: Vec<(K, V)> = data.into_iter().collect();
    let mut buf = Vec::new();

    // Header
    let entry_count =
        u32::try_from(data.len()).map_err(|_| BrArchiveError::TooManyEntries(data.len()))?;
    buf.write_u64::<LittleEndian>(MAGIC)?;
    buf.write_u32::<LittleEndian>(entry_count)?;
    buf.write_u32::<LittleEndian>(*VERSIONS.last().unwrap())?;

    // Compute offsets
    let mut current_offset: u32 = 0;
    // (content_offset, content_len, bytes_to_write_or_none)
    let mut entries: Vec<(u32, u32, Option<Vec<u8>>)> = Vec::with_capacity(data.len());

    if options.dedup {
        let mut content_index: std::collections::HashMap<Vec<u8>, u32> =
            std::collections::HashMap::new();
        for (_, content) in &data {
            let bytes = content.as_ref().to_vec();
            let len = bytes.len() as u32;
            if let Some(&existing_offset) = content_index.get(&bytes) {
                entries.push((existing_offset, len, None));
            } else {
                content_index.insert(bytes.clone(), current_offset);
                entries.push((current_offset, len, Some(bytes)));
                current_offset = current_offset
                    .checked_add(len)
                    .ok_or(BrArchiveError::ContentTooLarge)?;
            }
        }
    } else {
        for (_, content) in &data {
            let bytes = content.as_ref().to_vec();
            let len = bytes.len() as u32;
            entries.push((current_offset, len, Some(bytes)));
            current_offset = current_offset
                .checked_add(len)
                .ok_or(BrArchiveError::ContentTooLarge)?;
        }
    }

    // Write descriptors
    for ((name, _), (offset, len, _)) in data.iter().zip(entries.iter()) {
        let name_bytes = name.as_ref().as_bytes();
        if name_bytes.len() > ENTRY_NAME_LEN_MAX {
            return Err(BrArchiveError::EntryNameTooLong(name_bytes.len()));
        }
        buf.write_u8(name_bytes.len() as u8)?;
        let mut name_buf = [0u8; ENTRY_NAME_LEN_MAX];
        name_buf[..name_bytes.len()].copy_from_slice(name_bytes);
        buf.write_all(&name_buf)?;
        buf.write_u32::<LittleEndian>(*offset)?;
        buf.write_u32::<LittleEndian>(*len)?;
    }

    // Write content
    for (_, _, maybe_bytes) in entries {
        if let Some(bytes) = maybe_bytes {
            buf.extend_from_slice(&bytes);
        }
    }

    Ok(buf)
}

/// One entry of a .brarchive file: its name and where its contents sit in the
/// archive bytes.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Entry<'a> {
    /// The entry name, borrowed from the archive.
    pub name: &'a str,
    /// The byte range of the contents in the archive, checked to lie inside it.
    pub range: Range<usize>,
}

/// Iterator over the entries of a .brarchive file, in archive order. Returned
/// by [`entries`].
#[derive(Debug, Clone)]
pub struct Entries<'a> {
    data: &'a [u8],
    descriptors: ChunksExact<'a, u8>,
    data_start: usize,
}

impl<'a> Iterator for Entries<'a> {
    type Item = Result<Entry<'a>, BrArchiveError>;

    fn next(&mut self) -> Option<Self::Item> {
        let descriptor = self.descriptors.next()?;
        Some(v1::read_entry_descriptor(descriptor).and_then(|entry| {
            let range = v1::entry_contents_range(&entry, self.data_start, self.data.len())?;
            Ok(Entry {
                name: entry.name,
                range,
            })
        }))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.descriptors.size_hint()
    }
}

impl ExactSizeIterator for Entries<'_> {}

impl FusedIterator for Entries<'_> {}

/// Iterate over the entries of a .brarchive file without copying any content.
///
/// The header and the entry table are checked up front, so a truncated archive
/// or one whose entry count runs past its end is an error here. Each entry is
/// then checked on its own: a bad name or a content range past the end of
/// `data` is an error item, and iteration carries on with the next entry.
/// `&data[entry.range]` is the entry's contents.
///
/// ```rust
/// # fn main() -> Result<(), brarchive::error::BrArchiveError> {
/// let bytes = brarchive::serialize([("entity.json", "{}")])?;
/// for entry in brarchive::entries(&bytes)? {
///     let entry = entry?;
///     assert_eq!(entry.name, "entity.json");
///     assert_eq!(&bytes[entry.range], b"{}");
/// }
/// # Ok(())
/// # }
/// ```
pub fn entries(data: &[u8]) -> Result<Entries<'_>, BrArchiveError> {
    let header = v1::read_header(data)?;
    let (table, data_start) = v1::entry_table(data, &header)?;
    Ok(Entries {
        data,
        descriptors: table.chunks_exact(v1::DESCRIPTOR_LEN),
        data_start,
    })
}

/// Deserialize a .brarchive file into any collection constructible from
/// `(String, Vec<u8>)` pairs.
///
/// Entry content is returned as raw bytes because archives may contain binary
/// entries (e.g. Mojang's compiled `MCB` files); callers that expect UTF-8 text
/// can convert with [`String::from_utf8`].
///
/// Use a type annotation to select the output type:
/// ```rust
/// # fn main() -> Result<(), brarchive::error::BrArchiveError> {
/// # let bytes = brarchive::serialize([("k", "v")])?;
/// let map: std::collections::BTreeMap<String, Vec<u8>> = brarchive::deserialize(&bytes)?;
/// assert_eq!(map["k"], b"v");
/// # Ok(())
/// # }
/// ```
pub fn deserialize<C>(data: &[u8]) -> Result<C, BrArchiveError>
where
    C: FromIterator<(String, Vec<u8>)>,
{
    entries(data)?
        .map(|entry| entry.map(|entry| (entry.name.to_string(), data[entry.range].to_vec())))
        .collect()
}
