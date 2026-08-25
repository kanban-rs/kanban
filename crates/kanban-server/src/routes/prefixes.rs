use crate::error::AppError;
use crate::pagination::paginate_response;
use crate::state::AppState;
use axum::extract::{Query, State};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use kanban_domain::{KanbanError, Prefix};
use kanban_service::api::{PageParams, PrefixResponse};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct PrefixesQuery {
    name: Option<String>,
    page: Option<u32>,
    page_size: Option<u32>,
}

async fn prefixes_route(
    State(state): State<AppState>,
    Query(query): Query<PrefixesQuery>,
) -> Result<Response, AppError> {
    let ctx = state.ctx.lock().await;
    match query.name {
        Some(name) => {
            let normalized = Prefix::normalize(&name);
            let prefix = ctx
                .data_store()
                .get_prefix(&normalized)
                .map_err(|e| AppError::from(&e))?;
            match prefix {
                Some(prefix) => Ok(Json(PrefixResponse::from(&prefix)).into_response()),
                None => {
                    let available = ctx
                        .data_store()
                        .list_prefixes()
                        .map(|rows| rows.into_iter().map(|p| p.name).collect())
                        .unwrap_or_default();
                    Err(AppError::from(&KanbanError::not_found_by_name(
                        "Prefix",
                        &normalized,
                        available,
                    )))
                }
            }
        }
        None => {
            let prefixes = ctx
                .data_store()
                .list_prefixes()
                .map_err(|e| AppError::from(&e))?;
            let params = PageParams {
                page: query.page,
                page_size: query.page_size,
            };
            Ok(
                paginate_response(prefixes.iter().map(PrefixResponse::from).collect(), &params)?
                    .into_response(),
            )
        }
    }
}

pub fn read_router() -> Router<AppState> {
    Router::new().route("/v1/prefixes", get(prefixes_route))
}
