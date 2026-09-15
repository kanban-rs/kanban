use std::sync::Arc;

use kanban_backend_memory::InMemoryStore;
use kanban_core::EdgeBase;
use kanban_domain::{DependencyGraph, KanbanOperations, Model, NoProjections, SpawnsEdge};
use kanban_persistence_json::{JsonDataStore, JsonFileStore};
use kanban_service::{
    requestable, AppConfig, FetchPlan, FetchRound, KanbanBackend, KanbanContext, LoadedEntities,
};
use tempfile::tempdir;
use uuid::Uuid;

struct ArchivedBodiesPlan {
    card_bodies: bool,
    board_bodies: bool,
}

impl FetchPlan for ArchivedBodiesPlan {
    fn next_round(&self, loaded: &dyn LoadedEntities) -> FetchRound {
        let mut round = FetchRound {
            board_list: self.board_bodies && requestable(loaded.board_list()),
            ..Default::default()
        };

        if self.card_bodies {
            if requestable(loaded.archived_card_list()) {
                round.archived_card_list = true;
            } else if let Some(markers) = loaded.loaded_archived_card_markers() {
                let mut ids: Vec<Uuid> = markers.iter().map(|m| m.entity_id).collect();
                ids.sort_unstable();
                round.cards.extend(ids);
            }
        }

        if self.board_bodies {
            if requestable(loaded.archived_board_list()) {
                round.archived_board_list = true;
            } else if let Some(markers) = loaded.loaded_archived_board_markers() {
                let mut ids: Vec<Uuid> = markers
                    .iter()
                    .map(|m| m.entity_id)
                    .filter(|&id| requestable(loaded.board_in_collection(id)))
                    .collect();
                ids.sort_unstable();
                round.boards.extend(ids);
            }
        }

        round
    }
}

fn cards_plan() -> ArchivedBodiesPlan {
    ArchivedBodiesPlan {
        card_bodies: true,
        board_bodies: false,
    }
}

fn boards_plan() -> ArchivedBodiesPlan {
    ArchivedBodiesPlan {
        card_bodies: false,
        board_bodies: true,
    }
}

async fn open_sqlite_context(locator: &str, config: AppConfig) -> KanbanContext {
    let mut config = config;
    let mut stores = kanban_persistence::StoreRegistry::new();
    let mut backends = kanban_backend::KanbanBackendRegistry::new();
    backends.register(Box::new(kanban_persistence_sqlite::SqliteBackendFactory));
    stores.register(Box::new(kanban_persistence_json::JsonStoreFactory));
    backends.register(Box::new(kanban_persistence_json::JsonBackendFactory));
    let sm = kanban_service::StoreManager::new(stores, backends);
    sm.sync_backend_with_file(locator, &mut config);
    let backend = sm.make_backend(locator, &config).await.unwrap();
    KanbanContext::open(backend, config).await.unwrap()
}

async fn json_context() -> KanbanContext {
    let dir = tempdir().unwrap();
    let path = dir.path().join("test.json");
    let backend: Arc<dyn KanbanBackend> =
        Arc::new(JsonDataStore::new(Arc::new(JsonFileStore::new(&path))));
    KanbanContext::open(backend, AppConfig::default())
        .await
        .unwrap()
}

fn in_memory_context() -> KanbanContext {
    KanbanContext::open_deferred(Arc::new(InMemoryStore::new()), AppConfig::default())
}

async fn sqlite_context() -> KanbanContext {
    let dir = tempdir().unwrap();
    let path = dir.path().join("test.sqlite3");
    open_sqlite_context(path.to_str().unwrap(), AppConfig::default()).await
}

fn assert_an_archived_card_body_is_fetchable_without_a_snapshot(ctx: &mut KanbanContext) {
    let board = ctx.create_board("Board".into(), None).unwrap();
    let column = ctx.create_column(board.id, "Col".into(), None).unwrap();
    let card = ctx
        .create_card(
            board.id,
            column.id,
            "task".into(),
            kanban_domain::CreateCardOptions::default(),
        )
        .unwrap();
    let _ = ctx.archive_card_impl(card.id).unwrap();

    let mut model = Model::default();
    ctx.sync(&cards_plan(), &mut model, &mut NoProjections);
    ctx.sync(&cards_plan(), &mut model, &mut NoProjections);

    assert!(model.card_by_id_state(card.id).loaded().is_some());
    assert!(model.archived_card_ids().contains(&card.id));
}

fn assert_an_archived_board_head_is_fetchable_without_a_snapshot(ctx: &mut KanbanContext) {
    let board = ctx.create_board("Board".into(), None).unwrap();
    let column = ctx.create_column(board.id, "Col".into(), None).unwrap();
    let _card = ctx
        .create_card(
            board.id,
            column.id,
            "task".into(),
            kanban_domain::CreateCardOptions::default(),
        )
        .unwrap();
    let _sprint = ctx
        .create_sprint(board.id, Some("SPR".into()), Some("Sprint".into()))
        .unwrap();
    let _ = ctx.archive_board_impl(board.id).unwrap();

    let mut model = Model::default();
    ctx.sync(&boards_plan(), &mut model, &mut NoProjections);
    ctx.sync(&boards_plan(), &mut model, &mut NoProjections);

    assert!(model
        .boards_state()
        .loaded_or_empty()
        .iter()
        .any(|b| b.id == board.id));
    assert!(model.archived_board_ids().contains(&board.id));
}

fn assert_a_card_mutation_keeps_every_other_archived_body_present(ctx: &mut KanbanContext) {
    let board = ctx.create_board("Board".into(), None).unwrap();
    let column = ctx.create_column(board.id, "Col".into(), None).unwrap();
    let a = ctx
        .create_card(
            board.id,
            column.id,
            "a".into(),
            kanban_domain::CreateCardOptions::default(),
        )
        .unwrap();
    let b = ctx
        .create_card(
            board.id,
            column.id,
            "b".into(),
            kanban_domain::CreateCardOptions::default(),
        )
        .unwrap();
    let live = ctx
        .create_card(
            board.id,
            column.id,
            "live".into(),
            kanban_domain::CreateCardOptions::default(),
        )
        .unwrap();
    let _ = ctx.archive_card_impl(a.id).unwrap();
    let _ = ctx.archive_card_impl(b.id).unwrap();

    let mut model = Model::default();
    ctx.sync(&cards_plan(), &mut model, &mut NoProjections);
    ctx.sync(&cards_plan(), &mut model, &mut NoProjections);
    assert!(model.archived_card_ids().contains(&a.id));
    assert!(model.archived_card_ids().contains(&b.id));

    let (_updated, inv) = ctx
        .update_card_impl(
            live.id,
            kanban_service::CardUpdate {
                title: Some("live-renamed".into()),
                ..Default::default()
            },
        )
        .unwrap();

    ctx.resync_invalidated(inv, &cards_plan(), &mut model, &mut NoProjections);
    ctx.sync(&cards_plan(), &mut model, &mut NoProjections);

    assert!(model.card_by_id_state(a.id).loaded().is_some());
    assert!(model.card_by_id_state(b.id).loaded().is_some());
}

fn assert_archive_then_restore_is_the_identity_over_the_card_and_its_partition(
    ctx: &mut KanbanContext,
) {
    let board = ctx.create_board("Board".into(), None).unwrap();
    let column = ctx.create_column(board.id, "Col".into(), None).unwrap();
    let card = ctx
        .create_card(
            board.id,
            column.id,
            "task".into(),
            kanban_domain::CreateCardOptions::default(),
        )
        .unwrap();

    let (_card, inv) = ctx.archive_card_impl(card.id).unwrap();
    let mut model = Model::default();
    ctx.resync_invalidated(inv, &cards_plan(), &mut model, &mut NoProjections);
    ctx.sync(&cards_plan(), &mut model, &mut NoProjections);

    assert!(model.card_by_id_state(card.id).loaded().is_some());
    assert!(model.archived_card_ids().contains(&card.id));

    let (_card, inv) = ctx.restore_card_impl(card.id, None).unwrap();
    ctx.resync_invalidated(inv, &cards_plan(), &mut model, &mut NoProjections);
    ctx.sync(&cards_plan(), &mut model, &mut NoProjections);

    let restored = (*model
        .card_by_id_state(card.id)
        .loaded()
        .expect("card resolvable again after restore"))
    .clone();
    assert_eq!(restored.title, "task");
    assert_eq!(restored.column_id, column.id);
    assert!(!model.archived_card_ids().contains(&card.id));
}

fn assert_archive_then_restore_is_the_identity_over_the_board_and_its_subtree(
    ctx: &mut KanbanContext,
) {
    let board = ctx.create_board("Board".into(), None).unwrap();
    let column = ctx.create_column(board.id, "Col".into(), None).unwrap();
    let card_a = ctx
        .create_card(
            board.id,
            column.id,
            "a".into(),
            kanban_domain::CreateCardOptions::default(),
        )
        .unwrap();
    let card_b = ctx
        .create_card(
            board.id,
            column.id,
            "b".into(),
            kanban_domain::CreateCardOptions::default(),
        )
        .unwrap();
    let sprint = ctx
        .create_sprint(board.id, Some("SPR".into()), Some("Sprint".into()))
        .unwrap();

    let existing = ctx.data_store().get_graph().unwrap();
    let mut spawns: Vec<SpawnsEdge> = existing.spawns_edges().to_vec();
    spawns.push(SpawnsEdge {
        base: EdgeBase::new(card_a.id, card_b.id),
    });
    let graph = DependencyGraph::from_validated_per_kind_edges(
        spawns,
        existing.blocks_edges().to_vec(),
        existing.relates_edges().to_vec(),
    )
    .unwrap();
    ctx.data_store().set_graph(graph).unwrap();

    let inv = ctx.archive_board_impl(board.id).unwrap();
    let mut model = Model::default();
    ctx.resync_invalidated(inv, &boards_plan(), &mut model, &mut NoProjections);
    ctx.sync(&boards_plan(), &mut model, &mut NoProjections);

    assert!(model.archived_board_ids().contains(&board.id));

    let inv = ctx.restore_board_impl(board.id).unwrap();
    ctx.resync_invalidated(inv, &boards_plan(), &mut model, &mut NoProjections);
    ctx.sync(&boards_plan(), &mut model, &mut NoProjections);

    assert!(!model.archived_board_ids().contains(&board.id));
    let restored_board = model
        .boards_state()
        .loaded_or_empty()
        .iter()
        .find(|b| b.id == board.id)
        .unwrap();
    assert_eq!(restored_board.name, "Board");

    assert!(ctx
        .data_store()
        .list_columns_by_board(board.id)
        .unwrap()
        .iter()
        .any(|c| c.id == column.id));
    assert!(ctx
        .data_store()
        .list_cards_by_column(column.id)
        .unwrap()
        .iter()
        .any(|c| c.id == card_a.id));
    assert!(ctx
        .data_store()
        .list_sprints_by_board(board.id)
        .unwrap()
        .iter()
        .any(|s| s.id == sprint.id));
    let graph = ctx.data_store().get_graph().unwrap();
    assert!(graph
        .spawns_edges()
        .iter()
        .any(|e| e.base.source == card_a.id && e.base.target == card_b.id));
}

macro_rules! backend_parity_tests {
    ($name:ident, $assert_fn:ident) => {
        mod $name {
            use super::*;

            #[test]
            fn in_memory() {
                let mut ctx = in_memory_context();
                $assert_fn(&mut ctx);
            }

            #[tokio::test(flavor = "multi_thread")]
            async fn json() {
                let mut ctx = json_context().await;
                $assert_fn(&mut ctx);
            }

            #[tokio::test(flavor = "multi_thread")]
            async fn sqlite() {
                let mut ctx = sqlite_context().await;
                $assert_fn(&mut ctx);
            }
        }
    };
}

backend_parity_tests!(
    archived_card_body_fetchable,
    assert_an_archived_card_body_is_fetchable_without_a_snapshot
);
backend_parity_tests!(
    archived_board_head_fetchable,
    assert_an_archived_board_head_is_fetchable_without_a_snapshot
);
backend_parity_tests!(
    other_archived_bodies_survive_a_mutation,
    assert_a_card_mutation_keeps_every_other_archived_body_present
);
backend_parity_tests!(
    card_archive_restore_identity,
    assert_archive_then_restore_is_the_identity_over_the_card_and_its_partition
);
backend_parity_tests!(
    board_archive_restore_identity,
    assert_archive_then_restore_is_the_identity_over_the_board_and_its_subtree
);
