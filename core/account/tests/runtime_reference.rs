use std::{collections::BTreeMap, path::PathBuf};

use account_core::{
    RAW_ACCOUNT_SCHEMA_VERSION, RawAccount,
    import::reliquary::import_reliquary_archive,
    playable::conversion::{ConversionRequest, convert_playable_account},
    reference::{PlayableData, TraceCatalog},
};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/reference")
}

#[test]
fn loads_an_external_snapshot_and_converts_an_empty_account() {
    let root = root();
    let data = PlayableData::from_dir(&root, &root.join("traces.json")).unwrap();
    assert_eq!(data.manifest.snapshot_id, "synthetic-account");
    assert_eq!(data.traces.snapshot_id, "synthetic-traces");
    let raw = RawAccount {
        schema_version: RAW_ACCOUNT_SCHEMA_VERSION,
        config_id: "synthetic".into(),
        game_version: Some("test".into()),
        data_snapshot: None,
        characters: vec![],
        light_cones: vec![],
        relics: vec![],
    };
    let result = convert_playable_account(
        &raw,
        &data,
        &ConversionRequest {
            config_id: "converted".into(),
            character_aliases: BTreeMap::new(),
        },
    )
    .unwrap();
    assert_eq!(result.characters.len(), 1);
    assert_eq!(result.characters[0].character_id, "synthetic-trial");
    assert_eq!(
        result.characters[0].light_cone.light_cone_id,
        "synthetic-cone"
    );
}

#[test]
fn missing_and_invalid_files_identify_the_failed_path() {
    let root = root();
    let missing = root.join("missing.json");
    assert_eq!(TraceCatalog::from_path(&missing).unwrap_err().path, missing);
    let invalid = root.join("characters.json");
    assert_eq!(TraceCatalog::from_path(&invalid).unwrap_err().path, invalid);
    let missing_root = root.join("missing");
    assert_eq!(
        PlayableData::from_dir(&missing_root, &root.join("traces.json"))
            .unwrap_err()
            .path,
        missing_root.join("manifest.json")
    );
}

#[test]
fn imports_using_the_supplied_catalog_and_rejects_unknown_bindings() {
    let catalog = TraceCatalog::from_path(&root().join("traces.json")).unwrap();
    let source = serde_json::json!({
        "source":"reliquary_archiver", "version":4, "build":"test",
        "characters":[{"id":"999991","level":1,"ascension":0,"eidolon":0,"ability_version":0,
          "skills":{"basic":2},"traces":{"bonus":true}}],
        "light_cones":[], "relics":[]
    });
    let imported = import_reliquary_archive(&source.to_string(), "test", &catalog).unwrap();
    assert_eq!(
        imported.account.characters[0].traces.skill_levels["999991001"],
        2
    );
    assert!(imported.account.characters[0].traces.node_unlocks["999991002"]);
    let mut unknown = source;
    unknown["characters"][0]["id"] = serde_json::json!("999992");
    assert!(import_reliquary_archive(&unknown.to_string(), "test", &catalog).is_err());
}
