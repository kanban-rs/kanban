//! A non-success status is always an error, including a 404 -- there is no
//! `Ok(None)` short-circuit for a mutation, unlike the read-side `get_json`.
//! `CONFLICT_DETECTED` is remapped to `KanbanError::ConflictDetected`
//! explicitly, because `From<ApiError>` otherwise classifies it as a plain
//! validation error. A 404 or 405 whose body is not an error envelope (a
//! JSON object with a string `code` field) is a route the server has no
//! handler for, and becomes `KanbanError::UnsupportedByServer`. An envelope
//! with a `code` this client does not recognize (a newer server) is a real
//! server error, not an unsupported route, so it is reported as `Internal`
//! using the envelope's `message`.
//! No `If-Match` header is sent: v1 mutations are last-writer-wins.

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
        let operation_method = method.clone();
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
            return Err(map_mutation_error(
                &operation_method,
                path,
                self.base_url(),
                status,
                &body_text,
            ));
        }
        decode_mutation_body(&body_text)
    }
}

fn decode_mutation_body<T: DeserializeOwned>(body: &str) -> KanbanResult<T> {
    // A pre-0.11 server answers its DELETEs with 204 and no body; `{}` decodes
    // as a DeleteResponse carrying its Invalidation::All default.
    let body = if body.trim().is_empty() { "{}" } else { body };
    serde_json::from_str(body).map_err(|e| KanbanError::Serialization(e.to_string()))
}

fn map_mutation_error(
    method: &Method,
    path: &str,
    base_url: &str,
    status: StatusCode,
    body: &str,
) -> KanbanError {
    match serde_json::from_str::<ApiError>(body) {
        Ok(api_err) if api_err.code == ErrorCode::ConflictDetected => {
            KanbanError::ConflictDetected {
                path: format!("{base_url}{path}"),
                source: None,
            }
        }
        Ok(_) => crate::http::map_error_response(status, body),
        Err(_) => match envelope_message(body) {
            Some(message) => KanbanError::Internal(format!("HTTP {status}: {message}")),
            None if matches!(
                status,
                StatusCode::NOT_FOUND | StatusCode::METHOD_NOT_ALLOWED
            ) =>
            {
                KanbanError::unsupported_by_server(
                    format!("{method} {}", route_template(path)),
                    base_url,
                    kanban_core::KANBAN_VERSION,
                )
            }
            None => crate::http::map_error_response(status, body),
        },
    }
}

fn envelope_message(body: &str) -> Option<String> {
    let object = serde_json::from_str::<serde_json::Value>(body)
        .ok()
        .and_then(|v| v.as_object().cloned())?;
    match object.get("code")? {
        serde_json::Value::String(_) => Some(
            object
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or_default()
                .to_string(),
        ),
        _ => None,
    }
}

fn route_template(path: &str) -> String {
    let path = path.split_once('?').map_or(path, |(route, _)| route);
    path.split('/')
        .map(|segment| {
            if uuid::Uuid::parse_str(segment).is_ok() {
                "{id}"
            } else {
                segment
            }
        })
        .collect::<Vec<_>>()
        .join("/")
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
    async fn test_send_json_mutation_with_an_empty_2xx_body_for_a_mutation_response_still_fails_with_serialization(
    ) {
        let (url, handle) = stub_once("200 OK", "").await;
        let backend = HttpBackend::new(&url).unwrap();

        let result: KanbanResult<MutationResponse<BoardResponse>> = backend
            .send_json_mutation(Method::POST, "/v1/boards", None::<&()>)
            .await;

        assert!(
            matches!(result, Err(KanbanError::Serialization(_))),
            "got: {result:?}"
        );

        handle.await.unwrap();
    }

    #[test]
    fn test_route_template_replaces_uuid_segments_and_drops_the_query() {
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        assert_eq!(
            route_template(&format!("/v1/cards/{a}/move?column_id={b}")),
            "/v1/cards/{id}/move"
        );
        assert_eq!(
            route_template(&format!("/v1/cards/{a}/blocks/{b}")),
            "/v1/cards/{id}/blocks/{id}"
        );
        assert_eq!(route_template("/v1/boards"), "/v1/boards");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_send_json_mutation_maps_a_405_with_an_empty_body_to_unsupported_by_server() {
        let (url, handle) = stub_once("405 Method Not Allowed", "").await;
        let backend = HttpBackend::new(&url).unwrap();

        let result: KanbanResult<DeleteResponse> = backend
            .send_json_mutation::<(), DeleteResponse>(
                Method::POST,
                "/v1/cards/x/children/detach",
                None,
            )
            .await;

        let err = result.unwrap_err();
        assert!(err.is_unsupported(), "got: {err:?}");
        assert!(
            err.to_string().contains("POST /v1/cards/x/children/detach"),
            "got: {err}"
        );

        handle.await.unwrap();
    }

    #[test]
    fn test_envelope_message_an_html_404_body_is_not_an_envelope() {
        assert_eq!(envelope_message("<html>Not Found</html>"), None);
    }

    #[test]
    fn test_envelope_message_a_json_array_body_is_not_an_envelope() {
        assert_eq!(envelope_message(r#"["NOT_FOUND"]"#), None);
    }

    #[test]
    fn test_envelope_message_an_object_without_a_code_field_is_not_an_envelope() {
        assert_eq!(envelope_message(r#"{"message":"oops"}"#), None);
    }

    #[test]
    fn test_envelope_message_an_object_with_an_unknown_string_code_is_an_envelope() {
        assert_eq!(
            envelope_message(r#"{"code":"SOME_FUTURE_CODE","message":"thing not found"}"#),
            Some("thing not found".to_string())
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_send_json_mutation_with_a_404_envelope_carrying_an_unknown_code_is_not_unsupported(
    ) {
        let (url, handle) = stub_once(
            "404 Not Found",
            r#"{"code":"SOME_FUTURE_CODE","message":"thing not found"}"#,
        )
        .await;
        let backend = HttpBackend::new(&url).unwrap();

        let update = kanban_api::UpdateBoardRequest::default();
        let result: KanbanResult<MutationResponse<BoardResponse>> = backend
            .send_json_mutation(Method::PATCH, "/v1/boards/x", Some(&update))
            .await;

        let err = result.unwrap_err();
        assert!(!err.is_unsupported(), "got: {err:?}");
        assert!(err.to_string().contains("thing not found"), "got: {err}");

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
