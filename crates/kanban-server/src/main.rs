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
    let sm = kanban_server::stores::registered_store_manager();
    sm.sync_backend_with_file(locator, &mut config);
    let backend = sm.make_backend(locator, &config).await?;
    let ctx = kanban_service::KanbanContext::open(backend, config).await?;
    let is_sqlite = sm.is_sqlite(locator);
    let state = AppState::new(ctx);

    kanban_server::watch::watch_for_external_changes(state.clone(), locator, is_sqlite).await?;

    let socket_addr: std::net::SocketAddr = addr.parse().map_err(|_| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("invalid bind address '{addr}': expected an IP literal host:port, e.g. 0.0.0.0:5175 (hostnames like 'localhost' are not resolved)"),
        )
    })?;
    let listener = tokio::net::TcpListener::bind(socket_addr).await?;
    let shutdown_rx = install_shutdown_watch()?;
    tracing::info!(addr = %listener.local_addr()?, "kanban-server listening");

    let mut graceful_rx = shutdown_rx.clone();
    let mut drain_rx = shutdown_rx;
    let serve = axum::serve(
        listener,
        app::router_with(state, kanban_server::layers::LayerConfig::from_env()),
    )
    .with_graceful_shutdown(async move {
        let _ = graceful_rx.changed().await;
    });
    tokio::select! {
        r = serve => r?,
        _ = async {
            let _ = drain_rx.changed().await;
            tokio::time::sleep(drain_grace()).await;
        } => {
            tracing::warn!("shutdown drain window elapsed; closing remaining connections");
        }
    }
    Ok(())
}

const DEFAULT_SHUTDOWN_GRACE_SECS: u64 = 10;

fn parse_grace(raw: Option<String>) -> std::time::Duration {
    raw.and_then(|v| v.parse().ok())
        .map(std::time::Duration::from_secs)
        .unwrap_or_else(|| std::time::Duration::from_secs(DEFAULT_SHUTDOWN_GRACE_SECS))
}

fn drain_grace() -> std::time::Duration {
    parse_grace(std::env::var("KANBAN_SHUTDOWN_GRACE_SECS").ok())
}

/// Installs the shutdown signal handlers and returns a receiver that flips
/// to `true` once one fires.
#[cfg(unix)]
fn install_shutdown_watch() -> std::io::Result<tokio::sync::watch::Receiver<bool>> {
    use tokio::signal::unix::{signal, SignalKind};

    let mut term = signal(SignalKind::terminate())?;
    let mut int = signal(SignalKind::interrupt())?;
    let (tx, rx) = tokio::sync::watch::channel(false);
    tokio::spawn(async move {
        tokio::select! {
            _ = term.recv() => {}
            _ = int.recv() => {}
        }
        let _ = tx.send(true);
    });
    Ok(rx)
}

/// Installs the shutdown signal handlers and returns a receiver that flips
/// to `true` once one fires.
#[cfg(not(unix))]
fn install_shutdown_watch() -> std::io::Result<tokio::sync::watch::Receiver<bool>> {
    let (tx, rx) = tokio::sync::watch::channel(false);
    tokio::spawn(async move {
        let _ = tokio::signal::ctrl_c().await;
        let _ = tx.send(true);
    });
    Ok(rx)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_grace_defaults_to_ten_seconds_when_unset() {
        assert_eq!(parse_grace(None), Duration::from_secs(10));
    }

    #[test]
    fn test_grace_parses_env_value_as_seconds() {
        assert_eq!(parse_grace(Some("3".into())), Duration::from_secs(3));
    }

    #[test]
    fn test_grace_falls_back_to_default_on_unparseable_value() {
        assert_eq!(parse_grace(Some("soon".into())), Duration::from_secs(10));
    }

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
