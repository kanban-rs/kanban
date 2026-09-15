use kanban_backend_http::HttpBackend;
use kanban_domain::{DataStore, KanbanOperations};
use kanban_server::test_helpers::TestServer;
use kanban_service::{AppConfig, KanbanContext};
use std::sync::Arc;
use uuid::Uuid;

async fn ctx_over(server: &TestServer) -> KanbanContext {
    let backend = Arc::new(HttpBackend::new(&server.base_url()).unwrap());
    KanbanContext::open(backend, AppConfig::default())
        .await
        .unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn test_migrate_sprint_logs_over_http_declines_and_mutates_nothing() {
    let mut card_id = Uuid::nil();
    let server = TestServer::start_with(|ctx| {
        let board = ctx.create_board("B".into(), Some("TST".into())).unwrap();
        let col = ctx.create_column(board.id, "Col".into(), None).unwrap();
        let sprint = ctx
            .create_sprint(board.id, None, Some("Alpha".into()))
            .unwrap();
        let mut card = kanban_domain::Card::new(board.id, col.id, "Card", 0);
        card.sprint_id = Some(sprint.id);
        card_id = card.id;
        ctx.data_store().upsert_card(card).unwrap();
    })
    .await;
    let mut ctx = ctx_over(&server).await;

    let err = ctx.migrate_sprint_logs().unwrap_err();

    assert!(
        err.is_unsupported(),
        "expected a safe-no-op signal, got {err:?}"
    );
    let backend = HttpBackend::new(&server.base_url()).unwrap();
    let card = tokio::task::spawn_blocking(move || backend.get_card(card_id).unwrap())
        .await
        .unwrap()
        .unwrap();
    assert!(
        card.sprint_logs.is_empty(),
        "migration must not have written anything server-side"
    );

    server.shutdown().await;
}
