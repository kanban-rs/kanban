use kanban_domain::{Board, CardStatus, Column};
use kanban_tui::App;

#[test]
fn test_create_card_in_first_column_default() {
    let mut app = App::new(None);

    let board = Board::new("Test Board".to_string(), None);
    let column1 = Column::new(board.id, "Todo".to_string(), 0);
    let column2 = Column::new(board.id, "Doing".to_string(), 1);

    app.boards.push(board);
    app.columns.push(column1.clone());
    app.columns.push(column2);
    app.board_selection.set(Some(0));
    app.active_board_index = Some(0);
    app.input.set("Test Card".to_string());

    app.create_card();

    assert_eq!(app.cards.len(), 1);
    let card = &app.cards[0];
    assert_eq!(card.title, "Test Card");
    assert_eq!(card.column_id, column1.id);
    assert_eq!(card.status, CardStatus::Todo);
    assert!(card.completed_at.is_none());
}

#[test]
fn test_create_card_in_last_column_with_3_columns_auto_completes() {
    let mut app = App::new(None);

    let board = Board::new("Test Board".to_string(), None);
    let column_todo = Column::new(board.id, "Todo".to_string(), 0);
    let column_doing = Column::new(board.id, "Doing".to_string(), 1);
    let column_done = Column::new(board.id, "Done".to_string(), 2);

    app.boards.push(board);
    app.columns.push(column_todo);
    app.columns.push(column_doing);
    app.columns.push(column_done.clone());
    app.board_selection.set(Some(0));
    app.active_board_index = Some(0);

    app.switch_view_strategy(kanban_domain::TaskListView::GroupedByColumn);

    let grouped_strategy = app
        .view_strategy
        .as_any_mut()
        .downcast_mut::<kanban_tui::view_strategy::GroupedViewStrategy>()
        .unwrap();

    grouped_strategy.set_active_column_index(2);

    app.input.set("Complete Task".to_string());
    app.create_card();

    assert_eq!(app.cards.len(), 1);
    let card = &app.cards[0];
    assert_eq!(card.title, "Complete Task");
    assert_eq!(card.column_id, column_done.id);
    assert_eq!(card.status, CardStatus::Done);
    assert!(card.completed_at.is_some());
}

#[test]
fn test_create_card_in_last_column_with_2_columns_no_auto_complete() {
    let mut app = App::new(None);

    let board = Board::new("Test Board".to_string(), None);
    let column_todo = Column::new(board.id, "Todo".to_string(), 0);
    let column_done = Column::new(board.id, "Done".to_string(), 1);

    app.boards.push(board);
    app.columns.push(column_todo);
    app.columns.push(column_done.clone());
    app.board_selection.set(Some(0));
    app.active_board_index = Some(0);

    app.switch_view_strategy(kanban_domain::TaskListView::GroupedByColumn);

    let grouped_strategy = app
        .view_strategy
        .as_any_mut()
        .downcast_mut::<kanban_tui::view_strategy::GroupedViewStrategy>()
        .unwrap();

    grouped_strategy.set_active_column_index(1);

    app.input.set("Not Complete Task".to_string());
    app.create_card();

    assert_eq!(app.cards.len(), 1);
    let card = &app.cards[0];
    assert_eq!(card.title, "Not Complete Task");
    assert_eq!(card.column_id, column_done.id);
    assert_eq!(card.status, CardStatus::Todo);
    assert!(card.completed_at.is_none());
}

#[test]
fn test_create_card_in_middle_column_no_auto_complete() {
    let mut app = App::new(None);

    let board = Board::new("Test Board".to_string(), None);
    let column_todo = Column::new(board.id, "Todo".to_string(), 0);
    let column_doing = Column::new(board.id, "Doing".to_string(), 1);
    let column_done = Column::new(board.id, "Done".to_string(), 2);

    app.boards.push(board);
    app.columns.push(column_todo);
    app.columns.push(column_doing.clone());
    app.columns.push(column_done);
    app.board_selection.set(Some(0));
    app.active_board_index = Some(0);

    app.switch_view_strategy(kanban_domain::TaskListView::GroupedByColumn);

    let grouped_strategy = app
        .view_strategy
        .as_any_mut()
        .downcast_mut::<kanban_tui::view_strategy::GroupedViewStrategy>()
        .unwrap();

    grouped_strategy.set_active_column_index(1);

    app.input.set("In Progress Task".to_string());
    app.create_card();

    assert_eq!(app.cards.len(), 1);
    let card = &app.cards[0];
    assert_eq!(card.title, "In Progress Task");
    assert_eq!(card.column_id, column_doing.id);
    assert_eq!(card.status, CardStatus::Todo);
    assert!(card.completed_at.is_none());
}

#[test]
fn test_create_multiple_cards_in_same_column() {
    let mut app = App::new(None);

    let board = Board::new("Test Board".to_string(), None);
    let column = Column::new(board.id, "Todo".to_string(), 0);

    app.boards.push(board);
    app.columns.push(column.clone());
    app.board_selection.set(Some(0));
    app.active_board_index = Some(0);

    app.input.set("Card 1".to_string());
    app.create_card();

    app.input.set("Card 2".to_string());
    app.create_card();

    app.input.set("Card 3".to_string());
    app.create_card();

    assert_eq!(app.cards.len(), 3);
    assert_eq!(app.cards[0].title, "Card 1");
    assert_eq!(app.cards[1].title, "Card 2");
    assert_eq!(app.cards[2].title, "Card 3");

    assert_eq!(app.cards[0].position, 0);
    assert_eq!(app.cards[1].position, 1);
    assert_eq!(app.cards[2].position, 2);

    assert_eq!(app.cards[0].column_id, column.id);
    assert_eq!(app.cards[1].column_id, column.id);
    assert_eq!(app.cards[2].column_id, column.id);
}

#[test]
fn test_create_card_in_last_column_kanban_view() {
    let mut app = App::new(None);

    let board = Board::new("Test Board".to_string(), None);
    let column_todo = Column::new(board.id, "Todo".to_string(), 0);
    let column_doing = Column::new(board.id, "Doing".to_string(), 1);
    let column_done = Column::new(board.id, "Done".to_string(), 2);

    app.boards.push(board);
    app.columns.push(column_todo);
    app.columns.push(column_doing);
    app.columns.push(column_done.clone());
    app.board_selection.set(Some(0));
    app.active_board_index = Some(0);

    app.switch_view_strategy(kanban_domain::TaskListView::ColumnView);

    let kanban_strategy = app
        .view_strategy
        .as_any_mut()
        .downcast_mut::<kanban_tui::view_strategy::KanbanViewStrategy>()
        .unwrap();

    kanban_strategy.set_active_column_index(2);

    app.input.set("Kanban Done Task".to_string());
    app.create_card();

    assert_eq!(app.cards.len(), 1);
    let card = &app.cards[0];
    assert_eq!(card.title, "Kanban Done Task");
    assert_eq!(card.column_id, column_done.id);
    assert_eq!(card.status, CardStatus::Done);
    assert!(card.completed_at.is_some());
}

#[test]
fn test_create_card_with_4_columns_last_auto_completes() {
    let mut app = App::new(None);

    let board = Board::new("Test Board".to_string(), None);
    let col1 = Column::new(board.id, "Backlog".to_string(), 0);
    let col2 = Column::new(board.id, "Todo".to_string(), 1);
    let col3 = Column::new(board.id, "Doing".to_string(), 2);
    let col4 = Column::new(board.id, "Done".to_string(), 3);

    app.boards.push(board);
    app.columns.push(col1);
    app.columns.push(col2);
    app.columns.push(col3);
    app.columns.push(col4.clone());
    app.board_selection.set(Some(0));
    app.active_board_index = Some(0);

    app.switch_view_strategy(kanban_domain::TaskListView::GroupedByColumn);

    let grouped_strategy = app
        .view_strategy
        .as_any_mut()
        .downcast_mut::<kanban_tui::view_strategy::GroupedViewStrategy>()
        .unwrap();

    grouped_strategy.set_active_column_index(3);

    app.input.set("Four Column Task".to_string());
    app.create_card();

    assert_eq!(app.cards.len(), 1);
    let card = &app.cards[0];
    assert_eq!(card.title, "Four Column Task");
    assert_eq!(card.column_id, col4.id);
    assert_eq!(card.status, CardStatus::Done);
    assert!(card.completed_at.is_some());
}
