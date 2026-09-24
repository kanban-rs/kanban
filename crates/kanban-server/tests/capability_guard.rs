use kanban_domain::{capability_violations, CAPABILITY_MANIFEST};
use kanban_server::capabilities::DECLINED_CAPABILITIES;

const SOURCES: &[&str] = &[
    include_str!("../src/app.rs"),
    include_str!("../src/routes/boards.rs"),
    include_str!("../src/routes/cards.rs"),
    include_str!("../src/routes/cards_batch.rs"),
    include_str!("../src/routes/columns.rs"),
    include_str!("../src/routes/sprints.rs"),
    include_str!("../src/routes/sprints_lifecycle.rs"),
    include_str!("../src/routes/transfer.rs"),
    include_str!("../src/routes/graph.rs"),
    include_str!("../src/routes/prefixes.rs"),
    include_str!("../src/handlers/boards.rs"),
    include_str!("../src/handlers/cards.rs"),
    include_str!("../src/handlers/columns.rs"),
    include_str!("../src/handlers/sprints.rs"),
];

#[test]
fn test_every_capability_is_routed_or_declined() {
    let violations = capability_violations(CAPABILITY_MANIFEST, SOURCES, DECLINED_CAPABILITIES);
    assert!(
        violations.is_empty(),
        "server capability wiring gaps: {violations:#?}"
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
    let violations = capability_violations(&["frobnicate_card"], SOURCES, DECLINED_CAPABILITIES);
    assert_eq!(violations.len(), 1);
    assert!(violations[0].contains("frobnicate_card"));
}
