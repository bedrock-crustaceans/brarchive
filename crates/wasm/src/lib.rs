//! Thin wasm-bindgen layer over the `brarchive` crate.
//!
//! The public JavaScript API lives in `mod.ts`; these functions are the raw
//! boundary it calls into. Entry contents cross the boundary as one
//! concatenated byte buffer plus a parallel array of lengths, which keeps the
//! ABI to plain typed arrays and lets binary (MCB) entries through untouched.

use brarchive::SerializeOptions;
use wasm_bindgen::prelude::*;

/// A decoded archive in flat form: `names[i]` owns the `lengths[i]` bytes of
/// `data` that follow the previous entry's bytes.
#[wasm_bindgen(getter_with_clone)]
pub struct RawEntries {
    pub names: Vec<String>,
    pub data: Vec<u8>,
    pub lengths: Vec<u32>,
}

fn err(e: impl std::fmt::Display) -> JsError {
    JsError::new(&e.to_string())
}

/// Encode entries into `.brarchive` bytes. `data` holds every entry's content
/// back to back and `lengths[i]` is the byte length of `names[i]`.
#[wasm_bindgen]
pub fn serialize(
    names: Vec<String>,
    data: &[u8],
    lengths: &[u32],
    dedup: bool,
) -> Result<Vec<u8>, JsError> {
    if names.len() != lengths.len() {
        return Err(JsError::new("names and lengths must have the same length"));
    }
    let total: usize = lengths.iter().map(|&l| l as usize).sum();
    if total != data.len() {
        return Err(JsError::new("lengths do not add up to the size of data"));
    }

    let mut offset = 0;
    let entries = names.into_iter().zip(lengths).map(|(name, &len)| {
        let content = &data[offset..offset + len as usize];
        offset += len as usize;
        (name, content)
    });

    brarchive::serialize_with(entries, SerializeOptions { dedup }).map_err(err)
}

/// Decode `.brarchive` bytes into flat entries. See [`RawEntries`].
#[wasm_bindgen]
pub fn deserialize(data: &[u8]) -> Result<RawEntries, JsError> {
    let entries = brarchive::entries(data).map_err(err)?;
    let mut names = Vec::with_capacity(entries.len());
    let mut lengths = Vec::with_capacity(entries.len());
    let mut flat = Vec::new();
    for entry in entries {
        let entry = entry.map_err(err)?;
        names.push(entry.name.to_string());
        lengths.push(entry.range.len() as u32);
        flat.extend_from_slice(&data[entry.range]);
    }
    Ok(RawEntries {
        names,
        data: flat,
        lengths,
    })
}

/// List entry names without decoding any content.
#[wasm_bindgen]
pub fn list(data: &[u8]) -> Result<Vec<String>, JsError> {
    brarchive::entries(data)
        .map_err(err)?
        .map(|entry| entry.map(|entry| entry.name.to_string()).map_err(err))
        .collect()
}
