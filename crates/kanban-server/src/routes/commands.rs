use crate::error::{AppError, AppJson};
use crate::state::AppState;
use axum::extract::State;
use axum::routing::post;
use axum::{Json, Router};
use kanban_domain::CommandBatch;
use kanban_service::api::ChangeEventFrame;
use serde::Serialize;

#[derive(Debug, Serialize)]
struct BatchResult {
    executed: usize,
}

async fn execute_commands(
    State(state): State<AppState>,
    AppJson(batch): AppJson<CommandBatch>,
) -> Result<Json<BatchResult>, AppError> {
    let correlation_id = batch.correlation_id;
    let issued_by = batch.issued_by;
    let executed = batch.commands.len();

    let mut ctx = state.ctx.lock().await;
    ctx.execute(batch.commands)
        .map_err(|e| AppError::from(&e))?;
    ctx.save().await.map_err(|e| AppError::from(&e))?;
    let _ = state.event_tx.send(ChangeEventFrame::now(
        state.instance_id,
        correlation_id,
        issued_by,
    ));

    Ok(Json(BatchResult { executed }))
}

pub fn router() -> Router<AppState> {
    Router::new().route("/v1/commands", post(execute_commands))
}
