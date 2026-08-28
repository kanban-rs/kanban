use crate::error::AppError;
use axum::Json;
use kanban_core::paginated_list::{resolve_page_params, PaginatedList};
use kanban_service::api::{Page, PageParams};
use kanban_service::KanbanError;

/// # Errors
///
/// A zero or over-`MAX_PAGE_SIZE` parameter becomes a `VALIDATION_FAILED`
/// [`AppError`] (HTTP 422).
pub fn paginate_response<T>(items: Vec<T>, params: &PageParams) -> Result<Json<Page<T>>, AppError> {
    let (page, page_size) = resolve_page_params(params.page, params.page_size)
        .map_err(|e| AppError::from(&KanbanError::from(e)))?;
    let paginated = PaginatedList::paginate(items, page, page_size)
        .map_err(|e| AppError::from(&KanbanError::from(e)))?;
    Ok(Json(paginated.into()))
}
