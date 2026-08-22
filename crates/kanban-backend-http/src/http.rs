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
