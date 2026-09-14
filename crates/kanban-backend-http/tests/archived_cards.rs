use kanban_backend_http::HttpBackend;
use kanban_domain::{
    ArchivedCard, ArchivedFilter, CardListFilter, DataStore, Model, NoProjections,
};
use kanban_server::test_helpers::TestServer;
use kanban_service::fetch_plan::{requestable, FetchPlan, FetchRound, LoadedEntities};
use kanban_service::{AppConfig, KanbanContext, KanbanError, KanbanOperations};
use std::sync::Arc;
use uuid::Uuid;

async fn blocking<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
    tokio::task::spawn_blocking(f).await.unwrap()
}

fn seed_archived_card(ctx: &mut KanbanContext) -> (Uuid, Uuid, chrono::DateTime<chrono::Utc>) {
    let board = ctx
        .create_board("Archive Board".to_string(), Some("ARC".to_string()))
        .unwrap();
    let column = ctx
        .create_column(board.id, "Col".to_string(), None)
        .unwrap();
    let card = ctx
        .create_card(
            board.id,
            column.id,
            "Archive me".to_string(),
            Default::default(),
        )
        .unwrap();
    ctx.archive_card(card.id).unwrap();
    let seeded = ctx.list_archived_cards_by_board(board.id).unwrap();
    (board.id, card.id, seeded[0].metadata.archived_at)
}

fn seed_board_with_live_and_archived_card(
    ctx: &mut KanbanContext,
) -> (Uuid, Uuid, Uuid, chrono::DateTime<chrono::Utc>) {
    let board = ctx
        .create_board("Mixed Board".to_string(), Some("MIX".to_string()))
        .unwrap();
    let column = ctx
        .create_column(board.id, "Col".to_string(), None)
        .unwrap();
    let live = ctx
        .create_card(board.id, column.id, "Live".to_string(), Default::default())
        .unwrap();
    let archived = ctx
        .create_card(
            board.id,
            column.id,
            "Archived".to_string(),
            Default::default(),
        )
        .unwrap();
    ctx.archive_card(archived.id).unwrap();
    let seeded = ctx.list_archived_cards_by_board(board.id).unwrap();
    (
        board.id,
        live.id,
        archived.id,
        seeded[0].metadata.archived_at,
    )
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_archived_cards_by_board_round_trips_over_http() {
    let mut board_id = Uuid::nil();
    let mut card_id = Uuid::nil();
    let mut seeded_at = chrono::Utc::now();
    let server = TestServer::start_with(|ctx| {
        let (b, c, at) = seed_archived_card(ctx);
        board_id = b;
        card_id = c;
        seeded_at = at;
    })
    .await;
    let backend = HttpBackend::new(&server.base_url()).unwrap();

    let markers: Vec<ArchivedCard> =
        blocking(move || backend.list_archived_cards_by_board(board_id).unwrap()).await;

    assert_eq!(markers.len(), 1);
    assert_eq!(markers[0].entity_id, card_id);
    assert_eq!(markers[0].context.board_id, board_id);
    assert_eq!(markers[0].metadata.archived_at, seeded_at);

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_archived_cards_by_board_excludes_other_boards() {
    let mut board_a = Uuid::nil();
    let server = TestServer::start_with(|ctx| {
        let (b, _c, _at) = seed_archived_card(ctx);
        board_a = b;
        let other_board = ctx
            .create_board("Other Board".to_string(), Some("OTH".to_string()))
            .unwrap();
        let other_col = ctx
            .create_column(other_board.id, "Col".to_string(), None)
            .unwrap();
        let other_card = ctx
            .create_card(
                other_board.id,
                other_col.id,
                "Other archived".to_string(),
                Default::default(),
            )
            .unwrap();
        ctx.archive_card(other_card.id).unwrap();
    })
    .await;
    let backend = HttpBackend::new(&server.base_url()).unwrap();

    let markers: Vec<ArchivedCard> =
        blocking(move || backend.list_archived_cards_by_board(board_a).unwrap()).await;

    assert_eq!(markers.len(), 1);
    assert_eq!(markers[0].context.board_id, board_a);

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_archived_cards_by_missing_board_returns_empty() {
    let server = TestServer::start_with(|ctx| {
        seed_archived_card(ctx);
    })
    .await;
    let backend = HttpBackend::new(&server.base_url()).unwrap();

    let result: Result<Vec<ArchivedCard>, String> = blocking(move || {
        backend
            .list_archived_cards_by_board(Uuid::new_v4())
            .map_err(|e| e.to_string())
    })
    .await;

    let markers = result.expect("unknown board must resolve to an empty list, not an error");
    assert!(markers.is_empty());

    server.shutdown().await;
}

struct ArchivedCardsByBoardPlan {
    board_id: Uuid,
}

impl FetchPlan for ArchivedCardsByBoardPlan {
    fn next_round(&self, loaded: &dyn LoadedEntities) -> FetchRound {
        FetchRound {
            archived_cards_by_board: if requestable(loaded.archived_cards_of_board(self.board_id)) {
                vec![self.board_id]
            } else {
                Vec::new()
            },
            ..Default::default()
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn test_the_archived_card_tier_resolves_over_http() {
    let mut board_id = Uuid::nil();
    let mut card_id = Uuid::nil();
    let server = TestServer::start_with(|ctx| {
        let (b, c, _at) = seed_archived_card(ctx);
        board_id = b;
        card_id = c;
    })
    .await;
    let backend: Arc<dyn kanban_service::KanbanBackend> =
        Arc::new(HttpBackend::new(&server.base_url()).unwrap());

    let model = blocking(move || {
        let ctx = KanbanContext::open_deferred(backend, AppConfig::default());
        let mut model = Model::default();
        ctx.sync(
            &ArchivedCardsByBoardPlan { board_id },
            &mut model,
            &mut NoProjections,
        );
        model
    })
    .await;

    let state = model.board_archived_cards_state(board_id);
    let markers = state
        .loaded()
        .expect("archived-card tier should resolve to Loaded over HTTP");
    assert_eq!(markers.len(), 1);
    assert_eq!(markers[0].entity_id, card_id);

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_board_scoped_card_list_over_http_returns_live_and_archived() {
    let mut board_id = Uuid::nil();
    let mut live_id = Uuid::nil();
    let mut archived_id = Uuid::nil();
    let mut seeded_at = chrono::Utc::now();
    let server = TestServer::start_with(|ctx| {
        let (b, l, a, at) = seed_board_with_live_and_archived_card(ctx);
        board_id = b;
        live_id = l;
        archived_id = a;
        seeded_at = at;
    })
    .await;
    let backend: Arc<dyn kanban_service::KanbanBackend> =
        Arc::new(HttpBackend::new(&server.base_url()).unwrap());

    let pairs = blocking(move || {
        let ctx = KanbanContext::open_deferred(backend, AppConfig::default());
        ctx.list_cards_detailed(CardListFilter {
            board_id: Some(board_id),
            archived: ArchivedFilter::Include,
            ..Default::default()
        })
        .unwrap()
    })
    .await;

    assert_eq!(pairs.len(), 2);
    let live_pair = pairs.iter().find(|(c, _)| c.id == live_id).unwrap();
    let archived_pair = pairs.iter().find(|(c, _)| c.id == archived_id).unwrap();
    assert_eq!(live_pair.1, None);
    assert_eq!(archived_pair.1, Some(seeded_at));

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_unscoped_card_list_over_http_declines_under_list_archived_boards() {
    let server = TestServer::start_with(|ctx| {
        seed_archived_card(ctx);
    })
    .await;
    let backend: Arc<dyn kanban_service::KanbanBackend> =
        Arc::new(HttpBackend::new(&server.base_url()).unwrap());

    let result = blocking(move || {
        let ctx = KanbanContext::open_deferred(backend, AppConfig::default());
        ctx.list_cards_detailed(CardListFilter::default())
    })
    .await;

    match result {
        Err(KanbanError::Unsupported { operation }) => {
            assert_eq!(operation, "list_archived_boards");
        }
        other => panic!("expected Unsupported(\"list_archived_boards\"), got {other:?}"),
    }

    server.shutdown().await;
}
