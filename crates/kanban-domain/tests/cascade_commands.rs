mod common;
use common::TestContext;
use uuid::Uuid;

use kanban_domain::commands::cascade_commands::*;
use kanban_domain::DataStore;

#[test]
fn test_delete_card_edges_removes_all_edges_for_given_ids() {
    let tc = TestContext::new();
    let card_a = Uuid::new_v4();
    let card_b = Uuid::new_v4();
    let card_c = Uuid::new_v4();

    {
        let mut graph = tc.store.get_graph().unwrap();
        graph.set_block(card_a, card_b).unwrap();
        graph.set_block(card_b, card_c).unwrap();
        tc.store.set_graph(graph).unwrap();
    }
    assert_eq!(tc.store.get_graph().unwrap().len(), 2);

    let context = tc.as_command_context();
    let cmd = DeleteCardEdges {
        ids: vec![card_a, card_b],
    };
    cmd.execute(&context).unwrap();

    let graph = tc.store.get_graph().unwrap();
    assert_eq!(
        graph.len(),
        0,
        "edges incident to card_a or card_b should be removed"
    );
}

#[test]
fn test_delete_card_edges_with_empty_input_is_noop() {
    let tc = TestContext::new();
    let card_a = Uuid::new_v4();
    let card_b = Uuid::new_v4();
    {
        let mut graph = tc.store.get_graph().unwrap();
        graph.set_block(card_a, card_b).unwrap();
        tc.store.set_graph(graph).unwrap();
    }

    let context = tc.as_command_context();
    let cmd = DeleteCardEdges { ids: vec![] };
    cmd.execute(&context).unwrap();

    assert_eq!(tc.store.get_graph().unwrap().len(), 1);
}

#[test]
fn test_delete_cards_by_columns_removes_only_cards_in_given_columns() {
    let tc = TestContext::new();
    let mut board = kanban_domain::Board::new("B", Some("TST"));
    let col1 = kanban_domain::Column::new(board.id, "C1", 0);
    let col2 = kanban_domain::Column::new(board.id, "C2", 1);
    let col3 = kanban_domain::Column::new(board.id, "C3", 2);
    let card1 = kanban_domain::Card::new(&mut board, col1.id, "1", 0);
    let card2 = kanban_domain::Card::new(&mut board, col2.id, "2", 0);
    let card3 = kanban_domain::Card::new(&mut board, col3.id, "3", 0);
    let card3_id = card3.id;
    let col3_id = col3.id;
    tc.store.upsert_board(board).unwrap();
    tc.store.upsert_column(col1.clone()).unwrap();
    tc.store.upsert_column(col2.clone()).unwrap();
    tc.store.upsert_column(col3).unwrap();
    tc.store.upsert_card(card1).unwrap();
    tc.store.upsert_card(card2).unwrap();
    tc.store.upsert_card(card3).unwrap();

    let context = tc.as_command_context();
    let cmd = DeleteCardsByColumns {
        column_ids: vec![col1.id, col2.id],
    };
    cmd.execute(&context).unwrap();

    let remaining = tc.store.list_all_cards().unwrap();
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].id, card3_id);
    assert_eq!(remaining[0].column_id, col3_id);
}

#[test]
fn test_delete_archived_cards_removes_only_listed_ids_ignoring_columns() {
    // Board-scoped id list must delete archived records even when their
    // `original_column_id` dangles (column already gone).
    let tc = TestContext::new();
    let board_id = Uuid::new_v4();
    let mut board = kanban_domain::Board::new("B", Some("TST"));
    let dangling_col = Uuid::new_v4();
    let live_col = kanban_domain::Column::new(board_id, "Live", 0);
    let card1 = kanban_domain::Card::new(&mut board, dangling_col, "1", 0);
    let card2 = kanban_domain::Card::new(&mut board, live_col.id, "2", 0);
    let keep = kanban_domain::Card::new(&mut board, live_col.id, "keep", 1);
    let arch1_id = card1.id;
    let arch2_id = card2.id;
    let keep_id = keep.id;
    tc.store.upsert_card(card1).unwrap();
    tc.store.upsert_card(card2).unwrap();
    tc.store.upsert_card(keep).unwrap();
    tc.store
        .insert_archived_card(kanban_domain::ArchivedCard::new(arch1_id, board_id))
        .unwrap();
    tc.store
        .insert_archived_card(kanban_domain::ArchivedCard::new(arch2_id, board_id))
        .unwrap();
    tc.store
        .insert_archived_card(kanban_domain::ArchivedCard::new(keep_id, board_id))
        .unwrap();

    let context = tc.as_command_context();
    let cmd = DeleteArchivedCards {
        card_ids: vec![arch1_id, arch2_id],
    };
    cmd.execute(&context).unwrap();

    let remaining = tc.store.list_archived_cards().unwrap();
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].entity_id, keep_id);
}

#[test]
fn test_delete_columns_by_board_removes_all_columns_of_board() {
    let tc = TestContext::new();
    let board = kanban_domain::Board::new("B", None::<String>);
    let board_id = board.id;
    let other_board = kanban_domain::Board::new("Other", None::<String>);
    let other_board_id = other_board.id;
    let col1 = kanban_domain::Column::new(board_id, "C1", 0);
    let col2 = kanban_domain::Column::new(board_id, "C2", 1);
    let other_col = kanban_domain::Column::new(other_board_id, "OC", 0);
    tc.store.upsert_board(board).unwrap();
    tc.store.upsert_board(other_board).unwrap();
    tc.store.upsert_column(col1).unwrap();
    tc.store.upsert_column(col2).unwrap();
    tc.store.upsert_column(other_col).unwrap();

    let context = tc.as_command_context();
    let cmd = DeleteColumnsByBoard { board_id };
    cmd.execute(&context).unwrap();

    let remaining = tc.store.list_all_columns().unwrap();
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].board_id, other_board_id);
}

#[test]
fn test_delete_sprints_by_board_removes_all_sprints_of_board() {
    let tc = TestContext::new();
    let board = kanban_domain::Board::new("B", None::<String>);
    let board_id = board.id;
    let other_board = kanban_domain::Board::new("Other", None::<String>);
    let other_board_id = other_board.id;
    let sprint1 = kanban_domain::Sprint::new(board_id, 1, None, None::<String>);
    let sprint2 = kanban_domain::Sprint::new(board_id, 2, None, None::<String>);
    let other_sprint = kanban_domain::Sprint::new(other_board_id, 1, None, None::<String>);
    tc.store.upsert_board(board).unwrap();
    tc.store.upsert_board(other_board).unwrap();
    tc.store.upsert_sprint(sprint1).unwrap();
    tc.store.upsert_sprint(sprint2).unwrap();
    tc.store.upsert_sprint(other_sprint).unwrap();

    let context = tc.as_command_context();
    let cmd = DeleteSprintsByBoard { board_id };
    cmd.execute(&context).unwrap();

    let remaining = tc.store.list_all_sprints().unwrap();
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].board_id, other_board_id);
}
