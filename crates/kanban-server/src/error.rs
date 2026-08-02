use axum::extract::rejection::JsonRejection;
use axum::extract::{FromRequest, Request};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use kanban_service::api::{ApiError, ErrorCode};

pub struct AppError(pub ApiError);

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = StatusCode::from_u16(self.0.code.http_status())
            .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        (status, Json(self.0)).into_response()
    }
}

impl From<ApiError> for AppError {
    fn from(e: ApiError) -> Self {
        AppError(e)
    }
}

impl From<&kanban_domain::KanbanError> for AppError {
    fn from(e: &kanban_domain::KanbanError) -> Self {
        AppError(ApiError::from(e))
    }
}

/// Drop-in replacement for `axum::Json` that converts a deserialization
/// failure (missing/wrong-typed field, malformed syntax) into the same
/// `{"code", "message"}` envelope every other error already uses, instead of
/// axum's default plain-text rejection body. Use this for every request body
/// extractor on a write route, not `axum::Json` directly.
pub struct AppJson<T>(pub T);

impl<T, S> FromRequest<S> for AppJson<T>
where
    axum::Json<T>: FromRequest<S, Rejection = JsonRejection>,
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        match axum::Json::<T>::from_request(req, state).await {
            Ok(Json(value)) => Ok(AppJson(value)),
            Err(rejection) => Err(AppError(ApiError::new(
                ErrorCode::ValidationFailed,
                rejection.body_text(),
            ))),
        }
    }
}
