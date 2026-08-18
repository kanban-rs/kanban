mod helpers;

use helpers::{assert_ops, CountingBackend, ReadOp};
use kanban_backend_memory::InMemoryStore;
use kanban_domain::{Board, Card, Column, DataStore};
use std::sync::Arc;
use uuid::Uuid;

fn seeded_store() -> (Arc<InMemoryStore>, Uuid, Uuid) {
    let store = Arc::new(InMemoryStore::new());
    let board_a = Board::new("Board A", None::<String>);
    let board_b = Board::new("Board B", None::<String>);
    let column = Column::new(board_a.id, "Todo", 0);
    let card = Card::new(board_a.id, column.id, "Card A", 0);
    let (board_a_id, card_id) = (board_a.id, card.id);
    store.upsert_board(board_a).unwrap();
    store.upsert_board(board_b).unwrap();
    store.upsert_column(column).unwrap();
    store.upsert_card(card).unwrap();
    (store, board_a_id, card_id)
}

#[test]
fn test_read_op_log_records_get_card_with_its_id() {
    let (store, _board_id, card_id) = seeded_store();
    let (backend, _reads, ops) = CountingBackend::wrap(store);

    backend.as_data_store().get_card(card_id).unwrap();

    assert_ops(
        &ops,
        &[ReadOp {
            method: "get_card",
            ids: vec![card_id],
        }],
    );
}

#[test]
fn test_read_op_log_records_collection_reads_with_empty_ids() {
    let (store, _board_id, _card_id) = seeded_store();
    let (backend, _reads, ops) = CountingBackend::wrap(store);

    backend.as_data_store().list_boards().unwrap();

    assert_ops(
        &ops,
        &[ReadOp {
            method: "list_boards",
            ids: vec![],
        }],
    );
}

#[test]
fn test_read_op_log_preserves_call_order() {
    let (store, _board_id, card_id) = seeded_store();
    let (backend, _reads, ops) = CountingBackend::wrap(store);

    backend.as_data_store().get_card(card_id).unwrap();
    backend.as_data_store().list_boards().unwrap();

    assert_ops(
        &ops,
        &[
            ReadOp {
                method: "get_card",
                ids: vec![card_id],
            },
            ReadOp {
                method: "list_boards",
                ids: vec![],
            },
        ],
    );
}

#[test]
fn test_read_op_log_ignores_writes() {
    let (store, board_id, card_id) = seeded_store();
    let (backend, _reads, ops) = CountingBackend::wrap(store);

    let card = Card::new(board_id, Uuid::new_v4(), "Card B", 1);
    backend.as_data_store().upsert_card(card).unwrap();
    backend.as_data_store().delete_card(card_id).unwrap();

    assert_ops(&ops, &[]);
}

#[test]
fn test_assert_ops_accepts_an_exactly_matching_log() {
    let (store, _board_id, card_id) = seeded_store();
    let (backend, _reads, ops) = CountingBackend::wrap(store);

    backend.as_data_store().get_card(card_id).unwrap();

    assert_ops(
        &ops,
        &[ReadOp {
            method: "get_card",
            ids: vec![card_id],
        }],
    );
}

#[test]
fn test_assert_ops_rejects_an_unexpected_extra_op() {
    let (store, _board_id, card_id) = seeded_store();
    let (backend, _reads, ops) = CountingBackend::wrap(store);

    backend.as_data_store().get_card(card_id).unwrap();
    backend.as_data_store().list_boards().unwrap();

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        assert_ops(
            &ops,
            &[ReadOp {
                method: "get_card",
                ids: vec![card_id],
            }],
        );
    }));

    assert!(result.is_err());
}

#[test]
fn test_assert_ops_rejects_a_missing_expected_op() {
    let (store, _board_id, card_id) = seeded_store();
    let (backend, _reads, ops) = CountingBackend::wrap(store);

    backend.as_data_store().get_card(card_id).unwrap();

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        assert_ops(
            &ops,
            &[
                ReadOp {
                    method: "get_card",
                    ids: vec![card_id],
                },
                ReadOp {
                    method: "list_boards",
                    ids: vec![],
                },
            ],
        );
    }));

    assert!(result.is_err());
}

#[test]
fn test_counting_backend_counter_still_counts_every_recorded_read() {
    use std::sync::atomic::Ordering;

    let (store, board_id, card_id) = seeded_store();
    let (backend, reads, _ops) = CountingBackend::wrap(store);

    backend.as_data_store().get_card(card_id).unwrap();
    backend.as_data_store().list_boards().unwrap();
    let card = Card::new(board_id, Uuid::new_v4(), "Card C", 2);
    backend.as_data_store().upsert_card(card).unwrap();

    assert_eq!(reads.load(Ordering::SeqCst), 2);
}
