//! Builder entry point for the Kanban MCP server.
//!
//! Mirrors `kanban_cli::CliApp` in spirit: third-party backend crates
//! construct an `McpServer`, register their own `StoreFactory`, and call
//! `run` from their own `main`.

use crate::KanbanMcpServer;
use anyhow::{Context, Result};
use kanban_core::AppConfig;
use kanban_persistence::{StoreFactory, StoreRegistry};
use kanban_service::{validate_path, StoreManager};
use rmcp::transport::stdio;
use rmcp::ServiceExt;
use std::path::PathBuf;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

pub struct McpServer {
    registry: StoreRegistry,
    backends: kanban_backend::KanbanBackendRegistry,
    config: Option<AppConfig>,
    data_file: Option<String>,
}

impl Default for McpServer {
    /// Returns an empty `McpServer` with no registered backends. Callers
    /// must register at least one backend before `run` can produce a store.
    fn default() -> Self {
        Self {
            registry: StoreRegistry::new(),
            backends: kanban_backend::KanbanBackendRegistry::new(),
            config: None,
            data_file: None,
        }
    }
}

impl McpServer {
    /// Returns an `McpServer` pre-configured with both built-in backends.
    /// SQLite is registered first so content-sniffing prefers it; JSON is
    /// registered as the catch-all fallback.
    pub fn with_defaults() -> Self {
        let mut registry = kanban_persistence::StoreRegistry::new();
        let mut backends = kanban_backend::KanbanBackendRegistry::new();
        #[cfg(feature = "sqlite")]
        {
            registry.register(Box::new(kanban_persistence_sqlite::SqliteStoreFactory));
            backends.register(Box::new(kanban_persistence_sqlite::SqliteBackendFactory));
        }
        #[cfg(feature = "json")]
        {
            registry.register(Box::new(kanban_persistence_json::JsonStoreFactory));
            backends.register(Box::new(kanban_persistence_json::JsonBackendFactory));
        }
        Self {
            registry,
            backends,
            config: None,
            data_file: None,
        }
    }

    /// Registers an additional backend. `store_factory` builds the raw
    /// `PersistenceStore` (used by `make_store`/`make_store_with_config` for
    /// direct storage operations); `backend_factory` builds the
    /// `KanbanBackend` that `make_backend` dispatches to. A backend that
    /// only ever needs `make_backend` cannot skip `store_factory` — both are
    /// required together, so a factory registered through this method is
    /// never reachable through only one dispatch path and unreachable
    /// through the other. Order matters for content sniffing on both
    /// registries — factories registered earlier win when multiple match.
    ///
    /// # Example — third-party binary with a custom backend
    ///
    /// A crate that owns its own `main` can reuse the full MCP server while
    /// injecting a proprietary storage backend:
    ///
    /// ```no_run
    /// use kanban_mcp::McpServer;
    /// use kanban_backend::{KanbanBackend, KanbanBackendFactory};
    /// use kanban_core::AppConfig;
    /// use kanban_domain::KanbanResult;
    /// use kanban_persistence::{PersistenceError, PersistenceStore, StoreFactory};
    /// use std::sync::Arc;
    ///
    /// // A backend factory provided by a third-party crate.
    /// struct MyStoreFactory;
    /// impl StoreFactory for MyStoreFactory {
    ///     fn name(&self) -> &str { "my-backend" }
    ///     fn create(
    ///         &self,
    ///         locator: &str,
    ///     ) -> Result<Arc<dyn PersistenceStore + Send + Sync>, PersistenceError> {
    ///         unimplemented!()
    ///     }
    /// }
    ///
    /// struct MyBackendFactory;
    /// #[async_trait::async_trait]
    /// impl KanbanBackendFactory for MyBackendFactory {
    ///     fn name(&self) -> &str { "my-backend" }
    ///     async fn create(
    ///         &self,
    ///         locator: &str,
    ///         config: &AppConfig,
    ///     ) -> KanbanResult<Arc<dyn KanbanBackend>> {
    ///         unimplemented!()
    ///     }
    /// }
    ///
    /// #[tokio::main]
    /// async fn main() -> anyhow::Result<()> {
    ///     McpServer::with_defaults()
    ///         .register_backend(Box::new(MyStoreFactory), Box::new(MyBackendFactory))
    ///         .run()
    ///         .await
    /// }
    /// ```
    pub fn register_backend(
        mut self,
        store_factory: Box<dyn StoreFactory>,
        backend_factory: Box<dyn kanban_backend::KanbanBackendFactory>,
    ) -> Self {
        self.registry.register(store_factory);
        self.backends.register(backend_factory);
        self
    }

    /// Overrides the `AppConfig` that `run` would otherwise load from disk.
    pub fn with_config(mut self, config: AppConfig) -> Self {
        self.config = Some(config);
        self
    }

    /// Sets the data-file path the server should open. When omitted, the
    /// path is taken from `AppConfig::effective_storage_location`.
    pub fn with_data_file(mut self, path: impl Into<String>) -> Self {
        self.data_file = Some(path.into());
        self
    }

    /// Exposes the underlying registry for inspection and tests.
    pub fn registry(&self) -> &StoreRegistry {
        &self.registry
    }

    /// Exposes the underlying backend registry for inspection and tests.
    pub fn backends(&self) -> &kanban_backend::KanbanBackendRegistry {
        &self.backends
    }

    /// Consumes this builder and returns a ready-to-serve `KanbanMcpServer`.
    pub async fn build(self) -> Result<KanbanMcpServer> {
        let config = self.config.unwrap_or_else(kanban_service::config::load);
        let store_manager = StoreManager::new(self.registry, self.backends);
        if !store_manager.has_backends() {
            anyhow::bail!(
                "No storage backends registered. \
                 Use McpServer::with_defaults() or call register_backend() before build()."
            );
        }
        let data_file_path = match self.data_file {
            Some(path) => PathBuf::from(path),
            None => PathBuf::from(config.effective_storage_location()),
        };
        let validated = validate_path(&data_file_path)?;
        let data_file = validated.to_string_lossy().to_string();
        KanbanMcpServer::new(&store_manager, &data_file, config)
            .await
            .context("Failed to initialize KanbanMcpServer")
    }

    /// Initializes tracing, constructs the server, and serves it over stdio
    /// until the transport closes.
    pub async fn run(self) -> Result<()> {
        tracing_subscriber::registry()
            // "info" default: MCP server runs headlessly; startup/lifecycle events aid operators.
            .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
            .with(tracing_subscriber::fmt::layer().with_writer(std::io::stderr))
            .try_init()
            .ok();

        let server = self.build().await?;
        tracing::info!("Starting Kanban MCP server");
        let service = server.serve(stdio()).await?;
        tracing::info!("Kanban MCP server started successfully");
        service.waiting().await?;
        Ok(())
    }
}
