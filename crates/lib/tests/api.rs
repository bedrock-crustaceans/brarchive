use brarchive::SerializeOptions;
use std::collections::{BTreeMap, HashMap};

#[test]
fn deserialize_into_hashmap() {
    let bytes = brarchive::serialize([("a.json".to_string(), "{}".to_string())]).unwrap();
    let map: HashMap<String, Vec<u8>> = brarchive::deserialize(&bytes).unwrap();
    assert_eq!(map["a.json"], b"{}");
}

#[test]
fn deserialize_into_vec() {
    let bytes = brarchive::serialize([
        ("a.json".to_string(), "1".to_string()),
        ("b.json".to_string(), "2".to_string()),
    ])
    .unwrap();
    let vec: Vec<(String, Vec<u8>)> = brarchive::deserialize(&bytes).unwrap();
    assert_eq!(vec.len(), 2);
    assert_eq!(vec[0], ("a.json".to_string(), b"1".to_vec()));
    assert_eq!(vec[1], ("b.json".to_string(), b"2".to_vec()));
}

#[test]
fn serialize_str_literals() {
    let bytes = brarchive::serialize([("hello.json", "{}")]).unwrap();
    let map: BTreeMap<String, Vec<u8>> = brarchive::deserialize(&bytes).unwrap();
    assert_eq!(map["hello.json"], b"{}");
}

#[test]
fn serialize_binary_content_round_trip() {
    // Content need not be valid UTF-8: binary bytes (e.g. compiled MCB entries)
    // must survive a round trip byte-for-byte.
    let mcb = vec![0x7Fu8, b'M', b'C', b'B', 0x01, 0x00, 0xD2, 0x20, 0xDE, 0x77];
    let bytes = brarchive::serialize([("death.json", mcb.as_slice())]).unwrap();
    let map: BTreeMap<String, Vec<u8>> = brarchive::deserialize(&bytes).unwrap();
    assert_eq!(map["death.json"], mcb);
}

#[test]
fn serialize_btreemap_ref() {
    let map = BTreeMap::from([("x.json".to_string(), "data".to_string())]);
    let bytes = brarchive::serialize(&map).unwrap();
    let result: BTreeMap<String, Vec<u8>> = brarchive::deserialize(&bytes).unwrap();
    assert_eq!(result["x.json"], b"data");
}

#[test]
fn serialize_with_dedup_smaller_output() {
    let data = vec![
        ("a.json".to_string(), "same".to_string()),
        ("b.json".to_string(), "same".to_string()),
    ];
    let without = brarchive::serialize(data.clone()).unwrap();
    let with_dedup = brarchive::serialize_with(data, SerializeOptions { dedup: true }).unwrap();
    assert!(with_dedup.len() < without.len());
}

#[test]
fn serialize_with_dedup_round_trip() {
    let data = vec![
        ("a.json".to_string(), "shared".to_string()),
        ("b.json".to_string(), "shared".to_string()),
        ("c.json".to_string(), "unique".to_string()),
    ];
    let bytes = brarchive::serialize_with(data, SerializeOptions { dedup: true }).unwrap();
    let result: BTreeMap<String, Vec<u8>> = brarchive::deserialize(&bytes).unwrap();
    assert_eq!(result["a.json"], b"shared");
    assert_eq!(result["b.json"], b"shared");
    assert_eq!(result["c.json"], b"unique");
}

#[test]
fn serialize_with_no_dedup_matches_serialize() {
    let data = vec![("a.json".to_string(), "content".to_string())];
    let a = brarchive::serialize(data.clone()).unwrap();
    let b = brarchive::serialize_with(data, SerializeOptions { dedup: false }).unwrap();
    assert_eq!(a, b);
}
