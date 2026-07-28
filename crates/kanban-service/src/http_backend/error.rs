use kanban_domain::KanbanError;

/// Map a reqwest transport error to a KanbanError.
/// Transport errors include connection refused, timeouts, DNS failures, etc.
pub fn map_transport_error(err: reqwest::Error) -> KanbanError {
    KanbanError::Internal(format!("http transport error: {err}"))
}

/// Map an HTTP response error (non-2xx with ApiError body) to a KanbanError.
/// This is currently a minimal stub; fuller mapping will come in later cards
/// as actual HTTP responses are processed.
pub fn map_api_error(error_message: String) -> KanbanError {
    KanbanError::Internal(format!("http api error: {error_message}"))
}
