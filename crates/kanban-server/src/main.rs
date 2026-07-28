use clap::Parser;
use kanban_core::CLI_VERSION_DISPLAY;
use kanban_server::{app, state::AppState};
use kanban_service::AppConfig;

const DEFAULT_LOCATOR: &str = "kanban.json";

#[derive(Parser)]
#[command(
    name = "kanban-server",
    version = CLI_VERSION_DISPLAY,
    about = "HTTP API server for the kanban project management tool"
)]
struct Args {}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _args = Args::parse();
    tracing_subscriber::fmt::init();

    let locator = std::env::var("KANBAN_FILE").unwrap_or_else(|_| DEFAULT_LOCATOR.to_string());
    let ctx = kanban_service::open_context(&locator, AppConfig::default()).await?;
    let state = AppState::new(ctx);

    kanban_server::watch::watch_for_external_changes(state.clone(), &locator).await?;

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    tracing::info!(addr = %listener.local_addr()?, "kanban-server listening");
    axum::serve(listener, app::router(state)).await?;
    Ok(())
}
