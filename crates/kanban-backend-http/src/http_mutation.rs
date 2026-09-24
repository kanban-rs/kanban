//! A non-success status is always an error, including a 404 -- there is no
//! `Ok(None)` short-circuit for a mutation, unlike the read-side `get_json`.
//! `CONFLICT_DETECTED` is remapped to `KanbanError::ConflictDetected`
//! explicitly, because `From<ApiError>` otherwise classifies it as a plain
//! validation error. No `If-Match` header is sent: v1 mutations are
//! last-writer-wins.

use crate::HttpBackend;
use kanban_api::{ApiError, ErrorCode, CLIENT_ID_HEADER};
use kanban_backend::KanbanBackend;
use kanban_domain::{KanbanError, KanbanResult};
use reqwest::{Method, StatusCode};
use serde::de::DeserializeOwned;

impl HttpBackend {
    pub(crate) async fn send_json_mutation<B, T>(
        &self,
        method: Method,
        path: &str,
        body: Option<&B>,
    ) -> KanbanResult<T>
    where
        B: serde::Serialize + ?Sized,
        T: DeserializeOwned,
    {
        let url = format!("{}{}", self.base_url(), path);
        let mut request = self
            .client()
            .request(method, &url)
            .header(CLIENT_ID_HEADER, self.instance_id().to_string());
        if let Some(body) = body {
            request = request.json(body);
        }
        let resp = request
            .send()
            .await
            .map_err(|e| KanbanError::Transport(e.to_string()))?;
        let status = resp.status();
        let body_text = resp
            .text()
            .await
            .map_err(|e| KanbanError::Transport(e.to_string()))?;
        if !status.is_success() {
            return Err(map_mutation_error(status, &body_text, &url));
        }
        serde_json::from_str(&body_text).map_err(|e| KanbanError::Serialization(e.to_string()))
    }
}

fn map_mutation_error(status: StatusCode, body: &str, url: &str) -> KanbanError {
    if let Ok(api_err) = serde_json::from_str::<ApiError>(body) {
        if api_err.code == ErrorCode::ConflictDetected {
            return KanbanError::ConflictDetected {
                path: url.to_string(),
                source: None,
            };
        }
    }
    crate::http::map_error_response(status, body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use kanban_api::{BoardResponse, CreateBoardRequest, DeleteResponse, MutationResponse};
    use kanban_backend::KanbanBackend;
    use kanban_domain::Invalidation;
    use kanban_server::test_helpers::TestServer;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;
    use uuid::Uuid;

    async fn stub_once(
        status_line: &'static str,
        body: &'static str,
    ) -> (String, tokio::task::JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let url = format!("http://{addr}");
        let handle = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut buf = Vec::new();
            let mut tmp = [0u8; 4096];
            let mut content_length = 0usize;
            let mut headers_end = None;
            loop {
                let n = socket.read(&mut tmp).await.unwrap();
                buf.extend_from_slice(&tmp[..n]);
                if let Some(pos) = find_headers_end(&buf) {
                    headers_end = Some(pos);
                    let header_text = String::from_utf8_lossy(&buf[..pos]).to_lowercase();
                    content_length = header_text
                        .lines()
                        .find_map(|l| l.strip_prefix("content-length:"))
                        .map(|v| v.trim().parse::<usize>().unwrap_or(0))
                        .unwrap_or(0);
                }
                if let Some(pos) = headers_end {
                    if buf.len() >= pos + 4 + content_length {
                        break;
                    }
                }
                if n == 0 {
                    break;
                }
            }
            let response = format!(
                "HTTP/1.1 {status_line}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                body.len()
            );
            socket.write_all(response.as_bytes()).await.unwrap();
            socket.shutdown().await.ok();
            String::from_utf8_lossy(&buf).to_string()
        });
        (url, handle)
    }

    fn find_headers_end(buf: &[u8]) -> Option<usize> {
        buf.windows(4).position(|w| w == b"\r\n\r\n")
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_send_json_mutation_patch_returns_the_parsed_mutation_response() {
        let server = TestServer::start().await;
        let backend = HttpBackend::new(&server.base_url()).unwrap();
        let create_resp: MutationResponse<BoardResponse> = server
            .client()
            .post(format!("{}/v1/boards", server.base_url()))
            .json(&CreateBoardRequest {
                id: None,
                name: "Original".to_string(),
                description: None,
                sprint_prefix: None,
                card_prefix: None,
                task_sort_field: None,
                task_sort_order: None,
                sprint_duration_days: None,
                task_list_view: None,
            })
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        let id = create_resp.entity.id;

        let update = kanban_api::UpdateBoardRequest {
            name: Some("Renamed".to_string()),
            ..Default::default()
        };
        let resp: MutationResponse<BoardResponse> = backend
            .send_json_mutation(Method::PATCH, &format!("/v1/boards/{id}"), Some(&update))
            .await
            .unwrap();

        assert_eq!(resp.entity.name, "Renamed");
        let invalidation = Invalidation::from(&resp.invalidation);
        match invalidation {
            Invalidation::Entities(ids) => assert!(ids.boards.contains(&id)),
            Invalidation::All => panic!("expected a scoped invalidation"),
        }

        server.shutdown().await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_send_json_mutation_post_treats_201_created_as_success() {
        let server = TestServer::start().await;
        let backend = HttpBackend::new(&server.base_url()).unwrap();

        let create = CreateBoardRequest {
            id: None,
            name: "Fresh".to_string(),
            description: None,
            sprint_prefix: None,
            card_prefix: None,
            task_sort_field: None,
            task_sort_order: None,
            sprint_duration_days: None,
            task_list_view: None,
        };
        let resp: MutationResponse<BoardResponse> = backend
            .send_json_mutation(Method::POST, "/v1/boards", Some(&create))
            .await
            .unwrap();

        assert_eq!(resp.entity.name, "Fresh");

        server.shutdown().await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_send_json_mutation_delete_with_no_body_parses_the_delete_response() {
        let server = TestServer::start().await;
        let backend = HttpBackend::new(&server.base_url()).unwrap();
        let create_resp: MutationResponse<BoardResponse> = server
            .client()
            .post(format!("{}/v1/boards", server.base_url()))
            .json(&CreateBoardRequest {
                id: None,
                name: "ToDelete".to_string(),
                description: None,
                sprint_prefix: None,
                card_prefix: None,
                task_sort_field: None,
                task_sort_order: None,
                sprint_duration_days: None,
                task_list_view: None,
            })
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        let id = create_resp.entity.id;

        let resp: DeleteResponse = backend
            .send_json_mutation::<(), DeleteResponse>(
                Method::DELETE,
                &format!("/v1/boards/{id}"),
                None,
            )
            .await
            .unwrap();

        let invalidation = Invalidation::from(&resp.invalidation);
        match invalidation {
            Invalidation::Entities(ids) => assert!(ids.boards.contains(&id)),
            Invalidation::All => panic!("expected a scoped invalidation"),
        }

        server.shutdown().await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_send_json_mutation_maps_a_404_to_the_servers_not_found_error() {
        let server = TestServer::start().await;
        let backend = HttpBackend::new(&server.base_url()).unwrap();
        let missing = Uuid::new_v4();

        let update = kanban_api::UpdateBoardRequest::default();
        let result: KanbanResult<MutationResponse<BoardResponse>> = backend
            .send_json_mutation(
                Method::PATCH,
                &format!("/v1/boards/{missing}"),
                Some(&update),
            )
            .await;

        let err = result.unwrap_err();
        assert!(err.to_string().contains("NOT_FOUND"), "got: {err}");

        server.shutdown().await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_send_json_mutation_maps_a_conflict_detected_body_to_conflict_detected() {
        let (url, handle) = stub_once(
            "409 Conflict",
            r#"{"code":"CONFLICT_DETECTED","message":"reload and retry"}"#,
        )
        .await;
        let backend = HttpBackend::new(&url).unwrap();

        let update = kanban_api::UpdateBoardRequest::default();
        let result: KanbanResult<MutationResponse<BoardResponse>> = backend
            .send_json_mutation(Method::PATCH, "/v1/boards/x", Some(&update))
            .await;

        let err = result.unwrap_err();
        assert!(err.is_conflict_detected(), "got: {err:?}");
        if let KanbanError::ConflictDetected { path, .. } = &err {
            assert!(path.contains("/v1/boards/x"), "got path: {path}");
        } else {
            panic!("expected ConflictDetected, got {err:?}");
        }

        handle.await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_send_json_mutation_attaches_the_client_id_header_valued_with_the_instance_id() {
        let (url, handle) = stub_once("200 OK", r#"{"invalidation":{"scope":"all"}}"#).await;
        let backend = HttpBackend::new(&url).unwrap();
        let instance_id = backend.instance_id().to_string();

        let _result: KanbanResult<DeleteResponse> = backend
            .send_json_mutation::<(), DeleteResponse>(Method::DELETE, "/v1/boards/x", None)
            .await;

        let captured = handle.await.unwrap().to_lowercase();
        assert!(
            captured.contains(&format!(
                "x-kanban-client-id: {}",
                instance_id.to_lowercase()
            )),
            "got: {captured}"
        );
    }
}
