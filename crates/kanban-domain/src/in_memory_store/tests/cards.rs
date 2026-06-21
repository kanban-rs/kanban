use crate::data_store::DataStore;
use crate::in_memory_store::test_support::{make_board, make_card, make_column};
use crate::{DependencyGraph, InMemoryStore, Snapshot};
use uuid::Uuid;

// Card CRUD

#[test]
fn test_upsert_and_get_card() {
    let store = InMemoryStore::new();
    let mut board = make_board("B");
    let col = make_column(board.id, "Col", 0);
    let card = make_card(&mut board, col.id, "Card", 0);
    let card_id = card.id;
    store.upsert_card(card).unwrap();

    let fetched = store.get_card(card_id).unwrap().unwrap();
    assert_eq!(fetched.id, card_id);
    assert_eq!(fetched.title, "Card");
}

#[test]
fn test_list_cards_by_column() {
    let store = InMemoryStore::new();
    let mut board = make_board("B");
    let col1 = make_column(board.id, "C1", 0);
    let col2 = make_column(board.id, "C2", 1);
    let card1 = make_card(&mut board, col1.id, "Card1", 0);
    let card2 = make_card(&mut board, col1.id, "Card2", 1);
    let card3 = make_card(&mut board, col2.id, "Card3", 0);
    store.upsert_card(card1).unwrap();
    store.upsert_card(card2).unwrap();
    store.upsert_card(card3).unwrap();

    let cards = store.list_cards_by_column(col1.id).unwrap();
    assert_eq!(cards.len(), 2);
    assert!(cards.iter().all(|c| c.column_id == col1.id));
}

#[test]
fn test_list_cards_by_sprint() {
    let store = InMemoryStore::new();
    let mut board = make_board("B");
    let col = make_column(board.id, "C", 0);
    let sprint_id = Uuid::new_v4();
    let mut card1 = make_card(&mut board, col.id, "Card1", 0);
    card1.sprint_id = Some(sprint_id);
    let card2 = make_card(&mut board, col.id, "Card2", 1);
    store.upsert_card(card1).unwrap();
    store.upsert_card(card2).unwrap();

    let cards = store.list_cards_by_sprint(sprint_id).unwrap();
    assert_eq!(cards.len(), 1);
    assert_eq!(cards[0].sprint_id, Some(sprint_id));
}

#[test]
fn test_count_cards_in_column() {
    let store = InMemoryStore::new();
    let mut board = make_board("B");
    let col = make_column(board.id, "C", 0);
    let card1 = make_card(&mut board, col.id, "C1", 0);
    let card2 = make_card(&mut board, col.id, "C2", 1);
    store.upsert_card(card1).unwrap();
    store.upsert_card(card2).unwrap();

    assert_eq!(store.count_cards_in_column(col.id).unwrap(), 2);
}

#[test]
fn test_count_cards_in_column_excluding() {
    let store = InMemoryStore::new();
    let mut board = make_board("B");
    let col = make_column(board.id, "C", 0);
    let card1 = make_card(&mut board, col.id, "C1", 0);
    let card1_id = card1.id;
    let card2 = make_card(&mut board, col.id, "C2", 1);
    store.upsert_card(card1).unwrap();
    store.upsert_card(card2).unwrap();

    let count = store
        .count_cards_in_column_excluding(col.id, &[card1_id])
        .unwrap();
    assert_eq!(count, 1);
}

#[test]
fn test_delete_cards_by_columns() {
    let store = InMemoryStore::new();
    let mut board = make_board("B");
    let col1 = make_column(board.id, "C1", 0);
    let col2 = make_column(board.id, "C2", 1);
    let card1 = make_card(&mut board, col1.id, "Card1", 0);
    let card2 = make_card(&mut board, col2.id, "Card2", 0);
    let card2_id = card2.id;
    store.upsert_card(card1).unwrap();
    store.upsert_card(card2).unwrap();

    store.delete_cards_by_columns(&[col1.id]).unwrap();

    assert!(store.list_cards_by_column(col1.id).unwrap().is_empty());
    assert!(store.get_card(card2_id).unwrap().is_some());
}

// --- cards_by_column index maintenance ---

#[test]
fn test_column_index_upsert_new_card_indexes_under_target_column() {
    let store = InMemoryStore::new();
    let mut board = make_board("B");
    let col = make_column(board.id, "C", 0);
    let card = make_card(&mut board, col.id, "Card", 0);
    store.upsert_card(card).unwrap();
    assert_eq!(store.count_cards_in_column(col.id).unwrap(), 1);
}

#[test]
fn test_column_index_upsert_with_same_column_keeps_single_entry() {
    let store = InMemoryStore::new();
    let mut board = make_board("B");
    let col = make_column(board.id, "C", 0);
    let card = make_card(&mut board, col.id, "Card", 0);
    let mut card2 = card.clone();
    card2.title = "Renamed".to_string();
    store.upsert_card(card).unwrap();
    store.upsert_card(card2).unwrap();
    assert_eq!(
        store.count_cards_in_column(col.id).unwrap(),
        1,
        "re-upserting the same card must not double-count"
    );
}

#[test]
fn test_column_index_upsert_with_column_change_moves_index_entry() {
    let store = InMemoryStore::new();
    let mut board = make_board("B");
    let col_a = make_column(board.id, "A", 0);
    let col_b = make_column(board.id, "B", 1);
    let card = make_card(&mut board, col_a.id, "Card", 0);
    let card_id = card.id;
    store.upsert_card(card.clone()).unwrap();
    assert_eq!(store.count_cards_in_column(col_a.id).unwrap(), 1);
    assert_eq!(store.count_cards_in_column(col_b.id).unwrap(), 0);

    let mut moved = card;
    moved.column_id = col_b.id;
    store.upsert_card(moved).unwrap();

    assert_eq!(
        store.count_cards_in_column(col_a.id).unwrap(),
        0,
        "card must be removed from old column index"
    );
    assert_eq!(
        store.count_cards_in_column(col_b.id).unwrap(),
        1,
        "card must be added to new column index"
    );
    let fetched = store.get_card(card_id).unwrap().unwrap();
    assert_eq!(fetched.column_id, col_b.id);
}

#[test]
fn test_column_index_delete_card_removes_from_index() {
    let store = InMemoryStore::new();
    let mut board = make_board("B");
    let col = make_column(board.id, "C", 0);
    let card = make_card(&mut board, col.id, "Card", 0);
    let card_id = card.id;
    store.upsert_card(card).unwrap();
    assert_eq!(store.count_cards_in_column(col.id).unwrap(), 1);

    store.delete_card(card_id).unwrap();
    assert_eq!(store.count_cards_in_column(col.id).unwrap(), 0);
}

#[test]
fn test_column_index_delete_cards_by_columns_clears_target_columns() {
    let store = InMemoryStore::new();
    let mut board = make_board("B");
    let col_a = make_column(board.id, "A", 0);
    let col_b = make_column(board.id, "B", 1);
    let card_a1 = make_card(&mut board, col_a.id, "A1", 0);
    let card_a2 = make_card(&mut board, col_a.id, "A2", 1);
    let card_b1 = make_card(&mut board, col_b.id, "B1", 0);
    store.upsert_card(card_a1).unwrap();
    store.upsert_card(card_a2).unwrap();
    store.upsert_card(card_b1).unwrap();

    store.delete_cards_by_columns(&[col_a.id]).unwrap();

    assert_eq!(store.count_cards_in_column(col_a.id).unwrap(), 0);
    assert_eq!(store.count_cards_in_column(col_b.id).unwrap(), 1);
}

#[test]
fn test_column_index_apply_snapshot_rebuilds_from_snapshot_cards() {
    let store = InMemoryStore::new();
    let mut board = make_board("B");
    let col_a = make_column(board.id, "A", 0);
    let col_b = make_column(board.id, "B", 1);
    // Seed with pre-snapshot state so we can verify the rebuild overwrites it.
    let pre_card = make_card(&mut board, col_a.id, "Pre", 0);
    store.upsert_card(pre_card).unwrap();

    // Build a snapshot whose cards land in different columns than the pre-state.
    let board_id = board.id;
    let post_card_a = make_card(&mut board, col_a.id, "PostA", 0);
    let post_card_b1 = make_card(&mut board, col_b.id, "PostB1", 0);
    let post_card_b2 = make_card(&mut board, col_b.id, "PostB2", 1);
    let snapshot = Snapshot::from_data(
        vec![crate::Board {
            id: board_id,
            ..make_board("B")
        }],
        vec![col_a.clone(), col_b.clone()],
        vec![post_card_a, post_card_b1, post_card_b2],
        vec![],
        vec![],
        DependencyGraph::new(),
    );

    store.apply_snapshot(snapshot).unwrap();

    assert_eq!(
        store.count_cards_in_column(col_a.id).unwrap(),
        1,
        "snapshot rebuild must reset col_a index to snapshot contents"
    );
    assert_eq!(
        store.count_cards_in_column(col_b.id).unwrap(),
        2,
        "snapshot rebuild must populate col_b index from snapshot"
    );
}

#[test]
fn test_count_cards_in_column_excluding_with_multiple_excludes() {
    let store = InMemoryStore::new();
    let mut board = make_board("B");
    let col = make_column(board.id, "C", 0);
    let card1 = make_card(&mut board, col.id, "C1", 0);
    let card2 = make_card(&mut board, col.id, "C2", 1);
    let card3 = make_card(&mut board, col.id, "C3", 2);
    let c1 = card1.id;
    let c3 = card3.id;
    store.upsert_card(card1).unwrap();
    store.upsert_card(card2).unwrap();
    store.upsert_card(card3).unwrap();

    assert_eq!(
        store
            .count_cards_in_column_excluding(col.id, &[c1, c3])
            .unwrap(),
        1,
        "excluding two of three should leave one"
    );
    assert_eq!(
        store
            .count_cards_in_column_excluding(col.id, &[Uuid::new_v4()])
            .unwrap(),
        3,
        "excluding ids that aren't in the column should be a no-op"
    );
}
