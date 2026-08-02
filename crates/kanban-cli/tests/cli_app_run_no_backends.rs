use kanban_cli::CliApp;
use kanban_core::AppConfig;
use kanban_persistence_json::{JsonBackendFactory, JsonStoreFactory};

#[tokio::test]
async fn test_cli_app_default_run_with_board_subcommand_returns_no_backends_error() {
    let err = CliApp::default()
        .run_with_args(["kanban", "board", "list"])
        .await
        .unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("No storage backends") || msg.contains("register_backend"),
        "expected no-backends error, got: {msg}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_cli_app_with_backends_does_not_hit_no_backends_guard() {
    let dir = tempfile::tempdir().unwrap();
    let json_path = dir.path().join("kanban.json").to_string_lossy().to_string();
    let config = AppConfig {
        storage_location: Some(json_path.clone()),
        storage_backend: Some("json".into()),
        ..Default::default()
    };
    let result = CliApp::default()
        .register_backend(Box::new(JsonStoreFactory), Box::new(JsonBackendFactory))
        .with_config(config)
        .run_with_args(["kanban", "board", "list"])
        .await;
    // The file doesn't exist yet — it may fail, but must NOT hit the no-backends guard.
    if let Err(err) = result {
        let msg = err.to_string();
        assert!(
            !msg.contains("No storage backends"),
            "hit no-backends guard unexpectedly: {msg}"
        );
    }
}
