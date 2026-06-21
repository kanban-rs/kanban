use super::*;
use crate::command_store::CommandStore;
use crate::{Board, Card, Column, Sprint};

fn make_board(name: &str) -> Board {
    Board::new(name.to_string(), None::<String>)
}

fn make_column(board_id: Uuid, name: &str, pos: i32) -> Column {
    Column::new(board_id, name.to_string(), pos)
}

fn make_card(board: &mut Board, column_id: Uuid, title: &str, pos: i32) -> Card {
    Card::new(board, column_id, title.to_string(), pos)
}

// Board CRUD

#[test]
fn test_upsert_and_get_board() {
    let store = InMemoryStore::new();
    let board = make_board("Test Board");
    let id = board.id;
    store.upsert_board(board.clone()).unwrap();

    let fetched = store.get_board(id).unwrap().unwrap();
    assert_eq!(fetched.id, id);
    assert_eq!(fetched.name, "Test Board");
}

#[test]
fn test_list_boards_empty() {
    let store = InMemoryStore::new();
    let boards = store.list_boards().unwrap();
    assert!(boards.is_empty());
}

#[test]
fn test_delete_board_removes_it() {
    let store = InMemoryStore::new();
    let board = make_board("To Delete");
    let id = board.id;
    store.upsert_board(board).unwrap();
    store.delete_board(id).unwrap();
    assert!(store.get_board(id).unwrap().is_none());
}

// Column CRUD

#[test]
fn test_upsert_and_get_column() {
    let store = InMemoryStore::new();
    let board = make_board("B");
    let col = make_column(board.id, "Col", 0);
    let col_id = col.id;
    store.upsert_column(col.clone()).unwrap();

    let fetched = store.get_column(col_id).unwrap().unwrap();
    assert_eq!(fetched.id, col_id);
    assert_eq!(fetched.name, "Col");
}

#[test]
fn test_list_columns_by_board_filters_correctly() {
    let store = InMemoryStore::new();
    let board1 = make_board("B1");
    let board2 = make_board("B2");
    let col1 = make_column(board1.id, "C1", 0);
    let col2 = make_column(board1.id, "C2", 1);
    let col3 = make_column(board2.id, "C3", 0);
    store.upsert_column(col1).unwrap();
    store.upsert_column(col2).unwrap();
    store.upsert_column(col3).unwrap();

    let cols = store.list_columns_by_board(board1.id).unwrap();
    assert_eq!(cols.len(), 2);
    assert!(cols.iter().all(|c| c.board_id == board1.id));
}

#[test]
fn test_delete_columns_by_board() {
    let store = InMemoryStore::new();
    let board1 = make_board("B1");
    let board2 = make_board("B2");
    let col1 = make_column(board1.id, "C1", 0);
    let col2 = make_column(board2.id, "C2", 0);
    let col2_id = col2.id;
    store.upsert_column(col1).unwrap();
    store.upsert_column(col2).unwrap();

    store.delete_columns_by_board(board1.id).unwrap();

    assert!(store.list_columns_by_board(board1.id).unwrap().is_empty());
    assert!(store.get_column(col2_id).unwrap().is_some());
}

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
        vec![Board {
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

// Sprint CRUD

#[test]
fn test_upsert_and_get_sprint() {
    let store = InMemoryStore::new();
    let board = make_board("B");
    let sprint = Sprint::new(board.id, 1, None, None::<String>);
    let sprint_id = sprint.id;
    store.upsert_sprint(sprint).unwrap();

    let fetched = store.get_sprint(sprint_id).unwrap().unwrap();
    assert_eq!(fetched.id, sprint_id);
    assert_eq!(fetched.sprint_number, 1);
}

#[test]
fn test_list_sprints_by_board() {
    let store = InMemoryStore::new();
    let board1 = make_board("B1");
    let board2 = make_board("B2");
    let s1 = Sprint::new(board1.id, 1, None, None::<String>);
    let s2 = Sprint::new(board1.id, 2, None, None::<String>);
    let s3 = Sprint::new(board2.id, 1, None, None::<String>);
    store.upsert_sprint(s1).unwrap();
    store.upsert_sprint(s2).unwrap();
    store.upsert_sprint(s3).unwrap();

    let sprints = store.list_sprints_by_board(board1.id).unwrap();
    assert_eq!(sprints.len(), 2);
    assert!(sprints.iter().all(|s| s.board_id == board1.id));
}

#[test]
fn test_delete_sprints_by_board() {
    let store = InMemoryStore::new();
    let board1 = make_board("B1");
    let board2 = make_board("B2");
    let s1 = Sprint::new(board1.id, 1, None, None::<String>);
    let s2 = Sprint::new(board2.id, 1, None, None::<String>);
    let s2_id = s2.id;
    store.upsert_sprint(s1).unwrap();
    store.upsert_sprint(s2).unwrap();

    store.delete_sprints_by_board(board1.id).unwrap();

    assert!(store.list_sprints_by_board(board1.id).unwrap().is_empty());
    assert!(store.get_sprint(s2_id).unwrap().is_some());
}

// Archived card

#[test]
fn test_insert_and_get_archived_card() {
    let store = InMemoryStore::new();
    let mut board = make_board("B");
    let col = make_column(board.id, "C", 0);
    let card = make_card(&mut board, col.id, "Card", 0);
    let card_id = card.id;
    let ac = ArchivedCard::new(card, col.id, 0);
    store.insert_archived_card(ac).unwrap();

    let fetched = store.get_archived_card(card_id).unwrap().unwrap();
    assert_eq!(fetched.card.id, card_id);
}

#[test]
fn test_list_archived_cards() {
    let store = InMemoryStore::new();
    let mut board = make_board("B");
    let col = make_column(board.id, "C", 0);
    let card1 = make_card(&mut board, col.id, "C1", 0);
    let card2 = make_card(&mut board, col.id, "C2", 1);
    store
        .insert_archived_card(ArchivedCard::new(card1, col.id, 0))
        .unwrap();
    store
        .insert_archived_card(ArchivedCard::new(card2, col.id, 1))
        .unwrap();

    assert_eq!(store.list_archived_cards().unwrap().len(), 2);
}

#[test]
fn test_list_archived_cards_by_columns_filters_correctly() {
    let store = InMemoryStore::new();
    let mut board = make_board("B");
    let col1 = make_column(board.id, "C1", 0);
    let col2 = make_column(board.id, "C2", 1);
    let card1 = make_card(&mut board, col1.id, "Card1", 0);
    let card2 = make_card(&mut board, col2.id, "Card2", 0);
    store
        .insert_archived_card(ArchivedCard::new(card1, col1.id, 0))
        .unwrap();
    store
        .insert_archived_card(ArchivedCard::new(card2, col2.id, 0))
        .unwrap();

    let result = store.list_archived_cards_by_columns(&[col1.id]).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].original_column_id, col1.id);
}

#[test]
fn test_list_archived_cards_by_columns_empty_ids_returns_empty() {
    let store = InMemoryStore::new();
    let mut board = make_board("B");
    let col = make_column(board.id, "C", 0);
    let card = make_card(&mut board, col.id, "Card", 0);
    store
        .insert_archived_card(ArchivedCard::new(card, col.id, 0))
        .unwrap();

    let result = store.list_archived_cards_by_columns(&[]).unwrap();
    assert!(result.is_empty());
}

#[test]
fn test_clear_sprint_from_archived_cards() {
    let store = InMemoryStore::new();
    let mut board = make_board("B");
    let col = make_column(board.id, "C", 0);
    let sprint_id = Uuid::new_v4();
    let mut card = make_card(&mut board, col.id, "Card", 0);
    card.sprint_id = Some(sprint_id);
    let card_id = card.id;
    let before = card.updated_at;
    let ac = ArchivedCard::new(card, col.id, 0);
    store.insert_archived_card(ac).unwrap();

    let ts = chrono::Utc::now() + chrono::Duration::seconds(10);
    store
        .clear_sprint_from_archived_cards(sprint_id, ts)
        .unwrap();

    let ac = store.get_archived_card(card_id).unwrap().unwrap();
    assert!(ac.card.sprint_id.is_none());
    assert!(ac.card.updated_at > before);
    assert_eq!(ac.card.updated_at, ts);
}

#[test]
fn test_delete_archived_card() {
    let store = InMemoryStore::new();
    let mut board = make_board("B");
    let col = make_column(board.id, "C", 0);
    let card = make_card(&mut board, col.id, "Card", 0);
    let card_id = card.id;
    store
        .insert_archived_card(ArchivedCard::new(card, col.id, 0))
        .unwrap();
    store.delete_archived_card(card_id).unwrap();
    assert!(store.get_archived_card(card_id).unwrap().is_none());
}

// Graph

#[test]
fn test_modify_graph_atomic_on_error_leaves_graph_unchanged() {
    let store = InMemoryStore::new();
    let a = Uuid::new_v4();
    let b = Uuid::new_v4();

    let mut graph = store.get_graph().unwrap();
    graph.set_block(a, b).unwrap();
    store.set_graph(graph).unwrap();

    let result = store.modify_graph(Box::new(move |graph| {
        graph.remove_node(a);
        Err(crate::KanbanError::validation("rollback"))
    }));
    assert!(result.is_err());

    let graph = store.get_graph().unwrap();
    assert_eq!(
        graph.len(),
        1,
        "modify_graph should not apply partial changes on error"
    );
}

#[test]
fn test_set_and_get_graph() {
    let store = InMemoryStore::new();
    let graph = DependencyGraph::new();
    store.set_graph(graph.clone()).unwrap();
    let fetched = store.get_graph().unwrap();
    assert_eq!(fetched, graph);
}

// Snapshot

#[test]
fn test_snapshot_roundtrip() {
    let store = InMemoryStore::new();
    let mut board = make_board("B");
    let col = make_column(board.id, "C", 0);
    let card = make_card(&mut board, col.id, "Card", 0);
    let sprint = Sprint::new(board.id, 1, None, None::<String>);
    store.upsert_board(board).unwrap();
    store.upsert_column(col).unwrap();
    store.upsert_card(card).unwrap();
    store.upsert_sprint(sprint).unwrap();

    let snap = store.snapshot().unwrap();

    let store2 = InMemoryStore::new();
    store2.apply_snapshot(snap).unwrap();

    assert_eq!(store2.list_boards().unwrap().len(), 1);
    assert_eq!(store2.list_all_columns().unwrap().len(), 1);
    assert_eq!(store2.list_all_cards().unwrap().len(), 1);
    assert_eq!(store2.list_all_sprints().unwrap().len(), 1);
}

#[test]
fn test_snapshot_sorts_entities_by_position() {
    let store = InMemoryStore::new();
    let mut board_b = make_board("B");
    board_b.position = 1;
    let mut board_a = make_board("A");
    board_a.position = 0;
    store.upsert_board(board_b.clone()).unwrap();
    store.upsert_board(board_a.clone()).unwrap();

    let col_z = make_column(board_a.id, "Z", 2);
    let col_a = make_column(board_a.id, "A", 0);
    let col_m = make_column(board_a.id, "M", 1);
    store.upsert_column(col_z).unwrap();
    store.upsert_column(col_a.clone()).unwrap();
    store.upsert_column(col_m).unwrap();

    let card3 = make_card(&mut board_a.clone(), col_a.id, "C3", 2);
    let card1 = make_card(&mut board_a.clone(), col_a.id, "C1", 0);
    store.upsert_card(card3).unwrap();
    store.upsert_card(card1).unwrap();

    let s2 = Sprint::new(board_a.id, 2, None, None::<String>);
    let s1 = Sprint::new(board_a.id, 1, None, None::<String>);
    store.upsert_sprint(s2).unwrap();
    store.upsert_sprint(s1).unwrap();

    let snap = store.snapshot().unwrap();

    assert_eq!(
        snap.boards[0].name, "A",
        "boards should be sorted by position"
    );
    assert_eq!(snap.boards[1].name, "B");
    assert_eq!(
        snap.columns[0].name, "A",
        "columns should be sorted by position"
    );
    assert_eq!(snap.columns[1].name, "M");
    assert_eq!(snap.columns[2].name, "Z");
    assert_eq!(
        snap.cards[0].title, "C1",
        "cards should be sorted by position"
    );
    assert_eq!(snap.cards[1].title, "C3");
    assert_eq!(
        snap.sprints[0].sprint_number, 1,
        "sprints should be sorted by sprint_number"
    );
    assert_eq!(snap.sprints[1].sprint_number, 2);
}

#[test]
fn test_apply_snapshot_replaces_existing_data() {
    let store = InMemoryStore::new();
    let board_old = make_board("Old");
    store.upsert_board(board_old).unwrap();

    let board_new = make_board("New");
    let snap = Snapshot::from_data(
        vec![board_new],
        vec![],
        vec![],
        vec![],
        vec![],
        DependencyGraph::new(),
    );
    store.apply_snapshot(snap).unwrap();

    let boards = store.list_boards().unwrap();
    assert_eq!(boards.len(), 1);
    assert_eq!(boards[0].name, "New");
}

// Lock-safety contract tests

#[test]
fn test_all_data_store_methods_return_ok_not_panic() {
    let store = InMemoryStore::new();
    let mut board = make_board("B");
    let col = make_column(board.id, "C", 0);
    let card = make_card(&mut board, col.id, "Card", 0);
    let sprint = Sprint::new(board.id, 1, None, None::<String>);
    let ac = ArchivedCard::new(card.clone(), col.id, 0);

    assert!(store.upsert_board(board.clone()).is_ok());
    assert!(store.get_board(board.id).is_ok());
    assert!(store.list_boards().is_ok());
    assert!(store.upsert_column(col.clone()).is_ok());
    assert!(store.get_column(col.id).is_ok());
    assert!(store.list_columns_by_board(board.id).is_ok());
    assert!(store.list_all_columns().is_ok());
    assert!(store.upsert_card(card.clone()).is_ok());
    assert!(store.get_card(card.id).is_ok());
    assert!(store.list_all_cards().is_ok());
    assert!(store.list_cards_by_column(col.id).is_ok());
    assert!(store.list_cards_by_sprint(Uuid::new_v4()).is_ok());
    assert!(store.count_cards_in_column(col.id).is_ok());
    assert!(store.count_cards_in_column_excluding(col.id, &[]).is_ok());
    assert!(store
        .clear_sprint_from_cards(Uuid::new_v4(), chrono::Utc::now())
        .is_ok());
    assert!(store.insert_archived_card(ac).is_ok());
    assert!(store.get_archived_card(card.id).is_ok());
    assert!(store.list_archived_cards().is_ok());
    assert!(store.delete_archived_card(card.id).is_ok());
    assert!(store.upsert_sprint(sprint.clone()).is_ok());
    assert!(store.get_sprint(sprint.id).is_ok());
    assert!(store.list_sprints_by_board(board.id).is_ok());
    assert!(store.list_all_sprints().is_ok());
    assert!(store.get_graph().is_ok());
    assert!(store.set_graph(DependencyGraph::new()).is_ok());
    assert!(store.snapshot().is_ok());
    assert!(store.apply_snapshot(Snapshot::new()).is_ok());
    assert!(store.delete_card(card.id).is_ok());
    assert!(store.delete_cards_by_columns(&[col.id]).is_ok());
    assert!(store.delete_column(col.id).is_ok());
    assert!(store.delete_columns_by_board(board.id).is_ok());
    assert!(store.delete_sprint(sprint.id).is_ok());
    assert!(store.delete_sprints_by_board(board.id).is_ok());
    assert!(store.delete_board(board.id).is_ok());
}

#[test]
fn test_all_command_store_methods_return_ok_not_panic() {
    use crate::command_batch::CommandBatch;
    use crate::commands::{BoardCommand, Command, CreateBoard};
    let store = InMemoryStore::new();
    let batch = CommandBatch::from(vec![Command::Board(BoardCommand::Create(CreateBoard {
        id: Uuid::new_v4(),
        name: "B".into(),
        card_prefix: None,
        position: 0,
    }))]);

    assert!(store.batch_count().is_ok());
    assert_eq!(store.batch_count().unwrap(), 0);

    assert!(store.append_batch(&batch).is_ok());
    assert_eq!(store.batch_count().unwrap(), 1);

    assert!(store.load_batches(0, 1).is_ok());
    assert_eq!(store.load_batches(0, 1).unwrap().len(), 1);
}

// Concurrency test

#[test]
fn test_concurrent_reads_and_writes_no_panic() {
    use std::sync::Arc;
    use std::thread;

    let store = Arc::new(InMemoryStore::new());
    let mut handles = vec![];

    for i in 0..10 {
        let s = Arc::clone(&store);
        handles.push(thread::spawn(move || {
            let board = make_board(&format!("Board-{i}"));
            s.upsert_board(board.clone()).unwrap();
            let col = make_column(board.id, &format!("Col-{i}"), i);
            s.upsert_column(col).unwrap();
        }));
    }

    for _ in 0..10 {
        let s = Arc::clone(&store);
        handles.push(thread::spawn(move || {
            for _ in 0..10 {
                let _ = s.list_boards();
                let _ = s.list_all_columns();
                let _ = s.list_all_cards();
                let _ = s.snapshot();
            }
        }));
    }

    for h in handles {
        h.join().expect("thread should not panic");
    }

    let boards = store.list_boards().unwrap();
    assert_eq!(boards.len(), 10);
}

#[test]
fn test_load_batches_from_beyond_end_returns_empty() {
    use crate::command_batch::CommandBatch;
    let store = InMemoryStore::new();
    let make_batch = || {
        CommandBatch::from(vec![crate::commands::Command::Board(
            crate::commands::BoardCommand::Delete(crate::commands::DeleteBoard {
                board_id: Uuid::new_v4(),
            }),
        )])
    };
    store.append_batch(&make_batch()).unwrap();
    store.append_batch(&make_batch()).unwrap();
    store.append_batch(&make_batch()).unwrap();

    let result = store.load_batches(10, 20).unwrap();
    assert!(
        result.is_empty(),
        "Expected empty vec for out-of-bounds range"
    );
}

#[test]
fn test_append_batch_stores_full_batch_including_provenance() {
    // The batch is the audit log: the full CommandBatch is stored, with
    // commands AND provenance (issued_by, correlation_id, session_id)
    // preserved on write — nothing is stripped.
    use crate::command_batch::CommandBatch;
    use crate::commands::{BoardCommand, Command, CreateBoard};
    use kanban_core::ClientId;

    let store = InMemoryStore::new();
    let client = ClientId::new();
    let cmd = Command::Board(BoardCommand::Create(CreateBoard {
        id: Uuid::new_v4(),
        name: "Test".into(),
        card_prefix: None,
        position: 0,
    }));
    let batch = CommandBatch::wrap(vec![cmd.clone()], client);
    let expected_correlation = batch.correlation_id;
    let expected_session = batch.session_id;

    store.append_batch(&batch).unwrap();
    let loaded = store.load_batches(0, 1).unwrap();

    assert_eq!(loaded.len(), 1);
    let loaded = &loaded[0];
    assert_eq!(loaded.commands, vec![cmd]);
    assert_eq!(
        loaded.issued_by, client,
        "issued_by must survive — provenance is no longer stripped"
    );
    assert_eq!(loaded.correlation_id, expected_correlation);
    assert_eq!(loaded.session_id, expected_session);
    assert_eq!(
        loaded.app_type, batch.app_type,
        "app_type must survive the batch round-trip"
    );
}

#[test]
fn test_load_batches_inverted_range_returns_empty() {
    // An inverted range (from > to) must yield an empty slice, never panic
    // on `log[from..to]`.
    use crate::command_batch::CommandBatch;
    use crate::commands::{BoardCommand, Command, CreateBoard};

    let store = InMemoryStore::new();
    let make_cmd = |name: &str| {
        Command::Board(BoardCommand::Create(CreateBoard {
            id: Uuid::new_v4(),
            name: name.into(),
            card_prefix: None,
            position: 0,
        }))
    };
    store
        .append_batch(&CommandBatch::from(vec![make_cmd("B1")]))
        .unwrap();
    store
        .append_batch(&CommandBatch::from(vec![make_cmd("B2")]))
        .unwrap();

    let loaded = store.load_batches(2, 1).unwrap();
    assert!(
        loaded.is_empty(),
        "inverted range must return empty, not panic"
    );
}

#[test]
fn test_append_batch_preserves_batch_boundaries() {
    use crate::command_batch::CommandBatch;
    use crate::commands::{BoardCommand, Command, CreateBoard};

    let store = InMemoryStore::new();
    let make_cmd = |name: &str| {
        Command::Board(BoardCommand::Create(CreateBoard {
            id: Uuid::new_v4(),
            name: name.into(),
            card_prefix: None,
            position: 0,
        }))
    };

    store
        .append_batch(&CommandBatch::from(vec![make_cmd("B1"), make_cmd("B2")]))
        .unwrap();
    store
        .append_batch(&CommandBatch::from(vec![make_cmd("B3")]))
        .unwrap();

    let batches = store.load_batches(0, 2).unwrap();
    assert_eq!(batches.len(), 2, "two separate append calls = two batches");
    assert_eq!(batches[0].commands.len(), 2, "first batch had 2 commands");
    assert_eq!(batches[1].commands.len(), 1, "second batch had 1 command");
}
