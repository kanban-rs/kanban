use crate::in_memory_store::test_support::{make_board, make_card, make_column};
use crate::InMemoryStore;
use kanban_domain::data_store::DataStore;
use uuid::Uuid;

// --- list_cards_by_column via column index ---

#[test]
fn test_list_cards_by_column_returns_only_cards_in_that_column() {
    let store = InMemoryStore::new();
    let mut board = make_board("B");
    let col_a = make_column(board.id, "A", 0);
    let col_b = make_column(board.id, "B", 1);
    let a1 = make_card(&mut board, col_a.id, "A1", 0);
    let a2 = make_card(&mut board, col_a.id, "A2", 1);
    let b1 = make_card(&mut board, col_b.id, "B1", 0);
    let a1_id = a1.id;
    let a2_id = a2.id;
    store.upsert_card(a1).unwrap();
    store.upsert_card(a2).unwrap();
    store.upsert_card(b1).unwrap();

    let listed = store.list_cards_by_column(col_a.id).unwrap();
    let ids: Vec<Uuid> = listed.iter().map(|c| c.id).collect();
    assert_eq!(ids.len(), 2);
    assert!(ids.contains(&a1_id));
    assert!(ids.contains(&a2_id));
}

#[test]
fn test_list_cards_by_column_returns_cards_sorted_by_position() {
    let store = InMemoryStore::new();
    let mut board = make_board("B");
    let col = make_column(board.id, "C", 0);
    // Insert out of position order to confirm the sort applies.
    let c_pos2 = make_card(&mut board, col.id, "P2", 2);
    let c_pos0 = make_card(&mut board, col.id, "P0", 0);
    let c_pos1 = make_card(&mut board, col.id, "P1", 1);
    store.upsert_card(c_pos2).unwrap();
    store.upsert_card(c_pos0).unwrap();
    store.upsert_card(c_pos1).unwrap();

    let positions: Vec<i32> = store
        .list_cards_by_column(col.id)
        .unwrap()
        .iter()
        .map(|c| c.position)
        .collect();
    assert_eq!(positions, vec![0, 1, 2]);
}

#[test]
fn test_list_cards_by_column_after_column_change_reflects_new_membership() {
    let store = InMemoryStore::new();
    let mut board = make_board("B");
    let col_a = make_column(board.id, "A", 0);
    let col_b = make_column(board.id, "B", 1);
    let card = make_card(&mut board, col_a.id, "Card", 0);
    let card_id = card.id;
    store.upsert_card(card.clone()).unwrap();

    let mut moved = card;
    moved.column_id = col_b.id;
    store.upsert_card(moved).unwrap();

    assert!(store.list_cards_by_column(col_a.id).unwrap().is_empty());
    let in_b = store.list_cards_by_column(col_b.id).unwrap();
    assert_eq!(in_b.len(), 1);
    assert_eq!(in_b[0].id, card_id);
}

#[test]
fn test_list_cards_by_column_unknown_column_returns_empty() {
    let store = InMemoryStore::new();
    let listed = store.list_cards_by_column(Uuid::new_v4()).unwrap();
    assert!(listed.is_empty());
}

#[test]
fn test_clear_sprint_from_cards_sets_updated_at() {
    let store = InMemoryStore::new();
    let mut board = make_board("B");
    let col = make_column(board.id, "C", 0);
    let sprint_id = Uuid::new_v4();
    let mut card = make_card(&mut board, col.id, "C1", 0);
    card.sprint_id = Some(sprint_id);
    let card_id = card.id;
    store.upsert_card(card).unwrap();

    let later = chrono::Utc::now() + chrono::Duration::seconds(10);
    store.clear_sprint_from_cards(sprint_id, later).unwrap();

    let card = store.get_card(card_id).unwrap().unwrap();
    assert_eq!(
        card.updated_at, later,
        "clear_sprint_from_cards should set updated_at to the provided timestamp"
    );
}

#[test]
fn test_clear_sprint_from_cards() {
    let store = InMemoryStore::new();
    let mut board = make_board("B");
    let col = make_column(board.id, "C", 0);
    let sprint_id = Uuid::new_v4();
    let mut card1 = make_card(&mut board, col.id, "C1", 0);
    card1.sprint_id = Some(sprint_id);
    let card1_id = card1.id;
    let mut card2 = make_card(&mut board, col.id, "C2", 1);
    card2.sprint_id = Some(sprint_id);
    let card2_id = card2.id;
    store.upsert_card(card1).unwrap();
    store.upsert_card(card2).unwrap();

    store
        .clear_sprint_from_cards(sprint_id, chrono::Utc::now())
        .unwrap();

    assert!(store
        .get_card(card1_id)
        .unwrap()
        .unwrap()
        .sprint_id
        .is_none());
    assert!(store
        .get_card(card2_id)
        .unwrap()
        .unwrap()
        .sprint_id
        .is_none());
}
