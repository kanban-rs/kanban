//! Contract tests for `kanban_cli::CliApp`'s builder surface.
//!
//! These drive the public plug-in API that third-party backend crates will
//! consume: build a `CliApp`, optionally register custom backends, and
//! confirm that the resulting `StoreManager` can build stores for exactly
//! the factories that were registered.

use kanban_cli::CliApp;
use kanban_persistence_json::{JsonBackendFactory, JsonStoreFactory};

#[test]
fn test_cli_app_default_has_no_backends() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.json").to_string_lossy().to_string();
    let app = CliApp::default();
    match app.registry().create_store("json", &path) {
        Ok(_) => panic!("CliApp::default must not register any backends"),
        Err(err) => assert!(
            err.to_string().contains("json") || err.to_string().contains("Unsupported"),
            "expected unsupported-locator error, got: {err}"
        ),
    }
}

#[test]
fn test_cli_app_with_defaults_creates_json_store() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.json").to_string_lossy().to_string();
    let app = CliApp::with_defaults();
    let store = app
        .registry()
        .create_store("json", &path)
        .expect("with_defaults must register the JSON backend");
    assert!(store.path().to_str().unwrap().ends_with(".json"));
}

#[test]
fn test_cli_app_register_backend_adds_custom_factory() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.json").to_string_lossy().to_string();
    let app = CliApp::default()
        .register_backend(Box::new(JsonStoreFactory), Box::new(JsonBackendFactory));
    let store = app
        .registry()
        .create_store("json", &path)
        .expect("registered factory must be dispatchable");
    assert!(store.path().to_str().unwrap().ends_with(".json"));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_cli_app_register_backend_reachable_via_make_backend() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("board.json").to_string_lossy().to_string();

    // No subcommand: creates the empty data file via make_store_with_config
    // (the StoreFactory half of register_backend) if it doesn't exist yet.
    CliApp::default()
        .register_backend(Box::new(JsonStoreFactory), Box::new(JsonBackendFactory))
        .run_with_args(["kanban", &path])
        .await
        .expect("init run must succeed with a registered backend");

    // `board list` against the now-existing file goes through
    // CliContext::load -> StoreManager::make_backend, which only the
    // KanbanBackendFactory half of register_backend can satisfy.
    CliApp::default()
        .register_backend(Box::new(JsonStoreFactory), Box::new(JsonBackendFactory))
        .run_with_args(["kanban", &path, "board", "list"])
        .await
        .expect(
            "a backend registered only through register_backend must be reachable via make_backend",
        );
}

#[test]
fn test_cli_app_with_config_stores_override() {
    use kanban_core::AppConfig;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.json").to_string_lossy().to_string();
    // with_config must not disturb the registry built by with_defaults.
    let app = CliApp::with_defaults().with_config(AppConfig {
        storage_backend: Some("sqlite".into()),
        ..Default::default()
    });
    let store = app
        .registry()
        .create_store("json", &path)
        .expect("with_defaults backends must survive a with_config call");
    assert!(store.path().to_str().unwrap().ends_with(".json"));
}

#[test]
fn test_cli_app_with_defaults_populates_both_registries() {
    let app = CliApp::with_defaults();
    assert!(!app.registry().is_empty(), "registry() must be populated");
    assert!(!app.backends().is_empty(), "backends() must be populated");
    let names = app.backends().names();
    assert_eq!(
        names,
        vec!["sqlite", "json"],
        "sqlite must be registered before json so magic-byte sniffing wins"
    );
}

#[test]
fn test_cli_app_with_defaults_detects_json_backend() {
    // JSON is the only registry-backed backend; .json files must be detected.
    let app = CliApp::with_defaults();

    let dir = tempfile::tempdir().unwrap();
    let json_path = dir.path().join("board.json");
    std::fs::write(&json_path, b"{}").unwrap();

    let detected = app
        .registry()
        .detect_backend(json_path.to_str().unwrap())
        .expect("should detect json backend for .json file");
    assert_eq!(detected, "json");
}
