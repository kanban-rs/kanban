use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use kanban_domain::{Board, Card, Column, CreateCardOptions, KanbanOperations, Snapshot};
use kanban_service::git::{CommitRef, GitProvider};
use kanban_tui::app::focus::Focus;
use kanban_tui::app::mode::AppMode;
use kanban_tui::app::CommitsPanel;

struct FakeGitProvider {
    commits: Vec<CommitRef>,
    calls: Arc<AtomicUsize>,
}
impl GitProvider for FakeGitProvider {
    fn commits_for_tag(&self, _tag: &str) -> kanban_domain::KanbanResult<Vec<CommitRef>> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(self.commits.clone())
    }
}

struct ErrGitProvider;
impl GitProvider for ErrGitProvider {
    fn commits_for_tag(&self, _tag: &str) -> kanban_domain::KanbanResult<Vec<CommitRef>> {
        Err(kanban_domain::KanbanError::Internal("boom".to_string()))
    }
}

fn commit(hash: &str, subject: &str) -> CommitRef {
    CommitRef {
        short_hash: hash.into(),
        subject: subject.into(),
        author: "Dev".into(),
        committed_at: chrono::Utc::now(),
    }
}

fn setup_app_in_detail() -> kanban_tui::App {
    let mut app = kanban_tui::App::test_default();

    let mut board = Board::new("TestBoard", Some("KAN"));
    let col = Column::new(board.id, "Backlog", 0);
    let card = Card::new(&mut board, col.id, "Card Alpha", 0);
    let board_id = board.id;
    let card_id = card.id;

    app.model.load_from_snapshot(Snapshot {
        archived_boards: Vec::new(),
        boards: vec![board],
        columns: vec![col],
        cards: vec![card],
        ..Default::default()
    });

    app.selection.active_board_id = Some(board_id);
    app.selection.active_card_id = Some(card_id);
    app.mode = AppMode::CardDetail;
    app
}

fn render_to_string(app: &mut kanban_tui::App) -> String {
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
    let backend = TestBackend::new(120, 40);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| kanban_tui::ui::render(app, frame))
        .unwrap();
    let buffer = terminal.backend().buffer().clone();
    let mut result = String::new();
    for y in 0..buffer.area.height {
        for x in 0..buffer.area.width {
            result.push_str(buffer.cell((x, y)).map(|c| c.symbol()).unwrap_or(" "));
        }
        result.push('\n');
    }
    result
}

#[test]
fn test_card_detail_renders_commits_from_provider() {
    let mut app = setup_app_in_detail();
    let calls = Arc::new(AtomicUsize::new(0));
    let seeded = vec![
        commit("abc1234", "KAN-1 first"),
        commit("def5678", "KAN-1 second"),
    ];
    app.set_git_provider(Some(Box::new(FakeGitProvider {
        commits: seeded.clone(),
        calls: calls.clone(),
    })));

    app.refresh_card_commits();
    let rendered = render_to_string(&mut app);

    assert_eq!(app.commits_panel, CommitsPanel::Loaded(seeded));
    assert!(rendered.contains("abc1234"), "buffer missing first hash");
    assert!(
        rendered.contains("KAN-1 first"),
        "buffer missing first subject"
    );
    assert!(rendered.contains("def5678"), "buffer missing second hash");
    assert!(
        rendered.contains("KAN-1 second"),
        "buffer missing second subject"
    );
}

#[test]
fn test_card_detail_shows_no_commits_message_when_empty() {
    let mut app = setup_app_in_detail();
    let calls = Arc::new(AtomicUsize::new(0));
    app.set_git_provider(Some(Box::new(FakeGitProvider {
        commits: vec![],
        calls,
    })));

    app.refresh_card_commits();
    let rendered = render_to_string(&mut app);

    assert_eq!(app.commits_panel, CommitsPanel::Loaded(vec![]));
    assert!(rendered.contains("No linked commits"));
}

#[test]
fn test_card_detail_shows_unavailable_when_no_repo() {
    let mut app = setup_app_in_detail();
    // No provider set -> git_provider None.

    app.refresh_card_commits();
    let rendered = render_to_string(&mut app);

    assert_eq!(app.commits_panel, CommitsPanel::Unavailable);
    assert!(rendered.contains("Commits unavailable"));
}

#[test]
fn test_card_detail_shows_unavailable_when_provider_errors() {
    let mut app = setup_app_in_detail();
    app.set_git_provider(Some(Box::new(ErrGitProvider)));

    app.refresh_card_commits();

    assert_eq!(app.commits_panel, CommitsPanel::Unavailable);
}

/// Drives the REAL open path (Focus::Boards -> activate -> Focus::Cards ->
/// activate) that a keypress would take, rather than poking `app.mode` and
/// calling `refresh_card_commits()` directly. Proves the hook wired into
/// `handle_selection_activate` (navigation_handlers.rs) actually fires.
#[test]
fn test_opening_card_detail_via_real_handler_fetches_commits() {
    let mut app = kanban_tui::App::test_default();
    let board = app
        .ctx
        .create_board("TestBoard".to_string(), Some("KAN".to_string()))
        .unwrap();
    let col = app
        .ctx
        .create_column(board.id, "Backlog".to_string(), None)
        .unwrap();
    app.ctx
        .create_card(
            board.id,
            col.id,
            "Card Alpha".to_string(),
            CreateCardOptions::default(),
        )
        .unwrap();

    let calls = Arc::new(AtomicUsize::new(0));
    let expected = vec![commit("abc1234", "KAN-1 x")];
    app.set_git_provider(Some(Box::new(FakeGitProvider {
        commits: expected.clone(),
        calls: calls.clone(),
    })));

    app.prepare_frame();
    assert_eq!(
        app.focus.active,
        Focus::Boards,
        "starts on the boards panel"
    );
    app.selection.board.set(Some(0));
    app.handle_selection_activate();
    assert_eq!(
        app.focus.active,
        Focus::Cards,
        "board activation moves focus to cards"
    );
    assert_eq!(
        calls.load(Ordering::SeqCst),
        0,
        "opening a BOARD must not fetch commits"
    );

    if let Some(list) = app.view.strategy.get_active_task_list_mut() {
        list.set_selected_index(Some(0));
    }
    app.handle_selection_activate();

    assert_eq!(
        app.mode,
        AppMode::CardDetail,
        "card activation opens detail"
    );
    assert_eq!(
        calls.load(Ordering::SeqCst),
        1,
        "opening the card via the real handler must trigger exactly one git fetch"
    );
    assert_eq!(app.commits_panel, CommitsPanel::Loaded(expected));
}

#[test]
fn test_commits_fetched_once_on_open_not_per_render() {
    let mut app = setup_app_in_detail();
    let calls = Arc::new(AtomicUsize::new(0));
    app.set_git_provider(Some(Box::new(FakeGitProvider {
        commits: vec![commit("abc1234", "KAN-1 x")],
        calls: calls.clone(),
    })));

    app.refresh_card_commits();
    let _ = render_to_string(&mut app);
    let _ = render_to_string(&mut app);
    let _ = render_to_string(&mut app);

    assert_eq!(calls.load(Ordering::SeqCst), 1);
}
