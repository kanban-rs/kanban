//! Whole-store (unscoped) `DataStore` card reads over HTTP.

use kanban_backend_http::HttpBackend;
use kanban_backend_memory::InMemoryStore;
use kanban_domain::{ArchivedCard, Card, DataStore};
use kanban_server::test_helpers::TestServer;
use kanban_service::{AppConfig, KanbanBackend, KanbanContext, KanbanOperations};
use std::sync::Arc;
use uuid::Uuid;

async fn blocking<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
    tokio::task::spawn_blocking(f).await.unwrap()
}

fn seed_board_with_card(ctx: &mut KanbanContext, name: &str, prefix: &str) -> (Uuid, Uuid) {
    let board = ctx
        .create_board(name.to_string(), Some(prefix.to_string()))
        .unwrap();
    let column = ctx
        .create_column(board.id, "Col".to_string(), None)
        .unwrap();
    let card = ctx
        .create_card(
            board.id,
            column.id,
            format!("{name} card"),
            Default::default(),
        )
        .unwrap();
    (board.id, card.id)
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_all_cards_over_http_aggregates_across_boards() {
    let mut first = Uuid::nil();
    let mut second = Uuid::nil();
    let server = TestServer::start_with(|ctx| {
        let (_b, c1) = seed_board_with_card(ctx, "Alpha", "ALP");
        let (_b2, c2) = seed_board_with_card(ctx, "Beta", "BET");
        first = c1;
        second = c2;
    })
    .await;
    let backend = HttpBackend::new(&server.base_url()).unwrap();

    let cards: Vec<Card> = blocking(move || backend.list_all_cards().unwrap()).await;

    let ids: Vec<Uuid> = cards.iter().map(|c| c.id).collect();
    assert_eq!(cards.len(), 2, "one card from each of the two boards");
    assert!(ids.contains(&first), "card from the first board is missing");
    assert!(
        ids.contains(&second),
        "card from the second board is missing"
    );

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_all_cards_over_http_includes_archived_board_descendants() {
    let mut live_card = Uuid::nil();
    let mut buried_card = Uuid::nil();
    let server = TestServer::start_with(|ctx| {
        let (_live_board, lc) = seed_board_with_card(ctx, "Live", "LIV");
        let (gone_board, bc) = seed_board_with_card(ctx, "Gone", "GON");
        ctx.archive_board(gone_board).unwrap();
        live_card = lc;
        buried_card = bc;
    })
    .await;
    let backend = HttpBackend::new(&server.base_url()).unwrap();

    let cards: Vec<Card> = blocking(move || backend.list_all_cards().unwrap()).await;

    let ids: Vec<Uuid> = cards.iter().map(|c| c.id).collect();
    assert!(ids.contains(&live_card));
    assert!(
        ids.contains(&buried_card),
        "a raw DataStore read must keep archived-board descendants -- board \
         archival is a marker and the cards stay in the flat collection; \
         excluding them is the service tier's job, not the backend's"
    );

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_all_cards_over_http_excludes_archived_cards() {
    let mut live_card = Uuid::nil();
    let mut archived_card = Uuid::nil();
    let server = TestServer::start_with(|ctx| {
        let board = ctx
            .create_board("Mixed".to_string(), Some("MIX".to_string()))
            .unwrap();
        let column = ctx
            .create_column(board.id, "Col".to_string(), None)
            .unwrap();
        let live = ctx
            .create_card(board.id, column.id, "Live".to_string(), Default::default())
            .unwrap();
        let gone = ctx
            .create_card(board.id, column.id, "Gone".to_string(), Default::default())
            .unwrap();
        ctx.archive_card(gone.id).unwrap();
        live_card = live.id;
        archived_card = gone.id;
    })
    .await;
    let backend = HttpBackend::new(&server.base_url()).unwrap();

    let cards: Vec<Card> = blocking(move || backend.list_all_cards().unwrap()).await;

    let ids: Vec<Uuid> = cards.iter().map(|c| c.id).collect();
    assert!(ids.contains(&live_card));
    assert!(
        !ids.contains(&archived_card),
        "list_all_cards is live-only at the card level, matching the memory backend"
    );

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_archived_cards_over_http_aggregates_across_boards() {
    let mut first = Uuid::nil();
    let mut second = Uuid::nil();
    let server = TestServer::start_with(|ctx| {
        let (_b, c1) = seed_board_with_card(ctx, "Alpha", "ALP");
        let (_b2, c2) = seed_board_with_card(ctx, "Beta", "BET");
        ctx.archive_card(c1).unwrap();
        ctx.archive_card(c2).unwrap();
        first = c1;
        second = c2;
    })
    .await;
    let backend = HttpBackend::new(&server.base_url()).unwrap();

    let markers: Vec<ArchivedCard> = blocking(move || backend.list_archived_cards().unwrap()).await;

    let ids: Vec<Uuid> = markers.iter().map(|m| m.entity_id).collect();
    assert_eq!(markers.len(), 2, "one marker from each of the two boards");
    assert!(ids.contains(&first));
    assert!(ids.contains(&second));

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_archived_cards_over_http_is_ordered_by_archived_at() {
    let server = TestServer::start_with(|ctx| {
        let (_b, c1) = seed_board_with_card(ctx, "Alpha", "ALP");
        let (_b2, c2) = seed_board_with_card(ctx, "Beta", "BET");
        ctx.archive_card(c1).unwrap();
        ctx.archive_card(c2).unwrap();
    })
    .await;
    let backend = HttpBackend::new(&server.base_url()).unwrap();

    let markers: Vec<ArchivedCard> = blocking(move || backend.list_archived_cards().unwrap()).await;

    let stamps: Vec<_> = markers.iter().map(|m| m.metadata.archived_at).collect();
    let mut sorted = stamps.clone();
    sorted.sort();
    assert_eq!(
        stamps, sorted,
        "markers must come back ascending by archived_at, matching the local backends"
    );

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_archived_cards_over_http_includes_a_marker_whose_board_is_gone() {
    let store = Arc::new(InMemoryStore::new());
    let backend: Arc<dyn KanbanBackend> = store.clone();

    let (card_id, board_id) = {
        let mut ctx = KanbanContext::open(backend.clone(), AppConfig::default())
            .await
            .unwrap();
        let board = ctx
            .create_board("Gone".to_string(), Some("GON".to_string()))
            .unwrap();
        let column = ctx
            .create_column(board.id, "Col".to_string(), None)
            .unwrap();
        let card = ctx
            .create_card(
                board.id,
                column.id,
                "Marker".to_string(),
                Default::default(),
            )
            .unwrap();
        ctx.archive_card(card.id).unwrap();
        (card.id, board.id)
    };
    DataStore::delete_board(store.as_ref(), board_id).unwrap();

    let local_ids: Vec<Uuid> = DataStore::list_archived_cards(store.as_ref())
        .unwrap()
        .iter()
        .map(|m| m.entity_id)
        .collect();
    assert!(
        local_ids.contains(&card_id),
        "sanity: the reference store must keep the orphaned marker visible"
    );

    let server = TestServer::start_on_backend(backend).await;
    let http_backend = HttpBackend::new(&server.base_url()).unwrap();

    let markers: Vec<ArchivedCard> =
        blocking(move || http_backend.list_archived_cards().unwrap()).await;

    let http_ids: Vec<Uuid> = markers.iter().map(|m| m.entity_id).collect();
    assert_eq!(
        http_ids, local_ids,
        "HTTP must be behaviourally equivalent to the local store for a marker \
         whose board is gone from both /v1/boards and /v1/archived-boards -- \
         board-fan-out cannot reach it, only the flat /v1/archived-cards route can"
    );

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_whole_store_reads_are_empty_not_erroring_on_a_bare_store() {
    let server = TestServer::start_with(|_ctx| {}).await;
    let backend = HttpBackend::new(&server.base_url()).unwrap();

    let (cards, markers) = blocking(move || {
        (
            backend.list_all_cards().unwrap(),
            backend.list_archived_cards().unwrap(),
        )
    })
    .await;

    assert!(cards.is_empty());
    assert!(markers.is_empty());

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_all_columns_over_http_aggregates_across_boards() {
    let server = TestServer::start_with(|ctx| {
        seed_board_with_card(ctx, "Alpha", "ALP");
        seed_board_with_card(ctx, "Beta", "BET");
    })
    .await;
    let backend = HttpBackend::new(&server.base_url()).unwrap();

    let columns = blocking(move || backend.list_all_columns().unwrap()).await;

    assert_eq!(columns.len(), 2, "one column from each of the two boards");

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_all_sprints_over_http_aggregates_across_boards() {
    let server = TestServer::start_with(|ctx| {
        let a = ctx
            .create_board("Alpha".to_string(), Some("ALP".to_string()))
            .unwrap();
        let b = ctx
            .create_board("Beta".to_string(), Some("BET".to_string()))
            .unwrap();
        ctx.create_sprint(a.id, None, None).unwrap();
        ctx.create_sprint(b.id, None, None).unwrap();
    })
    .await;
    let backend = HttpBackend::new(&server.base_url()).unwrap();

    let sprints = blocking(move || backend.list_all_sprints().unwrap()).await;

    assert_eq!(sprints.len(), 2, "one sprint from each of the two boards");

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_unscoped_live_card_listing_survives_an_archived_board() {
    let mut live_card = Uuid::nil();
    let mut buried_card = Uuid::nil();
    let server = TestServer::start_with(|ctx| {
        let (_live_board, lc) = seed_board_with_card(ctx, "Live", "LIV");
        let (gone_board, bc) = seed_board_with_card(ctx, "Gone", "GON");
        ctx.archive_board(gone_board).unwrap();
        live_card = lc;
        buried_card = bc;
    })
    .await;
    let backend: std::sync::Arc<dyn kanban_service::KanbanBackend> =
        std::sync::Arc::new(HttpBackend::new(&server.base_url()).unwrap());

    let cards = blocking(move || {
        let ctx = KanbanContext::open_deferred(backend, kanban_service::AppConfig::default());
        ctx.list_all_cards().unwrap()
    })
    .await;

    let ids: Vec<Uuid> = cards.iter().map(|c| c.id).collect();
    assert!(ids.contains(&live_card));
    assert!(
        !ids.contains(&buried_card),
        "the service tier excludes archived-board descendants; reaching that \
         branch at all requires list_all_columns, which used to decline"
    );

    server.shutdown().await;
}
