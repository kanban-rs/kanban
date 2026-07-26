use kanban_server::{app, state::AppState};
use kanban_service::AppConfig;

const DEFAULT_LOCATOR: &str = "kanban.json";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let ctx = kanban_service::open_context(DEFAULT_LOCATOR, AppConfig::default()).await?;
    let state = AppState::new(ctx);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    tracing::info!(addr = %listener.local_addr()?, "kanban-server listening");
    axum::serve(listener, app::router(state)).await?;
    Ok(())
}
