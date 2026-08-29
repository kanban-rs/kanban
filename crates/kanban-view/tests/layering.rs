/// A text match over the manifest catches only a direct dependency edge, not
/// a transitive arrival through some other crate.
#[test]
fn test_kanban_view_does_not_depend_on_kanban_service() {
    let manifest_path = concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml");
    let manifest =
        std::fs::read_to_string(manifest_path).expect("kanban-view Cargo.toml should be readable");
    assert!(
        !manifest.contains("kanban-service"),
        "kanban-view must stay free of kanban-service; found a reference in {manifest_path}"
    );
}
