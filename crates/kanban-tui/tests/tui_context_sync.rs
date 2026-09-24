use kanban_domain::{
    CreateCardOptions, KanbanOperations, Model, MutationOperations, NoProjections, UndoOperations,
};
use kanban_service::{
    fetch_plan::{requestable, FetchPlan, FetchRound, LoadedEntities},
    AppConfig, KanbanContext, StoreManager,
};
use kanban_tui::tui_context::TuiContext;
use std::sync::Arc;
use tempfile::TempDir;
use uuid::Uuid;

struct BoardListPlan;
impl FetchPlan for BoardListPlan {
    fn next_round(&self, loaded: &dyn LoadedEntities) -> FetchRound {
        FetchRound {
            board_list: requestable(loaded.board_list()),
            ..Default::default()
        }
    }
}

struct CardByIdPlan {
    id: Uuid,
}
impl FetchPlan for CardByIdPlan {
    fn next_round(&self, loaded: &dyn LoadedEntities) -> FetchRound {
        FetchRound {
            cards: if requestable(loaded.card(self.id)) {
                vec![self.id]
            } else {
                Vec::new()
            },
            ..Default::default()
        }
    }
}

fn test_store_manager() -> StoreManager {
    let mut registry = kanban_persistence::StoreRegistry::new();
    let mut backends = kanban_backend::KanbanBackendRegistry::new();
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
    tui_ctx.sync(
        &CardByIdPlan { id: card.id },
        &mut model,
        &mut NoProjections,
    );
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
        &CardByIdPlan { id: card.id },
        &mut model,
        &mut NoProjections,
    );

    assert_eq!(
        model.card_by_id_state(card.id).loaded().unwrap().title,
        "after"
    );
}

#[test]
fn test_tui_context_has_no_whole_store_snapshot_pass_throughs() {
    let src = include_str!("../src/tui_context.rs");
    assert!(
        !src.contains("pub fn snapshot(&self)"),
        "TuiContext::snapshot must be deleted; callers should use kanban_service::read_full_snapshot(ctx.data_store())"
    );
    assert!(
        !src.contains("pub fn apply_snapshot(&mut self,"),
        "TuiContext::apply_snapshot must be deleted; callers should use kanban_service::write_full_snapshot(ctx.data_store(), snapshot)"
    );
}

#[test]
fn test_tui_context_has_no_inherent_impl_forwarders() {
    let src = include_str!("../src/tui_context.rs");
    for name in [
        "pub fn update_card_impl",
        "pub fn update_cards_impl",
        "pub fn move_card_impl",
        "pub fn carry_over_sprint_cards_impl",
        "pub fn attach_children_impl",
        "pub fn detach_children_impl",
    ] {
        assert!(
            !src.contains(name),
            "TuiContext must not carry a hand-written inherent forwarder for {name}; it should come from impl MutationOperations for TuiContext instead"
        );
    }
}

#[tokio::test]
async fn test_tui_context_mutation_operations_queues_a_flush() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("mutate.json");
    let sm = test_store_manager();
    let backend = sm
        .make_backend(path.to_str().unwrap(), &AppConfig::default())
        .await
        .unwrap();
    let ctx = KanbanContext::open(backend, AppConfig::default())
        .await
        .unwrap();
    let (mut tui_ctx, save_rx, _completion_rx) = TuiContext::new(ctx).unwrap();
    let mut save_rx = save_rx.expect("json backend must provide a save channel");

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

    while save_rx.try_recv().is_ok() {}

    let (updated, _invalidation) = MutationOperations::update_card_impl(
        &mut tui_ctx,
        card.id,
        kanban_domain::CardUpdate {
            title: Some("after".into()),
            ..Default::default()
        },
    )
    .unwrap();

    assert_eq!(updated.title, "after");
    assert!(
        save_rx.try_recv().is_ok(),
        "MutationOperations::update_card_impl must queue a save flush"
    );
}

#[test]
fn test_tui_context_has_no_inherent_undo_forwarders() {
    let src = include_str!("../src/tui_context.rs");
    for name in [
        "pub fn undo(",
        "pub fn redo(",
        "pub fn can_undo(",
        "pub fn can_redo(",
    ] {
        assert!(
            !src.contains(name),
            "TuiContext must not carry a hand-written inherent forwarder for {name}; it should come from impl UndoOperations for TuiContext instead"
        );
    }
}

#[tokio::test]
async fn test_tui_undo_returns_the_invalidation_and_queues_flush() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("undo.json");
    let sm = test_store_manager();
    let backend = sm
        .make_backend(path.to_str().unwrap(), &AppConfig::default())
        .await
        .unwrap();
    let ctx = KanbanContext::open(backend, AppConfig::default())
        .await
        .unwrap();
    let (mut tui_ctx, save_rx, _completion_rx) = TuiContext::new(ctx).unwrap();
    let mut save_rx = save_rx.expect("json backend must provide a save channel");

    let board = tui_ctx
        .create_board("Board".into(), Some("BRD".into()))
        .unwrap();
    let column = tui_ctx.create_column(board.id, "Col".into(), None).unwrap();
    tui_ctx
        .create_card(
            board.id,
            column.id,
            "Card".into(),
            CreateCardOptions::default(),
        )
        .unwrap();

    while save_rx.try_recv().is_ok() {}

    let inv = UndoOperations::undo(&mut tui_ctx).unwrap();
    assert!(inv.is_some(), "undo of a real batch must return Some");
    assert!(
        save_rx.try_recv().is_ok(),
        "an applied undo must queue a save flush"
    );

    while UndoOperations::can_undo(&tui_ctx) {
        UndoOperations::undo(&mut tui_ctx).unwrap();
    }
    while save_rx.try_recv().is_ok() {}

    let inv = UndoOperations::undo(&mut tui_ctx).unwrap();
    assert!(inv.is_none(), "undo on an empty stack must return None");
    assert!(
        save_rx.try_recv().is_err(),
        "a no-op undo must not queue a save flush"
    );
}
