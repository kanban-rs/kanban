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
    assert!(board.sprint_duration_days.is_none());
}

pub async fn test_board_update_all_optional_fields_roundtrip(factory: &BackendFactory) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let mut ctx = KanbanContext::open(factory(&path), AppConfig::default())
        .await
        .unwrap();

    let board = ctx.create_board("Board".into(), None).unwrap();
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
            position: None,
        },
    )
    .unwrap();

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

async fn board_with_columns(
    factory: &BackendFactory,
    path: &std::path::Path,
) -> (
    KanbanContext,
    kanban_domain::Board,
    Vec<kanban_domain::Column>,
) {
    let mut ctx = KanbanContext::open(factory(path), AppConfig::default())
        .await
        .unwrap();
    let board = ctx.create_board("Board".into(), None).unwrap();
    let cols = ["TODO", "Doing", "Done", "Decision"]
        .iter()
        .enumerate()
        .map(|(i, name)| {
            ctx.create_column(board.id, (*name).into(), Some(i as i32))
                .unwrap()
        })
        .collect();
    (ctx, board, cols)
}

pub async fn test_update_card_status_done_lands_in_configured_column_not_last_column(
    factory: &BackendFactory,
) {
    use kanban_domain::{CardStatus, CardUpdate, ColumnUpdate, CreateCardOptions};

    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let (mut ctx, board, cols) = board_with_columns(factory, &path).await;

    ctx.update_column(
        cols[2].id,
        ColumnUpdate {
            default_status: Some(Some(CardStatus::Done)),
            ..Default::default()
        },
    )
    .unwrap();

    let card = ctx
        .create_card(
            board.id,
            cols[0].id,
            "Card".into(),
            CreateCardOptions::default(),
        )
        .unwrap();

    let updated = ctx
        .update_card(
            card.id,
            CardUpdate {
                status: Some(CardStatus::Done),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(updated.status, CardStatus::Done);
    assert_eq!(
        updated.column_id, cols[2].id,
        "status=done must land in the CONFIGURED completion column, not the last column"
    );

    ctx.save().await.unwrap();
    let loaded = KanbanContext::open_deferred(factory(&path), AppConfig::default());
    let c = loaded.get_card(card.id).unwrap().unwrap();
    assert_eq!(c.status, CardStatus::Done);
    assert_eq!(c.column_id, cols[2].id);
}

pub async fn test_move_card_into_configured_completion_column_keeps_status_done(
    factory: &BackendFactory,
) {
    use kanban_domain::{CardStatus, CardUpdate, ColumnUpdate, CreateCardOptions};

    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let (mut ctx, board, cols) = board_with_columns(factory, &path).await;

    ctx.update_column(
        cols[2].id,
        ColumnUpdate {
            default_status: Some(Some(CardStatus::Done)),
            ..Default::default()
        },
    )
    .unwrap();

    let card = ctx
        .create_card(
            board.id,
            cols[0].id,
            "Card".into(),
            CreateCardOptions::default(),
        )
        .unwrap();
    ctx.update_card(
        card.id,
        CardUpdate {
            status: Some(CardStatus::Done),
            ..Default::default()
        },
    )
    .unwrap();

    // The other half of the historical oscillation: a move INTO the configured
    // completion column must not reset the status.
    let moved = ctx.move_card(card.id, cols[2].id, None).unwrap();
    assert_eq!(
        moved.status,
        CardStatus::Done,
        "moving into the completion column must leave status=done untouched"
    );
}

pub async fn test_delete_column_prunes_completion_configuration(factory: &BackendFactory) {
    use kanban_domain::{CardStatus, ColumnUpdate};

    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let (mut ctx, _board, cols) = board_with_columns(factory, &path).await;

    ctx.update_column(
        cols[2].id,
        ColumnUpdate {
            default_status: Some(Some(CardStatus::Done)),
            ..Default::default()
        },
    )
    .unwrap();
    ctx.update_column(
        cols[3].id,
        ColumnUpdate {
            default_status: Some(Some(CardStatus::Done)),
            ..Default::default()
        },
    )
    .unwrap();

    ctx.delete_column(cols[2].id).unwrap();

    assert!(
        ctx.get_column(cols[2].id).unwrap().is_none(),
        "deleting a completion column must remove it outright — nothing dangles \
         to prune since default_status alone is the source of completion"
    );
    assert_eq!(
        ctx.get_column(cols[3].id).unwrap().unwrap().default_status,
        Some(CardStatus::Done),
        "a sibling completion column must be untouched by the delete"
    );

    ctx.save().await.unwrap();
    let loaded = KanbanContext::open_deferred(factory(&path), AppConfig::default());
    assert_eq!(
        loaded
            .get_column(cols[3].id)
            .unwrap()
            .unwrap()
            .default_status,
        Some(CardStatus::Done),
        "the sibling's completion status must be the durable state, on every backend"
    );
}

pub async fn test_undo_column_delete_restores_completion_membership(factory: &BackendFactory) {
    use kanban_domain::{CardStatus, ColumnUpdate};

    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let (mut ctx, _board, cols) = board_with_columns(factory, &path).await;

    ctx.update_column(
        cols[2].id,
        ColumnUpdate {
            default_status: Some(Some(CardStatus::Done)),
            ..Default::default()
        },
    )
    .unwrap();

    ctx.delete_column(cols[2].id).unwrap();
    assert!(ctx.undo().unwrap(), "undo must apply");

    let restored = ctx
        .get_column(cols[2].id)
        .unwrap()
        .expect("undo must restore the column");
    assert_eq!(
        restored.default_status,
        Some(CardStatus::Done),
        "undo of a column delete must restore its completion membership"
    );
}
