// Every field and the client()/base_url() accessors below are only reached
// by this crate's own tests today; the DataStore/CommandStore stubs return
// early without touching them. Sibling cards implementing real reads/writes
// exercise them from production code.
#![allow(dead_code)]

mod command_store;
mod data_store;
mod remote_writes;

pub struct HttpBackend {
    base_url: String,
    client: reqwest::Client,
    runtime: tokio::runtime::Runtime,
    instance_id: uuid::Uuid,
}

#[async_trait::async_trait]
impl kanban_backend::KanbanBackend for HttpBackend {
    fn as_data_store(&self) -> &dyn kanban_domain::DataStore {
        self
    }

    fn instance_id(&self) -> uuid::Uuid {
        self.instance_id
    }

    fn with_transaction(
        &self,
        _f: &mut dyn FnMut() -> kanban_domain::KanbanResult<()>,
    ) -> kanban_domain::KanbanResult<()> {
        Err(kanban_domain::KanbanError::unsupported("with_transaction"))
    }
}

impl HttpBackend {
    pub fn new(base_url: &str) -> kanban_domain::KanbanResult<Self> {
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
            runtime,
            instance_id: uuid::Uuid::new_v4(),
        })
    }

    /// Bridge a synchronous DataStore/CommandStore call onto the dedicated
    /// runtime -- never the caller's ambient one.
    pub(crate) fn block_on<F: std::future::Future>(&self, fut: F) -> F::Output {
        self.runtime.block_on(fut)
    }

    pub(crate) fn base_url(&self) -> &str {
        &self.base_url
    }

    pub(crate) fn client(&self) -> &reqwest::Client {
        &self.client
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
        let result = backend.list_boards();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.is_unsupported());
        Ok(())
    }

    #[test]
    fn test_http_backend_instance_id_matches_accessor() -> kanban_domain::KanbanResult<()> {
        let backend = HttpBackend::new("http://example.com")?;
        let backend_ref: &dyn kanban_backend::KanbanBackend = &backend;
        assert_eq!(backend_ref.instance_id(), backend.instance_id);
        Ok(())
    }

    #[test]
    fn test_http_backend_with_transaction_returns_unsupported() -> kanban_domain::KanbanResult<()> {
        let backend = HttpBackend::new("http://example.com")?;
        let backend_ref: &dyn kanban_backend::KanbanBackend = &backend;
        let err = backend_ref.with_transaction(&mut || Ok(())).unwrap_err();
        assert!(err.is_unsupported());
        let msg = format!("{err}");
        assert!(
            msg.contains("with_transaction"),
            "error must name with_transaction itself, not a helper it happens to call \
             through a fallback (got: {msg:?})"
        );
        Ok(())
    }
}
