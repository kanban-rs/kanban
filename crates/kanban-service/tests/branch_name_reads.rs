//! Requires the `test-helpers` feature; run with
//! `cargo test -p kanban-service --features test-helpers`.
#![cfg(feature = "test-helpers")]

use std::sync::Arc;

use kanban_backend_memory::InMemoryStore;
use kanban_domain::{CreateCardOptions, DataStore, FieldUpdate, SprintUpdate};
use kanban_service::test_helpers::FaultInjectingBackend;
use kanban_service::{AppConfig, KanbanBackend, KanbanContext, KanbanOperations};
use uuid::Uuid;

struct Seeded {
    backend: Arc<FaultInjectingBackend>,
    ctx: KanbanContext,
    board_prefix_card: Uuid,
}

async fn seeded_sprintless() -> Seeded {
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

    let board = ctx.create_board("B".into(), Some("BRD".into())).unwrap();
    let col = ctx.create_column(board.id, "TODO".into(), None).unwrap();
    let card = ctx
        .create_card(
            board.id,
            col.id,
            "sprintless card".into(),
            CreateCardOptions::default(),
        )
        .unwrap();

    backend.clear_ops();

    Seeded {
        backend,
        ctx,
        board_prefix_card: card.id,
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_card_branch_name_never_reads_the_whole_sprint_store() {
    let Seeded {
        backend,
        ctx,
        board_prefix_card,
    } = seeded_sprintless().await;

    let _ = ctx.get_card_branch_name(board_prefix_card).unwrap();

    assert_eq!(backend.op_count("list_all_sprints"), 0);
    assert_eq!(backend.op_count("list_sprints_by_board"), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_card_git_checkout_never_reads_the_whole_sprint_store() {
    let Seeded {
        backend,
        ctx,
        board_prefix_card,
    } = seeded_sprintless().await;

    let _ = ctx.get_card_git_checkout(board_prefix_card).unwrap();

    assert_eq!(backend.op_count("list_all_sprints"), 0);
    assert_eq!(backend.op_count("list_sprints_by_board"), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_branch_name_for_a_sprintless_card_is_unchanged_by_the_scoped_sprint_read() {
    let Seeded {
        ctx,
        board_prefix_card,
        ..
    } = seeded_sprintless().await;

    let branch = ctx.get_card_branch_name(board_prefix_card).unwrap();

    assert_eq!(branch, "BRD-1/sprintless-card");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_branch_name_for_an_unprefixed_card_in_a_sprint_uses_the_sprints_card_prefix() {
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

    let board = ctx.create_board("B".into(), Some("BRD".into())).unwrap();
    let col = ctx.create_column(board.id, "TODO".into(), None).unwrap();
    let sprint = ctx.create_sprint(board.id, None, None).unwrap();
    ctx.update_sprint(
        sprint.id,
        SprintUpdate {
            card_prefix: FieldUpdate::Set("SPR".into()),
            ..Default::default()
        },
    )
    .unwrap();
    let card = ctx
        .create_card(
            board.id,
            col.id,
            "legacy card".into(),
            CreateCardOptions::default(),
        )
        .unwrap();
    ctx.assign_card_to_sprint(card.id, sprint.id).unwrap();

    // Force the legacy unmigrated shape: an empty stored prefix, which
    // `Card::branch_name` uses to trigger derivation from the sprint slice.
    let mut legacy_card = ctx.get_card(card.id).unwrap().unwrap();
    legacy_card.prefix = String::new();
    backend.upsert_card(legacy_card).unwrap();

    backend.clear_ops();

    let branch = ctx.get_card_branch_name(card.id).unwrap();

    assert_eq!(branch, format!("SPR-{}/legacy-card", card.card_number));
}
