use kanban_domain::{CreateCardOptions, KanbanOperations};
use kanban_tui::app::mode::{AppMode, DialogMode};
use kanban_tui::App;
use std::fs;
use std::path::Path;
use uuid::Uuid;

fn seed_board_column_sprint_card(app: &mut App) -> (Uuid, Uuid, Uuid, Uuid) {
    let board = app.ctx.create_board("Board".into(), None).unwrap();
    let column = app
        .ctx
        .create_column(board.id, "Todo".into(), None)
        .unwrap();
    let sprint = app.ctx.create_sprint(board.id, None, None).unwrap();
    let card = app
        .ctx
        .create_card(
            board.id,
            column.id,
            "Card".into(),
            CreateCardOptions::default(),
        )
        .unwrap();
    (board.id, column.id, sprint.id, card.id)
}

#[test]
fn test_push_mode_card_detail_populates_the_graph_tier() {
    let mut app = App::test_default();
    let (board_id, _column_id, _sprint_id, card_id) = seed_board_column_sprint_card(&mut app);
    app.selection.active_board_id = Some(board_id);
    app.selection.active_card_id = Some(card_id);

    assert!(!app.model.graph_state().is_loaded());

    app.push_mode(AppMode::CardDetail);

    assert!(app.model.graph_state().is_loaded());
}

#[test]
fn test_open_filter_options_dialog_populates_the_board_sprints_tier() {
    let mut app = App::test_default();
    let (board_id, ..) = seed_board_column_sprint_card(&mut app);
    app.selection.active_board_id = Some(board_id);

    assert!(!app.model.board_sprints_state(board_id).is_loaded());

    app.open_dialog(DialogMode::FilterOptions);

    assert_eq!(
        app.model
            .board_sprints_state(board_id)
            .loaded()
            .map(|s| s.len()),
        Some(1)
    );
}

#[test]
fn test_pop_mode_repopulates_the_scope_of_the_mode_it_returns_to() {
    let mut app = App::test_default();
    let (board_id, column_id, ..) = seed_board_column_sprint_card(&mut app);
    app.selection.active_board_id = Some(board_id);
    app.mode_stack.push(AppMode::Normal);
    app.mode = AppMode::Settings;

    assert!(!app.model.board_columns_state(board_id).is_loaded());
    assert!(!app.model.column_cards_state(column_id).is_loaded());

    app.pop_mode();

    assert_eq!(app.mode, AppMode::Normal);
    assert!(app.model.board_columns_state(board_id).is_loaded());
    assert!(app.model.column_cards_state(column_id).is_loaded());
}

#[test]
fn test_no_direct_mode_assignment_outside_the_mode_stack_seam() {
    let src_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut offenders = Vec::new();
    collect_offenders(&src_dir, &mut offenders);

    assert!(
        offenders.is_empty(),
        "found direct `self.mode = ...` assignments outside the mode_stack seam: {offenders:?}"
    );
}

fn collect_offenders(dir: &Path, offenders: &mut Vec<String>) {
    for entry in fs::read_dir(dir).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.is_dir() {
            collect_offenders(&path, offenders);
            continue;
        }
        if path.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        if path.ends_with("app/mode_stack.rs") {
            continue;
        }
        let contents = fs::read_to_string(&path).unwrap();
        for (idx, line) in contents.lines().enumerate() {
            let trimmed = line.trim_start();
            if !trimmed.starts_with("self.mode") {
                continue;
            }
            let rest = trimmed["self.mode".len()..].trim_start();
            let Some(after_eq) = rest.strip_prefix('=') else {
                continue;
            };
            if after_eq.starts_with('=') {
                continue;
            }
            offenders.push(format!("{}:{}", path.display(), idx + 1));
        }
    }
}
