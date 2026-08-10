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
    assert!(board.completion_column_ids.is_empty());
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
            completion_column_ids: None,
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

pub async fn test_update_board_completion_columns_persists_and_round_trips(
    factory: &BackendFactory,
) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let (mut ctx, board, cols) = board_with_columns(factory, &path).await;

    ctx.update_board(
        board.id,
        BoardUpdate {
            completion_column_ids: Some(vec![cols[2].id]),
            ..Default::default()
        },
    )
    .unwrap();

    ctx.save().await.unwrap();
    let loaded = KanbanContext::open_deferred(factory(&path), AppConfig::default());

    let b = loaded.get_board(board.id).unwrap().unwrap();
    assert_eq!(b.completion_column_ids, vec![cols[2].id]);
}

pub async fn test_update_board_completion_columns_preserves_order(factory: &BackendFactory) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let (mut ctx, board, cols) = board_with_columns(factory, &path).await;

    // Deliberately NOT column-position order.
    ctx.update_board(
        board.id,
        BoardUpdate {
            completion_column_ids: Some(vec![cols[3].id, cols[2].id]),
            ..Default::default()
        },
    )
    .unwrap();

    ctx.save().await.unwrap();
    let loaded = KanbanContext::open_deferred(factory(&path), AppConfig::default());

    let b = loaded.get_board(board.id).unwrap().unwrap();
    assert_eq!(
        b.completion_column_ids,
        vec![cols[3].id, cols[2].id],
        "element 0 is the primary completion column; order must survive"
    );
}

pub async fn test_update_board_with_column_from_other_board_returns_validation_error(
    factory: &BackendFactory,
) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let (mut ctx, board, _cols) = board_with_columns(factory, &path).await;

    let other = ctx.create_board("Other".into(), None).unwrap();
    let other_col = ctx.create_column(other.id, "Done".into(), Some(0)).unwrap();

    let err = ctx
        .update_board(
            board.id,
            BoardUpdate {
                completion_column_ids: Some(vec![other_col.id]),
                ..Default::default()
            },
        )
        .unwrap_err();
    assert!(
        err.is_validation(),
        "expected Validation error, got: {err:?}"
    );
}

pub async fn test_update_board_with_unknown_column_returns_validation_error(
    factory: &BackendFactory,
) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let (mut ctx, board, _cols) = board_with_columns(factory, &path).await;

    let err = ctx
        .update_board(
            board.id,
            BoardUpdate {
                completion_column_ids: Some(vec![uuid::Uuid::new_v4()]),
                ..Default::default()
            },
        )
        .unwrap_err();
    assert!(
        err.is_validation(),
        "expected Validation error, got: {err:?}"
    );
}

pub async fn test_update_board_with_duplicate_completion_columns_returns_validation_error(
    factory: &BackendFactory,
) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let (mut ctx, board, cols) = board_with_columns(factory, &path).await;

    let err = ctx
        .update_board(
            board.id,
            BoardUpdate {
                completion_column_ids: Some(vec![cols[2].id, cols[2].id]),
                ..Default::default()
            },
        )
        .unwrap_err();
    assert!(
        err.is_validation(),
        "expected Validation error, got: {err:?}"
    );
}

pub async fn test_update_board_with_empty_completion_columns_clears_configuration(
    factory: &BackendFactory,
) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let (mut ctx, board, cols) = board_with_columns(factory, &path).await;

    ctx.update_board(
        board.id,
        BoardUpdate {
            completion_column_ids: Some(vec![cols[2].id]),
            ..Default::default()
        },
    )
    .unwrap();
    ctx.update_board(
        board.id,
        BoardUpdate {
            completion_column_ids: Some(vec![]),
            ..Default::default()
        },
    )
    .unwrap();

    ctx.save().await.unwrap();
    let loaded = KanbanContext::open_deferred(factory(&path), AppConfig::default());

    let b = loaded.get_board(board.id).unwrap().unwrap();
    assert_eq!(b.completion_column_ids, Vec::<uuid::Uuid>::new());
}

pub async fn test_create_board_with_completion_columns_returns_validation_error(
    factory: &BackendFactory,
) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let mut ctx = KanbanContext::open(factory(&path), AppConfig::default())
        .await
        .unwrap();

    let spec = kanban_domain::NewBoard {
        name: "Board".into(),
        description: None,
        sprint_prefix: None,
        card_prefix: None,
        task_sort_field: None,
        task_sort_order: None,
        sprint_duration_days: None,
        task_list_view: None,
        completion_column_ids: vec![uuid::Uuid::new_v4()],
    };
    let err = ctx.create_board_from_spec(None, spec).unwrap_err();
    assert!(
        err.is_validation(),
        "expected Validation error, got: {err:?}"
    );
}

pub async fn test_rejected_completion_columns_update_leaves_board_unchanged(
    factory: &BackendFactory,
) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let (mut ctx, board, cols) = board_with_columns(factory, &path).await;

    ctx.update_board(
        board.id,
        BoardUpdate {
            completion_column_ids: Some(vec![cols[2].id]),
            ..Default::default()
        },
    )
    .unwrap();

    ctx.update_board(
        board.id,
        BoardUpdate {
            name: Some("Should not apply".into()),
            completion_column_ids: Some(vec![uuid::Uuid::new_v4()]),
            ..Default::default()
        },
    )
    .unwrap_err();

    ctx.save().await.unwrap();
    let loaded = KanbanContext::open_deferred(factory(&path), AppConfig::default());

    let b = loaded.get_board(board.id).unwrap().unwrap();
    assert_eq!(
        b.name, "Board",
        "a rejected update must not be half-applied"
    );
    assert_eq!(b.completion_column_ids, vec![cols[2].id]);
}

pub async fn test_update_card_status_done_lands_in_configured_column_not_last_column(
    factory: &BackendFactory,
) {
    use kanban_domain::{CardStatus, CardUpdate, CreateCardOptions};

    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let (mut ctx, board, cols) = board_with_columns(factory, &path).await;

    ctx.update_board(
        board.id,
        BoardUpdate {
            completion_column_ids: Some(vec![cols[2].id]),
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
    use kanban_domain::{CardStatus, CardUpdate, CreateCardOptions};

    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let (mut ctx, board, cols) = board_with_columns(factory, &path).await;

    ctx.update_board(
        board.id,
        BoardUpdate {
            completion_column_ids: Some(vec![cols[2].id]),
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
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let (mut ctx, board, cols) = board_with_columns(factory, &path).await;

    ctx.update_board(
        board.id,
        BoardUpdate {
            completion_column_ids: Some(vec![cols[2].id, cols[3].id]),
            ..Default::default()
        },
    )
    .unwrap();

    ctx.delete_column(cols[2].id).unwrap();

    let b = ctx.get_board(board.id).unwrap().unwrap();
    assert_eq!(
        b.completion_column_ids,
        vec![cols[3].id],
        "deleting a configured column must remove it from the completion set"
    );

    ctx.save().await.unwrap();
    let loaded = KanbanContext::open_deferred(factory(&path), AppConfig::default());
    let b = loaded.get_board(board.id).unwrap().unwrap();
    assert_eq!(
        b.completion_column_ids,
        vec![cols[3].id],
        "the pruned configuration must be the durable state, on every backend"
    );
}

pub async fn test_undo_column_delete_restores_completion_membership(factory: &BackendFactory) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let (mut ctx, board, cols) = board_with_columns(factory, &path).await;

    ctx.update_board(
        board.id,
        BoardUpdate {
            completion_column_ids: Some(vec![cols[2].id, cols[3].id]),
            ..Default::default()
        },
    )
    .unwrap();

    ctx.delete_column(cols[2].id).unwrap();
    assert!(ctx.undo().unwrap(), "undo must apply");

    assert!(
        ctx.get_column(cols[2].id).unwrap().is_some(),
        "undo must restore the column"
    );
    let b = ctx.get_board(board.id).unwrap().unwrap();
    assert_eq!(
        b.completion_column_ids,
        vec![cols[2].id, cols[3].id],
        "undo of a column delete must restore its completion membership in order"
    );
}

pub async fn test_apply_board_settings_rejects_unknown_completion_column(factory: &BackendFactory) {
    use kanban_domain::commands::{ApplyBoardSettings, BoardCommand, Command};
    use kanban_domain::BoardSettingsDto;

    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let (mut ctx, board, cols) = board_with_columns(factory, &path).await;

    ctx.update_board(
        board.id,
        BoardUpdate {
            completion_column_ids: Some(vec![cols[2].id]),
            ..Default::default()
        },
    )
    .unwrap();

    let mut dto = {
        use kanban_core::Editable;
        BoardSettingsDto::from_entity(&ctx.get_board(board.id).unwrap().unwrap())
    };
    dto.completion_column_ids = vec![uuid::Uuid::new_v4()];

    let err = ctx
        .execute(vec![Command::Board(BoardCommand::ApplySettings(
            ApplyBoardSettings {
                board_id: board.id,
                dto,
            },
        ))])
        .unwrap_err();
    assert!(
        err.is_validation(),
        "a dangling completion column in the settings DTO must be a validation error \
         on every backend, got: {err:?}"
    );

    let b = ctx.get_board(board.id).unwrap().unwrap();
    assert_eq!(
        b.completion_column_ids,
        vec![cols[2].id],
        "a rejected settings apply must leave the board unchanged"
    );
}

pub async fn test_apply_board_settings_rejects_other_boards_completion_column(
    factory: &BackendFactory,
) {
    use kanban_domain::commands::{ApplyBoardSettings, BoardCommand, Command};
    use kanban_domain::BoardSettingsDto;

    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let (mut ctx, board, _cols) = board_with_columns(factory, &path).await;
    let other = ctx.create_board("Other".into(), None).unwrap();
    let other_col = ctx.create_column(other.id, "Done".into(), Some(0)).unwrap();

    let mut dto = {
        use kanban_core::Editable;
        BoardSettingsDto::from_entity(&ctx.get_board(board.id).unwrap().unwrap())
    };
    dto.completion_column_ids = vec![other_col.id];

    let err = ctx
        .execute(vec![Command::Board(BoardCommand::ApplySettings(
            ApplyBoardSettings {
                board_id: board.id,
                dto,
            },
        ))])
        .unwrap_err();
    assert!(
        err.is_validation(),
        "another board's column in the settings DTO must be a validation error, got: {err:?}"
    );
}

pub async fn test_undo_board_delete_restores_completion_configuration(factory: &BackendFactory) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let (mut ctx, board, cols) = board_with_columns(factory, &path).await;

    ctx.update_board(
        board.id,
        BoardUpdate {
            completion_column_ids: Some(vec![cols[2].id, cols[3].id]),
            ..Default::default()
        },
    )
    .unwrap();

    ctx.delete_board(board.id).unwrap();
    assert!(ctx.undo().unwrap(), "undo must apply");

    let b = ctx.get_board(board.id).unwrap().expect("board restored");
    assert_eq!(
        b.completion_column_ids,
        vec![cols[2].id, cols[3].id],
        "undo of a board delete must restore the completion configuration in order"
    );

    ctx.save().await.unwrap();
    let loaded = KanbanContext::open_deferred(factory(&path), AppConfig::default());
    let b = loaded.get_board(board.id).unwrap().unwrap();
    assert_eq!(
        b.completion_column_ids,
        vec![cols[2].id, cols[3].id],
        "the restored configuration must be the durable state"
    );
}
