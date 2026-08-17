use kanban_domain::{CreateCardOptions, KanbanOperations};
use kanban_tui::components::build_description_lines;
use kanban_tui::App;

#[test]
fn test_card_description_appears_in_detail_view() {
    let mut app = App::test_default();

    // Create board and column
    let board = app
        .ctx
        .create_board("Test Board".to_string(), None)
        .unwrap();
    let column = app
        .ctx
        .create_column(board.id, "Todo".to_string(), None)
        .unwrap();

    // Create card with description
    let card = app
        .ctx
        .create_card(
            board.id,
            column.id,
            "Test Task".to_string(),
            CreateCardOptions {
                description: Some("This is a test description".to_string()),
                ..Default::default()
            },
        )
        .unwrap();

    // Setup app state to show the card detail view
    app.selection.active_board_id = app
        .ctx
        .data_store()
        .list_boards()
        .unwrap()
        .first()
        .map(|b| b.id);
    app.selection.active_card_id = Some(card.id);

    // Verify the card has the description
    app.reload_model();
    app.prepare_frame();
    let cards = app.model.all_cards();
    assert_eq!(cards.len(), 1);
    let displayed_card = &cards[0];

    println!("Card title: {}", displayed_card.title);
    println!("Card description: {:?}", displayed_card.description);

    assert_eq!(displayed_card.id, card.id);
    assert_eq!(displayed_card.title, "Test Task");
    assert_eq!(
        displayed_card.description,
        Some("This is a test description".to_string())
    );
}

#[test]
fn test_card_description_preserved_after_edit() {
    let mut app = App::test_default();

    // Create board and column
    let board = app
        .ctx
        .create_board("Test Board".to_string(), None)
        .unwrap();
    let column = app
        .ctx
        .create_column(board.id, "Todo".to_string(), None)
        .unwrap();

    // Create card with description
    let card = app
        .ctx
        .create_card(
            board.id,
            column.id,
            "Test Task".to_string(),
            CreateCardOptions {
                description: Some("Original description".to_string()),
                ..Default::default()
            },
        )
        .unwrap();

    // Verify description exists
    app.reload_model();
    app.prepare_frame();
    let cards_before = app.model.all_cards();
    assert_eq!(
        cards_before[0].description,
        Some("Original description".to_string())
    );

    // Update the card's title
    use kanban_domain::CardUpdate;
    let updates = CardUpdate {
        title: Some("Updated Title".to_string()),
        ..Default::default()
    };
    let cmd = kanban_domain::commands::Command::Card(kanban_domain::commands::CardCommand::Update(
        kanban_domain::commands::UpdateCard {
            card_id: card.id,
            updates,
        },
    ));
    app.execute_command(cmd).unwrap();

    // Verify description is still there after update
    app.reload_model();
    app.prepare_frame();
    let cards_after = app.model.all_cards();
    assert_eq!(cards_after.len(), 1);
    assert_eq!(cards_after[0].title, "Updated Title");
    assert_eq!(
        cards_after[0].description,
        Some("Original description".to_string()),
        "Description should be preserved after updating title"
    );
}

#[test]
fn test_markdown_rendering_of_description() {
    use kanban_domain::{Board, Card};
    use kanban_tui::components::*;

    let board = Board::new("Test Board", None::<String>);
    let column_id = uuid::Uuid::new_v4();

    // Test rendering of non-empty description
    let mut card = Card::new(board.id, column_id, "Test", 0);
    card.description = Some("# Heading\n\nSome **bold** text".to_string());

    let lines = build_description_lines(&card);
    println!("Description lines (non-empty): {:?}", lines);
    assert!(!lines.is_empty(), "Description should render some lines");

    // Test rendering of empty string
    card.description = Some("".to_string());
    let lines = build_description_lines(&card);
    println!("Description lines (empty string): {:?}", lines);
    assert!(
        !lines.is_empty(),
        "Empty string should still render 'No description' text"
    );
    assert_eq!(
        lines[0].to_string(),
        "No description",
        "Empty description should show placeholder text"
    );

    // Test rendering of None
    card.description = None;
    let lines = build_description_lines(&card);
    println!("Description lines (None): {:?}", lines);
    assert!(
        !lines.is_empty(),
        "None description should render 'No description' text"
    );
}

#[test]
fn test_card_with_empty_string_description_displays_placeholder() {
    let mut app = App::test_default();

    // Create board and column
    let board = app
        .ctx
        .create_board("Test Board".to_string(), None)
        .unwrap();
    let column = app
        .ctx
        .create_column(board.id, "Todo".to_string(), None)
        .unwrap();

    // Create card with empty string description (simulating user clearing a description)
    let card = app
        .ctx
        .create_card(
            board.id,
            column.id,
            "Test Task".to_string(),
            CreateCardOptions {
                description: Some("".to_string()), // Empty description
                ..Default::default()
            },
        )
        .unwrap();

    // Setup app state
    app.selection.active_board_id = app
        .ctx
        .data_store()
        .list_boards()
        .unwrap()
        .first()
        .map(|b| b.id);
    app.selection.active_card_id = Some(card.id);

    // Verify the card has an empty string description (not None)
    app.reload_model();
    app.prepare_frame();
    let cards = app.model.all_cards();
    assert_eq!(cards[0].description, Some("".to_string()));

    // Verify rendering shows placeholder text instead of blank
    let lines = build_description_lines(&cards[0]);
    assert!(
        !lines.is_empty(),
        "Empty string description should show 'No description' placeholder"
    );
    assert_eq!(lines[0].to_string(), "No description");
}
