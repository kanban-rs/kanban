use kanban_backend_http::HttpBackend;
use kanban_domain::{
    ArchivedBoard, ArchivedCard, Board, Card, Column, DataStore, DependencyGraph, KanbanError,
    Prefix, Sprint,
};
use uuid::Uuid;

fn unreachable_backend() -> HttpBackend {
    HttpBackend::new("http://127.0.0.1:1").unwrap()
}

fn assert_declines_under_its_own_name<T>(result: kanban_domain::KanbanResult<T>, expected: &str) {
    match result {
        Err(KanbanError::Unsupported { operation }) => {
            assert_eq!(operation, expected, "declined under the wrong name");
        }
        Err(other) => panic!("expected Unsupported({expected:?}), got {other:?}"),
        Ok(_) => panic!("expected Unsupported({expected:?}), got Ok"),
    }
}

#[test]
fn test_get_card_by_board_and_number_declines_under_its_own_name() {
    let backend = unreachable_backend();
    let result = backend.get_card_by_board_and_number(Uuid::new_v4(), 1);
    assert_declines_under_its_own_name(result, "get_card_by_board_and_number");
}

#[test]
fn test_upsert_prefix_declines_under_its_own_name() {
    let backend = unreachable_backend();
    let result = backend.upsert_prefix(Prefix::new("kan"));
    assert_declines_under_its_own_name(result, "upsert_prefix");
}

#[test]
fn test_list_all_cards_and_siblings_stay_unsupported_under_their_own_names() {
    let backend = unreachable_backend();
    assert_declines_under_its_own_name(backend.list_all_cards(), "list_all_cards");
    assert_declines_under_its_own_name(backend.list_all_columns(), "list_all_columns");
    assert_declines_under_its_own_name(backend.list_all_sprints(), "list_all_sprints");
}

#[test]
fn test_set_graph_stays_declined_under_its_own_name() {
    let backend = unreachable_backend();
    let result = backend.set_graph(DependencyGraph::default());
    assert_declines_under_its_own_name(result, "set_graph");
}

#[test]
fn test_get_graph_no_longer_declines_it_reaches_the_transport() {
    let backend = unreachable_backend();
    let err = backend
        .get_graph()
        .expect_err("no server is listening on port 1");
    assert!(err.is_transport(), "expected transport error, got {err:?}");
    assert!(!err.is_unsupported());
}

#[test]
fn test_list_archived_boards_no_longer_declines_it_reaches_the_transport() {
    let backend = unreachable_backend();
    let err = backend
        .list_archived_boards()
        .expect_err("no server is listening on port 1");
    assert!(err.is_transport(), "expected transport error, got {err:?}");
    assert!(!err.is_unsupported());
}

#[test]
fn test_count_cards_in_column_filtered_live_only_arm_declines_under_its_own_name() {
    let backend = unreachable_backend();
    let result = backend
        .count_cards_in_column_filtered(Uuid::new_v4(), kanban_domain::ArchivedFilter::LiveOnly)
        .map(|_| ());
    assert_declines_under_its_own_name(result, "count_cards_in_column_filtered");
}

// Skips the conditional decliners `list_cards_by_column_filtered` and `count_cards_in_column_filtered`; their per-arm pins live above.
#[test]
fn test_every_declining_datastore_method_declines_under_its_own_name() {
    let backend = unreachable_backend();
    let now = chrono::Utc::now();
    let id = Uuid::new_v4();

    let cases: Vec<(&str, kanban_domain::KanbanResult<()>)> = vec![
        ("upsert_prefix", backend.upsert_prefix(Prefix::new("kan"))),
        (
            "upsert_board",
            backend.upsert_board(Board::new("b", None::<String>)),
        ),
        ("delete_board", backend.delete_board(id)),
        ("list_all_columns", backend.list_all_columns().map(|_| ())),
        (
            "upsert_column",
            backend.upsert_column(Column::new(id, "c", 0)),
        ),
        ("delete_column", backend.delete_column(id)),
        (
            "delete_columns_by_board",
            backend.delete_columns_by_board(id),
        ),
        ("list_all_cards", backend.list_all_cards().map(|_| ())),
        (
            "count_cards_in_column",
            backend.count_cards_in_column(id).map(|_| ()),
        ),
        (
            "count_cards_in_column_excluding",
            backend.count_cards_in_column_excluding(id, &[]).map(|_| ()),
        ),
        (
            "upsert_card",
            backend.upsert_card(Card::new(id, id, "t", 0)),
        ),
        ("delete_card", backend.delete_card(id)),
        (
            "delete_cards_by_columns",
            backend.delete_cards_by_columns(&[id]),
        ),
        (
            "clear_sprint_from_cards",
            backend.clear_sprint_from_cards(id, now),
        ),
        (
            "clear_sprint_from_archived_cards",
            backend.clear_sprint_from_archived_cards(id, now),
        ),
        (
            "get_archived_card",
            backend.get_archived_card(id).map(|_| ()),
        ),
        (
            "list_archived_cards",
            backend.list_archived_cards().map(|_| ()),
        ),
        (
            "insert_archived_card",
            backend.insert_archived_card(ArchivedCard::new(id, id)),
        ),
        ("delete_archived_card", backend.delete_archived_card(id)),
        (
            "get_archived_board",
            backend.get_archived_board(id).map(|_| ()),
        ),
        (
            "insert_archived_board",
            backend.insert_archived_board(ArchivedBoard::now(id)),
        ),
        ("delete_archived_board", backend.delete_archived_board(id)),
        ("unarchive_board", backend.unarchive_board(id)),
        ("list_all_sprints", backend.list_all_sprints().map(|_| ())),
        (
            "upsert_sprint",
            backend.upsert_sprint(Sprint::new(id, 1, None, None::<String>)),
        ),
        ("delete_sprint", backend.delete_sprint(id)),
        (
            "delete_sprints_by_board",
            backend.delete_sprints_by_board(id),
        ),
        ("set_graph", backend.set_graph(DependencyGraph::default())),
        (
            "modify_graph",
            backend.modify_graph(Box::new(|_graph| Ok(()))),
        ),
        (
            "get_card_by_board_and_number",
            backend.get_card_by_board_and_number(id, 1).map(|_| ()),
        ),
    ];

    assert_eq!(cases.len(), 30, "unconditional decliner census drifted");
    for (name, result) in cases {
        assert_declines_under_its_own_name(result, name);
    }
}

#[test]
fn test_conditional_decliners_decline_archived_filters_under_their_own_names() {
    let backend = unreachable_backend();
    let archived = kanban_domain::ArchivedFilter::ArchivedOnly;
    assert_declines_under_its_own_name(
        backend
            .list_cards_by_column_filtered(Uuid::new_v4(), archived)
            .map(|_| ()),
        "list_cards_by_column_filtered",
    );
    assert_declines_under_its_own_name(
        backend
            .count_cards_in_column_filtered(Uuid::new_v4(), archived)
            .map(|_| ()),
        "count_cards_in_column_filtered",
    );
}
