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

/// Create a board + card, set up labels and labels, load the model, and return
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

    // Load card into the model so handle_manage_labels can read card.labels.
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
    assert_eq!(card.labels, vec!["design".to_string()]);

    // The managed card carries the card's labels, and the row list reflects them:
    // design (applied) ordered above p1 (not applied).
    assert!(
        app.label_manager
            .cards
            .iter()
            .any(|c| c.labels == vec!["design"]),
        "managed card must carry the card's labels, got: {:?}",
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
        !card.labels.contains(&"design".to_string()),
        "card must no longer carry the removed label, got: {:?}",
        card.labels
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
        card.labels.contains(&"p1".to_string()),
        "card must now carry the newly applied label, got: {:?}",
        card.labels
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
async fn test_new_label_row_enter_with_name_creates_only_not_applied() {
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
    // ... but creation must NOT auto-apply it: assigning is an explicit step
    // (the row's apply action), so the managed card is unchanged.
    let card = app.ctx.get_card(card_id).unwrap().unwrap();
    assert!(
        !card.labels.contains(&"urgent".to_string()),
        "creating must not assign; card must not carry 'urgent', got: {:?}",
        card.labels
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
async fn test_new_label_row_esc_in_insert_exits_to_normal_mode() {
    // After typing (INSERT mode), Esc exits to NORMAL mode:
    //   - dialog stays open
    //   - typed text is preserved in the input buffer
    //   - cursor stays on the Create row
    let (mut app, _) = card_with_labels_fixture();
    select_create_row(&mut app);
    let mut terminal = headless_terminal();
    let events = EventHandler::new();
    app.handle_manage_labels_popup(KeyCode::Char('u'), &mut terminal, &events);
    assert_eq!(app.input.as_str(), "u");
    assert!(
        app.label_manager.create_insert,
        "typing a char must activate insert mode"
    );
    app.handle_manage_labels_popup(KeyCode::Esc, &mut terminal, &events);
    assert_eq!(
        app.mode,
        AppMode::Dialog(DialogMode::ManageLabels),
        "Esc in INSERT mode must NOT close the dialog"
    );
    assert!(
        !app.label_manager.create_insert,
        "Esc in INSERT mode must switch back to normal mode"
    );
    assert_eq!(app.input.as_str(), "u", "typed text must be preserved");
    assert_eq!(
        app.label_manager.current_row(),
        Some(kanban_tui::app::LabelRow::Create),
        "cursor must remain on the Create row"
    );
}

#[tokio::test]
async fn test_new_label_row_second_esc_in_normal_closes_dialog() {
    // A second Esc (in NORMAL mode) closes the dialog and clears the input.
    let (mut app, _) = card_with_labels_fixture();
    select_create_row(&mut app);
    let mut terminal = headless_terminal();
    let events = EventHandler::new();
    app.handle_manage_labels_popup(KeyCode::Char('u'), &mut terminal, &events);
    assert_eq!(app.input.as_str(), "u");
    // First Esc: INSERT → NORMAL
    app.handle_manage_labels_popup(KeyCode::Esc, &mut terminal, &events);
    assert_eq!(app.mode, AppMode::Dialog(DialogMode::ManageLabels));
    // Second Esc: NORMAL → close dialog
    app.handle_manage_labels_popup(KeyCode::Esc, &mut terminal, &events);
    assert_ne!(
        app.mode,
        AppMode::Dialog(DialogMode::ManageLabels),
        "second Esc (normal mode) must close the dialog"
    );
    assert!(app.input.is_empty(), "input must be cleared on Esc close");
}

#[tokio::test]
async fn test_new_label_row_esc_in_normal_mode_closes_directly() {
    // If the user navigates to the Create row WITHOUT typing (normal mode),
    // a single Esc closes the dialog directly.
    let (mut app, _) = card_with_labels_fixture();
    select_create_row(&mut app);
    let mut terminal = headless_terminal();
    let events = EventHandler::new();
    // Navigate to the Create row via label_manager (does NOT set create_insert).
    // create_insert should be false at this point (initialised by Default::default).
    assert!(
        !app.label_manager.create_insert,
        "create_insert must be false after navigation to Create row without typing"
    );
    app.handle_manage_labels_popup(KeyCode::Esc, &mut terminal, &events);
    assert_ne!(
        app.mode,
        AppMode::Dialog(DialogMode::ManageLabels),
        "Esc on the Create row in NORMAL mode must close the dialog"
    );
}

#[tokio::test]
async fn test_navigating_to_create_row_enters_insert_mode() {
    // Pressing j (down) from the last label row lands on the Create row and
    // activates insert mode (the "focus on the input box enters input mode" rule).
    let (mut app, _) = card_with_labels_fixture();
    let mut terminal = headless_terminal();
    let events = EventHandler::new();

    // Fixture cursor is on Label(0) "design".  Move to Label(1) "p1" first
    // (p1 has no labels on the card, so it's the last label before Create).
    let p1_row = app
        .label_manager
        .rows
        .iter()
        .position(|r| r.name == "p1")
        .expect("the 'p1' row must exist");
    let p1_flat = app
        .label_manager
        .entries()
        .iter()
        .position(|r| *r == kanban_tui::app::LabelRow::Label(p1_row))
        .expect("the 'p1' flat row must exist");
    app.label_manager.selection.set(Some(p1_flat));
    assert_eq!(
        app.label_manager.current_row(),
        Some(kanban_tui::app::LabelRow::Label(p1_row)),
        "cursor must be on the 'p1' row before navigating down"
    );

    // Navigate down from "p1" to the Create row.
    app.handle_manage_labels_popup(KeyCode::Char('j'), &mut terminal, &events);
    assert_eq!(
        app.label_manager.current_row(),
        Some(kanban_tui::app::LabelRow::Create),
        "cursor must land on the Create row after navigating down from 'p1'"
    );
    assert!(
        app.label_manager.create_insert,
        "navigating to the Create row must activate insert mode (red border)"
    );
}

#[tokio::test]
async fn test_jk_in_insert_mode_types_chars_not_navigates() {
    // In INSERT mode, j and k are typed as characters, not navigation keys.
    let (mut app, _) = card_with_labels_fixture();
    select_create_row(&mut app);
    app.label_manager.create_insert = true; // simulate arriving on Create
    let mut terminal = headless_terminal();
    let events = EventHandler::new();

    app.handle_manage_labels_popup(KeyCode::Char('j'), &mut terminal, &events);
    assert_eq!(app.input.as_str(), "j", "'j' must be typed in insert mode");
    assert_eq!(
        app.label_manager.current_row(),
        Some(kanban_tui::app::LabelRow::Create),
        "cursor must stay on the Create row in insert mode after 'j'"
    );

    app.handle_manage_labels_popup(KeyCode::Char('k'), &mut terminal, &events);
    assert_eq!(app.input.as_str(), "jk", "'k' must be typed in insert mode");
    assert_eq!(
        app.label_manager.current_row(),
        Some(kanban_tui::app::LabelRow::Create),
        "cursor must stay on the Create row in insert mode after 'k'"
    );
}

#[tokio::test]
async fn test_k_in_normal_mode_on_create_navigates_up() {
    // In NORMAL mode on the Create row, k (or Up) navigates up to the
    // previous row and exits create mode.
    let (mut app, _) = card_with_labels_fixture();
    select_create_row(&mut app);
    // create_insert is false (default) = NORMAL mode.
    assert!(
        !app.label_manager.create_insert,
        "must start in normal mode"
    );
    let mut terminal = headless_terminal();
    let events = EventHandler::new();

    // k → navigate up (leaves the Create row).
    app.handle_manage_labels_popup(KeyCode::Char('k'), &mut terminal, &events);
    assert_ne!(
        app.label_manager.current_row(),
        Some(kanban_tui::app::LabelRow::Create),
        "in normal mode 'k' on the Create row must navigate away from it"
    );
    assert!(
        app.label_manager.current_row().is_some(),
        "cursor must land on a valid row after navigating up"
    );
}

#[tokio::test]
async fn test_i_in_normal_mode_enters_insert_mode_without_typing() {
    // Pressing 'i' on the Create row in NORMAL mode switches to INSERT mode
    // (red border) without typing the 'i' character into the buffer.
    let (mut app, _) = card_with_labels_fixture();
    select_create_row(&mut app);
    // create_insert is false (default) = NORMAL mode.
    assert!(
        !app.label_manager.create_insert,
        "must start in NORMAL mode"
    );
    let mut terminal = headless_terminal();
    let events = EventHandler::new();

    app.handle_manage_labels_popup(KeyCode::Char('i'), &mut terminal, &events);

    assert!(
        app.label_manager.create_insert,
        "'i' in NORMAL mode must enter INSERT mode"
    );
    assert!(
        app.input.is_empty(),
        "'i' must NOT be typed into the buffer (pure mode switch)"
    );
    assert_eq!(
        app.label_manager.current_row(),
        Some(kanban_tui::app::LabelRow::Create),
        "cursor must stay on the Create row"
    );
    assert_eq!(
        app.mode,
        AppMode::Dialog(DialogMode::ManageLabels),
        "dialog must stay open"
    );
}

#[tokio::test]
async fn test_n_in_normal_mode_enters_insert_mode_without_typing() {
    // Pressing 'n' on the Create row in NORMAL mode also switches to INSERT
    // mode without typing the 'n' character.
    let (mut app, _) = card_with_labels_fixture();
    select_create_row(&mut app);
    assert!(
        !app.label_manager.create_insert,
        "must start in NORMAL mode"
    );
    let mut terminal = headless_terminal();
    let events = EventHandler::new();

    app.handle_manage_labels_popup(KeyCode::Char('n'), &mut terminal, &events);

    assert!(
        app.label_manager.create_insert,
        "'n' in NORMAL mode must enter INSERT mode"
    );
    assert!(
        app.input.is_empty(),
        "'n' must NOT be typed into the buffer (pure mode switch)"
    );
    assert_eq!(
        app.label_manager.current_row(),
        Some(kanban_tui::app::LabelRow::Create),
        "cursor must stay on the Create row"
    );
}

#[tokio::test]
async fn test_i_in_insert_mode_is_typed_as_character() {
    // In INSERT mode 'i' is a normal typed character, not a mode switch.
    let (mut app, _) = card_with_labels_fixture();
    select_create_row(&mut app);
    app.label_manager.create_insert = true; // already in INSERT mode
    let mut terminal = headless_terminal();
    let events = EventHandler::new();

    app.handle_manage_labels_popup(KeyCode::Char('i'), &mut terminal, &events);

    assert!(app.label_manager.create_insert, "must stay in INSERT mode");
    assert_eq!(
        app.input.as_str(),
        "i",
        "'i' in INSERT mode must be typed as a character"
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
