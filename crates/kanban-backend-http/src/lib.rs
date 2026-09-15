mod backend_factory;
mod command_store;
mod conversions;
mod conversions_out;
mod data_store;
mod events;
mod http;
mod http_mutation;
mod remote_writes;

pub use backend_factory::HttpBackendFactory;

pub struct HttpBackend {
    base_url: String,
    client: reqwest::Client,
    runtime: Option<tokio::runtime::Runtime>,
    instance_id: uuid::Uuid,
}

/// Dropping a `tokio::runtime::Runtime` blocks the current thread, and
/// blocking is forbidden on a thread already inside another runtime -- which
/// is where every application drops this backend.
impl Drop for HttpBackend {
    fn drop(&mut self) {
        if let Some(runtime) = self.runtime.take() {
            runtime.shutdown_background();
        }
    }
}

#[async_trait::async_trait]
impl kanban_backend::KanbanBackend for HttpBackend {
    fn as_data_store(&self) -> &dyn kanban_domain::DataStore {
        self
    }

    fn instance_id(&self) -> uuid::Uuid {
        self.instance_id
    }

    fn as_any(&self) -> Option<&dyn std::any::Any> {
        Some(self)
    }

    async fn probe(&self) -> kanban_domain::KanbanResult<()> {
        let url = format!("{}/health", self.base_url());
        let resp = self.client().get(&url).send().await.map_err(|e| {
            kanban_domain::KanbanError::Transport(format!("health probe of '{url}' failed: {e}"))
        })?;
        if !resp.status().is_success() {
            return Err(kanban_domain::KanbanError::Transport(format!(
                "health probe of '{url}' returned HTTP {}",
                resp.status()
            )));
        }
        Ok(())
    }

    fn remote_writes(&self) -> Option<&dyn kanban_backend::RemoteWrites> {
        Some(self)
    }

    /// Declines without running the closure. The remote server owns the state,
    /// so there is nothing local to roll back and no way to make the batch
    /// atomic from this side.
    fn with_transaction(
        &self,
        _f: kanban_backend::TransactionFn<'_>,
    ) -> kanban_domain::KanbanResult<()> {
        Err(kanban_domain::KanbanError::unsupported("with_transaction"))
    }
}

impl HttpBackend {
    pub fn new(base_url: &str) -> kanban_domain::KanbanResult<Self> {
        if !matches!(kanban_core::scheme_of(base_url), Some("http" | "https")) {
            return Err(kanban_domain::KanbanError::validation(format!(
                "HttpBackend requires an http:// or https:// URL, got '{base_url}'"
            )));
        }
        let base_url = base_url.trim_end_matches('/').to_string();
        let client = reqwest::Client::builder().build().map_err(|e| {
            kanban_domain::KanbanError::Internal(format!("failed to build http client: {e}"))
        })?;
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .map_err(|e| {
                kanban_domain::KanbanError::Internal(format!(
                    "failed to build http_backend runtime: {e}"
                ))
            })?;
        Ok(Self {
            base_url,
            client,
            runtime: Some(runtime),
            instance_id: uuid::Uuid::new_v4(),
        })
    }

    /// Bridge a synchronous DataStore/CommandStore call onto the dedicated
    /// runtime -- never the caller's ambient one. With no ambient runtime on
    /// the calling thread, this blocks directly. Inside a multi-thread
    /// ambient runtime, it enters via `tokio::task::block_in_place` so the
    /// dedicated runtime can be driven without nesting. A current_thread
    /// ambient runtime is rejected: it has no worker to hand off to, so
    /// `block_in_place` deadlocks or panics.
    pub(crate) fn block_on<F: std::future::Future>(&self, fut: F) -> F::Output {
        let runtime = self
            .runtime
            .as_ref()
            .expect("the runtime is taken only while dropping");
        match tokio::runtime::Handle::try_current() {
            Ok(handle) => {
                debug_assert!(
                    handle.runtime_flavor() == tokio::runtime::RuntimeFlavor::MultiThread,
                    "HttpBackend requires a multi-threaded Tokio runtime (e.g. #[tokio::main]). \
                     The current_thread runtime is not supported because synchronous DataStore \
                     methods need to block on async HTTP I/O."
                );
                tokio::task::block_in_place(|| runtime.block_on(fut))
            }
            Err(_) => runtime.block_on(fut),
        }
    }

    pub(crate) fn base_url(&self) -> &str {
        &self.base_url
    }

    pub(crate) fn client(&self) -> &reqwest::Client {
        &self.client
    }

    pub fn subscribe(&self) -> tokio::sync::mpsc::Receiver<kanban_api::ChangeEventFrame> {
        let (tx, rx) = tokio::sync::mpsc::channel(32);
        let client = self.client().clone();
        let url = format!("{}/v1/events", self.base_url());
        let runtime = self
            .runtime
            .as_ref()
            .expect("the runtime is taken only while dropping");

        runtime.handle().spawn(async move {
            let mut backoff = std::time::Duration::from_secs(1);
            loop {
                if let Ok(mut resp) = client.get(&url).send().await {
                    if resp.status().is_success() {
                        backoff = std::time::Duration::from_secs(1);
                        let synthetic = kanban_api::ChangeEventFrame::now(
                            uuid::Uuid::nil(),
                            uuid::Uuid::new_v4(),
                            kanban_core::ClientId::nil(),
                        );
                        if tx.send(synthetic).await.is_err() {
                            return;
                        }
                        let mut parser = events::SseParser::default();
                        while let Ok(Some(chunk)) = resp.chunk().await {
                            for frame in parser.push(&chunk) {
                                if tx.send(frame).await.is_err() {
                                    return;
                                }
                            }
                        }
                    } else {
                        tracing::warn!(
                            "SSE subscription to {url} rejected with status {}",
                            resp.status()
                        );
                    }
                } else {
                    tracing::warn!("SSE subscription to {url} failed to connect");
                }
                tokio::time::sleep(backoff).await;
                backoff = events::next_backoff(backoff);
            }
        });

        rx
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kanban_backend::KanbanBackend;
    use kanban_domain::DataStore;

    #[test]
    fn test_http_backend_new_normalizes_trailing_slash() -> kanban_domain::KanbanResult<()> {
        let backend = HttpBackend::new("http://example.com/")?;
        assert_eq!(backend.base_url(), "http://example.com");
        Ok(())
    }

    #[test]
    fn test_http_backend_new_no_trailing_slash() -> kanban_domain::KanbanResult<()> {
        let backend = HttpBackend::new("http://example.com")?;
        assert_eq!(backend.base_url(), "http://example.com");
        Ok(())
    }

    #[test]
    fn test_http_backend_block_on_bridges_sync_call_without_ambient_runtime(
    ) -> kanban_domain::KanbanResult<()> {
        let backend = HttpBackend::new("http://example.com")?;
        let result = backend.block_on(async { 1 + 1 });
        assert_eq!(result, 2);
        Ok(())
    }

    #[tokio::test(flavor = "current_thread")]
    #[should_panic(expected = "HttpBackend requires a multi-threaded Tokio runtime")]
    async fn test_block_on_inside_current_thread_runtime_panics_with_flavor_message() {
        let backend = HttpBackend::new("http://example.com").unwrap();
        let _ = backend.block_on(async { 1 + 1 });
    }

    #[test]
    fn test_http_backend_new_mints_nonnil_instance_id() -> kanban_domain::KanbanResult<()> {
        let backend = HttpBackend::new("http://example.com")?;
        assert_ne!(backend.instance_id(), uuid::Uuid::nil());
        Ok(())
    }

    #[test]
    fn test_http_backend_implements_kanban_backend_object_safe() -> kanban_domain::KanbanResult<()>
    {
        let backend = HttpBackend::new("http://example.com")?;
        let _: &dyn kanban_backend::KanbanBackend = &backend;
        Ok(())
    }

    #[test]
    fn test_http_backend_as_data_store_returns_self() -> kanban_domain::KanbanResult<()> {
        let backend = HttpBackend::new("http://example.com")?;
        let backend_ref: &dyn kanban_backend::KanbanBackend = &backend;
        let _: &dyn kanban_domain::DataStore = backend_ref.as_data_store();
        Ok(())
    }

    #[test]
    fn test_http_backend_stub_method_returns_unsupported_error() -> kanban_domain::KanbanResult<()>
    {
        let backend = HttpBackend::new("http://example.com")?;
        let result = backend.upsert_board(kanban_domain::Board::new("x", None::<String>));
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.is_unsupported());
        Ok(())
    }

    #[test]
    fn test_http_backend_with_transaction_returns_unsupported() -> kanban_domain::KanbanResult<()> {
        let backend = HttpBackend::new("http://example.com")?;
        let backend_ref: &dyn kanban_backend::KanbanBackend = &backend;

        let result = backend_ref.with_transaction(Box::new(|| Ok(())));

        let err = result.unwrap_err();
        assert!(
            err.is_unsupported(),
            "an HTTP backend has no local state to roll back, so it must decline \
             rather than silently run the closure unprotected (got: {err:?})"
        );
        Ok(())
    }

    #[test]
    fn test_http_backend_with_transaction_does_not_run_the_closure(
    ) -> kanban_domain::KanbanResult<()> {
        let backend = HttpBackend::new("http://example.com")?;
        let backend_ref: &dyn kanban_backend::KanbanBackend = &backend;
        let ran = std::cell::Cell::new(false);

        let _ = backend_ref.with_transaction(Box::new(|| {
            ran.set(true);
            Ok(())
        }));

        assert!(
            !ran.get(),
            "declining must happen before the closure runs; running it would apply \
             mutations with no transaction around them"
        );
        Ok(())
    }

    #[test]
    fn test_http_backend_new_rejects_a_locator_without_a_scheme() {
        let result = HttpBackend::new("boards.json");
        let Err(err) = result else {
            panic!("expected an error for a schemeless locator");
        };
        let err = err.to_string();
        assert!(err.contains("boards.json"), "Got: {err}");
        assert!(err.contains("http"), "Got: {err}");
    }

    #[test]
    fn test_http_backend_new_rejects_a_non_http_scheme() {
        assert!(HttpBackend::new("ftp://example.com").is_err());
        assert!(HttpBackend::new("notes://draft.json").is_err());
    }

    #[test]
    fn test_http_backend_instance_id_matches_accessor() -> kanban_domain::KanbanResult<()> {
        let backend = HttpBackend::new("http://example.com")?;
        let backend_ref: &dyn kanban_backend::KanbanBackend = &backend;
        assert_eq!(backend_ref.instance_id(), backend.instance_id);
        Ok(())
    }

    #[test]
    fn test_http_backend_as_any_downcasts_to_http_backend() -> kanban_domain::KanbanResult<()> {
        let backend = HttpBackend::new("http://example.com")?;
        let backend_ref: &dyn kanban_backend::KanbanBackend = &backend;
        let downcast = backend_ref
            .as_any()
            .and_then(|a| a.downcast_ref::<HttpBackend>());
        let downcast = downcast.expect("expected as_any to downcast to HttpBackend");
        assert_eq!(downcast.instance_id(), backend.instance_id());
        Ok(())
    }
}
