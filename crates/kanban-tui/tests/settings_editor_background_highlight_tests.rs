use kanban_tui::app::focus::Focus;
use kanban_tui::components::BannerVariant;
use kanban_tui::App;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::style::Color;
use ratatui::Terminal;
use std::path::Path;

static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

struct KanbanConfigEnv(Option<std::ffi::OsString>);

impl KanbanConfigEnv {
    fn set(path: &Path) -> Self {
        let prev = std::env::var_os("KANBAN_CONFIG");
        std::env::set_var("KANBAN_CONFIG", path);
        Self(prev)
    }
}

impl Drop for KanbanConfigEnv {
    fn drop(&mut self) {
        match self.0.take() {
            Some(v) => std::env::set_var("KANBAN_CONFIG", v),
            None => std::env::remove_var("KANBAN_CONFIG"),
        }
    }
}

fn settings_app(path: &Path, file_body: Option<&str>, editing_format: &str) -> App {
    if let Some(body) = file_body {
        std::fs::write(path, body).unwrap();
    }
    let mut app = App::test_default();
    let mut cfg = kanban_service::config::load_from(path);
    cfg.configuration_location = Some(path.display().to_string());
    cfg.editing_format = Some(editing_format.to_string());
    app.app_config = cfg;
    app.focus.active = Focus::Boards;
    app.handle_open_settings();
    app
}

fn edit(app: &mut App, from: &str, to: &str) {
    let (format, sent) = app.config_editor_buffer();
    assert!(sent.contains(from), "editor buffer lacks {from:?}:\n{sent}");
    app.finish_config_edit(&format, &sent, Ok(Some(sent.replacen(from, to, 1))));
}

fn render(app: &mut App) -> Buffer {
    app.clear_banner();
    let backend = TestBackend::new(120, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| kanban_tui::ui::render(app, frame))
        .unwrap();
    terminal.backend().buffer().clone()
}

fn find(buf: &Buffer, needle: &str) -> (u16, u16) {
    let chars: Vec<char> = needle.chars().collect();
    for y in 0..buf.area.height {
        for x in 0..buf.area.width {
            let mut matched = true;
            for (i, ch) in chars.iter().enumerate() {
                let expected = ch.to_string();
                match buf.cell((x + i as u16, y)) {
                    Some(cell) if cell.symbol() == expected => {}
                    _ => {
                        matched = false;
                        break;
                    }
                }
            }
            if matched {
                return (x, y);
            }
        }
    }
    panic!("needle {needle:?} not found in buffer");
}

fn rgb() -> Color {
    Color::Rgb(0x2E, 0x34, 0x40)
}

#[test]
fn test_editor_buffer_lists_background_highlight_as_the_default_hex_when_unset() {
    let _env = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let dir = tempfile::tempdir().unwrap();

    let json_path = dir.path().join("config.json");
    let app = settings_app(&json_path, None, "json");
    let (_, sent) = app.config_editor_buffer();
    assert!(
        sent.contains("\"background_highlight\": \"#0000FF\""),
        "buffer:\n{sent}"
    );

    let toml_path = dir.path().join("config.toml");
    let app = settings_app(&toml_path, None, "toml");
    let (_, sent) = app.config_editor_buffer();
    assert!(
        sent.contains("background_highlight = \"#0000FF\""),
        "buffer:\n{sent}"
    );
}

#[test]
fn test_editor_buffer_shows_the_configured_highlight_as_uppercase_hex() {
    let _env = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    let app = settings_app(&path, Some("background_highlight = \"#2e3440\"\n"), "json");

    let (_, sent) = app.config_editor_buffer();
    assert!(
        sent.contains("\"background_highlight\": \"#2E3440\""),
        "buffer:\n{sent}"
    );
}

#[test]
fn test_editor_edit_sets_background_highlight_saves_it_and_repaints_live() {
    let _env = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    let mut app = settings_app(&path, Some("default_card_prefix = \"feat\"\n"), "json");

    edit(
        &mut app,
        "\"background_highlight\": \"#0000FF\"",
        "\"background_highlight\": \"#2e3440\"",
    );

    assert!(
        !matches!(
            &app.ui_state.banner,
            Some(b) if b.variant == BannerVariant::Error
        ),
        "banner: {:?}",
        app.ui_state.banner
    );
    let saved = std::fs::read_to_string(&path).unwrap();
    assert!(saved.contains("background_highlight = \"#2E3440\""));
    assert!(saved.contains("default_card_prefix = \"feat\""));
    assert_eq!(
        app.app_config.background_highlight.as_deref(),
        Some("#2E3440")
    );
    assert_eq!(app.highlight_bg(), rgb());

    let buf = render(&mut app);
    let (x, y) = find(&buf, "Configuration Format: ");
    assert_eq!(buf.cell((x, y)).unwrap().bg, rgb());

    let (_, highlight_y) = find(&buf, "Background Highlight: #2E3440");
    let (sample_x, _) = find(&buf, " Sample ");
    for i in 0..(" Sample ".len() as u16) {
        assert_eq!(
            buf.cell((sample_x + i, highlight_y)).unwrap().bg,
            rgb(),
            "sample cell {i} on the Background Highlight row"
        );
    }
}

#[test]
fn test_editor_edit_in_toml_sets_background_highlight() {
    let _env = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    let mut app = settings_app(&path, None, "toml");

    edit(
        &mut app,
        "background_highlight = \"#0000FF\"",
        "background_highlight = \"#2E3440\"",
    );

    let saved = std::fs::read_to_string(&path).unwrap();
    assert!(saved.contains("background_highlight = \"#2E3440\""));
    assert_eq!(app.highlight_bg(), rgb());
}

#[test]
fn test_editor_edit_saving_the_default_hex_keeps_the_highlight_to_default() {
    let _env = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    let file_body = "background_highlight = \"#2E3440\"\ndefault_card_prefix = \"feat\"\n";
    let mut app = settings_app(&path, Some(file_body), "json");

    edit(
        &mut app,
        "\"background_highlight\": \"#2E3440\"",
        "\"background_highlight\": \"#0000FF\"",
    );

    assert!(
        !matches!(
            &app.ui_state.banner,
            Some(b) if b.variant == BannerVariant::Error
        ),
        "banner: {:?}",
        app.ui_state.banner
    );
    let saved = std::fs::read_to_string(&path).unwrap();
    assert!(!saved.contains("background_highlight"), "saved:\n{saved}");
    assert!(saved.contains("default_card_prefix = \"feat\""));
    assert_eq!(app.highlight_bg(), Color::Blue);

    let buf = render(&mut app);
    find(&buf, "Background Highlight: default (#0000FF)");
    let (x, y) = find(&buf, "Configuration Format: ");
    assert_eq!(buf.cell((x, y)).unwrap().bg, Color::Blue);
}

#[test]
fn test_editor_edit_with_an_invalid_hex_is_rejected_and_nothing_changes() {
    let _env = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    let file_body = "background_highlight = \"#2E3440\"\ndefault_card_prefix = \"feat\"\n";
    let mut app = settings_app(&path, Some(file_body), "json");
    let before = std::fs::read(&path).unwrap();

    let (format, sent) = app.config_editor_buffer();
    assert!(
        sent.contains("\"background_highlight\": \"#2E3440\""),
        "buffer:\n{sent}"
    );
    let edited = sent
        .replacen("\"#2E3440\"", "\"#2E3\"", 1)
        .replacen("\"feat\"", "\"fix\"", 1);
    app.finish_config_edit(&format, &sent, Ok(Some(edited)));

    let banner = app
        .ui_state
        .banner
        .clone()
        .expect("expected an error banner");
    assert_eq!(banner.variant, BannerVariant::Error);
    assert!(
        banner.message.contains("background_highlight"),
        "message: {}",
        banner.message
    );
    assert!(
        banner.message.contains("#RRGGBB"),
        "message: {}",
        banner.message
    );
    let after = std::fs::read(&path).unwrap();
    assert_eq!(
        before, after,
        "file must be byte-identical after a rejected edit"
    );
    assert_eq!(app.app_config.default_card_prefix.as_deref(), Some("feat"));
    assert_eq!(
        app.app_config.background_highlight.as_deref(),
        Some("#2E3440")
    );

    let buf = render(&mut app);
    let (x, y) = find(&buf, "Configuration Format: ");
    assert_eq!(buf.cell((x, y)).unwrap().bg, rgb());
}

#[test]
fn test_editor_edit_emptying_the_value_returns_the_highlight_to_default() {
    let _env = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    let file_body = "background_highlight = \"#2E3440\"\ndefault_card_prefix = \"feat\"\n";
    let mut app = settings_app(&path, Some(file_body), "json");

    edit(
        &mut app,
        "\"background_highlight\": \"#2E3440\"",
        "\"background_highlight\": \"\"",
    );

    let saved = std::fs::read_to_string(&path).unwrap();
    assert!(!saved.contains("background_highlight"));
    assert!(saved.contains("default_card_prefix = \"feat\""));
    assert_eq!(app.highlight_bg(), Color::Blue);

    let buf = render(&mut app);
    find(&buf, "Background Highlight: default (#0000FF)");
    let (x, y) = find(&buf, "Configuration Format: ");
    assert_eq!(buf.cell((x, y)).unwrap().bg, Color::Blue);
}

#[test]
fn test_editor_edit_deleting_the_key_returns_the_highlight_to_default() {
    let _env = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    let file_body = "background_highlight = \"#2E3440\"\ndefault_card_prefix = \"feat\"\n";
    let mut app = settings_app(&path, Some(file_body), "json");

    edit(&mut app, "\"background_highlight\": \"#2E3440\",\n", "");

    let saved = std::fs::read_to_string(&path).unwrap();
    assert!(!saved.contains("background_highlight"));
    assert!(saved.contains("default_card_prefix = \"feat\""));
    assert_eq!(app.highlight_bg(), Color::Blue);

    let buf = render(&mut app);
    let (x, y) = find(&buf, "Configuration Format: ");
    assert_eq!(buf.cell((x, y)).unwrap().bg, Color::Blue);
}

#[test]
fn test_unrelated_edit_on_a_legacy_file_writes_no_background_highlight() {
    let _env = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    let mut app = settings_app(&path, Some("default_card_prefix = \"feat\"\n"), "toml");

    edit(
        &mut app,
        "default_card_prefix = \"feat\"",
        "default_card_prefix = \"fix\"",
    );

    let saved = std::fs::read_to_string(&path).unwrap();
    assert!(saved.contains("default_card_prefix = \"fix\""));
    assert!(!saved.contains("background_highlight"), "saved:\n{saved}");
}

#[test]
fn test_unchanged_editor_buffer_keeps_an_invalid_file_value() {
    let _env = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    let file_body = "background_highlight = \"blue\"\ndefault_card_prefix = \"feat\"\n";
    let mut app = settings_app(&path, Some(file_body), "toml");
    let before = std::fs::read(&path).unwrap();

    let (format, sent) = app.config_editor_buffer();
    app.finish_config_edit(&format, &sent, Ok(Some(sent.clone())));

    let after = std::fs::read(&path).unwrap();
    assert_eq!(before, after);
    assert!(app.ui_state.banner.is_none());
}

#[test]
fn test_unrelated_edit_drops_an_invalid_file_value() {
    let _env = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    let file_body = "background_highlight = \"blue\"\ndefault_card_prefix = \"feat\"\n";
    let mut app = settings_app(&path, Some(file_body), "json");

    let (_, sent) = app.config_editor_buffer();
    assert!(
        sent.contains("\"background_highlight\": \"#0000FF\""),
        "buffer:\n{sent}"
    );

    edit(
        &mut app,
        "\"default_card_prefix\": \"feat\"",
        "\"default_card_prefix\": \"fix\"",
    );

    assert!(
        !matches!(
            &app.ui_state.banner,
            Some(b) if b.variant == BannerVariant::Error
        ),
        "banner: {:?}",
        app.ui_state.banner
    );
    let saved = std::fs::read_to_string(&path).unwrap();
    assert!(saved.contains("default_card_prefix = \"fix\""));
    assert!(!saved.contains("background_highlight"), "saved:\n{saved}");
}

#[test]
fn test_editor_edit_keeps_a_config_file_that_only_holds_background_highlight() {
    let _env = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    let _cfg_env = KanbanConfigEnv::set(&path);
    let file_body = "background_highlight = \"#2E3440\"\ndefault_card_prefix = \"feat\"\n";
    std::fs::write(&path, file_body).unwrap();

    let mut app = App::test_default();
    let mut cfg = kanban_service::config::load_from(&path);
    cfg.configuration_location = None;
    cfg.editing_format = Some("json".to_string());
    app.app_config = cfg;
    app.focus.active = Focus::Boards;
    app.handle_open_settings();

    edit(
        &mut app,
        "\"default_card_prefix\": \"feat\"",
        "\"default_card_prefix\": \"task\"",
    );

    assert!(path.exists());
    let saved = std::fs::read_to_string(&path).unwrap();
    assert!(saved.contains("background_highlight = \"#2E3440\""));
    assert!(!saved.contains("default_card_prefix"));
    assert_eq!(app.highlight_bg(), rgb());
}

#[test]
fn test_config_editor_buffer_in_cli_override_mode_comments_out_storage_fields() {
    let _env = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    let mut app = settings_app(&path, None, "json");
    app.cli_file_override = true;
    app.has_data_file = true;
    app.config_storage_backend = "sqlite".to_string();

    let (_, sent) = app.config_editor_buffer();

    assert!(
        sent.lines().any(|line| {
            let t = line.trim_start();
            t.starts_with("//") && t.contains("\"storage_backend\"")
        }),
        "buffer lacks a commented storage_backend line:\n{sent}"
    );
    assert!(
        !sent
            .lines()
            .any(|line| line.trim_start().starts_with("\"storage_backend\"")),
        "buffer still has an uncommented storage_backend line:\n{sent}"
    );
}

#[test]
fn test_finish_config_edit_with_an_editor_error_shows_failed_to_edit_banner() {
    let _env = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    let mut app = settings_app(&path, Some("default_card_prefix = \"feat\"\n"), "toml");
    let before_config = app.app_config.clone();

    let (format, sent) = app.config_editor_buffer();
    app.finish_config_edit(&format, &sent, Err(std::io::Error::other("boom")));

    let banner = app
        .ui_state
        .banner
        .clone()
        .expect("expected an error banner");
    assert_eq!(banner.variant, BannerVariant::Error);
    assert_eq!(banner.message, "Failed to edit config: boom");
    assert_eq!(
        app.app_config.default_card_prefix,
        before_config.default_card_prefix
    );
    assert_eq!(
        app.app_config.background_highlight,
        before_config.background_highlight
    );
}

#[test]
fn test_finish_config_edit_with_a_cancelled_editor_changes_nothing() {
    let _env = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    let mut app = settings_app(&path, Some("default_card_prefix = \"feat\"\n"), "toml");
    let before_config = app.app_config.clone();
    let before_file = std::fs::read(&path).unwrap();

    let (format, sent) = app.config_editor_buffer();
    app.finish_config_edit(&format, &sent, Ok(None));

    assert!(app.ui_state.banner.is_none());
    assert_eq!(
        app.app_config.default_card_prefix,
        before_config.default_card_prefix
    );
    assert_eq!(
        app.app_config.background_highlight,
        before_config.background_highlight
    );
    let after_file = std::fs::read(&path).unwrap();
    assert_eq!(before_file, after_file);
}

#[test]
fn test_finish_config_edit_with_whitespace_only_change_skips_apply() {
    let _env = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    let mut app = settings_app(&path, Some("default_card_prefix = \"feat\"\n"), "toml");
    let before_file = std::fs::read(&path).unwrap();

    let (format, sent) = app.config_editor_buffer();
    let edited = format!("{}\n", sent);
    app.finish_config_edit(&format, &sent, Ok(Some(edited)));

    assert!(app.ui_state.banner.is_none());
    let after_file = std::fs::read(&path).unwrap();
    assert_eq!(
        before_file, after_file,
        "a whitespace-only change must not write the config file"
    );
}

#[test]
fn test_editor_edit_still_deletes_a_config_file_left_with_only_defaults() {
    let _env = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    let _cfg_env = KanbanConfigEnv::set(&path);
    std::fs::write(&path, "default_card_prefix = \"feat\"\n").unwrap();

    let mut app = App::test_default();
    let mut cfg = kanban_service::config::load_from(&path);
    cfg.configuration_location = None;
    cfg.editing_format = Some("json".to_string());
    app.app_config = cfg;
    app.focus.active = Focus::Boards;
    app.handle_open_settings();

    edit(
        &mut app,
        "\"default_card_prefix\": \"feat\"",
        "\"default_card_prefix\": \"task\"",
    );

    assert!(!path.exists());
}
