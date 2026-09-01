use kanban_domain::{CreateCardOptions, KanbanOperations, Model, NoProjections};
use kanban_service::{
    fetch_plan::{requestable, FetchPlan, FetchRound, LoadedEntities},
    AppConfig, KanbanContext, StoreManager,
};
use kanban_tui::tui_context::TuiContext;
use std::sync::Arc;
use tempfile::TempDir;

struct BoardListPlan;
impl FetchPlan for BoardListPlan {
    fn next_round(&self, loaded: &dyn LoadedEntities) -> FetchRound {
        FetchRound {
            board_list: requestable(loaded.board_list()),
            ..Default::default()
        }
    }
}

struct CardListPlan;
impl FetchPlan for CardListPlan {
    fn next_round(&self, loaded: &dyn LoadedEntities) -> FetchRound {
        FetchRound {
            card_list: requestable(loaded.card_list()),
            ..Default::default()
        }
    }
}

fn test_store_manager() -> StoreManager {
    let mut registry = kanban_persistence::StoreRegistry::new();
    let mut backends = kanban_backend::KanbanBackendRegistry::new();
    registry.register(Box::new(kanban_persistence_sqlite::SqliteStoreFactory));
    backends.register(Box::new(kanban_persistence_sqlite::SqliteBackendFactory));
    registry.register(Box::new(kanban_persistence_json::JsonStoreFactory));
    backends.register(Box::new(kanban_persistence_json::JsonBackendFactory));
    StoreManager::new(registry, backends)
}

#[test]
fn test_tui_context_sync_applies_a_resolved_pass_into_the_model() {
    let store = kanban_backend_memory::InMemoryStore::new();
    let board = kanban_domain::Board::new("Seeded", None::<String>);
    kanban_domain::DataStore::upsert_board(&store, board.clone()).unwrap();
    let ctx = KanbanContext::open_deferred(Arc::new(store), AppConfig::default());
    let (tui_ctx, _save_rx, _completion_rx) = TuiContext::new(ctx).unwrap();

    let mut model = Model::default();
    assert!(model.boards_state().is_not_loaded());

    tui_ctx.sync(&BoardListPlan, &mut model, &mut NoProjections);

    assert!(model.boards_state().is_loaded());
    assert!(model
        .boards_state()
        .loaded_or_empty()
        .iter()
        .any(|b| b.id == board.id));
}

#[tokio::test]
async fn test_tui_context_sync_does_not_queue_a_save() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("sync.json");
    let sm = test_store_manager();
    let backend = sm
        .make_backend(path.to_str().unwrap(), &AppConfig::default())
        .await
        .unwrap();
    let ctx = KanbanContext::open(backend, AppConfig::default())
        .await
        .unwrap();
    let (tui_ctx, save_rx, _completion_rx) = TuiContext::new(ctx).unwrap();
    let mut save_rx = save_rx.expect("json backend must provide a save channel");

    let mut model = Model::default();
    tui_ctx.sync(&BoardListPlan, &mut model, &mut NoProjections);

    assert!(
        save_rx.try_recv().is_err(),
        "sync must not queue a save flush"
    );
}

#[test]
fn test_tui_context_sync_invalidated_refetches_before_planning() {
    let store = kanban_backend_memory::InMemoryStore::new();
    let ctx = KanbanContext::open_deferred(Arc::new(store), AppConfig::default());
    let (mut tui_ctx, _save_rx, _completion_rx) = TuiContext::new(ctx).unwrap();

    let board = tui_ctx
        .create_board("Board".into(), Some("BRD".into()))
        .unwrap();
    let column = tui_ctx.create_column(board.id, "Col".into(), None).unwrap();
    let card = tui_ctx
        .create_card(
            board.id,
            column.id,
            "before".into(),
            CreateCardOptions::default(),
        )
        .unwrap();

    let mut model = Model::default();
    tui_ctx.sync(&CardListPlan, &mut model, &mut NoProjections);
    assert_eq!(
        model.card_by_id_state(card.id).loaded().unwrap().title,
        "before"
    );

    tui_ctx
        .update_card(
            card.id,
            kanban_domain::CardUpdate {
                title: Some("after".into()),
                ..Default::default()
            },
        )
        .unwrap();

    tui_ctx.sync_invalidated(
        kanban_domain::Invalidation::All,
        &CardListPlan,
        &mut model,
        &mut NoProjections,
    );

    assert_eq!(
        model.card_by_id_state(card.id).loaded().unwrap().title,
        "after"
    );
}
