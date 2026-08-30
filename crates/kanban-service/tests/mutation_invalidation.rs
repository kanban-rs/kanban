use kanban_backend_memory::InMemoryStore;
use kanban_domain::{
    BoardUpdate, Card, CardUpdate, DataStore, EntityIds, Invalidation, KanbanOperations,
    KanbanResult, Model, Prefix, Sprint,
};
use kanban_service::{FetchPlan, FetchRound, KanbanContext, LoadedEntities};
use std::collections::HashSet;
use std::sync::Arc;

async fn make_ctx() -> KanbanContext {
    KanbanContext::open(
        Arc::new(InMemoryStore::new()),
        kanban_core::AppConfig::default(),
    )
    .await
    .unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn test_execute_returns_the_computed_invalidation() -> KanbanResult<()> {
    let mut ctx = make_ctx().await;
    let board = ctx.create_board("B".into(), None)?;

    use kanban_domain::commands::{BoardCommand, Command, UpdateBoard};
    let inv = ctx.execute(vec![Command::Board(BoardCommand::Update(UpdateBoard {
        board_id: board.id,
        updates: BoardUpdate {
            name: Some("Renamed".into()),
            ..Default::default()
        },
    }))])?;

    assert_eq!(inv, Invalidation::Entities(EntityIds::boards([board.id])));
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_a_card_only_batch_returns_an_invalidation_naming_no_boards() -> KanbanResult<()> {
    let mut ctx = make_ctx().await;
    let board = ctx.create_board("B".into(), None)?;
    let col = ctx.create_column(board.id, "C".into(), None)?;
    let card = ctx.create_card(board.id, col.id, "A".into(), Default::default())?;

    let (_card, inv) = ctx.update_card_impl(
        card.id,
        CardUpdate {
            title: Some("Renamed".into()),
            ..Default::default()
        },
    )?;

    match inv {
        Invalidation::Entities(ids) => {
            assert!(ids.cards.contains(&card.id));
            assert!(ids.boards.is_empty());
        }
        Invalidation::All => panic!("expected Entities, got All"),
    }
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_reload_returns_all() -> KanbanResult<()> {
    use kanban_persistence_json::{JsonDataStore, JsonFileStore};
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("board.json");
    let backend = JsonDataStore::new(Arc::new(JsonFileStore::new(&path)));
    let mut ctx = KanbanContext::open(Arc::new(backend), kanban_core::AppConfig::default()).await?;
    let inv = ctx.reload().await?;
    assert_eq!(inv, Invalidation::All);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_replace_backend_returns_all() -> KanbanResult<()> {
    let mut ctx = make_ctx().await;
    let inv = ctx.replace_backend(Arc::new(InMemoryStore::new()));
    assert_eq!(inv, Invalidation::All);
    Ok(())
}

fn seed_prefix(store: &InMemoryStore, prefix: &str) {
    store.upsert_prefix(Prefix::new(prefix)).unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn test_migrate_sprint_logs_invalidates_only_the_cards_it_rewrote() -> KanbanResult<()> {
    let store = InMemoryStore::new();
    seed_prefix(&store, "KAN");

    let now = chrono::Utc::now();
    let board = kanban_domain::Board::create(
        kanban_domain::NewBoard {
            name: "B".into(),
            description: None,
            sprint_prefix: None,
            card_prefix: Some("KAN".into()),
            task_sort_field: None,
            task_sort_order: None,
            sprint_duration_days: None,
            task_list_view: None,
        },
        uuid::Uuid::new_v4(),
        now,
    )?;
    store.upsert_board(board.clone())?;

    let column = kanban_domain::Column::create(
        kanban_domain::NewColumn {
            board_id: board.id,
            name: "C".into(),
            wip_limit: None,
            default_status: None,
        },
        uuid::Uuid::new_v4(),
        0,
        now,
    )?;
    store.upsert_column(column.clone())?;

    let sprint: Sprint = Sprint::new(board.id, 1, None, None::<String>);
    let sprint_id = sprint.id;
    store.upsert_sprint(sprint)?;

    let mut eligible = Card::create(
        kanban_domain::NewCard {
            column_id: column.id,
            title: "Eligible".into(),
            description: None,
            priority: kanban_domain::CardPriority::Medium,
            due_date: None,
            points: None,
            sprint_id: None,
        },
        uuid::Uuid::new_v4(),
        1,
        "KAN".into(),
        now,
        board.id,
    )?;
    eligible.sprint_id = Some(sprint_id);
    eligible.sprint_logs = Vec::new();
    let eligible_id = eligible.id;
    store.upsert_card(eligible)?;

    let untouched = Card::create(
        kanban_domain::NewCard {
            column_id: column.id,
            title: "Untouched".into(),
            description: None,
            priority: kanban_domain::CardPriority::Medium,
            due_date: None,
            points: None,
            sprint_id: None,
        },
        uuid::Uuid::new_v4(),
        2,
        "KAN".into(),
        now,
        board.id,
    )?;
    let untouched_id = untouched.id;
    store.upsert_card(untouched)?;

    let mut ctx = KanbanContext::open(Arc::new(store), kanban_core::AppConfig::default()).await?;
    let (n, inv) = ctx.migrate_sprint_logs()?;
    assert_eq!(n, 1);
    match inv {
        Some(Invalidation::Entities(ids)) => {
            assert_eq!(ids.cards, HashSet::from([eligible_id]));
            assert!(!ids.cards.contains(&untouched_id));
        }
        other => panic!("expected Some(Entities), got {other:?}"),
    }
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_a_no_op_sprint_log_migration_returns_no_invalidation() -> KanbanResult<()> {
    let mut ctx = make_ctx().await;
    let result = ctx.migrate_sprint_logs()?;
    assert_eq!(result, (0, None));
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_import_board_derives_its_invalidation_from_the_forward_batch() -> KanbanResult<()> {
    let mut ctx = make_ctx().await;
    let board = ctx.create_board("B".into(), None)?;
    let json = ctx.export_board(Some(board.id))?;

    let mut dest = make_ctx().await;
    let (_board, inv) = dest.import_board_impl(&json)?;
    assert_eq!(inv, Invalidation::All);
    Ok(())
}

struct BoardListPlan;
impl FetchPlan for BoardListPlan {
    fn next_round(&self, _loaded: &dyn LoadedEntities) -> FetchRound {
        FetchRound {
            board_list: true,
            ..Default::default()
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn test_resolve_through_the_context_reaches_the_backend() -> KanbanResult<()> {
    let mut ctx = make_ctx().await;
    let board = ctx.create_board("B".into(), None)?;

    let resolved = ctx.resolve(&BoardListPlan, &Model::default());
    let v = resolved
        .boards
        .all
        .loaded()
        .expect("expected Loaded, got something else");
    assert_eq!(v.len(), 1);
    assert_eq!(v[0].id, board.id);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_resolve_takes_a_shared_borrow() -> KanbanResult<()> {
    let ctx = make_ctx().await;
    let borrowed = &ctx;
    let _a = borrowed.resolve(&BoardListPlan, &Model::default());
    let _b = borrowed.resolve(&BoardListPlan, &Model::default());
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_clear_history_returns_no_invalidation_because_it_mutates_no_entity(
) -> KanbanResult<()> {
    let mut ctx = make_ctx().await;
    let board = ctx.create_board("B".into(), None)?;
    let col = ctx.create_column(board.id, "C".into(), None)?;
    let card = ctx.create_card(board.id, col.id, "A".into(), Default::default())?;

    let _unit: () = ctx.clear_history()?;

    assert!(ctx.get_board(board.id)?.is_some());
    assert!(ctx.get_column(col.id)?.is_some());
    assert!(ctx.get_card(card.id)?.is_some());
    Ok(())
}

/// Guards `Send`, which kanban-server's `Arc<tokio::sync::Mutex<KanbanContext>>`
/// needs. Would NOT catch a `RefCell` inside `KanbanContext`, because
/// `tokio::sync::Mutex<T>` is `Sync` for any `T: Send`.
#[test]
fn test_kanban_context_is_send_and_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<KanbanContext>();
}
