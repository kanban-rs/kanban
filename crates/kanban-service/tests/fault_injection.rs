//! Requires the `test-helpers` feature; run with
//! `cargo test -p kanban-service --features test-helpers`.
#![cfg(feature = "test-helpers")]

use std::sync::Arc;

use kanban_backend_memory::InMemoryStore;
use kanban_domain::data_store::DataStore;
use kanban_domain::{Board, Card, Column};
use kanban_persistence_sqlite::SqliteBackend;
use kanban_service::test_helpers::{faultable, BackendFactory, FaultInjectingBackend, ReadOp};
use kanban_service::KanbanBackend;

fn wrapped_in_memory() -> (FaultInjectingBackend, Board) {
    let inner = InMemoryStore::new();
    let board = Board::new("Seeded", None::<String>);
    inner.upsert_board(board.clone()).unwrap();
    (
        FaultInjectingBackend::new(Arc::new(inner) as Arc<dyn KanbanBackend>),
        board,
    )
}

#[test]
fn test_a_faulted_read_returns_an_error_from_the_wrapper() {
    let (backend, _board) = wrapped_in_memory();
    backend.fail("list_boards");
    assert!(backend.list_boards().is_err());
}

#[test]
fn test_an_unfaulted_read_delegates_to_the_wrapped_backend() {
    let (backend, board) = wrapped_in_memory();
    let boards = backend.list_boards().unwrap();
    assert_eq!(boards.len(), 1);
    assert_eq!(boards[0].id, board.id);
    assert_eq!(boards[0].name, "Seeded");
}

#[test]
fn test_a_fault_on_one_method_does_not_affect_another() {
    let (backend, _board) = wrapped_in_memory();
    backend.fail("get_card");
    assert!(backend.get_card(uuid::Uuid::new_v4()).is_err());
    assert!(backend.list_boards().is_ok());
}

#[test]
fn test_clearing_a_fault_restores_the_read() {
    let (backend, _board) = wrapped_in_memory();
    backend.fail("list_boards");
    assert!(backend.list_boards().is_err());
    backend.clear_faults();
    assert_eq!(backend.list_boards().unwrap().len(), 1);
}

#[test]
#[should_panic(expected = "not a faultable read")]
fn test_faulting_an_unknown_method_name_panics() {
    let (backend, _board) = wrapped_in_memory();
    backend.fail("list_bords");
}

#[test]
fn test_writes_are_never_faulted() {
    let (backend, _board) = wrapped_in_memory();
    for method in kanban_service::test_helpers::FAULTABLE_READS {
        backend.fail(method);
    }
    assert!(backend
        .upsert_board(Board::new("Written anyway", None::<String>))
        .is_ok());
}

/// The pinning test. A wrapper that omits a delegation for a *defaulted*
/// `DataStore` method silently resolves to the domain default instead of the
/// backend's override, which would make every downstream parity assertion test
/// the wrapper rather than the backend. SQLite is the backend with real
/// index-backed overrides of defaulted methods.
#[tokio::test(flavor = "multi_thread")]
async fn test_the_wrapper_delegates_a_backend_overridden_default_method() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("pin.sqlite3");
    let inner: Arc<dyn KanbanBackend> = Arc::new(
        SqliteBackend::open(path.to_str().unwrap())
            .await
            .expect("open sqlite backend"),
    );

    let board = Board::new("Pinned", Some("PIN"));
    let column = Column::new(board.id, "Todo", 0);
    let mut card = Card::new(board.id, column.id, "Pinned card", 0);
    card.card_number = 7;
    card.prefix = "PIN".to_string();
    inner
        .upsert_prefix(kanban_domain::Prefix::new("PIN"))
        .unwrap();
    inner.upsert_board(board.clone()).unwrap();
    inner.upsert_column(column).unwrap();
    inner.upsert_card(card.clone()).unwrap();

    let backend = FaultInjectingBackend::new(inner.clone());

    let through_wrapper = backend.get_card_by_board_and_number(board.id, 7).unwrap();
    let direct = inner.get_card_by_board_and_number(board.id, 7).unwrap();

    assert_eq!(through_wrapper.as_ref().map(|c| c.id), Some(card.id));
    assert_eq!(
        through_wrapper.as_ref().map(|c| c.id),
        direct.as_ref().map(|c| c.id),
        "the wrapper must answer from the backend's override, not the trait default"
    );

    // The equality above cannot fail on its own: the trait default is
    // `self.list_all_cards().find(..)`, which routes back through the
    // wrapper's own delegated `list_all_cards` and returns the same answer.
    // The side channel is what discriminates. `list_all_cards` is recorded, so
    // a missing delegation shows up as a non-zero count here.
    assert_eq!(
        backend.op_count("list_all_cards"),
        0,
        "a missing delegation would have fallen back to the trait default, \
         which reaches the backend via list_all_cards"
    );
}

#[test]
fn test_an_intercepted_read_is_recorded_in_call_order() {
    let (backend, _board) = wrapped_in_memory();
    let card_id = uuid::Uuid::new_v4();
    backend.list_boards().unwrap();
    backend.get_card(card_id).unwrap();
    assert_eq!(
        backend.ops(),
        vec![
            ReadOp {
                method: "list_boards",
                ids: vec![],
            },
            ReadOp {
                method: "get_card",
                ids: vec![card_id],
            },
        ]
    );
}

#[test]
fn test_a_faulted_read_is_still_recorded() {
    let (backend, _board) = wrapped_in_memory();
    let card_id = uuid::Uuid::new_v4();
    backend.fail("get_card");
    assert!(backend.get_card(card_id).is_err());
    assert_eq!(
        backend.ops(),
        vec![ReadOp {
            method: "get_card",
            ids: vec![card_id],
        }]
    );
}

#[test]
fn test_a_write_is_not_recorded() {
    let (backend, _board) = wrapped_in_memory();
    backend
        .upsert_board(Board::new("Unlogged", None::<String>))
        .unwrap();
    assert_eq!(backend.ops(), vec![]);
}

#[test]
fn test_clear_ops_empties_the_log() {
    let (backend, _board) = wrapped_in_memory();
    backend.list_boards().unwrap();
    assert_eq!(backend.ops().len(), 1);
    backend.clear_ops();
    assert_eq!(backend.ops(), vec![]);
}

#[test]
fn test_an_unread_method_is_absent_from_the_log() {
    let (backend, _board) = wrapped_in_memory();
    backend.list_boards().unwrap();
    assert_eq!(backend.op_count("list_boards"), 1);
    assert_eq!(backend.op_count("get_card"), 0);
}

/// `faultable` must not cache by path. A durable backend hands back a fresh
/// store on each open of the same path, sharing on-disk state, and every reload
/// assertion in the contract suite depends on that. Caching would turn a reopen
/// into "the same in-memory instance" and silently defeat them.
#[tokio::test(flavor = "multi_thread")]
async fn test_faultable_preserves_reload_semantics_for_a_durable_backend() {
    use kanban_persistence_json::{JsonDataStore, JsonFileStore};

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("reload.json");

    let json: BackendFactory = Box::new(|p: &std::path::Path| {
        Arc::new(JsonDataStore::new(Arc::new(JsonFileStore::new(p)))) as Arc<dyn KanbanBackend>
    });
    let (factory, handles) = faultable(json);

    let first = factory(&path);
    let board = Board::new("Persisted", None::<String>);
    first.upsert_board(board.clone()).unwrap();
    first.flush().await.unwrap();

    let second = factory(&path);
    assert!(
        !Arc::ptr_eq(&first, &second),
        "a reopen must build a fresh backend, not return the cached one"
    );
    assert_eq!(
        second.get_board(board.id).unwrap().map(|b| b.id),
        Some(board.id),
        "the second open must read the state the first one flushed to disk"
    );

    let recorded = handles.lock().unwrap();
    assert_eq!(
        recorded.get(&path).map(|v| v.len()),
        Some(2),
        "both wrappers must be reachable, in construction order"
    );
}
