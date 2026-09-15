//! Requires the `test-helpers` feature; run with
//! `cargo test -p kanban-service --features test-helpers`.
#![cfg(feature = "test-helpers")]

use std::sync::Arc;

use kanban_backend_memory::InMemoryStore;
use kanban_domain::{ArchivedFilter, CardListFilter, CreateCardOptions};
use kanban_service::test_helpers::FaultInjectingBackend;
use kanban_service::{AppConfig, KanbanBackend, KanbanContext, KanbanOperations};
use uuid::Uuid;

struct Seeded {
    backend: Arc<FaultInjectingBackend>,
    ctx: KanbanContext,
    b1: Uuid,
    live: Uuid,
    archived: Uuid,
}

async fn seeded() -> Seeded {
    let inner = InMemoryStore::new();
    let backend = Arc::new(FaultInjectingBackend::new(
        Arc::new(inner) as Arc<dyn KanbanBackend>
    ));
    let mut ctx = KanbanContext::open(
        backend.clone() as Arc<dyn KanbanBackend>,
        AppConfig::default(),
    )
    .await
    .unwrap();

    let b1 = ctx.create_board("B1".into(), Some("B1".into())).unwrap();
    let col1 = ctx.create_column(b1.id, "Col".into(), None).unwrap();
    let live = ctx
        .create_card(b1.id, col1.id, "Live".into(), CreateCardOptions::default())
        .unwrap();
    let archived = ctx
        .create_card(
            b1.id,
            col1.id,
            "Archived".into(),
            CreateCardOptions::default(),
        )
        .unwrap();
    ctx.archive_card(archived.id).unwrap();

    let b2 = ctx.create_board("B2".into(), Some("B2".into())).unwrap();
    let col2 = ctx.create_column(b2.id, "Col".into(), None).unwrap();
    let b2_archived = ctx
        .create_card(
            b2.id,
            col2.id,
            "B2 archived".into(),
            CreateCardOptions::default(),
        )
        .unwrap();
    ctx.archive_card(b2_archived.id).unwrap();

    backend.clear_ops();

    Seeded {
        backend,
        ctx,
        b1: b1.id,
        live: live.id,
        archived: archived.id,
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn test_board_scoped_list_cards_detailed_never_reads_the_global_marker_collection() {
    let Seeded {
        backend,
        ctx,
        b1,
        live,
        archived,
    } = seeded().await;

    let pairs = ctx
        .list_cards_detailed(CardListFilter {
            board_id: Some(b1),
            archived: ArchivedFilter::Include,
            ..Default::default()
        })
        .unwrap();

    assert_eq!(
        backend.op_count("list_archived_cards"),
        0,
        "a board-scoped card listing must never read the workspace-global archival marker collection"
    );
    assert!(backend
        .ops()
        .iter()
        .any(|op| op.method == "list_archived_cards_by_board" && op.ids == vec![b1]));

    assert_eq!(pairs.len(), 2);
    let live_pair = pairs.iter().find(|(c, _)| c.id == live).unwrap();
    let archived_pair = pairs.iter().find(|(c, _)| c.id == archived).unwrap();
    assert_eq!(live_pair.1, None);
    assert!(archived_pair.1.is_some());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_board_scoped_list_cards_detailed_succeeds_when_global_marker_read_is_unsupported() {
    let Seeded {
        backend,
        ctx,
        b1,
        live,
        archived,
    } = seeded().await;

    let expected_at = backend
        .inner()
        .list_archived_cards_by_board(b1)
        .unwrap()
        .into_iter()
        .find(|ac| ac.entity_id == archived)
        .unwrap()
        .metadata
        .archived_at;

    backend.fail("list_archived_cards");

    let pairs = ctx
        .list_cards_detailed(CardListFilter {
            board_id: Some(b1),
            archived: ArchivedFilter::Include,
            ..Default::default()
        })
        .unwrap();

    assert_eq!(pairs.len(), 2);
    let live_pair = pairs.iter().find(|(c, _)| c.id == live).unwrap();
    let archived_pair = pairs.iter().find(|(c, _)| c.id == archived).unwrap();
    assert_eq!(live_pair.1, None);
    assert_eq!(archived_pair.1, Some(expected_at));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_unscoped_live_only_list_cards_never_reads_the_global_marker_collection() {
    let Seeded { backend, ctx, .. } = seeded().await;

    let _ = ctx.list_cards_detailed(CardListFilter::default()).unwrap();

    assert_eq!(
        backend.op_count("list_archived_cards"),
        0,
        "a LiveOnly result never contains an archived card, so the global archival marker \
         read is unnecessary work (and unblocks this path against backends, like HTTP, that \
         only support board-scoped archived reads)"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_unscoped_include_list_cards_still_reads_the_global_marker_collection() {
    let Seeded { backend, ctx, .. } = seeded().await;

    let _ = ctx
        .list_cards_detailed(CardListFilter {
            archived: ArchivedFilter::Include,
            ..Default::default()
        })
        .unwrap();

    assert!(
        backend.op_count("list_archived_cards") >= 1,
        "an unscoped Include/ArchivedOnly listing has no board to narrow by, so it must still \
         fall back to the global read"
    );
}
