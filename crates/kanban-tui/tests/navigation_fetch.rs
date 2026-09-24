mod helpers;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use helpers::{CountingBackend, ReadOp, ReadOpLog};
use kanban_domain::{CreateCardOptions, KanbanOperations, LoadState};
use kanban_tui::app::focus::Focus;
use kanban_tui::events::EventHandler;
use kanban_tui::App;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::Rect;
use ratatui::{Terminal, TerminalOptions, Viewport};
use std::io;
use uuid::Uuid;

struct Seed {
    b1: Uuid,
    b2: Uuid,
    b2_col: Uuid,
}

fn seed_two_boards(app: &mut App) -> Seed {
    let board1 = app.ctx.create_board("Board 1".to_string(), None).unwrap();
    let col1 = app
        .ctx
        .create_column(board1.id, "Todo".to_string(), None)
        .unwrap();
    app.ctx
        .create_card(
            board1.id,
            col1.id,
            "Card 1".to_string(),
            CreateCardOptions::default(),
        )
        .unwrap();
    app.ctx
        .create_sprint(board1.id, None, Some("Sprint 1".to_string()))
        .unwrap();

    let board2 = app.ctx.create_board("Board 2".to_string(), None).unwrap();
    let col2 = app
        .ctx
        .create_column(board2.id, "Todo".to_string(), None)
        .unwrap();
    app.ctx
        .create_card(
            board2.id,
            col2.id,
            "Card 2".to_string(),
            CreateCardOptions::default(),
        )
        .unwrap();
    app.ctx
        .create_sprint(board2.id, None, Some("Sprint 2".to_string()))
        .unwrap();

    Seed {
        b1: board1.id,
        b2: board2.id,
        b2_col: col2.id,
    }
}

async fn prime(app: &mut App) -> ReadOpLog {
    let (backend, _reads, ops) = CountingBackend::wrap(app.ctx.backend());
    app.ctx.replace_backend(backend);
    app.load_initial_state().await;
    ops.lock().unwrap().clear();
    ops
}

fn refetch_ops(log: &ReadOpLog) -> Vec<ReadOp> {
    log.lock()
        .unwrap()
        .iter()
        .filter(|op| {
            (op.method == "snapshot" || op.method == "get_graph" || op.method.starts_with("list_"))
                && op.method != "list_prefixes"
        })
        .cloned()
        .collect()
}

fn has_op_with_id(ops: &[ReadOp], method: &str, id: Uuid) -> bool {
    ops.iter()
        .any(|op| op.method == method && op.ids.contains(&id))
}

fn has_op(ops: &[ReadOp], method: &str) -> bool {
    ops.iter().any(|op| op.method == method)
}

fn headless_terminal() -> Terminal<CrosstermBackend<io::Stdout>> {
    Terminal::with_options(
        CrosstermBackend::new(io::stdout()),
        TerminalOptions {
            viewport: Viewport::Fixed(Rect::new(0, 0, 80, 40)),
        },
    )
    .unwrap()
}

#[tokio::test]
async fn test_activating_a_highlighted_board_fetches_only_that_boards_subtree() {
    let mut app = App::test_default();
    let seed = seed_two_boards(&mut app);

    let ops = prime(&mut app).await;

    assert!(app.model.board_columns_state(seed.b1).is_loaded());
    assert!(!app.model.board_columns_state(seed.b2).is_loaded());

    app.board_list.inner_mut().set_selected_index(Some(1));
    ops.lock().unwrap().clear();
    app.focus.active = Focus::Boards;
    app.handle_selection_activate();

    let ops = refetch_ops(&ops);

    assert!(
        has_op_with_id(&ops, "list_columns_by_board", seed.b2),
        "expected list_columns_by_board for board 2, got {ops:?}"
    );
    assert!(
        has_op_with_id(&ops, "list_cards_by_column", seed.b2_col),
        "expected list_cards_by_column for board 2's column, got {ops:?}"
    );
    assert!(!has_op(&ops, "list_boards"), "got {ops:?}");
    assert!(!has_op(&ops, "list_all_columns"), "got {ops:?}");
    assert!(!has_op(&ops, "list_all_cards"), "got {ops:?}");
    assert!(!has_op(&ops, "list_all_sprints"), "got {ops:?}");
    assert!(!has_op(&ops, "snapshot"), "got {ops:?}");
}

#[tokio::test]
async fn test_activating_a_board_loads_its_columns_and_their_cards_in_one_action() {
    let mut app = App::test_default();
    let seed = seed_two_boards(&mut app);

    prime(&mut app).await;

    app.board_list.inner_mut().set_selected_index(Some(1));
    app.focus.active = Focus::Boards;
    app.handle_selection_activate();

    assert!(app.model.board_columns_state(seed.b2).is_loaded());
    assert!(matches!(
        app.model.column_cards_state(seed.b2_col),
        LoadState::Loaded(_)
    ));
}

#[tokio::test]
async fn test_moving_the_projects_highlight_fetches_the_newly_highlighted_boards_subtree() {
    let mut app = App::test_default();
    let seed = seed_two_boards(&mut app);

    let ops = prime(&mut app).await;

    app.focus.active = Focus::Boards;
    ops.lock().unwrap().clear();

    let mut terminal = headless_terminal();
    let events = EventHandler::new();
    app.handle_key_event(KeyEvent::from(KeyCode::Char('j')), &mut terminal, &events);

    assert_eq!(app.board_list.get_selected_board_id(), Some(seed.b2));
    assert!(app.model.board_columns_state(seed.b2).is_loaded());
    assert!(matches!(
        app.model.column_cards_state(seed.b2_col),
        LoadState::Loaded(_)
    ));

    let log = ops.lock().unwrap().clone();
    assert!(
        has_op_with_id(&log, "list_columns_by_board", seed.b2),
        "expected list_columns_by_board for board 2, got {log:?}"
    );
}

#[tokio::test]
async fn test_every_early_return_in_the_key_dispatcher_still_fetches_the_view() {
    async fn fresh_primed_app() -> (App, Seed) {
        let mut app = App::test_default();
        let seed = seed_two_boards(&mut app);
        prime(&mut app).await;
        app.focus.active = Focus::Boards;
        app.board_list.inner_mut().set_selected_index(Some(1));
        (app, seed)
    }

    let mut terminal = headless_terminal();
    let events = EventHandler::new();

    // (a) banner clear
    {
        let (mut app, seed) = fresh_primed_app().await;
        app.set_error("x");
        app.handle_key_event(KeyEvent::from(KeyCode::Char('x')), &mut terminal, &events);
        assert!(
            app.model.board_columns_state(seed.b2).is_loaded(),
            "banner-clear early return must still fetch"
        );
    }

    // (b) Ctrl+a
    {
        let (mut app, seed) = fresh_primed_app().await;
        app.handle_key_event(
            KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL),
            &mut terminal,
            &events,
        );
        assert!(
            app.model.board_columns_state(seed.b2).is_loaded(),
            "ctrl+a early return must still fetch"
        );
    }

    // (c) 'q' with no pending saves
    {
        let (mut app, seed) = fresh_primed_app().await;
        app.handle_key_event(KeyEvent::from(KeyCode::Char('q')), &mut terminal, &events);
        assert!(
            app.model.board_columns_state(seed.b2).is_loaded(),
            "quit early return must still fetch"
        );
    }

    // (d) F12 already resolves via open_error_log/push_mode
    {
        let (mut app, seed) = fresh_primed_app().await;
        app.handle_key_event(KeyEvent::from(KeyCode::F(12)), &mut terminal, &events);
        assert!(
            app.model.board_columns_state(seed.b2).is_loaded(),
            "F12 must stay Loaded"
        );
    }

    // (e) '?' already resolves via set_mode(Help)
    {
        let (mut app, seed) = fresh_primed_app().await;
        app.handle_key_event(KeyEvent::from(KeyCode::Char('?')), &mut terminal, &events);
        assert!(
            app.model.board_columns_state(seed.b2).is_loaded(),
            "'?' must stay Loaded"
        );
    }
}

#[tokio::test]
async fn test_a_failed_scoped_fetch_is_recorded_once_and_retried_on_the_next_key() {
    let mut app = App::test_default();
    let seed = seed_two_boards(&mut app);
    app.load_initial_state().await;

    let inner = app.ctx.backend();
    let failing = CountingBackend::wrap_failing(inner.clone(), "list_columns_by_board");
    let (outer, _reads, ops) = CountingBackend::wrap(failing);
    app.ctx.replace_backend(outer);

    app.focus.active = Focus::Boards;
    app.board_list.inner_mut().set_selected_index(Some(1));
    ops.lock().unwrap().clear();

    let mut terminal = headless_terminal();
    let events = EventHandler::new();
    app.handle_key_event(KeyEvent::from(KeyCode::Char('j')), &mut terminal, &events);

    assert!(app.model.board_columns_state(seed.b2).is_failed());
    let log = ops.lock().unwrap().clone();
    let failing_reads: Vec<_> = log
        .iter()
        .filter(|op| op.method == "list_columns_by_board" && op.ids.contains(&seed.b2))
        .collect();
    assert_eq!(
        failing_reads.len(),
        1,
        "expected exactly one failing read, got {log:?}"
    );

    for _ in 0..5 {
        app.prepare_frame();
    }
    let log_after_frames = ops.lock().unwrap().clone();
    assert_eq!(
        log_after_frames.len(),
        log.len(),
        "prepare_frame must never fetch"
    );

    let (healthy, _reads2, ops2) = CountingBackend::wrap(inner.clone());
    app.ctx.replace_backend(healthy);
    app.handle_key_event(KeyEvent::from(KeyCode::Char('j')), &mut terminal, &events);

    assert!(app.model.board_columns_state(seed.b2).is_loaded());
    let retry_log = ops2.lock().unwrap().clone();
    let retries: Vec<_> = retry_log
        .iter()
        .filter(|op| op.method == "list_columns_by_board" && op.ids.contains(&seed.b2))
        .collect();
    assert_eq!(
        retries.len(),
        1,
        "expected exactly one retry read, got {retry_log:?}"
    );
}

#[tokio::test]
async fn test_reactivating_an_already_loaded_board_reads_nothing() {
    let mut app = App::test_default();
    let seed = seed_two_boards(&mut app);
    let ops = prime(&mut app).await;

    app.board_list.inner_mut().set_selected_index(Some(1));
    app.focus.active = Focus::Boards;
    app.handle_selection_activate();
    assert!(app.model.board_columns_state(seed.b2).is_loaded());

    ops.lock().unwrap().clear();
    app.handle_escape_key();
    app.handle_selection_activate();

    let ops = refetch_ops(&ops);
    assert!(ops.is_empty(), "expected no refetch reads, got {ops:?}");
}

#[tokio::test]
async fn test_navigating_away_from_a_failed_panel_does_not_refetch_it() {
    let mut app = App::test_default();
    let seed = seed_two_boards(&mut app);
    app.load_initial_state().await;

    let inner = app.ctx.backend();
    let failing = CountingBackend::wrap_failing(inner.clone(), "list_columns_by_board");
    let (outer, _reads, ops) = CountingBackend::wrap(failing);
    app.ctx.replace_backend(outer);

    app.focus.active = Focus::Boards;
    app.board_list.inner_mut().set_selected_index(Some(1));
    app.handle_selection_activate();
    assert!(app.model.board_columns_state(seed.b2).is_failed());

    ops.lock().unwrap().clear();
    app.handle_escape_key();
    app.board_list.inner_mut().set_selected_index(Some(0));
    app.handle_selection_activate();

    let log = ops.lock().unwrap().clone();
    assert!(
        !has_op_with_id(&log, "list_columns_by_board", seed.b2),
        "expected no re-read of board 2's failed tier, got {log:?}"
    );
}
