use kanban_backend_memory::InMemoryStore;
use kanban_domain::{KanbanOperations, KanbanResult};
use kanban_persistence_json::{JsonDataStore, JsonFileStore};
use kanban_persistence_sqlite::SqliteBackend;
use kanban_service::{AppConfig, KanbanBackend, KanbanContext};
use std::sync::Arc;
use tempfile::tempdir;

async fn open_json_ctx(path: &std::path::Path) -> KanbanContext {
    let backend: Arc<dyn KanbanBackend> =
        Arc::new(JsonDataStore::new(Arc::new(JsonFileStore::new(path))));
    KanbanContext::open(backend, AppConfig::default())
        .await
        .unwrap()
}

async fn open_sqlite_ctx(path: &std::path::Path) -> KanbanContext {
    let backend: Arc<dyn KanbanBackend> =
        Arc::new(SqliteBackend::open(path.to_str().unwrap()).await.unwrap());
    KanbanContext::open(backend, AppConfig::default())
        .await
        .unwrap()
}

async fn open_memory_ctx() -> KanbanContext {
    let backend: Arc<dyn KanbanBackend> = Arc::new(InMemoryStore::new());
    KanbanContext::open(backend, AppConfig::default())
        .await
        .unwrap()
}

fn assert_sprint_bound(ctx: &KanbanContext, card_id: uuid::Uuid, sprint_id: uuid::Uuid) {
    let card = ctx
        .get_card(card_id)
        .unwrap()
        .expect("card must survive import");
    assert_eq!(card.sprint_id, Some(sprint_id));
    assert!(
        ctx.get_sprint(sprint_id).unwrap().is_some(),
        "sprint must survive import"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_sprint_bound_card_import_round_trips_on_json() -> KanbanResult<()> {
    let dir = tempdir().unwrap();
    let source_path = dir.path().join("source.json");
    let dest_path = dir.path().join("dest.json");

    let (card_id, sprint_id, json) = {
        let mut ctx = open_json_ctx(&source_path).await;
        let board = ctx.create_board("B".into(), None)?;
        let col = ctx.create_column(board.id, "Todo".into(), None)?;
        let card = ctx.create_card(board.id, col.id, "C".into(), Default::default())?;
        let sprint = ctx.create_sprint(board.id, Some("S".into()), None)?;
        ctx.assign_card_to_sprint(card.id, sprint.id)?;
        ctx.save().await?;
        let json = ctx.export_board(Some(board.id))?;
        (card.id, sprint.id, json)
    };

    {
        let mut dest = open_json_ctx(&dest_path).await;
        dest.import_board(&json)?;
        dest.save().await?;
    }

    let dest = open_json_ctx(&dest_path).await;
    assert_sprint_bound(&dest, card_id, sprint_id);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_sprint_bound_card_import_round_trips_on_sqlite() -> KanbanResult<()> {
    let dir = tempdir().unwrap();
    let source_path = dir.path().join("source.sqlite");
    let dest_path = dir.path().join("dest.sqlite");

    let (card_id, sprint_id, json) = {
        let mut ctx = open_sqlite_ctx(&source_path).await;
        let board = ctx.create_board("B".into(), None)?;
        let col = ctx.create_column(board.id, "Todo".into(), None)?;
        let card = ctx.create_card(board.id, col.id, "C".into(), Default::default())?;
        let sprint = ctx.create_sprint(board.id, Some("S".into()), None)?;
        ctx.assign_card_to_sprint(card.id, sprint.id)?;
        ctx.save().await?;
        let json = ctx.export_board(Some(board.id))?;
        (card.id, sprint.id, json)
    };

    {
        let mut dest = open_sqlite_ctx(&dest_path).await;
        dest.import_board(&json)?;
        dest.save().await?;
    }

    let dest = open_sqlite_ctx(&dest_path).await;
    assert_sprint_bound(&dest, card_id, sprint_id);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_sprint_bound_card_import_round_trips_in_memory() -> KanbanResult<()> {
    let mut ctx = open_memory_ctx().await;
    let board = ctx.create_board("B".into(), None)?;
    let col = ctx.create_column(board.id, "Todo".into(), None)?;
    let card = ctx.create_card(board.id, col.id, "C".into(), Default::default())?;
    let sprint = ctx.create_sprint(board.id, Some("S".into()), None)?;
    ctx.assign_card_to_sprint(card.id, sprint.id)?;
    let json = ctx.export_board(Some(board.id))?;

    let mut dest = open_memory_ctx().await;
    dest.import_board(&json)?;

    assert_sprint_bound(&dest, card.id, sprint.id);
    Ok(())
}
