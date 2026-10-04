use kanban_api::{CardGraphResponse, SprintResponse};
use kanban_backend::{RemoteCardWrites, RemoteGraphWrites, RemoteSprintWrites};
use kanban_backend_http::HttpBackend;
use kanban_domain::{DependencyGraph, Invalidation, KanbanResult, Severity, Sprint, SprintUpdate};
use kanban_server::test_helpers::{StubReply, StubServer};
use uuid::Uuid;

#[tokio::test(flavor = "multi_thread")]
async fn test_graph_unblock_answered_204_empty_by_an_old_server_returns_ok_with_invalidation_all(
) -> KanbanResult<()> {
    let (a, b) = (Uuid::new_v4(), Uuid::new_v4());
    let route = format!("/v1/cards/{a}/blocks/{b}");
    let expected = route.clone();
    let stub = StubServer::start(move |method, path| {
        if method == "DELETE" && path == expected {
            StubReply::empty(204)
        } else {
            StubReply::empty(404)
        }
    })
    .await;
    let backend = HttpBackend::new(&stub.base_url())?;

    assert_eq!(backend.unblock(a, b)?, Invalidation::All);
    assert_eq!(stub.requests(), vec![("DELETE".to_string(), route)]);

    drop(backend);
    stub.shutdown().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_graph_dissociate_answered_204_empty_returns_ok_with_invalidation_all(
) -> KanbanResult<()> {
    let (a, b) = (Uuid::new_v4(), Uuid::new_v4());
    let route = format!("/v1/cards/{a}/related/{b}");
    let expected = route.clone();
    let stub = StubServer::start(move |method, path| {
        if method == "DELETE" && path == expected {
            StubReply::empty(204)
        } else {
            StubReply::empty(404)
        }
    })
    .await;
    let backend = HttpBackend::new(&stub.base_url())?;

    assert_eq!(backend.dissociate(a, b)?, Invalidation::All);

    drop(backend);
    stub.shutdown().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_sprint_delete_answered_204_empty_returns_ok_with_invalidation_all() -> KanbanResult<()>
{
    let id = Uuid::new_v4();
    let route = format!("/v1/sprints/{id}");
    let expected = route.clone();
    let stub = StubServer::start(move |method, path| {
        if method == "DELETE" && path == expected {
            StubReply::empty(204)
        } else {
            StubReply::empty(404)
        }
    })
    .await;
    let backend = HttpBackend::new(&stub.base_url())?;

    assert_eq!(backend.delete_sprint(id)?, Invalidation::All);

    drop(backend);
    stub.shutdown().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_graph_block_answered_with_a_bare_card_graph_response_returns_ok_with_invalidation_all(
) -> KanbanResult<()> {
    let (a, b) = (Uuid::new_v4(), Uuid::new_v4());
    let route = format!("/v1/cards/{a}/blocks");
    let expected = route.clone();
    let body = serde_json::to_string(&CardGraphResponse::from_graph(
        a,
        &DependencyGraph::default(),
    ))
    .unwrap();
    let stub = StubServer::start(move |method, path| {
        if method == "POST" && path == expected {
            StubReply::json(200, body.clone())
        } else {
            StubReply::empty(404)
        }
    })
    .await;
    let backend = HttpBackend::new(&stub.base_url())?;

    assert_eq!(backend.block(a, b, Severity::High)?, Invalidation::All);

    drop(backend);
    stub.shutdown().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_sprint_update_answered_with_a_bare_sprint_response_returns_ok_with_invalidation_all(
) -> KanbanResult<()> {
    let sprint = Sprint::new(Uuid::new_v4(), 1, None, Some("SPR"));
    let route = format!("/v1/sprints/{}", sprint.id);
    let expected = route.clone();
    let body = serde_json::to_string(&SprintResponse::new(&sprint, None)).unwrap();
    let stub = StubServer::start(move |method, path| {
        if method == "PATCH" && path == expected {
            StubReply::json(200, body.clone())
        } else {
            StubReply::empty(404)
        }
    })
    .await;
    let backend = HttpBackend::new(&stub.base_url())?;

    let (updated, invalidation) = backend.update_sprint(
        sprint.id,
        &SprintUpdate {
            name: Some("Renamed".into()),
            ..Default::default()
        },
    )?;

    assert_eq!(updated.id, sprint.id);
    assert_eq!(invalidation, Invalidation::All);

    drop(backend);
    stub.shutdown().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_card_move_against_a_server_without_the_route_returns_unsupported_naming_the_route(
) -> KanbanResult<()> {
    let stub = StubServer::start(|_, _| StubReply::empty(404)).await;
    let backend = HttpBackend::new(&stub.base_url())?;

    let err = backend
        .move_card(Uuid::new_v4(), Uuid::new_v4(), None)
        .unwrap_err();

    assert!(err.is_unsupported(), "got: {err:?}");
    let msg = err.to_string();
    assert!(msg.contains("POST /v1/cards/{id}/move"), "got: {msg}");
    assert!(msg.contains(&stub.base_url()), "got: {msg}");
    assert!(msg.contains("Upgrade the server"), "got: {msg}");

    drop(backend);
    stub.shutdown().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_sprint_activate_against_a_server_without_the_flat_route_returns_unsupported_naming_the_route(
) -> KanbanResult<()> {
    let stub = StubServer::start(|_, _| StubReply::empty(404)).await;
    let backend = HttpBackend::new(&stub.base_url())?;

    let err = backend.activate_sprint(Uuid::new_v4(), None).unwrap_err();

    assert!(err.is_unsupported(), "got: {err:?}");
    let msg = err.to_string();
    assert!(msg.contains("POST /v1/sprints/{id}/activate"), "got: {msg}");
    assert!(msg.contains(&stub.base_url()), "got: {msg}");
    assert!(msg.contains("Upgrade the server"), "got: {msg}");

    drop(backend);
    stub.shutdown().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_detach_children_answered_405_by_an_old_server_returns_unsupported_naming_the_route(
) -> KanbanResult<()> {
    let stub = StubServer::start(|_, _| StubReply::empty(405)).await;
    let backend = HttpBackend::new(&stub.base_url())?;
    let (a, b) = (Uuid::new_v4(), Uuid::new_v4());

    let err = backend.detach_children(a, &[b]).unwrap_err();

    assert!(err.is_unsupported(), "got: {err:?}");
    let msg = err.to_string();
    assert!(
        msg.contains("POST /v1/cards/{id}/children/detach"),
        "got: {msg}"
    );
    assert!(msg.contains(&stub.base_url()), "got: {msg}");
    assert!(msg.contains("Upgrade the server"), "got: {msg}");

    drop(backend);
    stub.shutdown().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_a_404_with_a_not_found_envelope_still_maps_to_the_servers_not_found_error(
) -> KanbanResult<()> {
    let stub = StubServer::start(|_, _| {
        StubReply::json(404, r#"{"code":"NOT_FOUND","message":"Card x not found"}"#)
    })
    .await;
    let backend = HttpBackend::new(&stub.base_url())?;
    let (a, b) = (Uuid::new_v4(), Uuid::new_v4());

    let err = backend.block(a, b, Severity::High).unwrap_err();

    assert!(!err.is_unsupported(), "got: {err:?}");
    assert!(err.to_string().contains("NOT_FOUND"), "got: {err}");

    drop(backend);
    stub.shutdown().await;
    Ok(())
}
