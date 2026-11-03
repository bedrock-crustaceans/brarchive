use brarchive::error::BrArchiveError;
use brarchive::{Entry, SerializeOptions};

/// Archives shipped with the game, from empty ones to a few hundred entries.
const GAME_ARCHIVES: &[(&str, &[u8])] = &[
    ("items", include_bytes!("items.brarchive")),
    ("entities", include_bytes!("entities.brarchive")),
    ("recipes", include_bytes!("recipes.brarchive")),
    (
        "animation_controllers",
        include_bytes!("fixtures/animation_controllers.brarchive"),
    ),
    (
        "animations",
        include_bytes!("fixtures/animations.brarchive"),
    ),
    (
        "attachables",
        include_bytes!("fixtures/attachables.brarchive"),
    ),
    ("ddui", include_bytes!("fixtures/ddui.brarchive")),
    ("entity", include_bytes!("fixtures/entity.brarchive")),
    ("models", include_bytes!("fixtures/models.brarchive")),
    ("particles", include_bytes!("fixtures/particles.brarchive")),
    (
        "render_controllers",
        include_bytes!("fixtures/render_controllers.brarchive"),
    ),
    ("sounds", include_bytes!("fixtures/sounds.brarchive")),
    ("textures", include_bytes!("fixtures/textures.brarchive")),
];

fn collect_entries(bytes: &[u8]) -> Vec<Entry<'_>> {
    brarchive::entries(bytes)
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
}

#[test]
fn entries_ranges_slice_to_deserialized_contents_of_game_archives() {
    for (name, bytes) in GAME_ARCHIVES {
        let entries = collect_entries(bytes);
        let decoded: Vec<(String, Vec<u8>)> = brarchive::deserialize(bytes).unwrap();
        assert_eq!(entries.len(), decoded.len(), "{name}: entry count");
        for (entry, (decoded_name, contents)) in entries.iter().zip(&decoded) {
            assert_eq!(entry.name, decoded_name, "{name}: entry name");
            assert_eq!(
                &bytes[entry.range.clone()],
                contents,
                "{name}: {decoded_name}"
            );
        }
        let listed = brarchive::list(bytes).unwrap();
        let names: Vec<&str> = entries.iter().map(|entry| entry.name).collect();
        assert_eq!(names, listed, "{name}: names agree with list");
    }
}

#[test]
fn entries_of_a_built_archive_point_past_the_entry_table() {
    let bytes = brarchive::serialize([("a.json", "1"), ("b.json", "22")]).unwrap();
    // 16-byte header, two 256-byte descriptors, then the contents back to back.
    let data_start = 16 + 2 * 256;
    assert_eq!(
        collect_entries(&bytes),
        vec![
            Entry {
                name: "a.json",
                range: data_start..data_start + 1,
            },
            Entry {
                name: "b.json",
                range: data_start + 1..data_start + 3,
            },
        ]
    );
}

#[test]
fn entries_of_deduplicated_contents_share_a_range() {
    let bytes = brarchive::serialize_with(
        [("a.json", "same"), ("b.json", "same")],
        SerializeOptions { dedup: true },
    )
    .unwrap();
    let entries = collect_entries(&bytes);
    assert_eq!(entries[0].range, entries[1].range);
}

#[test]
fn entries_reports_its_exact_length() {
    let bytes = brarchive::serialize([("a", "1"), ("b", "2"), ("c", "3")]).unwrap();
    let mut entries = brarchive::entries(&bytes).unwrap();
    assert_eq!(entries.len(), 3);
    entries.next();
    assert_eq!(entries.len(), 2);
}

#[test]
fn entries_truncated_header_is_an_error() {
    let bytes = brarchive::serialize([("k", "v")]).unwrap();
    assert!(matches!(
        brarchive::entries(&bytes[..10]),
        Err(BrArchiveError::TruncatedHeader(10))
    ));
}

#[test]
fn entries_count_past_buffer_is_an_error() {
    let mut bytes = brarchive::serialize([("k", "v")]).unwrap();
    bytes[8..12].copy_from_slice(&1000u32.to_le_bytes());
    assert!(matches!(
        brarchive::entries(&bytes),
        Err(BrArchiveError::TruncatedEntryTable(1000))
    ));
}

#[test]
fn entries_range_past_buffer_is_an_error() {
    let mut bytes = brarchive::serialize([("k", "v"), ("ok", "fine")]).unwrap();
    // Entry 0's u32 length sits at 0xFC inside its descriptor.
    let len = 16 + 0xFC;
    bytes[len..len + 4].copy_from_slice(&1000u32.to_le_bytes());
    let items: Vec<_> = brarchive::entries(&bytes).unwrap().collect();
    assert_eq!(
        items[0],
        Err(BrArchiveError::EntryOutOfBounds("k".to_string()))
    );
    assert!(matches!(&items[1], Ok(Entry { name: "ok", .. })));
}
