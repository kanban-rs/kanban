use kanban_domain::{Board, Card, Column, Snapshot};

/// Verify that `get_card_for_detail_view` resolves to the card whose UUID is
/// held in `active_card`, regardless of iteration order.
#[test]
fn test_active_card_detail_shows_selected_card() {
    let mut app = kanban_tui::App::test_default();

    let mut board = Board::new("TestBoard", None::<String>);
    let col = Column::new(board.id, "Backlog", 0);
    let card_a = Card::new(&mut board, col.id, "Card Alpha", 0);
    let card_b = Card::new(&mut board, col.id, "Card Beta", 1);

    let card_b_id = card_b.id;

    app.model.load_from_snapshot(Snapshot {
        archived_boards: Vec::new(),
        cards: vec![card_a, card_b],
        ..Default::default()
    });

    // Select the second card by id.
    app.selection.active_card_id = Some(card_b_id);

    let detail_card = app
        .get_card_for_detail_view()
        .expect("should return a card when active_card is set");

    assert_eq!(
        detail_card.id, card_b_id,
        "detail view must show the card whose UUID matches active_card.id()"
    );
}

/// When `active_card` is None, `get_card_for_detail_view` returns None.
#[test]
fn test_active_card_detail_returns_none_when_no_card_selected() {
    let app = kanban_tui::App::test_default();
    assert!(app.selection.active_card_id.is_none());
    assert!(app.get_card_for_detail_view().is_none());
}
