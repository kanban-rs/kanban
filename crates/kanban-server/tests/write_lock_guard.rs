const RAW_LOCK: &str = "state.ctx.lock()";

const ROUTE_MODULES: &[(&str, &str)] = &[
    ("boards", include_str!("../src/routes/boards.rs")),
    ("cards", include_str!("../src/routes/cards.rs")),
    ("cards_batch", include_str!("../src/routes/cards_batch.rs")),
    ("columns", include_str!("../src/routes/columns.rs")),
    ("events", include_str!("../src/routes/events.rs")),
    ("graph", include_str!("../src/routes/graph.rs")),
    ("prefixes", include_str!("../src/routes/prefixes.rs")),
    ("sprints", include_str!("../src/routes/sprints.rs")),
    (
        "sprints_lifecycle",
        include_str!("../src/routes/sprints_lifecycle.rs"),
    ),
    ("transfer", include_str!("../src/routes/transfer.rs")),
];

#[test]
fn test_no_route_module_acquires_the_raw_context_lock() {
    let offenders: Vec<&str> = ROUTE_MODULES
        .iter()
        .filter(|(_, src)| src.contains(RAW_LOCK))
        .map(|(name, _)| *name)
        .collect();

    assert!(
        offenders.is_empty(),
        "route modules still acquire the raw context lock instead of state.lock_for_write(client): {offenders:?}"
    );
}
