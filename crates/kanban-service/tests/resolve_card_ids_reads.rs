//! Requires the `test-helpers` feature; run with
//! `cargo test -p kanban-service --features test-helpers`.
#![cfg(feature = "test-helpers")]

use std::sync::Arc;

use kanban_backend_memory::InMemoryStore;
use kanban_domain::CreateCardOptions;
use kanban_service::test_helpers::FaultInjectingBackend;
use kanban_service::{AppConfig, KanbanBackend, KanbanContext, KanbanOperations};
use uuid::Uuid;

struct Seeded {
    backend: Arc<FaultInjectingBackend>,
    ctx: KanbanContext,
    c1: Uuid,
    c2: Uuid,
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

    let board = ctx.create_board("B".into(), Some("KAN".into())).unwrap();
    let col = ctx.create_column(board.id, "TODO".into(), None).unwrap();
    let c1 = ctx
        .create_card(board.id, col.id, "one".into(), CreateCardOptions::default())
        .unwrap();
    let c2 = ctx
        .create_card(board.id, col.id, "two".into(), CreateCardOptions::default())
        .unwrap();

    backend.clear_ops();

    Seeded {
        backend,
        ctx,
        c1: c1.id,
        c2: c2.id,
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn test_resolve_card_ids_with_only_uuid_inputs_reads_nothing_from_the_store() {
    let Seeded {
        backend,
        ctx,
        c1,
        c2,
    } = seeded().await;

    let resolved = ctx
        .resolve_card_ids(&[c1.to_string(), c2.to_string()])
        .unwrap();

    assert_eq!(resolved, vec![c1, c2]);
    assert_eq!(backend.op_count("list_all_cards"), 0);
    assert_eq!(backend.op_count("list_cards_by_prefix_and_number"), 0);
    assert_eq!(backend.op_count("list_cards_by_number"), 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_resolve_card_ids_performs_one_indexed_lookup_per_identifier() {
    let Seeded {
        backend,
        ctx,
        c1,
        c2,
    } = seeded().await;

    let resolved = ctx
        .resolve_card_ids(&["KAN-1".to_string(), "KAN-2".to_string()])
        .unwrap();

    assert_eq!(resolved, vec![c1, c2]);
    assert_eq!(backend.op_count("list_cards_by_prefix_and_number"), 2);
    assert_eq!(backend.op_count("list_all_cards"), 0);

    backend.clear_ops();
    let resolved = ctx.resolve_card_ids(&["1".to_string()]).unwrap();
    assert_eq!(resolved, vec![c1]);
    assert_eq!(backend.op_count("list_cards_by_number"), 1);
}
