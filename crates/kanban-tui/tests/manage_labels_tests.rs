use crossterm::event::KeyCode;
use kanban_domain::{CreateCardOptions, KanbanOperations, Label};
use kanban_tui::app::{AppMode, DialogMode};
use kanban_tui::events::EventHandler;
use kanban_tui::keybindings::KeybindingProvider;
use kanban_tui::App;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::Rect;
use ratatui::{Terminal, TerminalOptions, Viewport};

fn headless_terminal() -> Terminal<CrosstermBackend<std::io::Stdout>> {
    Terminal::with_options(
        CrosstermBackend::new(std::io::stdout()),
        TerminalOptions {
            viewport: Viewport::Fixed(Rect::new(0, 0, 80, 40)),
        },
    )
    .unwrap()
}

/// Create a board + card, set up labels and tags, load the model, and return
/// the App with the active card selected and the ManageLabels dialog open.
fn card_with_labels_fixture() -> (App, uuid::Uuid) {
    let mut app = App::test_default();
    let board = app.ctx.create_board("Board".to_string(), None).unwrap();
    let column = app
        .ctx
        .create_column(board.id, "TODO".to_string(), None)
        .unwrap();
    let card = app
        .ctx
        .create_card(
            board.id,
            column.id,
            "Test card".to_string(),
            CreateCardOptions::default(),
        )
        .unwrap();

    // Seed two labels in the registry.
    app.ctx.upsert_label(Label::named("design")).unwrap();
    app.ctx.upsert_label(Label::named("p1")).unwrap();

    // Apply "design" to the card (auto-registers already done above).
    app.ctx.assign_label_to_cards("design", &[card.id]).unwrap();

    // Load card into the model so handle_manage_labels can read card.tags.
    let snap = kanban_domain::Snapshot {
        archived_boards: Vec::new(),
        boards: app.ctx.data_store().list_boards().unwrap(),
        columns: app.ctx.data_store().list_all_columns().unwrap(),
        cards: app.ctx.data_store().list_all_cards().unwrap(),
        archived_cards: app.ctx.data_store().list_archived_cards().unwrap(),
        sprints: app.ctx.data_store().list_all_sprints().unwrap(),
        labels: app.ctx.data_store().list_labels().unwrap(),
        graph: app.ctx.data_store().get_graph().unwrap(),
        prefixes: Vec::new(),
    };
    app.load_snapshot(snap);
    app.selection.active_card_id = Some(card.id);

    app.handle_manage_labels();
    (app, card.id)
}

#[test]
fn test_handle_manage_labels_opens_dialog_with_applied_and_registry() {
    let (app, card_id) = card_with_labels_fixture();

    assert_eq!(
        app.mode,
        AppMode::Dialog(DialogMode::ManageLabels),
        "dialog must be open"
    );

    let card = app.ctx.get_card(card_id).unwrap().unwrap();
    assert_eq!(card.tags, vec!["design".to_string()]);

    assert_eq!(
        app.label_manager.applied,
        vec!["design".to_string()],
        "applied list must reflect the card's tags"
    );
    let registry_names: Vec<_> = app
        .label_manager
        .registry
        .iter()
        .map(|l| l.name.clone())
        .collect();
    assert_eq!(registry_names, vec!["design".to_string(), "p1".to_string()]);

    assert_eq!(
        app.label_manager.selection.get(),
        Some(0),
        "cursor must start on the first row"
    );
}

#[tokio::test]
async fn test_handle_manage_labels_popup_esc_closes_dialog() {
    let (mut app, _) = card_with_labels_fixture();
    let mut terminal = headless_terminal();
    let events = EventHandler::new();

    app.handle_manage_labels_popup(KeyCode::Esc, &mut terminal, &events);

    assert_ne!(
        app.mode,
        AppMode::Dialog(DialogMode::ManageLabels),
        "dialog must be closed after Esc"
    );
}

#[tokio::test]
async fn test_handle_manage_labels_popup_x_on_applied_row_removes_from_card() {
    let (mut app, card_id) = card_with_labels_fixture();
    // selection is 0 → applied row for "design"
    assert_eq!(
        app.label_manager.current_row(),
        Some(kanban_tui::app::LabelRow::Applied(0))
    );

    let mut terminal = headless_terminal();
    let events = EventHandler::new();

    app.handle_manage_labels_popup(KeyCode::Char('x'), &mut terminal, &events);

    let card = app.ctx.get_card(card_id).unwrap().unwrap();
    assert!(
        !card.tags.contains(&"design".to_string()),
        "card must no longer carry the removed label, got: {:?}",
        card.tags
    );
    // Registry entry must still exist
    let labels = app.ctx.list_labels().unwrap();
    assert!(
        labels.iter().any(|l| l.name == "design"),
        "registry entry must survive a remove-from-card, got: {:?}",
        labels
    );
}

#[tokio::test]
async fn test_handle_manage_labels_popup_enter_on_registry_row_applies_to_card() {
    let (mut app, card_id) = card_with_labels_fixture();
    // Navigate to "p1" (registry row at flat index 1 = applied.len()[1] + registry_idx 0)
    // applied=["design"], registry=["design","p1"]
    // Flat: 0=Applied(design), 1=Registry(design), 2=Registry(p1), 3=Create
    app.label_manager.selection.set(Some(2));
    assert_eq!(
        app.label_manager.current_row(),
        Some(kanban_tui::app::LabelRow::Registry(1)),
        "cursor must be on 'p1' registry row"
    );

    let mut terminal = headless_terminal();
    let events = EventHandler::new();

    app.handle_manage_labels_popup(KeyCode::Enter, &mut terminal, &events);

    let card = app.ctx.get_card(card_id).unwrap().unwrap();
    assert!(
        card.tags.contains(&"p1".to_string()),
        "card must now carry the newly applied label, got: {:?}",
        card.tags
    );
}

#[test]
fn test_handle_manage_labels_declines_when_card_not_loaded() {
    let mut app = App::test_default();
    // No card created or loaded, active_card_id is None.
    app.selection.active_card_id = None;

    app.handle_manage_labels();

    assert_ne!(
        app.mode,
        AppMode::Dialog(DialogMode::ManageLabels),
        "dialog must not open when there is no active card"
    );
}

#[test]
fn test_manage_labels_keybinding_in_card_detail_help() {
    use kanban_tui::app::focus::CardFocus;
    use kanban_tui::keybindings::card_detail::CardDetailProvider;
    use kanban_tui::keybindings::KeybindingAction;

    let context = CardDetailProvider::new(CardFocus::Title).get_context();
    let has_labels_binding = context
        .bindings
        .iter()
        .any(|kb| kb.key == "l" && kb.action == KeybindingAction::ManageLabels);

    assert!(
        has_labels_binding,
        "'l' -> ManageLabels must appear in the Card Detail help menu"
    );
}
