use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use kanban_service::api::ApiError;

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
