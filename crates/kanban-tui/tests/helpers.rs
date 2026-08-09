#![allow(dead_code)]

use kanban_tui::app::focus::Focus;
use kanban_tui::app::mode::{AppMode, DialogMode};
use kanban_tui::app::ExportDialogState;
use kanban_tui::App;

pub fn render_widget_to_string<F>(width: u16, height: u16, draw_fn: F) -> String
where
    F: FnOnce(&mut ratatui::Frame),
{
    use ratatui::{backend::TestBackend, Terminal};
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(draw_fn).unwrap();
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

pub fn render_to_string(app: &App) -> String {
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    let backend = TestBackend::new(120, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| {
            kanban_tui::ui::render_settings_view(app, frame, frame.area());
        })
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

pub fn render_to_string_with_colors(app: &App) -> Vec<(String, Option<ratatui::style::Color>)> {
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
    let backend = TestBackend::new(120, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| {
            kanban_tui::ui::render_settings_view(app, frame, frame.area());
        })
        .unwrap();
    let buffer = terminal.backend().buffer().clone();
    let mut result = Vec::new();
    for y in 0..buffer.area.height {
        for x in 0..buffer.area.width {
            let cell = buffer.cell((x, y)).unwrap();
            result.push((cell.symbol().to_string(), cell.style().fg));
        }
    }
    result
}

pub fn setup_settings_app() -> App {
    let mut app = App::test_default();
    app.focus.active = Focus::Boards;
    app.handle_open_settings();
    app
}

pub fn setup_app_with_export_dialog(board_count: usize) -> App {
    use kanban_domain::KanbanOperations;
    let mut app = App::test_default();
    app.focus.active = Focus::Boards;
    app.push_mode(AppMode::Settings);
    let board_ids: Vec<uuid::Uuid> = (0..board_count)
        .map(|i| {
            app.ctx
                .create_board(format!("Board{}", i + 1), None)
                .unwrap()
                .id
        })
        .collect();
    app.export_dialog = Some(ExportDialogState::new(board_ids));
    app.push_mode(AppMode::Dialog(DialogMode::ExportBoards));
    app
}

pub async fn create_test_json_file(dir: &std::path::Path, name: &str, boards: &[&str]) -> String {
    use kanban_persistence::{PersistenceMetadata, PersistenceStore, StoreSnapshot};

    let path = dir.join(name);
    let path_str = path.to_str().unwrap().to_string();
    let store = kanban_persistence_json::JsonFileStore::new(&path_str);

    let domain_boards: Vec<kanban_domain::Board> = boards
        .iter()
        .map(|n| kanban_domain::Board::new(n.to_string(), None::<String>))
        .collect();
    let snapshot = kanban_domain::Snapshot {
        archived_boards: Vec::new(),
        boards: domain_boards,
        columns: vec![],
        cards: vec![],
        archived_cards: vec![],
        sprints: vec![],
        graph: Default::default(),
    };

    let store_snapshot = StoreSnapshot {
        data: serde_json::to_vec(&snapshot).unwrap(),
        metadata: PersistenceMetadata::new(store.instance_id()),
    };
    store.save(store_snapshot).await.unwrap();

    path_str
}

pub async fn create_test_sqlite_file(dir: &std::path::Path, name: &str, boards: &[&str]) -> String {
    use kanban_domain::DataStore;

    let path = dir.join(name);
    let path_str = path.to_str().unwrap().to_string();
    let store = kanban_persistence_sqlite::SqliteStore::open(&path_str)
        .await
        .unwrap();

    let domain_boards: Vec<kanban_domain::Board> = boards
        .iter()
        .map(|n| kanban_domain::Board::new(n.to_string(), None::<String>))
        .collect();
    let snapshot = kanban_domain::Snapshot {
        archived_boards: Vec::new(),
        boards: domain_boards,
        columns: vec![],
        cards: vec![],
        archived_cards: vec![],
        sprints: vec![],
        graph: Default::default(),
    };
    store.apply_snapshot(snapshot).unwrap();

    path_str
}

pub async fn setup_app_with_json_file(dir: &std::path::Path) -> App {
    let path = create_test_json_file(dir, "source.json", &["OriginalBoard"]).await;
    let (mut app, _rx) = App::new(Some(path)).await.unwrap();
    app.load_initial_state().await;
    app
}
