use crate::client_ident::ClientIdent;
use crate::error::{AppError, AppJson};
use crate::model_read::{require_loaded, require_loaded_entity};
use crate::scope::RouteScope;
use crate::state::AppState;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use kanban_domain::{DependencyGraph, Model, NoProjections};
use kanban_service::api::{
    AddBlockRequest, AddRelatedRequest, AttachChildrenRequest, CardGraphResponse, ChangeKind,
    EntityType,
};
use kanban_service::KanbanContext;
use uuid::Uuid;

async fn get_card_graph(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<CardGraphResponse>, AppError> {
    let guard = state.lock_session().await;
    let mut model = Model::default();
    guard.sync(&RouteScope::CardGraph(id), &mut model, &mut NoProjections);
    require_loaded_entity(model.card_id_status(id), "Card", id)?;
    let graph = require_loaded(model.graph_state().as_ref(), "card graph")?;
    Ok(Json(CardGraphResponse::from_graph(id, graph)))
}

async fn get_graph(State(state): State<AppState>) -> Result<Json<DependencyGraph>, AppError> {
    let guard = state.lock_session().await;
    let mut model = Model::default();
    guard.sync(&RouteScope::Graph, &mut model, &mut NoProjections);
    let graph = require_loaded(model.graph_state().as_ref(), "dependency graph")?;
    Ok(Json(graph.clone()))
}

pub fn read_router() -> Router<AppState> {
    Router::new()
        .route("/v1/graph", get(get_graph))
        .route("/v1/cards/{id}/graph", get(get_card_graph))
}

fn graph_mutation_response(
    ctx: &KanbanContext,
    id: Uuid,
) -> Result<Json<CardGraphResponse>, AppError> {
    let mut model = Model::default();
    ctx.sync(&RouteScope::CardGraph(id), &mut model, &mut NoProjections);
    require_loaded_entity(model.card_id_status(id), "Card", id)?;
    let graph = require_loaded(model.graph_state().as_ref(), "card graph")?;
    Ok(Json(CardGraphResponse::from_graph(id, graph)))
}

async fn attach_children_route(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    ClientIdent(client): ClientIdent,
    AppJson(req): AppJson<AttachChildrenRequest>,
) -> Result<Json<CardGraphResponse>, AppError> {
    let body = {
        let mut ctx = state.lock_for_write(client).await;
        let invalidation =
            crate::state::mutate_unit(&mut ctx, |c| c.attach_children_impl(id, req.children))
                .map_err(|e| AppError::from(&e))?;
        let body = graph_mutation_response(&ctx.ctx, id)?;
        state
            .persist_and_broadcast(
                &ctx,
                EntityType::Card,
                id,
                ChangeKind::Updated,
                &invalidation,
            )
            .await
            .map_err(|e| AppError::from(&e))?;
        body
    };
    Ok(body)
}

async fn detach_child_route(
    State(state): State<AppState>,
    Path((id, child_id)): Path<(Uuid, Uuid)>,
    ClientIdent(client): ClientIdent,
) -> Result<StatusCode, AppError> {
    {
        let mut ctx = state.lock_for_write(client).await;
        let invalidation =
            crate::state::mutate_unit(&mut ctx, |c| c.detach_children_impl(id, vec![child_id]))
                .map_err(|e| AppError::from(&e))?;
        state
            .persist_and_broadcast(
                &ctx,
                EntityType::Card,
                id,
                ChangeKind::Updated,
                &invalidation,
            )
            .await
            .map_err(|e| AppError::from(&e))?;
    }
    Ok(StatusCode::NO_CONTENT)
}

async fn add_block_route(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    ClientIdent(client): ClientIdent,
    AppJson(req): AppJson<AddBlockRequest>,
) -> Result<Json<CardGraphResponse>, AppError> {
    let body = {
        let mut ctx = state.lock_for_write(client).await;
        let invalidation = crate::state::mutate_unit(&mut ctx, |c| {
            c.block_impl(id, req.blocked, req.severity.into())
        })
        .map_err(|e| AppError::from(&e))?;
        let body = graph_mutation_response(&ctx.ctx, id)?;
        state
            .persist_and_broadcast(
                &ctx,
                EntityType::Card,
                id,
                ChangeKind::Updated,
                &invalidation,
            )
            .await
            .map_err(|e| AppError::from(&e))?;
        body
    };
    Ok(body)
}

async fn remove_block_route(
    State(state): State<AppState>,
    Path((id, blocked_id)): Path<(Uuid, Uuid)>,
    ClientIdent(client): ClientIdent,
) -> Result<StatusCode, AppError> {
    {
        let mut ctx = state.lock_for_write(client).await;
        let invalidation = crate::state::mutate_unit(&mut ctx, |c| c.unblock_impl(id, blocked_id))
            .map_err(|e| AppError::from(&e))?;
        state
            .persist_and_broadcast(
                &ctx,
                EntityType::Card,
                id,
                ChangeKind::Updated,
                &invalidation,
            )
            .await
            .map_err(|e| AppError::from(&e))?;
    }
    Ok(StatusCode::NO_CONTENT)
}

async fn add_related_route(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    ClientIdent(client): ClientIdent,
    AppJson(req): AppJson<AddRelatedRequest>,
) -> Result<Json<CardGraphResponse>, AppError> {
    let body = {
        let mut ctx = state.lock_for_write(client).await;
        let invalidation =
            crate::state::mutate_unit(&mut ctx, |c| c.relate_impl(id, req.other, req.kind.into()))
                .map_err(|e| AppError::from(&e))?;
        let body = graph_mutation_response(&ctx.ctx, id)?;
        state
            .persist_and_broadcast(
                &ctx,
                EntityType::Card,
                id,
                ChangeKind::Updated,
                &invalidation,
            )
            .await
            .map_err(|e| AppError::from(&e))?;
        body
    };
    Ok(body)
}

async fn remove_related_route(
    State(state): State<AppState>,
    Path((id, other_id)): Path<(Uuid, Uuid)>,
    ClientIdent(client): ClientIdent,
) -> Result<StatusCode, AppError> {
    {
        let mut ctx = state.lock_for_write(client).await;
        let invalidation = crate::state::mutate_unit(&mut ctx, |c| c.dissociate_impl(id, other_id))
            .map_err(|e| AppError::from(&e))?;
        state
            .persist_and_broadcast(
                &ctx,
                EntityType::Card,
                id,
                ChangeKind::Updated,
                &invalidation,
            )
            .await
            .map_err(|e| AppError::from(&e))?;
    }
    Ok(StatusCode::NO_CONTENT)
}

pub fn write_router() -> Router<AppState> {
    Router::new()
        .route("/v1/cards/{id}/children", post(attach_children_route))
        .route(
            "/v1/cards/{id}/children/{child_id}",
            delete(detach_child_route),
        )
        .route("/v1/cards/{id}/blocks", post(add_block_route))
        .route(
            "/v1/cards/{id}/blocks/{blocked_id}",
            delete(remove_block_route),
        )
        .route("/v1/cards/{id}/related", post(add_related_route))
        .route(
            "/v1/cards/{id}/related/{other_id}",
            delete(remove_related_route),
        )
}
