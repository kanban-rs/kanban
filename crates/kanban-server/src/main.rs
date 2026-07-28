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
struct Args {}

#[tokio::main]
async fn main() {
    let _args = Args::parse();
    tracing_subscriber::fmt::init();

    let config = config::load();
    let locator =
        std::env::var("KANBAN_FILE").unwrap_or_else(|_| config::resolve_storage_location(&config));

    if let Err(e) = run(&locator, config).await {
        eprintln!("Error: failed to start kanban-server with data file '{locator}': {e}");
        std::process::exit(1);
    }
}

async fn run(
    locator: &str,
    config: kanban_service::AppConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    let ctx = kanban_service::open_context(locator, config).await?;
    let state = AppState::new(ctx);

    kanban_server::watch::watch_for_external_changes(state.clone(), locator).await?;

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    tracing::info!(addr = %listener.local_addr()?, "kanban-server listening");
    axum::serve(listener, app::router(state)).await?;
    Ok(())
}
