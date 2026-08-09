use kanban_domain::{CardUpdate, CreateCardOptions, FieldUpdate, KanbanOperations};
use kanban_tui::App;

#[test]
fn test_prepare_frame_populates_model_from_snapshot() {
    let mut app = App::test_default();

    let board = app.ctx.create_board("Board".to_string(), None).unwrap();
    let column = app
        .ctx
        .create_column(board.id, "Todo".to_string(), None)
        .unwrap();
    let card = app
        .ctx
        .create_card(
            board.id,
            column.id,
            "Task".to_string(),
            CreateCardOptions::default(),
        )
        .unwrap();

    app.selection.active_board_id = app
        .ctx
        .data_store()
        .list_boards()
        .unwrap()
        .first()
        .map(|b| b.id);
    app.prepare_frame();

    assert_eq!(app.model.boards().len(), 1);
    assert_eq!(app.model.boards()[0].name, "Board");
    assert_eq!(app.model.columns().len(), 1);
    assert_eq!(app.model.all_cards().len(), 1);
    assert_eq!(app.model.card_by_id(card.id).unwrap().title, "Task");
}

#[test]
fn test_model_reflects_mutation_after_prepare_frame() {
    let mut app = App::test_default();

    let board = app.ctx.create_board("Board".to_string(), None).unwrap();
    let column = app
        .ctx
        .create_column(board.id, "Todo".to_string(), None)
        .unwrap();
    let card = app
        .ctx
        .create_card(
            board.id,
            column.id,
            "Original".to_string(),
            CreateCardOptions::default(),
        )
        .unwrap();

    app.selection.active_board_id = app
        .ctx
        .data_store()
        .list_boards()
        .unwrap()
        .first()
        .map(|b| b.id);
    app.prepare_frame();

    assert_eq!(app.model.card_by_id(card.id).unwrap().title, "Original");

    let cmd = kanban_domain::commands::Command::Card(kanban_domain::commands::CardCommand::Update(
        kanban_domain::commands::UpdateCard {
            card_id: card.id,
            updates: CardUpdate {
                title: Some("Updated".to_string()),
                ..Default::default()
            },
        },
    ));
    app.execute_command(cmd).unwrap();
    app.prepare_frame();

    assert_eq!(
        app.model.card_by_id(card.id).unwrap().title,
        "Updated",
        "model must reflect the mutated title after prepare_frame"
    );
}

#[test]
fn test_model_description_reflects_mutation() {
    let mut app = App::test_default();

    let board = app.ctx.create_board("Board".to_string(), None).unwrap();
    let column = app
        .ctx
        .create_column(board.id, "Todo".to_string(), None)
        .unwrap();
    let card = app
        .ctx
        .create_card(
            board.id,
            column.id,
            "Task".to_string(),
            CreateCardOptions {
                description: Some("Initial desc".to_string()),
                ..Default::default()
            },
        )
        .unwrap();

    app.selection.active_board_id = app
        .ctx
        .data_store()
        .list_boards()
        .unwrap()
        .first()
        .map(|b| b.id);
    app.prepare_frame();

    assert_eq!(
        app.model.card_by_id(card.id).unwrap().description,
        Some("Initial desc".to_string())
    );

    let cmd = kanban_domain::commands::Command::Card(kanban_domain::commands::CardCommand::Update(
        kanban_domain::commands::UpdateCard {
            card_id: card.id,
            updates: CardUpdate {
                description: FieldUpdate::Set("Updated desc".to_string()),
                ..Default::default()
            },
        },
    ));
    app.execute_command(cmd).unwrap();
    app.prepare_frame();

    assert_eq!(
        app.model.card_by_id(card.id).unwrap().description,
        Some("Updated desc".to_string()),
        "model must reflect the updated description after prepare_frame"
    );
}

fn type_query(app: &mut App, query: &str) {
    app.filter.board_search.activate();
    for c in query.chars() {
        app.filter.board_search.input.insert_char(c);
    }
}

#[test]
fn test_board_search_query_narrows_projects_panel_to_matching_boards() {
    let mut app = App::test_default();
    app.ctx
        .create_board("Alpha Project".to_string(), None)
        .unwrap();
    app.ctx
        .create_board("Beta Project".to_string(), None)
        .unwrap();
    app.prepare_frame();
    assert_eq!(
        app.displayed_boards().len(),
        2,
        "both boards visible before search"
    );

    type_query(&mut app, "alpha");
    app.prepare_frame();

    let displayed = app.displayed_boards();
    assert_eq!(displayed.len(), 1, "search narrows the projects panel");
    assert_eq!(displayed[0].name, "Alpha Project");
    assert_eq!(
        app.board_list.len(),
        1,
        "board_list's selectable set stays aligned with the filtered displayed_boards"
    );
}

#[test]
fn test_board_search_cleared_restores_full_board_list() {
    let mut app = App::test_default();
    app.ctx
        .create_board("Alpha Project".to_string(), None)
        .unwrap();
    app.ctx
        .create_board("Beta Project".to_string(), None)
        .unwrap();
    app.prepare_frame();

    type_query(&mut app, "alpha");
    app.prepare_frame();
    assert_eq!(
        app.displayed_boards().len(),
        1,
        "narrowed while search is active"
    );

    app.filter.board_search.deactivate();
    app.prepare_frame();

    assert_eq!(
        app.displayed_boards().len(),
        2,
        "clearing the search query restores the full board list"
    );
}
