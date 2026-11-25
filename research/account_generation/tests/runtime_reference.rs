use account_generator::{
    AccountReference, AccountRequest, ChaCha8Rng, ReferenceOverrides, SeedableRng, generate_account,
};
use std::path::PathBuf;

#[test]
fn loads_external_catalog_and_generates_reproducibly() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/reference");
    let reference =
        AccountReference::from_dir(&root, None, &ReferenceOverrides::default()).unwrap();
    let request = AccountRequest::new("synthetic", 0);
    let first = generate_account(&request, &reference, &mut ChaCha8Rng::seed_from_u64(42)).unwrap();
    let second =
        generate_account(&request, &reference, &mut ChaCha8Rng::seed_from_u64(42)).unwrap();
    assert_eq!(first, second);
    assert_eq!(
        first.account.data_snapshot.as_deref(),
        Some("synthetic-generator")
    );
    assert!(first.account.characters.is_empty());
    let populated = AccountRequest::new("synthetic-populated", 1);
    let first =
        generate_account(&populated, &reference, &mut ChaCha8Rng::seed_from_u64(42)).unwrap();
    let second =
        generate_account(&populated, &reference, &mut ChaCha8Rng::seed_from_u64(42)).unwrap();
    assert_eq!(first, second);
    assert_eq!(first.account.characters[0].character_id, "999991");
    assert_eq!(first.account.light_cones[0].light_cone_id, "299991");
    assert!(
        AccountReference::from_dir(&root.join("missing"), None, &ReferenceOverrides::default())
            .is_err()
    );
}
