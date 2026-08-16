// The runtime/client fields below now back the real DataStore read methods
// (get_board/list_boards/get_column/list_columns_by_board/
// list_cards_by_column); get_card and every write path are still
// unsupported() stubs. Sibling cards implement the remaining reads/writes.
#![allow(dead_code)]

mod command_store;
mod conversions;
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

    /// `GET {base_url}{path}`, returning `Ok(None)` on 404 and `Ok(Some(_))`
    /// after decoding the body as `T` on any other 2xx. A non-2xx status
    /// (other than 404), a transport failure, or a decode error all become
    /// `KanbanError::Internal`.
    pub(crate) async fn get_optional<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
    ) -> kanban_domain::KanbanResult<Option<T>> {
        let url = format!("{}{}", self.base_url, path);
        let response =
            self.client.get(&url).send().await.map_err(|e| {
                kanban_domain::KanbanError::Internal(format!("GET {url} failed: {e}"))
            })?;
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        if !response.status().is_success() {
            return Err(kanban_domain::KanbanError::Internal(format!(
                "GET {url} returned {}",
                response.status()
            )));
        }
        response.json::<T>().await.map(Some).map_err(|e| {
            kanban_domain::KanbanError::Internal(format!("GET {url} decode failed: {e}"))
        })
    }

    /// `GET {base_url}{path}`, decoding the body as `Vec<T>`. A 404 is treated
    /// as an empty list (no known list route in this crate 404s today, but an
    /// empty list is the safer read than propagating an error for one).
    pub(crate) async fn get_list<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
    ) -> kanban_domain::KanbanResult<Vec<T>> {
        Ok(self.get_optional::<Vec<T>>(path).await?.unwrap_or_default())
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
        // get_card has no working implementation yet: no route returns a
        // CardResponse with its board_id, so it stays unsupported() (see
        // conversions.rs' card_from_response doc). list_boards/list_columns_by_board/
        // list_cards_by_column are real now -- covered by tests/remote_reads.rs
        // against a live TestServer instead of a stub-error assertion.
        let backend = HttpBackend::new("http://example.com")?;
        let result = backend.get_card(uuid::Uuid::new_v4());
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
    fn test_http_backend_instance_id_matches_accessor() -> kanban_domain::KanbanResult<()> {
        let backend = HttpBackend::new("http://example.com")?;
        let backend_ref: &dyn kanban_backend::KanbanBackend = &backend;
        assert_eq!(backend_ref.instance_id(), backend.instance_id);
        Ok(())
    }
}
