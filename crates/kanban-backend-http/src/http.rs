use crate::HttpBackend;
use kanban_api::{ApiError, Page};
use kanban_domain::{KanbanError, KanbanResult};
use serde::de::DeserializeOwned;

fn map_error_response(status: reqwest::StatusCode, body: &str) -> KanbanError {
    match serde_json::from_str::<ApiError>(body) {
        Ok(api_err) => KanbanError::from(api_err),
        Err(_) => KanbanError::Internal(format!("HTTP {status}: {body}")),
    }
}

impl HttpBackend {
    pub(crate) async fn get_json<T: DeserializeOwned>(
        &self,
        path: &str,
    ) -> KanbanResult<Option<T>> {
        self.get_json_with_query(path, &[]).await
    }

    /// Like [`Self::get_json`], but attaches `query` via reqwest's own query
    /// builder rather than string interpolation, so a value is percent-encoded
    /// (and an empty value stays addressable) instead of corrupting the path.
    pub(crate) async fn get_json_with_query<T: DeserializeOwned>(
        &self,
        path: &str,
        query: &[(&str, &str)],
    ) -> KanbanResult<Option<T>> {
        let url = format!("{}{}", self.base_url(), path);
        let resp = self
            .client()
            .get(&url)
            .query(query)
            .send()
            .await
            .map_err(|e| KanbanError::Transport(e.to_string()))?;
        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        let status = resp.status();
        let body = resp
            .text()
            .await
            .map_err(|e| KanbanError::Transport(e.to_string()))?;
        if !status.is_success() {
            return Err(map_error_response(status, &body));
        }
        serde_json::from_str(&body)
            .map(Some)
            .map_err(|e| KanbanError::Serialization(e.to_string()))
    }

    /// Deserializes each response as `Page<T>` and follows `total_pages` to
    /// completion, since `DataStore::list_*` returns the whole collection and
    /// cannot honestly return one page at a time.
    pub(crate) async fn get_json_list<T: DeserializeOwned>(
        &self,
        path: &str,
    ) -> KanbanResult<Vec<T>> {
        let url = format!("{}{}", self.base_url(), path);
        let mut out = Vec::new();
        let mut page = 1u32;
        loop {
            let resp = self
                .client()
                .get(&url)
                .query(&[("page", page)])
                .send()
                .await
                .map_err(|e| KanbanError::Transport(e.to_string()))?;
            let status = resp.status();
            let body = resp
                .text()
                .await
                .map_err(|e| KanbanError::Transport(e.to_string()))?;
            if !status.is_success() {
                return Err(map_error_response(status, &body));
            }
            let page_data: Page<T> = serde_json::from_str(&body)
                .map_err(|e| KanbanError::Serialization(e.to_string()))?;
            let total_pages = page_data.total_pages;
            out.extend(page_data.items);
            if page >= total_pages {
                break;
            }
            page += 1;
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use crate::HttpBackend;
    use kanban_api::BoardResponse;
    use kanban_server::test_helpers::TestServer;

    #[tokio::test(flavor = "multi_thread")]
    async fn test_get_json_maps_a_non_404_error_status_to_err_rather_than_ok_none() {
        let server = TestServer::start().await;
        let backend = HttpBackend::new(&server.base_url()).unwrap();

        let result: kanban_domain::KanbanResult<Option<BoardResponse>> =
            tokio::task::spawn_blocking(move || {
                backend.block_on(backend.get_json::<BoardResponse>("/v1/boards/not-a-uuid"))
            })
            .await
            .unwrap();

        assert!(
            result.is_err(),
            "a 400 from the Path extractor must not be reported as Ok(None): {result:?}"
        );

        server.shutdown().await;
    }
}
