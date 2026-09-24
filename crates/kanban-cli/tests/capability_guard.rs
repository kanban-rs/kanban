use kanban_cli::capabilities::DECLINED_CAPABILITIES;
use kanban_domain::{capability_violations, CAPABILITY_MANIFEST};

const SOURCES: &[&str] = &[
    include_str!("../src/cli.rs"),
    include_str!("../src/app.rs"),
    include_str!("../src/handlers/board.rs"),
    include_str!("../src/handlers/card.rs"),
    include_str!("../src/handlers/column.rs"),
    include_str!("../src/handlers/export.rs"),
    include_str!("../src/handlers/migrate.rs"),
    include_str!("../src/handlers/relation.rs"),
    include_str!("../src/handlers/sprint.rs"),
];

#[test]
fn test_every_capability_is_routed_or_declined() {
    let violations = capability_violations(CAPABILITY_MANIFEST, SOURCES, DECLINED_CAPABILITIES);
    assert!(
        violations.is_empty(),
        "cli capability wiring gaps: {violations:#?}"
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
