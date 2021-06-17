/// Stubs for things that might break validation

/// This test shows a way to create a delete with a rejected header
/// and keep it in an agents cache.
/// The same test can be done with an update.
#[test]
#[ignore = "stub"]
fn get_rejected_delete_into_cache_as_valid() {
    // - Create a timestamp earlier then the last chain item.
    // - Send this to sys validation results in rejected header.
    // - Send the delete op to Bob.
    // - Get Alice to retrieve the original element from bob.
    // - Bob will return the original element as deleted.
    // - Now Bob has the original element as deleted in their cache.
}
