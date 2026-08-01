//! Contract tests for `kanban_mcp::McpServer`'s builder surface.
//!
//! These drive the public plug-in API that third-party backend crates will
//! consume: build an `McpServer`, optionally register custom backends, and
//! confirm that the resulting registry can build stores for exactly the
//! factories that were registered.

use kanban_core::AppConfig;
use kanban_mcp::McpServer;
use kanban_persistence_json::{JsonBackendFactory, JsonStoreFactory};

#[test]
fn test_mcp_server_default_has_no_backends() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.json").to_string_lossy().to_string();
    let server = McpServer::default();
    match server.registry().create_store("json", &path) {
        Ok(_) => panic!("McpServer::default must not register any backends"),
        Err(err) => assert!(
            err.to_string().contains("json") || err.to_string().contains("Unsupported"),
            "expected unsupported-locator error, got: {err}"
        ),
    }
}

#[test]
fn test_mcp_server_with_defaults_creates_json_store() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.json").to_string_lossy().to_string();
    let server = McpServer::with_defaults();
    let store = server
        .registry()
        .create_store("json", &path)
        .expect("with_defaults must register the JSON backend");
    assert!(store.path().to_str().unwrap().ends_with(".json"));
}

#[test]
fn test_mcp_server_register_backend_adds_custom_factory() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.json").to_string_lossy().to_string();
    let server = McpServer::default()
        .register_backend(Box::new(JsonStoreFactory), Box::new(JsonBackendFactory));
    let store = server
        .registry()
        .create_store("json", &path)
        .expect("registered factory must be dispatchable");
    assert!(store.path().to_str().unwrap().ends_with(".json"));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_mcp_server_register_backend_registers_reachable_via_make_backend() {
    let dir = tempfile::tempdir().unwrap();
    let json_path = dir.path().join("test.json");

    // Build with only register_backend (no with_defaults()), so the only way
    // either registry is populated is through the public register_backend API.
    // McpContext::new calls StoreManager::make_backend directly, so build()
    // succeeding proves the KanbanBackendFactory half is actually reachable
    // through make_backend, not merely that a StoreFactory was registered.
    McpServer::default()
        .register_backend(Box::new(JsonStoreFactory), Box::new(JsonBackendFactory))
        .with_data_file(json_path.to_string_lossy().to_string())
        .build()
        .await
        .expect("build must succeed with a backend registered only via register_backend");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_mcp_server_with_config_build_uses_override() {
    let dir = tempfile::tempdir().unwrap();
    let json_path = dir.path().join("test.json");
    let config = AppConfig {
        storage_location: Some(json_path.to_string_lossy().to_string()),
        storage_backend: Some("json".into()),
        ..Default::default()
    };
    McpServer::with_defaults()
        .with_config(config)
        .build()
        .await
        .expect("build must succeed with a valid json config override");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_mcp_server_default_build_returns_no_backends_error() {
    match McpServer::default().build().await {
        Ok(_) => panic!("build with no backends must return Err"),
        Err(err) => {
            let msg = err.to_string();
            assert!(
                msg.contains("No storage backends") || msg.contains("register_backend"),
                "expected no-backends error, got: {msg}"
            );
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn test_mcp_server_build_no_data_file_uses_config_location() {
    let dir = tempfile::tempdir().unwrap();
    let json_path = dir.path().join("test.json");
    let config = AppConfig {
        storage_location: Some(json_path.to_string_lossy().to_string()),
        storage_backend: Some("json".into()),
        ..Default::default()
    };
    // No .with_data_file() — must fall through to config.effective_storage_location().
    McpServer::default()
        .register_backend(Box::new(JsonStoreFactory), Box::new(JsonBackendFactory))
        .with_config(config)
        .build()
        .await
        .expect("build must succeed when config provides storage_location");
}

#[test]
fn test_mcp_server_with_defaults_populates_both_registries() {
    let server = McpServer::with_defaults();
    assert!(!server.registry().is_empty(), "registry() must be populated");
    assert!(!server.backends().is_empty(), "backends() must be populated");
    let names = server.backends().names();
    assert_eq!(
        names,
        vec!["sqlite", "json"],
        "sqlite must be registered before json so magic-byte sniffing wins"
    );
}

#[test]
fn test_mcp_server_with_defaults_detects_json_backend() {
    // JSON is the only registry-backed backend; .json files must be detected.
    let server = McpServer::with_defaults();

    let dir = tempfile::tempdir().unwrap();
    let json_path = dir.path().join("board.json");
    std::fs::write(&json_path, b"{}").unwrap();

    let detected = server
        .registry()
        .detect_backend(json_path.to_str().unwrap())
        .expect("should detect json backend for .json file");
    assert_eq!(detected, "json");
}
