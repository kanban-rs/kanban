use kanban_backend_http::HttpBackend;
use kanban_tui::keybindings::KeybindingAction;
use kanban_tui::App;
use std::sync::Arc;

fn app_on_unreachable_remote_backend() -> App {
    let mut app = App::test_default();
    let backend = Arc::new(HttpBackend::new("http://127.0.0.1:1").unwrap());
    app.ctx.replace_backend(backend);
    app
}

#[test]
fn test_undo_on_remote_backend_shows_decline_not_nothing_to_undo() {
    let mut app = app_on_unreachable_remote_backend();

    app.execute_action(&KeybindingAction::Undo);

    let banner = app
        .ui_state
        .banner
        .as_ref()
        .expect("undo over a remote backend must show an error banner");
    assert!(
        !banner.message.contains("Nothing to undo"),
        "must not claim nothing happened, got: {}",
        banner.message
    );
    assert!(
        banner.message.contains("HTTP backend"),
        "expected the decline message, got: {}",
        banner.message
    );
}
