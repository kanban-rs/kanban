#[cfg(not(any(feature = "json", feature = "sqlite")))]
compile_error!("kanban-mcp binary requires at least one backend feature: `json` or `sqlite`.");

use anyhow::Result;
use clap::Parser;
use kanban_core::CLI_VERSION_DISPLAY;
use kanban_mcp::McpServer;

#[derive(Parser)]
#[command(
    name = "kanban-mcp",
    version = CLI_VERSION_DISPLAY,
    about = "Model Context Protocol server for the kanban project management tool"
)]
struct Args {
    /// Path to the kanban data file (or set KANBAN_FILE env var)
    #[arg(value_name = "FILE", env = "KANBAN_FILE")]
    data_file: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let mut server = McpServer::with_defaults();
    if let Some(path) = args.data_file {
        server = server.with_data_file(path);
    }
    server.run().await
}
