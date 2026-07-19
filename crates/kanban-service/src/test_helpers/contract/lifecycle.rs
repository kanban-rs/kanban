use super::super::helpers::fully_populated_snapshot;
use super::super::BackendFactory;
use crate::KanbanContext;
use kanban_core::{AppConfig, EdgeBase};
use kanban_domain::board::{SortField, SortOrder};
use kanban_domain::card::{CardPriority, CardStatus};
use kanban_domain::sprint::SprintStatus;
use kanban_domain::task_list_view::TaskListView;
use kanban_domain::{
    BlocksEdge, BoardUpdate, CardUpdate, ColumnUpdate, CreateCardOptions, DependencyGraph,
    FieldUpdate, KanbanOperations, KanbanResult, RelatesEdge, RelatesKind, Severity, SpawnsEdge,
};
use tempfile::TempDir;

pub async fn test_multiple_boards_roundtrip(factory: &BackendFactory) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let mut ctx = KanbanContext::open(factory(&path), AppConfig::default())
        .await
        .unwrap();

    let board1 = ctx
        .create_board("Board One".into(), Some("B1".into()))
        .unwrap();
    let board2 = ctx
        .create_board("Board Two".into(), Some("B2".into()))
        .unwrap();

    let col1 = ctx.create_column(board1.id, "Col1".into(), None).unwrap();
    let col2 = ctx.create_column(board2.id, "Col2".into(), None).unwrap();

    ctx.create_card(
        board1.id,
        col1.id,
        "Card in B1".into(),
        CreateCardOptions::default(),
    )
    .unwrap();
    ctx.create_card(
        board2.id,
        col2.id,
        "Card in B2".into(),
        CreateCardOptions::default(),
    )
    .unwrap();

    ctx.save().await.unwrap();
    let ctx = KanbanContext::open_deferred(factory(&path), AppConfig::default());

    let boards = ctx.list_boards().unwrap();
    assert_eq!(boards.len(), 2);

    let cols1 = ctx.list_columns(board1.id).unwrap();
    assert_eq!(cols1.len(), 1);
    assert_eq!(cols1[0].board_id, board1.id);

    let cols2 = ctx.list_columns(board2.id).unwrap();
    assert_eq!(cols2.len(), 1);
    assert_eq!(cols2[0].board_id, board2.id);
}

pub async fn test_incremental_save_preserves_prior_data(factory: &BackendFactory) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let mut ctx = KanbanContext::open(factory(&path), AppConfig::default())
        .await
        .unwrap();

    let board = ctx.create_board("Board".into(), None).unwrap();
    let col = ctx.create_column(board.id, "Col".into(), None).unwrap();
    ctx.save().await.unwrap();

    let card = ctx
        .create_card(
            board.id,
            col.id,
            "New Card".into(),
            CreateCardOptions::default(),
        )
        .unwrap();
    ctx.save().await.unwrap();

    let ctx = KanbanContext::open_deferred(factory(&path), AppConfig::default());

    let boards = ctx.list_boards().unwrap();
    assert_eq!(boards.len(), 1);
    assert!(ctx.get_card(card.id).unwrap().is_some());
}

pub async fn test_delete_archived_card_roundtrip(factory: &BackendFactory) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let mut ctx = KanbanContext::open(factory(&path), AppConfig::default())
        .await
        .unwrap();

    let board = ctx.create_board("Board".into(), None).unwrap();
    let col = ctx.create_column(board.id, "Col".into(), None).unwrap();

    let card = ctx
        .create_card(
            board.id,
            col.id,
            "Delete Me".into(),
            CreateCardOptions::default(),
        )
        .unwrap();
    ctx.archive_card(card.id).unwrap();
    ctx.save().await.unwrap();

    assert_eq!(ctx.list_archived_cards().unwrap().len(), 1);

    ctx.delete_card(card.id).unwrap();
    ctx.save().await.unwrap();

    let ctx = KanbanContext::open_deferred(factory(&path), AppConfig::default());
    assert!(ctx.list_archived_cards().unwrap().is_empty());
}

pub async fn test_delete_column_roundtrip(factory: &BackendFactory) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let mut ctx = KanbanContext::open(factory(&path), AppConfig::default())
        .await
        .unwrap();

    let board = ctx.create_board("Board".into(), None).unwrap();
    let col = ctx.create_column(board.id, "Col".into(), None).unwrap();
    ctx.save().await.unwrap();

    ctx.delete_column(col.id).unwrap();
    ctx.save().await.unwrap();

    let ctx = KanbanContext::open_deferred(factory(&path), AppConfig::default());
    assert!(ctx.get_column(col.id).unwrap().is_none());
}

pub async fn test_delete_sprint_roundtrip(factory: &BackendFactory) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let mut ctx = KanbanContext::open(factory(&path), AppConfig::default())
        .await
        .unwrap();

    let board = ctx.create_board("Board".into(), None).unwrap();
    let sprint = ctx.create_sprint(board.id, None, None).unwrap();
    ctx.save().await.unwrap();

    ctx.delete_sprint(sprint.id).unwrap();
    ctx.save().await.unwrap();

    let ctx = KanbanContext::open_deferred(factory(&path), AppConfig::default());
    assert!(ctx.get_sprint(sprint.id).unwrap().is_none());
}

pub async fn test_full_populated_context_roundtrip(factory: &BackendFactory) -> KanbanResult<()> {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let mut ctx = KanbanContext::open(factory(&path), AppConfig::default())
        .await
        .unwrap();

    let board = ctx
        .create_board("Full Board".into(), Some("FB".into()))
        .unwrap();
    {
        let mut b = ctx.data_store().get_board(board.id).unwrap().unwrap();
        b.sprint_names = vec!["Alpha".into(), "Beta".into()];
        b.sprint_name_used_count = 1;
        b.card_counter = 10;
        b.sprint_counters.insert("SP".into(), 5);
        ctx.data_store().upsert_board(b).unwrap();
    }

    let col_todo = ctx.create_column(board.id, "Todo".into(), Some(0)).unwrap();
    let col_done = ctx.create_column(board.id, "Done".into(), Some(1)).unwrap();
    ctx.update_column(
        col_done.id,
        ColumnUpdate {
            wip_limit: FieldUpdate::Set(10),
            ..Default::default()
        },
    )
    .unwrap();

    ctx.update_board(
        board.id,
        BoardUpdate {
            completion_column_id: FieldUpdate::Set(col_done.id),
            description: FieldUpdate::Set("Full desc".into()),
            sprint_prefix: FieldUpdate::Set("SP".into()),
            task_sort_field: Some(SortField::Points),
            task_sort_order: Some(SortOrder::Descending),
            sprint_duration_days: FieldUpdate::Set(21),
            task_list_view: Some(TaskListView::GroupedByColumn),
            ..Default::default()
        },
    )
    .unwrap();

    let sprint = ctx
        .create_sprint(board.id, Some("SP".into()), None)
        .unwrap();
    ctx.activate_sprint(sprint.id, Some(14)).unwrap();

    ctx.update_board(
        board.id,
        BoardUpdate {
            active_sprint_id: FieldUpdate::Set(sprint.id),
            sprint_duration_days: FieldUpdate::Set(21),
            ..Default::default()
        },
    )
    .unwrap();

    let card1 = ctx
        .create_card(
            board.id,
            col_todo.id,
            "Rich Card".into(),
            CreateCardOptions {
                description: Some("Full description".into()),
                priority: Some(CardPriority::Critical),
                points: Some(13),
                due_date: Some(chrono::Utc::now()),
                ..Default::default()
            },
        )
        .unwrap();
    ctx.assign_card_to_sprint(card1.id, sprint.id).unwrap();
    ctx.update_card(
        card1.id,
        CardUpdate {
            status: Some(CardStatus::InProgress),
            ..Default::default()
        },
    )
    .unwrap();

    let card2 = ctx
        .create_card(
            board.id,
            col_todo.id,
            "Minimal Card".into(),
            CreateCardOptions::default(),
        )
        .unwrap();

    let card3 = ctx
        .create_card(
            board.id,
            col_done.id,
            "Done Card".into(),
            CreateCardOptions::default(),
        )
        .unwrap();
    ctx.update_card(
        card3.id,
        CardUpdate {
            status: Some(CardStatus::Done),
            ..Default::default()
        },
    )
    .unwrap();

    let card4 = ctx
        .create_card(
            board.id,
            col_todo.id,
            "Archived Card".into(),
            CreateCardOptions {
                description: Some("will be archived".into()),
                priority: Some(CardPriority::High),
                points: Some(5),
                ..Default::default()
            },
        )
        .unwrap();
    ctx.assign_card_to_sprint(card4.id, sprint.id).unwrap();
    ctx.archive_card(card4.id).unwrap();

    let now = chrono::Utc::now();
    {
        let spawns = vec![SpawnsEdge {
            base: EdgeBase {
                source: card2.id,
                target: card3.id,
                created_at: now,
                archived_at: None,
            },
        }];
        let blocks = vec![BlocksEdge {
            base: EdgeBase {
                source: card1.id,
                target: card2.id,
                created_at: now,
                archived_at: None,
            },
            severity: Severity::default(),
        }];
        let relates = vec![RelatesEdge {
            base: EdgeBase {
                source: card1.id,
                target: card3.id,
                created_at: now,
                archived_at: Some(now),
            },
            kind: RelatesKind::default(),
        }];
        let graph = DependencyGraph::from_validated_per_kind_edges(spawns, blocks, relates)
            .expect("test fixture edges must validate");
        ctx.data_store().set_graph(graph).unwrap();
    }

    ctx.save().await.unwrap();
    let loaded = KanbanContext::open_deferred(factory(&path), AppConfig::default());

    let b = loaded.get_board(board.id).unwrap().unwrap();
    assert_eq!(b.name, "Full Board");
    assert_eq!(b.description.as_deref(), Some("Full desc"));
    assert_eq!(b.sprint_prefix.as_deref(), Some("SP"));
    assert_eq!(b.card_prefix.as_deref(), Some("FB"));
    assert_eq!(b.task_sort_field, SortField::Points);
    assert_eq!(b.task_sort_order, SortOrder::Descending);
    assert_eq!(b.sprint_duration_days, Some(21));
    assert_eq!(b.task_list_view, TaskListView::GroupedByColumn);
    assert_eq!(b.active_sprint_id, Some(sprint.id));
    assert_eq!(b.completion_column_id, Some(col_done.id));
    assert_eq!(b.sprint_names, vec!["Alpha", "Beta"]);
    assert_eq!(b.sprint_name_used_count, 1);
    assert_eq!(b.card_counter, 14);
    assert_eq!(b.sprint_counters.get("SP"), Some(&6));

    let cols = loaded.list_columns(board.id).unwrap();
    assert_eq!(cols.len(), 2);
    let done_col = cols.iter().find(|c| c.id == col_done.id).unwrap();
    assert_eq!(done_col.wip_limit, Some(10));

    let s = loaded.get_sprint(sprint.id).unwrap().unwrap();
    assert_eq!(s.status, SprintStatus::Active);
    assert!(s.start_date.is_some());

    let c1 = loaded.get_card(card1.id).unwrap().unwrap();
    assert_eq!(c1.title, "Rich Card");
    assert_eq!(c1.priority, CardPriority::Critical);
    assert_eq!(c1.status, CardStatus::InProgress);
    assert_eq!(c1.points, Some(13));
    assert!(c1.due_date.is_some());
    assert_eq!(c1.sprint_id, Some(sprint.id));
    assert!(!c1.sprint_logs.is_empty());

    let c2 = loaded.get_card(card2.id).unwrap().unwrap();
    assert_eq!(c2.title, "Minimal Card");
    assert!(c2.description.is_none());
    assert!(c2.sprint_id.is_none());

    let c3 = loaded.get_card(card3.id).unwrap().unwrap();
    assert_eq!(c3.status, CardStatus::Done);
    assert!(c3.completed_at.is_some());

    let archived = loaded.list_archived_cards().unwrap();
    assert_eq!(archived.len(), 1);
    assert_eq!(archived[0].entity_id, card4.id);
    assert_eq!(archived[0].context.board_id, board.id);
    let c4 = loaded.get_card(archived[0].entity_id).unwrap().unwrap();
    assert_eq!(c4.title, "Archived Card");
    assert_eq!(c4.priority, CardPriority::High);
    assert_eq!(c4.points, Some(5));
    assert_eq!(c4.column_id, col_todo.id);

    let graph = loaded.graph()?;
    assert_eq!(graph.len(), 3, "expected 3 edges total");
    Ok(())
}

pub async fn test_full_roundtrip_preserves_all_fields(factory: &BackendFactory) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let original = fully_populated_snapshot();

    {
        let ctx = KanbanContext::open_deferred(factory(&path), AppConfig::default());
        ctx.apply_snapshot(original.clone()).unwrap();
        ctx.save().await.unwrap();
    }

    let ctx = KanbanContext::open_deferred(factory(&path), AppConfig::default());
    let loaded = ctx.snapshot().unwrap();
    assert_eq!(original, loaded);
}

pub async fn test_load_save_reload_roundtrip(factory: &BackendFactory) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let mut ctx = KanbanContext::open(factory(&path), AppConfig::default())
        .await
        .unwrap();

    ctx.create_board("My Board".into(), Some("MB".into()))
        .unwrap();
    ctx.save().await.unwrap();

    let reloaded = KanbanContext::open_deferred(factory(&path), AppConfig::default());
    let boards = reloaded.list_boards().unwrap();
    assert_eq!(boards.len(), 1);
    assert_eq!(boards[0].name, "My Board");
}

pub async fn test_save_overwrites_correctly(factory: &BackendFactory) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let mut ctx = KanbanContext::open(factory(&path), AppConfig::default())
        .await
        .unwrap();

    ctx.create_board("Board One".into(), None).unwrap();
    ctx.save().await.unwrap();

    ctx.create_board("Board Two".into(), None).unwrap();
    ctx.save().await.unwrap();

    let reloaded = KanbanContext::open_deferred(factory(&path), AppConfig::default());
    let boards = reloaded.list_boards().unwrap();
    assert_eq!(boards.len(), 2);
    assert!(boards.iter().any(|b| b.name == "Board One"));
    assert!(boards.iter().any(|b| b.name == "Board Two"));
}

pub async fn test_reload_picks_up_external_changes(factory: &BackendFactory) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");

    let mut ctx_a = KanbanContext::open(factory(&path), AppConfig::default())
        .await
        .unwrap();
    ctx_a.create_board("Board A".into(), None).unwrap();
    ctx_a.save().await.unwrap();

    let mut ctx_b = KanbanContext::open(factory(&path), AppConfig::default())
        .await
        .unwrap();
    ctx_b.create_board("Board B".into(), None).unwrap();
    ctx_b.save().await.unwrap();

    ctx_a.reload().await.unwrap();
    let boards = ctx_a.list_boards().unwrap();
    assert_eq!(boards.len(), 2);
    assert!(boards.iter().any(|b| b.name == "Board B"));
}

pub async fn test_save_with_stale_metadata_returns_conflict(factory: &BackendFactory) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");

    // Store A saves a board
    let mut ctx_a = KanbanContext::open(factory(&path), AppConfig::default())
        .await
        .unwrap();
    ctx_a.create_board("Board A".into(), None).unwrap();
    ctx_a.save().await.unwrap();

    // Store B loads, modifies, and saves — updates metadata
    let mut ctx_b = KanbanContext::open(factory(&path), AppConfig::default())
        .await
        .unwrap();
    ctx_b.create_board("Board B".into(), None).unwrap();
    ctx_b.save().await.unwrap();

    // Store A tries to save again with stale metadata
    ctx_a.create_board("Board C".into(), None).unwrap();
    let result = ctx_a.save().await;

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        matches!(err, kanban_domain::KanbanError::ConflictDetected { .. }),
        "Expected ConflictDetected error, got: {err:?}"
    );
}
