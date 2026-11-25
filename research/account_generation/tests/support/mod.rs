#![allow(dead_code)]

use account_core::reference::{PlayableData, TraceCatalog};
use account_generator::{AccountReference, ReferenceOverrides};
use std::{fs, path::PathBuf};

fn data_root() -> PathBuf {
    PathBuf::from(
        std::env::var_os("HYACIMICON_DATA_DIR")
            .expect("Set HYACIMICON_DATA_DIR to the runtime snapshot directory"),
    )
}

pub fn trace_catalog() -> TraceCatalog {
    TraceCatalog::from_path(&data_root().join("account/reliquary-traces-v4.json")).unwrap()
}

pub fn playable_data() -> PlayableData {
    let root = data_root().join("account");
    PlayableData::from_dir(
        &root.join("s2-v4.7"),
        &root.join("reliquary-traces-v4.json"),
    )
    .unwrap()
}

pub fn account_reference() -> AccountReference {
    AccountReference::from_dir(
        &data_root().join("account-generation"),
        Some(&trace_catalog()),
        &ReferenceOverrides::default(),
    )
    .unwrap()
}

pub fn read_reference(path: &str) -> String {
    fs::read_to_string(data_root().join("account-generation").join(path)).unwrap()
}
