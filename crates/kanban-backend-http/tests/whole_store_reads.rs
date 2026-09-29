//! Whole-store (unscoped) `DataStore` card reads over HTTP.

use kanban_backend_http::HttpBackend;
use kanban_domain::{ArchivedCard, Card, DataStore};
use kanban_server::test_helpers::TestServer;
use kanban_service::{KanbanContext, KanbanOperations};
use uuid::Uuid;

fn requests_to(log: &kanban_server::test_helpers::RequestLog, path: &str) -> usize {
    log.lock()
        .unwrap()
        .iter()
        .filter(|(_, p)| p == path)
        .count()
}

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

#[tokio::test(flavor = "multi_thread")]
async fn test_list_all_sprints_over_http_fetches_an_archived_boards_body_once() {
    let mut live_id = Uuid::nil();
    let mut archived_id = Uuid::nil();
    let (server, log) = TestServer::start_recording(|ctx| {
        let live = ctx
            .create_board("Live".to_string(), Some("LIV".to_string()))
            .unwrap();
        ctx.create_sprint(live.id, None, None).unwrap();
        let gone = ctx
            .create_board("Gone".to_string(), Some("GON".to_string()))
            .unwrap();
        ctx.create_sprint(gone.id, None, None).unwrap();
        ctx.archive_board(gone.id).unwrap();
        live_id = live.id;
        archived_id = gone.id;
    })
    .await;
    let backend = HttpBackend::new(&server.base_url()).unwrap();
    let log_for_backend = log.clone();

    let sprints = blocking(move || backend.list_all_sprints().unwrap()).await;

    assert_eq!(sprints.len(), 2, "one sprint from each board");
    assert_eq!(
        requests_to(&log_for_backend, &format!("/v1/boards/{archived_id}")),
        1,
        "the archived board's body must be fetched exactly once for its sprint_names"
    );
    assert_eq!(
        requests_to(&log_for_backend, &format!("/v1/boards/{live_id}")),
        0,
        "a live board's body is already available from /v1/boards; refetching it per-id is the bug this card fixes"
    );

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_all_sprints_over_http_fetches_each_live_board_once() {
    let (server, log) = TestServer::start_recording(|ctx| {
        for (name, prefix) in [("Alpha", "ALP"), ("Beta", "BET"), ("Gamma", "GAM")] {
            let board = ctx
                .create_board(name.to_string(), Some(prefix.to_string()))
                .unwrap();
            ctx.create_sprint(board.id, None, None).unwrap();
        }
    })
    .await;
    let backend = HttpBackend::new(&server.base_url()).unwrap();

    let sprints = blocking(move || backend.list_all_sprints().unwrap()).await;

    assert_eq!(sprints.len(), 3, "one sprint from each of the three boards");
    assert_eq!(
        requests_to(&log, "/v1/boards"),
        1,
        "the live-board list must be fetched exactly once"
    );
    let per_board_gets = log
        .lock()
        .unwrap()
        .iter()
        .filter(|(_, p)| {
            p.starts_with("/v1/boards/") && !p.ends_with("/sprints") && p != "/v1/archived-boards"
        })
        .count();
    assert_eq!(
        per_board_gets, 0,
        "live boards' bodies are already in hand from /v1/boards; no per-id refetch should occur"
    );

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_all_sprints_over_http_still_names_sprints_on_live_and_archived_boards() {
    let mut live_id = Uuid::nil();
    let mut archived_id = Uuid::nil();
    let server = TestServer::start_with(|ctx| {
        let live = ctx
            .create_board("Live".to_string(), Some("LIV".to_string()))
            .unwrap();
        ctx.create_sprint(
            live.id,
            Some("LIV".to_string()),
            Some("live-one".to_string()),
        )
        .unwrap();
        ctx.create_sprint(
            live.id,
            Some("LIV".to_string()),
            Some("live-two".to_string()),
        )
        .unwrap();
        let gone = ctx
            .create_board("Gone".to_string(), Some("GON".to_string()))
            .unwrap();
        ctx.create_sprint(
            gone.id,
            Some("GON".to_string()),
            Some("gone-one".to_string()),
        )
        .unwrap();
        ctx.create_sprint(
            gone.id,
            Some("GON".to_string()),
            Some("gone-two".to_string()),
        )
        .unwrap();
        ctx.archive_board(gone.id).unwrap();
        live_id = live.id;
        archived_id = gone.id;
    })
    .await;
    let backend = HttpBackend::new(&server.base_url()).unwrap();
    let live_id_for_lookup = live_id;
    let archived_id_for_lookup = archived_id;

    let (sprints, live_board, archived_board) = blocking(move || {
        let sprints = backend.list_all_sprints().unwrap();
        let live_board = backend.get_board(live_id_for_lookup).unwrap().unwrap();
        let archived_board = backend.get_board(archived_id_for_lookup).unwrap().unwrap();
        (sprints, live_board, archived_board)
    })
    .await;

    let order: Vec<(Uuid, u32)> = sprints
        .iter()
        .map(|s| (s.board_id, s.sprint_number))
        .collect();
    assert_eq!(
        order,
        vec![
            (live_id, 1),
            (archived_id, 1),
            (live_id, 2),
            (archived_id, 2),
        ],
        "ties on sprint_number must break in live-then-archived iteration order, \
         and every sprint must resolve without erroring on either board kind"
    );

    let names: Vec<Option<&str>> = sprints
        .iter()
        .map(|s| {
            let board = if s.board_id == live_id {
                &live_board
            } else {
                &archived_board
            };
            s.get_name(board)
        })
        .collect();
    assert_eq!(
        names,
        vec![
            Some("live-one"),
            Some("gone-one"),
            Some("live-two"),
            Some("gone-two"),
        ],
        "the live arm must resolve names from the board's own sprint_names pool, \
         not an empty one"
    );

    server.shutdown().await;
}
