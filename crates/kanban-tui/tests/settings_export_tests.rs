mod helpers;

use kanban_domain::KanbanOperations;
use kanban_tui::app::focus::Focus;
use kanban_tui::app::mode::{AppMode, DialogMode};
use kanban_tui::app::{ExportDialogState, ExportFormat, ExportStep};
use kanban_tui::keybindings::KeybindingRegistry;
use kanban_tui::App;
use std::collections::HashSet;
use uuid::Uuid;

fn ids(count: usize) -> Vec<Uuid> {
    (0..count).map(|_| Uuid::new_v4()).collect()
}

#[test]
fn test_export_dialog_state_new_defaults_none_selected() {
    let board_ids = ids(3);
    let state = ExportDialogState::new(board_ids.clone());
    assert_eq!(state.board_ids, board_ids);
    assert_eq!(state.board_selections, vec![false, false, false]);
    assert_eq!(state.cursor, 0);
    assert_eq!(state.step, ExportStep::SelectBoards);
}

#[test]
fn test_export_dialog_state_toggle_board() {
    let mut state = ExportDialogState::new(ids(3));
    assert!(!state.board_selections[1]);
    state.toggle(1);
    assert!(state.board_selections[1]);
    state.toggle(1);
    assert!(!state.board_selections[1]);
}

#[test]
fn test_export_dialog_state_select_all() {
    let mut state = ExportDialogState::new(ids(3));
    state.select_all();
    assert!(state.board_selections.iter().all(|&s| s));
    state.select_all();
    assert!(state.board_selections.iter().all(|&s| !s));
}

#[test]
fn test_export_dialog_format_default_is_json() {
    let state = ExportDialogState::new(ids(1));
    assert_eq!(state.format, ExportFormat::Json);
}

#[test]
fn test_export_dialog_from_selection_preselects_checkboxes_for_multi_selected_boards() {
    let board_ids = ids(3);
    let mut preselected = HashSet::new();
    preselected.insert(board_ids[0]);
    preselected.insert(board_ids[2]);

    let state = ExportDialogState::from_selection(&board_ids, &preselected);

    assert_eq!(state.board_ids, board_ids);
    assert_eq!(state.board_selections, vec![true, false, true]);
}

#[test]
fn test_export_dialog_from_selection_falls_back_to_none_selected_when_selection_empty() {
    let board_ids = ids(2);

    let state = ExportDialogState::from_selection(&board_ids, &HashSet::new());

    assert_eq!(state.board_ids, board_ids);
    assert_eq!(state.board_selections, vec![false, false]);
}

#[test]
fn test_settings_x_keybinding_registered() {
    let mut app = App::test_default();
    app.push_mode(AppMode::Settings);

    let provider = KeybindingRegistry::get_provider(&app);
    let context = provider.get_context();
    let keys: Vec<&str> = context.bindings.iter().map(|b| b.key.as_str()).collect();
    assert!(keys.contains(&"x"), "Missing 'x' keybinding in Settings");
}

#[test]
fn test_export_dialog_esc_cancels() {
    use crossterm::event::KeyCode;

    let mut app = helpers::setup_app_with_export_dialog(1);
    assert!(matches!(
        app.mode,
        AppMode::Dialog(DialogMode::ExportBoards)
    ));

    app.handle_export_boards_dialog(KeyCode::Esc);
    assert_eq!(app.mode, AppMode::Settings);
    assert!(app.export_dialog.is_none());
}

#[test]
fn test_export_dialog_board_selection_space_toggles() {
    use crossterm::event::KeyCode;

    let mut app = helpers::setup_app_with_export_dialog(2);

    app.handle_export_boards_dialog(KeyCode::Char(' '));
    assert!(app.export_dialog.as_ref().unwrap().board_selections[0]);
}

#[test]
fn test_export_dialog_board_selection_a_selects_all() {
    use crossterm::event::KeyCode;

    let mut app = helpers::setup_app_with_export_dialog(2);

    app.handle_export_boards_dialog(KeyCode::Char('a'));
    let dialog = app.export_dialog.as_ref().unwrap();
    assert!(dialog.board_selections.iter().all(|&s| s));
}

#[test]
fn test_export_dialog_enter_proceeds_to_options() {
    use crossterm::event::KeyCode;

    let mut app = helpers::setup_app_with_export_dialog(1);

    app.handle_export_boards_dialog(KeyCode::Char(' '));
    app.handle_export_boards_dialog(KeyCode::Enter);
    assert_eq!(
        app.export_dialog.as_ref().unwrap().step,
        ExportStep::ExportOptions
    );
}

#[test]
fn test_export_dialog_enter_with_none_selected_does_not_proceed() {
    use crossterm::event::KeyCode;

    let mut app = helpers::setup_app_with_export_dialog(1);

    app.handle_export_boards_dialog(KeyCode::Enter);
    assert_eq!(
        app.export_dialog.as_ref().unwrap().step,
        ExportStep::SelectBoards
    );
}

#[test]
fn test_render_export_boards_select_step_shows_board_names() {
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    let mut app = App::test_default();
    app.ctx.create_board("MyTestBoard".into(), None).unwrap();
    app.selection.active_board_id = app
        .ctx
        .data_store()
        .list_boards()
        .unwrap()
        .first()
        .map(|b| b.id);
    app.prepare_frame();
    let board_id = app.selection.active_board_id.unwrap();
    app.export_dialog = Some(ExportDialogState::new(vec![board_id]));
    app.push_mode(AppMode::Settings);
    app.push_mode(AppMode::Dialog(DialogMode::ExportBoards));

    let backend = TestBackend::new(120, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| {
            kanban_tui::ui::render(&mut app, frame);
        })
        .unwrap();

    let buffer = terminal.backend().buffer().clone();
    let mut result = String::new();
    for y in 0..buffer.area.height {
        for x in 0..buffer.area.width {
            result.push_str(buffer.cell((x, y)).map(|c| c.symbol()).unwrap_or(" "));
        }
    }
    assert!(
        result.contains("MyTestBoard"),
        "Board name not found in render output"
    );
}

#[test]
fn test_render_export_boards_options_step_shows_filename() {
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    let mut app = App::test_default();
    let board = app.ctx.create_board("Board1".into(), None).unwrap();
    let mut dialog = ExportDialogState::new(vec![board.id]);
    dialog.step = ExportStep::ExportOptions;
    dialog.board_selections[0] = true;
    app.export_dialog = Some(dialog);
    app.push_mode(AppMode::Settings);
    app.push_mode(AppMode::Dialog(DialogMode::ExportBoards));

    let backend = TestBackend::new(120, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| {
            kanban_tui::ui::render(&mut app, frame);
        })
        .unwrap();

    let buffer = terminal.backend().buffer().clone();
    let mut result = String::new();
    for y in 0..buffer.area.height {
        for x in 0..buffer.area.width {
            result.push_str(buffer.cell((x, y)).map(|c| c.symbol()).unwrap_or(" "));
        }
    }
    assert!(
        result.contains("export.json"),
        "Default filename not found in render output"
    );
}

#[test]
fn test_export_boards_json_creates_file() {
    use crossterm::event::KeyCode;

    let dir = tempfile::TempDir::new().unwrap();
    let export_path = dir.path().join("test_export.json");

    let mut app = App::test_default();
    app.focus.active = Focus::Boards;
    app.push_mode(AppMode::Settings);

    let board = app.ctx.create_board("ExportTest".into(), None).unwrap();
    app.ctx
        .create_column(board.id, "Todo".into(), None)
        .unwrap();

    app.prepare_frame();
    app.export_dialog = Some(ExportDialogState::new(vec![board.id]));
    app.push_mode(AppMode::Dialog(DialogMode::ExportBoards));

    app.handle_export_boards_dialog(KeyCode::Char(' '));
    app.handle_export_boards_dialog(KeyCode::Enter);

    {
        let dialog = app.export_dialog.as_mut().unwrap();
        dialog.filename = export_path.to_string_lossy().to_string();
    }

    app.handle_export_boards_dialog(KeyCode::Enter);

    assert!(export_path.exists(), "Export file was not created");
    let content = std::fs::read_to_string(&export_path).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
    assert!(parsed["boards"].is_array());
    assert!(content.contains("ExportTest"));
}

#[test]
fn test_execute_export_uses_explicit_board_ids_not_live_boards_snapshot_at_confirm_time() {
    use crossterm::event::KeyCode;

    let dir = tempfile::TempDir::new().unwrap();
    let export_path = dir.path().join("test_export.json");

    let mut app = App::test_default();
    app.focus.active = Focus::Boards;
    app.push_mode(AppMode::Settings);

    let board_a = app.ctx.create_board("BoardA".into(), None).unwrap();
    let board_b = app.ctx.create_board("BoardB".into(), None).unwrap();
    app.prepare_frame();

    let mut preselected = std::collections::HashSet::new();
    preselected.insert(board_b.id);
    let live_ids: Vec<Uuid> = vec![board_a.id, board_b.id];
    app.export_dialog = Some(ExportDialogState::from_selection(&live_ids, &preselected));
    app.push_mode(AppMode::Dialog(DialogMode::ExportBoards));

    // A board is created AFTER the dialog was seeded, changing what
    // `self.model.live_boards()` would return if `execute_export` re-derived
    // positions from it at confirm time instead of using the dialog's own
    // captured `board_ids`.
    app.ctx.create_board("BoardC".into(), None).unwrap();
    app.prepare_frame();

    app.handle_export_boards_dialog(KeyCode::Enter);
    {
        let dialog = app.export_dialog.as_mut().unwrap();
        dialog.filename = export_path.to_string_lossy().to_string();
    }
    app.handle_export_boards_dialog(KeyCode::Enter);

    assert!(export_path.exists(), "Export file was not created");
    let content = std::fs::read_to_string(&export_path).unwrap();
    assert!(
        content.contains("BoardB"),
        "the board selected at dialog-seed time must be exported"
    );
    assert!(
        !content.contains("BoardA"),
        "only the pre-selected board must be exported, not every live board"
    );
    assert!(
        !content.contains("BoardC"),
        "a board created after the dialog was seeded must not sneak into the export"
    );
}
