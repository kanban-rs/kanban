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

/// Move the dialog cursor onto the "New label" (Create) row, found by scanning
/// the flat entry list so this does not depend on the header/notes layout.
fn select_create_row(app: &mut App) {
    let entries = app.label_manager.entries();
    let idx = entries
        .iter()
        .position(|r| matches!(r, kanban_tui::app::LabelRow::Create))
        .expect("dialog must contain a Create row");
    app.label_manager.selection.set(Some(idx));
    assert_eq!(
        app.label_manager.current_row(),
        Some(kanban_tui::app::LabelRow::Create),
        "cursor must be on the New label input row"
    );
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

    // The managed card carries the card's tags, and the row list reflects them:
    // design (applied) ordered above p1 (not applied).
    assert!(
        app.label_manager
            .cards
            .iter()
            .any(|c| c.tags == vec!["design"]),
        "managed card must carry the card's tags, got: {:?}",
        app.label_manager.cards
    );
    let row_names: Vec<_> = app
        .label_manager
        .rows
        .iter()
        .map(|r| r.name.clone())
        .collect();
    assert_eq!(row_names, vec!["design".to_string(), "p1".to_string()]);

    // The cursor starts on the first selectable row — the applied "design" row.
    assert_eq!(
        app.label_manager.current_row(),
        Some(kanban_tui::app::LabelRow::Label(0)),
        "cursor must start on the first applied row"
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
async fn test_handle_manage_labels_popup_enter_on_applied_row_removes_from_card() {
    let (mut app, card_id) = card_with_labels_fixture();
    // The cursor starts on the applied "design" row. Removing a label from the
    // card is Enter's job (x and e are reserved for defined labels).
    assert_eq!(
        app.label_manager.current_row(),
        Some(kanban_tui::app::LabelRow::Label(0))
    );

    let mut terminal = headless_terminal();
    let events = EventHandler::new();

    app.handle_manage_labels_popup(KeyCode::Enter, &mut terminal, &events);

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
    // Put the cursor on the "p1" row, found by its name so this does not depend
    // on the divider/rows layout.
    let row_index = app
        .label_manager
        .rows
        .iter()
        .position(|r| r.name == "p1")
        .expect("the 'p1' row must exist");
    let target = kanban_tui::app::LabelRow::Label(row_index);
    let idx = app
        .label_manager
        .entries()
        .iter()
        .position(|r| *r == target)
        .expect("the 'p1' flat row must exist");
    app.label_manager.selection.set(Some(idx));
    assert_eq!(
        app.label_manager.current_row(),
        Some(target),
        "cursor must be on the 'p1' row"
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

#[tokio::test]
async fn test_new_label_row_captures_typed_chars() {
    let (mut app, _) = card_with_labels_fixture();
    select_create_row(&mut app);

    let mut terminal = headless_terminal();
    let events = EventHandler::new();
    app.handle_manage_labels_popup(KeyCode::Char('u'), &mut terminal, &events);
    assert_eq!(app.input.as_str(), "u");
    assert_eq!(
        app.mode,
        AppMode::Dialog(DialogMode::ManageLabels),
        "dialog must stay open while typing"
    );
}

#[tokio::test]
async fn test_new_label_row_backspace_edits() {
    let (mut app, _) = card_with_labels_fixture();
    select_create_row(&mut app);
    let mut terminal = headless_terminal();
    let events = EventHandler::new();
    app.handle_manage_labels_popup(KeyCode::Char('a'), &mut terminal, &events);
    app.handle_manage_labels_popup(KeyCode::Char('b'), &mut terminal, &events);
    assert_eq!(app.input.as_str(), "ab");
    app.handle_manage_labels_popup(KeyCode::Backspace, &mut terminal, &events);
    assert_eq!(app.input.as_str(), "a");
}

#[tokio::test]
async fn test_new_label_row_enter_with_name_creates_and_applies() {
    let (mut app, card_id) = card_with_labels_fixture();
    select_create_row(&mut app);
    let mut terminal = headless_terminal();
    let events = EventHandler::new();
    for c in "urgent".chars() {
        app.handle_manage_labels_popup(KeyCode::Char(c), &mut terminal, &events);
    }
    assert_eq!(app.input.as_str(), "urgent");
    app.handle_manage_labels_popup(KeyCode::Enter, &mut terminal, &events);

    // Registry gained the label ...
    let labels = app.ctx.list_labels().unwrap();
    assert!(
        labels.iter().any(|l| l.name == "urgent"),
        "registry must contain 'urgent', got: {:?}",
        labels.iter().map(|l| l.name.as_str()).collect::<Vec<_>>()
    );
    // ... and it is applied to the managed card.
    let card = app.ctx.get_card(card_id).unwrap().unwrap();
    assert!(
        card.tags.contains(&"urgent".to_string()),
        "card must carry 'urgent', got: {:?}",
        card.tags
    );
    // Input clears after creating, and the dialog stays open to show the result.
    assert!(app.input.is_empty());
    assert_eq!(app.mode, AppMode::Dialog(DialogMode::ManageLabels));
}

#[tokio::test]
async fn test_new_label_row_space_is_typed_not_confirmed() {
    let (mut app, _) = card_with_labels_fixture();
    select_create_row(&mut app);
    let mut terminal = headless_terminal();
    let events = EventHandler::new();
    // A space is an ordinary typed character on the input row, not a confirm:
    // it must land in the buffer (and keep the cursor on the create row) rather
    // than falling through to `resolve()` and opening the editor mid-typing.
    app.handle_manage_labels_popup(KeyCode::Char('a'), &mut terminal, &events);
    app.handle_manage_labels_popup(KeyCode::Char(' '), &mut terminal, &events);
    app.handle_manage_labels_popup(KeyCode::Char('b'), &mut terminal, &events);
    assert_eq!(
        app.input.as_str(),
        "a b",
        "the space must be part of the typed name"
    );
    assert_eq!(
        app.label_manager.current_row(),
        Some(kanban_tui::app::LabelRow::Create),
        "cursor must remain on the New label input row"
    );
    assert_eq!(app.mode, AppMode::Dialog(DialogMode::ManageLabels));
}

#[tokio::test]
async fn test_new_label_row_esc_closes_dialog() {
    let (mut app, _) = card_with_labels_fixture();
    select_create_row(&mut app);
    let mut terminal = headless_terminal();
    let events = EventHandler::new();
    app.handle_manage_labels_popup(KeyCode::Char('u'), &mut terminal, &events);
    assert_eq!(app.input.as_str(), "u");
    app.handle_manage_labels_popup(KeyCode::Esc, &mut terminal, &events);
    assert_ne!(
        app.mode,
        AppMode::Dialog(DialogMode::ManageLabels),
        "Esc on the input row must close the dialog"
    );
    assert!(app.input.is_empty(), "input must be cleared on Esc");
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
