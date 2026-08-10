use super::super::BackendFactory;
use crate::KanbanContext;
use kanban_core::AppConfig;
use kanban_domain::board::{SortField, SortOrder};
use kanban_domain::task_list_view::TaskListView;
use kanban_domain::{BoardUpdate, FieldUpdate, KanbanOperations};
use tempfile::TempDir;

pub async fn test_board_basic_fields_roundtrip(factory: &BackendFactory) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let mut ctx = KanbanContext::open(factory(&path), AppConfig::default())
        .await
        .unwrap();

    let board = ctx
        .create_board("Test Board".into(), Some("TB".into()))
        .unwrap();
    assert_eq!(board.name, "Test Board");
    assert_eq!(board.card_prefix, Some("TB".into()));

    ctx.save().await.unwrap();
    let ctx = KanbanContext::open_deferred(factory(&path), AppConfig::default());

    let board = ctx.get_board(board.id).unwrap().unwrap();
    assert_eq!(board.name, "Test Board");
    assert_eq!(board.card_prefix, Some("TB".into()));
    assert!(board.description.is_none());
    assert!(board.sprint_prefix.is_none());
    assert!(board.active_sprint_id.is_none());
    assert!(board.completion_column_id.is_none());
    assert!(board.sprint_duration_days.is_none());
}

pub async fn test_board_update_all_optional_fields_roundtrip(factory: &BackendFactory) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let mut ctx = KanbanContext::open(factory(&path), AppConfig::default())
        .await
        .unwrap();

    let board = ctx.create_board("Board".into(), None).unwrap();
    let col = ctx.create_column(board.id, "Done".into(), None).unwrap();
    let sprint = ctx.create_sprint(board.id, None, None).unwrap();

    ctx.update_board(
        board.id,
        BoardUpdate {
            name: Some("Updated Board".into()),
            description: FieldUpdate::Set("A description".into()),
            sprint_prefix: FieldUpdate::Set("SP".into()),
            card_prefix: FieldUpdate::Set("UB".into()),
            task_sort_field: Some(SortField::Priority),
            task_sort_order: Some(SortOrder::Descending),
            sprint_duration_days: FieldUpdate::Set(14),
            task_list_view: Some(TaskListView::GroupedByColumn),
            active_sprint_id: FieldUpdate::Set(sprint.id),
            completion_column_id: FieldUpdate::NoChange,
            position: None,
        },
    )
    .unwrap();

    // The durable completion configuration is the ordered list, not the legacy
    // single id; set it through the data store until BoardUpdate carries it.
    {
        let mut b = ctx.data_store().get_board(board.id).unwrap().unwrap();
        b.update_completion_column_ids(vec![col.id]);
        ctx.data_store().upsert_board(b).unwrap();
    }

    ctx.save().await.unwrap();
    let ctx = KanbanContext::open_deferred(factory(&path), AppConfig::default());

    let b = ctx.get_board(board.id).unwrap().unwrap();
    assert_eq!(b.name, "Updated Board");
    assert_eq!(b.description.as_deref(), Some("A description"));
    assert_eq!(b.sprint_prefix.as_deref(), Some("SP"));
    assert_eq!(b.card_prefix.as_deref(), Some("UB"));
    assert_eq!(b.task_sort_field, SortField::Priority);
    assert_eq!(b.task_sort_order, SortOrder::Descending);
    assert_eq!(b.sprint_duration_days, Some(14));
    assert_eq!(b.task_list_view, TaskListView::GroupedByColumn);
    assert_eq!(b.active_sprint_id, Some(sprint.id));
    assert_eq!(b.completion_column_ids, vec![col.id]);
}

pub async fn test_board_sprint_names_roundtrip(factory: &BackendFactory) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let mut ctx = KanbanContext::open(factory(&path), AppConfig::default())
        .await
        .unwrap();

    let board = ctx.create_board("Board".into(), Some("B".into())).unwrap();

    let mut b = ctx.data_store().get_board(board.id).unwrap().unwrap();
    b.sprint_names = vec!["Alpha".into(), "Beta".into(), "Gamma".into()];
    b.sprint_name_used_count = 1;
    ctx.data_store().upsert_board(b).unwrap();

    ctx.save().await.unwrap();
    let ctx = KanbanContext::open_deferred(factory(&path), AppConfig::default());

    let b = ctx.get_board(board.id).unwrap().unwrap();
    assert_eq!(b.sprint_names, vec!["Alpha", "Beta", "Gamma"]);
    assert_eq!(b.sprint_name_used_count, 1);
}

pub async fn test_board_card_counter_roundtrip(factory: &BackendFactory) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let mut ctx = KanbanContext::open(factory(&path), AppConfig::default())
        .await
        .unwrap();

    let board = ctx
        .create_board("Board".into(), Some("PFX".into()))
        .unwrap();

    let mut b = ctx.data_store().get_board(board.id).unwrap().unwrap();
    b.card_counter = 10;
    b.sprint_counters.insert("SP".into(), 3);
    b.sprint_counters.insert("SPRINT".into(), 7);
    ctx.data_store().upsert_board(b).unwrap();

    ctx.save().await.unwrap();
    let ctx = KanbanContext::open_deferred(factory(&path), AppConfig::default());

    let b = ctx.get_board(board.id).unwrap().unwrap();
    assert_eq!(b.card_counter, 10);
    assert_eq!(b.sprint_counters.get("SP"), Some(&3));
    assert_eq!(b.sprint_counters.get("SPRINT"), Some(&7));
}

pub async fn test_board_next_sprint_number_roundtrip(factory: &BackendFactory) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let mut ctx = KanbanContext::open(factory(&path), AppConfig::default())
        .await
        .unwrap();

    let board = ctx.create_board("Board".into(), None).unwrap();

    let mut b = ctx.data_store().get_board(board.id).unwrap().unwrap();
    b.next_sprint_number = 42;
    ctx.data_store().upsert_board(b).unwrap();

    ctx.save().await.unwrap();
    let ctx = KanbanContext::open_deferred(factory(&path), AppConfig::default());

    let b = ctx.get_board(board.id).unwrap().unwrap();
    assert_eq!(b.next_sprint_number, 42);
}
