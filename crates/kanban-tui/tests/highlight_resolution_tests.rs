use kanban_core::AppConfig;
use kanban_domain::{CreateCardOptions, KanbanOperations};
use kanban_tui::app::focus::Focus;
use kanban_tui::App;
use ratatui::backend::TestBackend;
use ratatui::style::Color;
use ratatui::Terminal;

fn seed_app() -> App {
    let mut app = App::test_default();

    let board = app.ctx.create_board("Board".to_string(), None).unwrap();
    let col = app
        .ctx
        .create_column(board.id, "Todo".to_string(), None)
        .unwrap();
    let card_a = app
        .ctx
        .create_card(
            board.id,
            col.id,
            "Card A".to_string(),
            CreateCardOptions::default(),
        )
        .unwrap();
    app.ctx
        .create_card(
            board.id,
            col.id,
            "Card B".to_string(),
            CreateCardOptions::default(),
        )
        .unwrap();
    let sprint = app
        .ctx
        .create_sprint(board.id, None, Some("Sprint 1".to_string()))
        .unwrap();

    app.ctx.assign_card_to_sprint(card_a.id, sprint.id).unwrap();

    app.selection.active_board_id = Some(board.id);

    app.reload_model();
    app.prepare_frame();
    app.focus.active = Focus::Cards;
    app.view
        .strategy
        .get_active_task_list_mut()
        .unwrap()
        .set_selected_index(Some(0));

    app
}

fn render_buffer(app: &mut App) -> ratatui::buffer::Buffer {
    let backend = TestBackend::new(120, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| {
            kanban_tui::ui::render(app, frame);
        })
        .unwrap();
    terminal.backend().buffer().clone()
}

fn find_row_containing(buffer: &ratatui::buffer::Buffer, needle: &str) -> u16 {
    for y in 0..buffer.area.height {
        let mut row = String::new();
        for x in 0..buffer.area.width {
            row.push_str(buffer.cell((x, y)).map(|c| c.symbol()).unwrap_or(" "));
        }
        if row.contains(needle) {
            return y;
        }
    }
    panic!("row containing {needle:?} not found in buffer");
}

fn find_col_containing(buffer: &ratatui::buffer::Buffer, y: u16, needle: &str) -> u16 {
    for x in 0..buffer.area.width {
        if buffer
            .cell((x, y))
            .map(|c| c.symbol() == needle)
            .unwrap_or(false)
        {
            return x;
        }
    }
    panic!("column containing {needle:?} not found on row {y}");
}

#[test]
fn test_highlight_background_defaults_to_terminal_blue() {
    assert_eq!(
        kanban_tui::theme::highlight_background(&AppConfig::default()),
        Color::Blue
    );
}

#[test]
fn test_highlight_background_maps_a_valid_value_to_rgb() {
    let lower = AppConfig {
        background_highlight: Some("#2e3440".to_string()),
        ..Default::default()
    };
    let upper = AppConfig {
        background_highlight: Some("#2E3440".to_string()),
        ..Default::default()
    };
    assert_eq!(
        kanban_tui::theme::highlight_background(&lower),
        Color::Rgb(0x2E, 0x34, 0x40)
    );
    assert_eq!(
        kanban_tui::theme::highlight_background(&upper),
        Color::Rgb(0x2E, 0x34, 0x40)
    );
}

#[test]
fn test_highlight_background_falls_back_for_an_invalid_value() {
    for raw in ["blue", "#2E3", "2E3440"] {
        let config = AppConfig {
            background_highlight: Some(raw.to_string()),
            ..Default::default()
        };
        assert_eq!(
            kanban_tui::theme::highlight_background(&config),
            Color::Blue,
            "expected fallback to Color::Blue for {raw:?}"
        );
    }
}

#[test]
fn test_selected_card_row_uses_terminal_blue_when_nothing_is_configured() {
    let mut app = seed_app();
    let buffer = render_buffer(&mut app);
    let row = find_row_containing(&buffer, "Card A");
    let col = find_col_containing(&buffer, row, "C");
    let cell = buffer.cell((col, row)).unwrap();
    assert_eq!(cell.bg, Color::Blue);
}

#[test]
fn test_selected_card_row_uses_the_configured_background_highlight() {
    let mut app = seed_app();
    app.app_config.background_highlight = Some("#2E3440".to_string());

    let buffer = render_buffer(&mut app);
    let row = find_row_containing(&buffer, "Card A");
    let expected = Color::Rgb(0x2E, 0x34, 0x40);

    let priority_col = find_col_containing(&buffer, row, "\u{25cf}");
    assert_eq!(
        buffer.cell((priority_col, row)).unwrap().bg,
        expected,
        "priority dot cell"
    );

    let checkbox_col = find_col_containing(&buffer, row, "[");
    assert_eq!(
        buffer.cell((checkbox_col, row)).unwrap().bg,
        expected,
        "checkbox cell"
    );

    let title_col = find_col_containing(&buffer, row, "C");
    assert_eq!(
        buffer.cell((title_col, row)).unwrap().bg,
        expected,
        "title cell"
    );

    let suffix_col = find_col_containing(&buffer, row, "(");
    assert_eq!(
        buffer.cell((suffix_col, row)).unwrap().bg,
        expected,
        "sprint suffix cell"
    );

    let points_col = priority_col + 2;
    let points_cell = buffer.cell((points_col, row)).unwrap();
    assert_eq!(points_cell.symbol(), " ", "points cell symbol");
    assert_eq!(points_cell.bg, expected, "points cell");

    let separator_cell = buffer.cell((priority_col + 3, row)).unwrap();
    assert_ne!(
        separator_cell.bg, expected,
        "raw separator cell should not carry the configured bg, to anchor the offset"
    );
}

#[test]
fn test_selected_card_row_keeps_its_text_colour_on_the_configured_background() {
    let mut app = seed_app();
    app.app_config.background_highlight = Some("#2E3440".to_string());

    let buffer = render_buffer(&mut app);
    let row = find_row_containing(&buffer, "Card A");
    let title_col = find_col_containing(&buffer, row, "C");
    assert_eq!(buffer.cell((title_col, row)).unwrap().fg, Color::White);
}

#[test]
fn test_unselected_card_row_has_no_highlight_background() {
    let mut app = seed_app();
    app.app_config.background_highlight = Some("#2E3440".to_string());

    let buffer = render_buffer(&mut app);
    let row = find_row_containing(&buffer, "Card B");
    let title_col = find_col_containing(&buffer, row, "C");
    let bg = buffer.cell((title_col, row)).unwrap().bg;
    assert_ne!(bg, Color::Blue);
    assert_ne!(bg, Color::Rgb(0x2E, 0x34, 0x40));
}

#[test]
fn test_unfocused_task_list_shows_no_highlight() {
    let mut app = seed_app();
    app.app_config.background_highlight = Some("#2E3440".to_string());
    app.focus.active = Focus::Boards;

    let buffer = render_buffer(&mut app);
    let row = find_row_containing(&buffer, "Card A");
    let title_col = find_col_containing(&buffer, row, "C");
    let bg = buffer.cell((title_col, row)).unwrap().bg;
    assert_ne!(bg, Color::Rgb(0x2E, 0x34, 0x40));
}

mod startup {
    use kanban_tui::error_log::InMemoryLogLayer;
    use kanban_tui::App;
    use ratatui::style::Color;
    use std::sync::{Arc, Mutex};
    use tracing_subscriber::layer::SubscriberExt;

    fn test_store_manager() -> kanban_service::StoreManager {
        let registry = kanban_persistence::StoreRegistry::new();
        let backends = kanban_backend::KanbanBackendRegistry::new();
        kanban_service::StoreManager::new(registry, backends)
    }

    #[tokio::test]
    async fn test_startup_with_an_invalid_background_highlight_logs_one_warning_and_opens() {
        use tracing::instrument::WithSubscriber;

        let dir = tempfile::tempdir().unwrap();
        let sm = test_store_manager();
        let config = kanban_core::AppConfig {
            configuration_location: Some(
                dir.path().join("config.toml").to_string_lossy().to_string(),
            ),
            background_highlight: Some("#2E3".to_string()),
            ..Default::default()
        };

        let log = Arc::new(Mutex::new(kanban_tui::error_log::ErrorLogState::default()));
        let subscriber =
            tracing_subscriber::registry().with(InMemoryLogLayer::new(Arc::clone(&log)));

        let result = App::new_with_store_and_config(sm, None, config)
            .with_subscriber(subscriber)
            .await;

        let app = match result {
            Ok((app, _save_rx)) => app,
            Err(e) => panic!("expected startup to succeed with an invalid value, got: {e}"),
        };

        {
            let state = log.lock().unwrap();
            let warnings: Vec<_> = state
                .entries
                .iter()
                .filter(|entry| entry.message.contains("background_highlight"))
                .collect();
            assert_eq!(
                warnings.len(),
                1,
                "expected exactly one warning entry, got: {:?}",
                state.entries
            );
            assert!(
                matches!(warnings[0].level, kanban_tui::error_log::LogLevel::Warn),
                "expected the background_highlight entry to be logged at Warn level, got: {:?}",
                warnings[0]
            );
            assert_eq!(
                state.error_count, 0,
                "an invalid background_highlight must not auto-open the F12 panel"
            );
        }

        assert_eq!(app.highlight_bg(), Color::Blue);
    }

    #[tokio::test]
    async fn test_startup_with_a_valid_background_highlight_logs_nothing() {
        use tracing::instrument::WithSubscriber;

        let dir = tempfile::tempdir().unwrap();
        let sm = test_store_manager();
        let config = kanban_core::AppConfig {
            configuration_location: Some(
                dir.path().join("config.toml").to_string_lossy().to_string(),
            ),
            background_highlight: Some("#2E3440".to_string()),
            ..Default::default()
        };

        let log = Arc::new(Mutex::new(kanban_tui::error_log::ErrorLogState::default()));
        let subscriber =
            tracing_subscriber::registry().with(InMemoryLogLayer::new(Arc::clone(&log)));

        let result = App::new_with_store_and_config(sm, None, config)
            .with_subscriber(subscriber)
            .await;

        match result {
            Ok((app, _save_rx)) => drop(app),
            Err(e) => panic!("expected startup to succeed with a valid value, got: {e}"),
        }

        {
            let state = log.lock().unwrap();
            let warnings: Vec<_> = state
                .entries
                .iter()
                .filter(|entry| entry.message.contains("background_highlight"))
                .collect();
            assert!(
                warnings.is_empty(),
                "expected no background_highlight warning, got: {:?}",
                state.entries
            );
        }
    }
}
