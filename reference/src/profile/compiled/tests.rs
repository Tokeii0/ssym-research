use super::*;

fn fixture() -> KernelProfile {
    let fields = BTreeMap::from([
        (
            "Unknown".into(),
            FieldProfile {
                offset: u64::MAX,
                size: None,
                bit_position: None,
                bit_length: None,
            },
        ),
        (
            "Wide".into(),
            FieldProfile {
                offset: 1u64 << 54,
                size: Some(u64::MAX),
                bit_position: Some(0),
                bit_length: Some(63),
            },
        ),
        (
            "Zero".into(),
            FieldProfile {
                offset: 0,
                size: Some(0),
                bit_position: None,
                bit_length: Some(0),
            },
        ),
        (
            "字段".into(),
            FieldProfile {
                offset: 32,
                size: Some(8),
                bit_position: Some(1),
                bit_length: Some(2),
            },
        ),
    ]);
    KernelProfile {
        schema_version: 3,
        pdb_guid: "01234567-89AB-CDEF-0123-456789ABCDEF".into(),
        pdb_age: u32::MAX,
        type_name: "_EPROCESS".into(),
        type_size: u64::MAX,
        fields: BTreeMap::from([("RootOnly".into(), fields["Wide"].clone())]),
        types: BTreeMap::from([
            (
                "_EMPTY".into(),
                TypeLayout {
                    size: 0,
                    fields: BTreeMap::new(),
                },
            ),
            (
                "_EPROCESS".into(),
                TypeLayout {
                    size: u64::MAX,
                    fields,
                },
            ),
        ]),
        symbols: BTreeMap::from([("First".into(), 0), ("Last".into(), u32::MAX)]),
    }
}

fn encoded() -> Vec<u8> {
    encode(&fixture(), "ntkrnlmp.pdb", [42; 32], 1).unwrap()
}

fn resign(bytes: &mut [u8]) {
    let digest = checksum(bytes);
    bytes[144..176].copy_from_slice(&digest);
}

#[test]
fn lossless_roundtrip_and_index_queries() {
    let bytes = encoded();
    let view = CompiledProfile::parse(&bytes).unwrap();
    assert_eq!(
        serde_json::to_value(view.to_profile()).unwrap(),
        serde_json::to_value(fixture()).unwrap()
    );
    assert_eq!(bytes, encoded(), "encoding must be deterministic");
    assert_eq!(view.pdb_sha256(), &[42; 32]);
    assert_eq!(view.field("RootOnly").unwrap().size, Some(u64::MAX));
    assert_eq!(
        view.type_field("_EPROCESS", "Wide").unwrap().offset,
        1u64 << 54
    );
    assert_eq!(view.type_field("_EPROCESS", "Unknown").unwrap().size, None);
    assert_eq!(view.type_field("_EPROCESS", "Zero").unwrap().size, Some(0));
    assert_eq!(
        view.type_field("_EPROCESS", "字段").unwrap().bit_length,
        Some(2)
    );
    assert_eq!(view.type_size("_EMPTY"), Some(0));
    assert_eq!(view.symbol_rva("Last"), Some(u32::MAX));
    assert_eq!(view.symbol_rva("First"), Some(0));
    for name in ["", "0", "Between", "Zzz", "不存在"] {
        assert!(view.symbol_rva(name).is_none());
        assert!(view.field(name).is_none());
    }
    assert!(view.type_field("_EMPTY", "Wide").is_none());
    assert!(view.type_field("_MISSING", "Wide").is_none());
    assert!(
        view.validate_identity("NTKRNLMP.PDB", "0123456789abcdef0123456789abcdef", 1)
            .is_ok()
    );
    assert!(
        view.validate_identity("ntkrnlmp.pdb", view.pdb_guid(), 2)
            .is_err()
    );
    assert!(
        view.validate_identity("tcpip.pdb", view.pdb_guid(), 1)
            .is_err()
    );
    assert!(
        view.validate_identity("ntkrnlmp.pdb", "FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF", 1)
            .is_err()
    );
    assert!(view.require_current_recipe().is_ok());
    assert_eq!(view.pdb_age(), u32::MAX);
    assert_eq!(view.original_age(), 1);
}

#[test]
fn rejects_corruption_and_every_truncation() {
    let bytes = encoded();
    for len in 0..bytes.len() {
        assert!(CompiledProfile::parse(&bytes[..len]).is_err(), "len={len}");
    }
    for pos in 0..bytes.len() {
        let mut broken = bytes.clone();
        broken[pos] ^= 1;
        assert!(CompiledProfile::parse(&broken).is_err(), "pos={pos}");
    }
    let mut trailing = bytes.clone();
    trailing.push(0);
    assert!(CompiledProfile::parse(&trailing).is_err());
}

#[test]
fn rejects_valid_checksum_but_invalid_structure() {
    let bytes = encoded();
    let fields = CompiledProfile::parse(&bytes).unwrap().sections.fields;
    for (position, value) in [
        (8, 2),
        (12, 0),
        (32, u32::MAX),
        (36, u32::MAX),
        (56, u32::MAX),
        (60, u32::MAX),
        (64, u32::MAX),
        (68, u32::MAX),
        (72, u32::MAX),
        (76, 1),
        (184, 0),
        (HEADER + 16, 0),
        (HEADER + TYPE + 20, u32::MAX),
        (fields + 24, 8),
    ] {
        let mut broken = bytes.clone();
        put32(&mut broken, position, value);
        resign(&mut broken);
        assert!(CompiledProfile::parse(&broken).is_err(), "pos={position}");
    }
    // Duplicate type index keys, well-formed bounds and digest.
    let mut duplicate = bytes.clone();
    duplicate.copy_within(HEADER..HEADER + 8, HEADER + TYPE);
    resign(&mut duplicate);
    assert!(CompiledProfile::parse(&duplicate).is_err());
    let mut utf8 = bytes.clone();
    let string_start = CompiledProfile::parse(&bytes).unwrap().sections.strings;
    utf8[string_start] = 0xff;
    resign(&mut utf8);
    assert!(CompiledProfile::parse(&utf8).is_err());
    let mut old_recipe = bytes.clone();
    old_recipe[112] ^= 1;
    resign(&mut old_recipe);
    assert!(
        CompiledProfile::parse(&old_recipe)
            .unwrap()
            .require_current_recipe()
            .is_err()
    );
}

#[test]
fn reads_file_and_empty_tables() {
    let mut profile = fixture();
    profile.types.clear();
    profile.fields.clear();
    profile.symbols.clear();
    let bytes = encode(&profile, "ntkrnlmp.pdb", [0; 32], 1).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.ssym");
    std::fs::write(&path, bytes).unwrap();
    let file = CompiledProfileFile::open(&path).unwrap();
    assert_eq!(
        serde_json::to_value(file.view().to_profile()).unwrap(),
        serde_json::to_value(profile).unwrap()
    );
    assert!(file.view().field("Missing").is_none());
    assert!(is_compiled_path(Path::new("TEST.SSYM")));
    assert!(!is_compiled_path(Path::new("test.json")));
}
