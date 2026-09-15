use kanban_domain::{capability_violations, CAPABILITY_MANIFEST};

const SOURCES: &[&str] = &[
    include_str!("../src/tools/board.rs"),
    include_str!("../src/tools/card_batch.rs"),
    include_str!("../src/tools/card_crud.rs"),
    include_str!("../src/tools/card_relations.rs"),
    include_str!("../src/tools/column.rs"),
    include_str!("../src/tools/sprint.rs"),
    include_str!("../src/tools/transfer.rs"),
];

const DECLINED: &[(&str, &str)] = &[];

#[test]
fn test_every_capability_is_routed_or_declined() {
    let violations = capability_violations(CAPABILITY_MANIFEST, SOURCES, DECLINED);
    assert!(
        violations.is_empty(),
        "mcp capability wiring gaps: {violations:#?}"
    );

    for src in SOURCES {
        let stripped = kanban_domain::capabilities::strip_test_modules(src);
        assert!(
            !stripped.contains("#[test]"),
            "stripped source still contains #[test]; the block-scoped stripper missed a cfg(test) module"
        );
    }
}

#[test]
fn test_unknown_capability_is_reported() {
    let violations = capability_violations(&["frobnicate_card"], SOURCES, DECLINED);
    assert_eq!(violations.len(), 1);
    assert!(violations[0].contains("frobnicate_card"));
}
