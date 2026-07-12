use kanban_domain::{CardPriority, CreateCardOptions, KanbanOperations};
use kanban_tui::app::focus::Focus;
use kanban_tui::app::mode::{AppMode, DialogMode};
use kanban_tui::App;

/// Issue #360: the cards list advertises `p: priority`, but pressing `p`
/// did nothing — the binding pointed at `EditCard` and the input router had
/// no lowercase `p` arm. `handle_set_card_priority_key` must open the
/// single-card priority dialog for the highlighted card, with the dialog
/// preselecting that card's current priority.
#[test]
fn test_p_on_cards_list_opens_single_card_priority_dialog() {
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
                priority: Some(CardPriority::High),
                ..CreateCardOptions::default()
            },
        )
        .unwrap();

    app.selection.active_board_index = Some(0);
    app.focus.active = Focus::Cards;
    app.prepare_frame();
    app.select_card_by_id(card.id);

    app.handle_set_card_priority_key();

    assert_eq!(
        app.mode,
        AppMode::Dialog(DialogMode::SetCardPriority),
        "pressing p on a highlighted card must open the single-card priority dialog"
    );
    assert_eq!(
        app.selection.active_card_id,
        Some(card.id),
        "the highlighted card must become the active card for the priority dialog"
    );
    assert_eq!(
        app.dialog_input.priority_selection.get(),
        Some(2),
        "the dialog must preselect the card's current priority (High -> index 2)"
    );
}

/// Guard: `p` is a cards-panel action. With focus on the projects panel it
/// must be inert (no priority dialog, no mode change).
#[test]
fn test_p_on_boards_panel_does_not_open_priority_dialog() {
    let mut app = App::test_default();

    let board = app.ctx.create_board("Board".to_string(), None).unwrap();
    let column = app
        .ctx
        .create_column(board.id, "Todo".to_string(), None)
        .unwrap();
    app.ctx
        .create_card(
            board.id,
            column.id,
            "Task".to_string(),
            CreateCardOptions::default(),
        )
        .unwrap();

    app.selection.active_board_index = Some(0);
    app.focus.active = Focus::Boards;
    app.prepare_frame();

    app.handle_set_card_priority_key();

    assert_eq!(
        app.mode,
        AppMode::Normal,
        "p on the projects panel must not open the priority dialog"
    );
}
