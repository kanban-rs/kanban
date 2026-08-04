use clap::Parser;
use kanban_core::CLI_VERSION_DISPLAY;
use kanban_server::{app, state::AppState};
use kanban_service::config;

#[derive(Parser)]
#[command(
    name = "kanban-server",
    version = CLI_VERSION_DISPLAY,
    about = "HTTP API server for the kanban project management tool"
)]
struct Args {
    /// Path to kanban data file (or set KANBAN_FILE env var)
    #[arg(value_name = "FILE", env = "KANBAN_FILE")]
    file: Option<String>,

    /// Address to bind as host:port (or set KANBAN_ADDR). Falls back to the
    /// `server_addr` config value, then 127.0.0.1:0.
    #[arg(long, env = "KANBAN_ADDR")]
    addr: Option<String>,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    tracing_subscriber::fmt::init();

    let config = config::load();
    let locator = args
        .file
        .unwrap_or_else(|| config::resolve_storage_location(&config));
    let addr = args
        .addr
        .unwrap_or_else(|| config::resolve_server_addr(&config));

    if let Err(e) = run(&locator, config, &addr).await {
        eprintln!(
            "Error: failed to start kanban-server on '{addr}' with data file '{locator}': {e}"
        );
        std::process::exit(1);
    }
}

async fn run(
    locator: &str,
    config: kanban_service::AppConfig,
    addr: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut config = config;
    let mut stores = kanban_persistence::StoreRegistry::new();
    let mut backends = kanban_backend::KanbanBackendRegistry::new();
    stores.register(Box::new(kanban_persistence_sqlite::SqliteStoreFactory));
    backends.register(Box::new(kanban_persistence_sqlite::SqliteBackendFactory));
    stores.register(Box::new(kanban_persistence_json::JsonStoreFactory));
    backends.register(Box::new(kanban_persistence_json::JsonBackendFactory));
    let sm = kanban_service::StoreManager::new(stores, backends);
    sm.sync_backend_with_file(locator, &mut config);
    let backend = sm.make_backend(locator, &config).await?;
    let ctx = kanban_service::KanbanContext::open(backend, config).await?;
    let state = AppState::new(ctx);

    kanban_server::watch::watch_for_external_changes(state.clone(), locator).await?;

    let socket_addr: std::net::SocketAddr = addr.parse().map_err(|_| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("invalid bind address '{addr}': expected an IP literal host:port, e.g. 0.0.0.0:5175 (hostnames like 'localhost' are not resolved)"),
        )
    })?;
    let listener = tokio::net::TcpListener::bind(socket_addr).await?;
    tracing::info!(addr = %listener.local_addr()?, "kanban-server listening");
    axum::serve(listener, app::router(state)).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_addr_defaults_to_none_without_flag_or_env() {
        let args = Args::parse_from(["kanban-server"]);
        assert!(args.addr.is_none());
    }

    #[test]
    fn test_addr_long_flag_sets_value() {
        let args = Args::parse_from(["kanban-server", "--addr", "0.0.0.0:5175"]);
        assert_eq!(args.addr, Some("0.0.0.0:5175".into()));
    }

    #[test]
    fn test_file_and_addr_parse_together() {
        let args = Args::parse_from(["kanban-server", "board.json", "--addr", "127.0.0.1:9999"]);
        assert_eq!(args.file, Some("board.json".into()));
        assert_eq!(args.addr, Some("127.0.0.1:9999".into()));
    }
}
