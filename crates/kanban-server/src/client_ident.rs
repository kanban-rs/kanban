use crate::error::AppError;
use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use kanban_core::ClientId;
use kanban_service::api::{ApiError, ErrorCode};
use uuid::Uuid;

pub use kanban_service::api::CLIENT_ID_HEADER;

/// Unauthenticated, client-supplied identity carried on `X-Kanban-Client-Id`.
/// Absent header resolves to [`ClientId::nil`]; a malformed value rejects
/// with a 422 rather than defaulting.
#[derive(Debug)]
pub struct ClientIdent(pub ClientId);

impl<S: Send + Sync> FromRequestParts<S> for ClientIdent {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        match parts.headers.get(CLIENT_ID_HEADER) {
            None => Ok(ClientIdent(ClientId::nil())),
            Some(value) => value
                .to_str()
                .ok()
                .and_then(|s| Uuid::parse_str(s).ok())
                .map(|uuid| ClientIdent(ClientId::from(uuid)))
                .ok_or_else(|| {
                    AppError(ApiError::new(
                        ErrorCode::ValidationFailed,
                        format!("{CLIENT_ID_HEADER} must be a UUID"),
                    ))
                }),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::client_ident::{ClientIdent, CLIENT_ID_HEADER};
    use axum::extract::FromRequestParts;
    use axum::http::{HeaderValue, Request};
    use axum::response::IntoResponse;
    use kanban_core::ClientId;
    use uuid::Uuid;

    #[tokio::test]
    async fn test_absent_header_yields_nil_client_id() {
        let mut parts = Request::builder().uri("/").body(()).unwrap().into_parts().0;
        let result = ClientIdent::from_request_parts(&mut parts, &()).await;
        match result {
            Ok(ClientIdent(id)) => assert_eq!(id, ClientId::nil()),
            Err(_) => panic!("expected Ok"),
        }
    }

    #[tokio::test]
    async fn test_valid_uuid_header_yields_that_client_id() {
        let uuid = Uuid::new_v4();
        let mut parts = Request::builder()
            .uri("/")
            .header(CLIENT_ID_HEADER, uuid.to_string())
            .body(())
            .unwrap()
            .into_parts()
            .0;
        let result = ClientIdent::from_request_parts(&mut parts, &()).await;
        match result {
            Ok(ClientIdent(id)) => assert_eq!(id, ClientId::from(uuid)),
            Err(_) => panic!("expected Ok"),
        }
    }

    #[tokio::test]
    async fn test_header_named_by_the_api_constant_yields_that_client_id() {
        let uuid = Uuid::new_v4();
        let mut parts = Request::builder()
            .uri("/")
            .header(kanban_service::api::CLIENT_ID_HEADER, uuid.to_string())
            .body(())
            .unwrap()
            .into_parts()
            .0;
        let result = ClientIdent::from_request_parts(&mut parts, &()).await;
        match result {
            Ok(ClientIdent(id)) => assert_eq!(id, ClientId::from(uuid)),
            Err(_) => panic!("expected Ok"),
        }
    }

    #[tokio::test]
    async fn test_malformed_header_returns_422() {
        let mut parts = Request::builder()
            .uri("/")
            .header(CLIENT_ID_HEADER, "not-a-uuid")
            .body(())
            .unwrap()
            .into_parts()
            .0;
        let result = ClientIdent::from_request_parts(&mut parts, &()).await;
        let err = result.unwrap_err();
        assert_eq!(err.0.code, kanban_service::api::ErrorCode::ValidationFailed);
        assert_eq!(err.into_response().status(), 422);
    }

    #[tokio::test]
    async fn test_non_utf8_header_returns_422() {
        let mut parts = Request::builder().uri("/").body(()).unwrap().into_parts().0;
        parts
            .headers
            .insert(CLIENT_ID_HEADER, HeaderValue::from_bytes(&[0xff]).unwrap());
        let result = ClientIdent::from_request_parts(&mut parts, &()).await;
        let err = result.unwrap_err();
        assert_eq!(err.into_response().status(), 422);
    }
}
