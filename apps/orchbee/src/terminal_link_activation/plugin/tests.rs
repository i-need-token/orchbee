use super::select_entries;

#[test]
fn empty_catalog_is_a_definite_no_handler() {
    let result = select_entries::<()>(&[], "https://example.test", || Ok(())).unwrap();
    assert!(result.is_none());
}
