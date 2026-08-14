//! Per-command inverse contract tests. Each test executes a forward
//! command, verifies the visible state, undoes, and asserts the
//! pre-execute state is restored.

use kanban_backend_memory::InMemoryStore;
use kanban_core::AppConfig;
use kanban_domain::commands::{
    ActivateSprint, AddBlocks, AddRelates, AddSpawns, ApplyBoardSettings, ApplyCardMetadata,
    ArchiveCards, AssignCardsToSprint, BoardCommand, CancelSprint, CardCommand, ColumnCommand,
    Command, CompactColumnPositions, CompleteSprint, CreateBoard, CreateColumn,
    CreateSubcardCommand, DeleteColumn, DependencyCommand, MoveCard, RemoveBlocks, RemoveSpawns,
    SetBoardTaskListView, SetBoardTaskSort, SprintCommand, UnassignCardFromSprint, UpdateBoard,
    UpdateCard, UpdateColumn, UpdateSprint,
};
use kanban_domain::{
    BoardUpdate, CardPriority, CardUpdate, ColumnUpdate, FieldUpdate, KanbanOperations,
    KanbanResult, SortField, SortOrder, SprintStatus, SprintUpdate, TaskListView,
};
use kanban_service::KanbanContext;
use std::sync::Arc;
use uuid::Uuid;

async fn make_ctx() -> KanbanContext {
    KanbanContext::open(Arc::new(InMemoryStore::new()), AppConfig::default())
        .await
        .unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn test_inverse_create_board_restores_state() -> KanbanResult<()> {
    let mut ctx = make_ctx().await;
    let id = Uuid::new_v4();

    ctx.execute(vec![Command::Board(BoardCommand::Create(CreateBoard {
        id,
        name: "Tier1 inverse".into(),
        card_prefix: None,
        position: 0,
    }))])?;

    assert_eq!(ctx.boards()?.len(), 1, "forward execute creates board");
    assert!(ctx.can_undo(), "undo is available after execute");

    assert!(ctx.undo()?, "undo via inverse-command path");
    assert_eq!(
        ctx.boards()?.len(),
        0,
        "undo of CreateBoard via inverse must leave the board count at 0"
    );

    // After undoing the only command in the session, no further undo work.
    assert!(!ctx.can_undo());
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_inverse_create_column_restores_state() -> KanbanResult<()> {
    let mut ctx = make_ctx().await;
    // Need a board first to host the column.
    let board_id = Uuid::new_v4();
    ctx.execute(vec![Command::Board(BoardCommand::Create(CreateBoard {
        id: board_id,
        name: "Host".into(),
        card_prefix: None,
        position: 0,
    }))])?;

    let col_id = Uuid::new_v4();
    ctx.execute(vec![Command::Column(ColumnCommand::Create(CreateColumn {
        id: col_id,
        board_id,
        name: "TODO".into(),
        position: 0,
        default_status: None,
    }))])?;
    assert_eq!(ctx.columns()?.len(), 1, "forward execute creates column");

    assert!(ctx.undo()?, "undo via inverse-command path");
    assert_eq!(
        ctx.columns()?.len(),
        0,
        "undo of CreateColumn via inverse removes the column"
    );
    // Board still present — only the column was undone.
    assert_eq!(ctx.boards()?.len(), 1);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_inverse_update_column_restores_prior_fields() -> KanbanResult<()> {
    let mut ctx = make_ctx().await;
    let board_id = Uuid::new_v4();
    let col_id = Uuid::new_v4();
    ctx.execute(vec![Command::Board(BoardCommand::Create(CreateBoard {
        id: board_id,
        name: "Host".into(),
        card_prefix: None,
        position: 0,
    }))])?;
    ctx.execute(vec![Command::Column(ColumnCommand::Create(CreateColumn {
        id: col_id,
        board_id,
        name: "Original".into(),
        position: 5,
        default_status: None,
    }))])?;

    // Update both name and position; leave wip_limit unchanged.
    ctx.execute(vec![Command::Column(ColumnCommand::Update(UpdateColumn {
        column_id: col_id,
        updates: ColumnUpdate {
            name: Some("Renamed".into()),
            position: Some(99),
            wip_limit: FieldUpdate::NoChange,
            default_status: None,
        },
    }))])?;
    let after = &ctx.columns()?[0];
    assert_eq!(after.name, "Renamed");
    assert_eq!(after.position, 99);

    assert!(ctx.undo()?, "undo via inverse-command path");
    let restored = &ctx.columns()?[0];
    assert_eq!(restored.name, "Original", "name restored");
    assert_eq!(restored.position, 5, "position restored");
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_inverse_update_card_restores_prior_fields() -> KanbanResult<()> {
    let mut ctx = make_ctx().await;
    let board = ctx.create_board("B".into(), None)?;
    let col = ctx.create_column(board.id, "TODO".into(), None)?;
    let card = ctx.create_card(
        board.id,
        col.id,
        "Original title".into(),
        Default::default(),
    )?;
    ctx.clear_history()?;

    ctx.execute(vec![Command::Card(CardCommand::Update(UpdateCard {
        card_id: card.id,
        updates: CardUpdate {
            title: Some("Renamed".into()),
            priority: Some(CardPriority::High),
            ..Default::default()
        },
    }))])?;
    let after = ctx.get_card(card.id)?.unwrap();
    assert_eq!(after.title, "Renamed");
    assert_eq!(after.priority, CardPriority::High);

    assert!(ctx.undo()?, "undo via inverse-command path");
    let restored = ctx.get_card(card.id)?.unwrap();
    assert_eq!(restored.title, "Original title", "title restored");
    assert_eq!(
        restored.priority,
        CardPriority::Medium,
        "priority restored to default"
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_inverse_move_card_restores_column_and_position() -> KanbanResult<()> {
    let mut ctx = make_ctx().await;
    let board = ctx.create_board("B".into(), None)?;
    let col_a = ctx.create_column(board.id, "A".into(), None)?;
    let col_b = ctx.create_column(board.id, "B".into(), None)?;
    let card = ctx.create_card(board.id, col_a.id, "C".into(), Default::default())?;
    ctx.clear_history()?;

    let before_col = card.column_id;
    let before_pos = card.position;

    ctx.execute(vec![Command::Card(CardCommand::Move(MoveCard {
        card_id: card.id,
        new_column_id: col_b.id,
        new_position: 7,
    }))])?;
    let moved = ctx.get_card(card.id)?.unwrap();
    assert_eq!(moved.column_id, col_b.id);
    assert_eq!(moved.position, 7);

    assert!(ctx.undo()?);
    let restored = ctx.get_card(card.id)?.unwrap();
    assert_eq!(restored.column_id, before_col);
    assert_eq!(restored.position, before_pos);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_inverse_unassign_card_from_sprint_reassigns() -> KanbanResult<()> {
    let mut ctx = make_ctx().await;
    let board = ctx.create_board("B".into(), None)?;
    let col = ctx.create_column(board.id, "TODO".into(), None)?;
    let card = ctx.create_card(board.id, col.id, "C".into(), Default::default())?;
    let sprint = ctx.create_sprint(board.id, None, None)?;
    ctx.execute(vec![Command::Card(CardCommand::AssignToSprint(
        AssignCardsToSprint {
            ids: vec![card.id],
            sprint_id: sprint.id,
        },
    ))])?;
    ctx.clear_history()?;

    ctx.execute(vec![Command::Card(CardCommand::UnassignFromSprint(
        UnassignCardFromSprint {
            card_id: card.id,
            timestamp: chrono::Utc::now(),
        },
    ))])?;
    assert!(ctx.get_card(card.id)?.unwrap().sprint_id.is_none());

    assert!(ctx.undo()?, "undo re-assigns the card to its prior sprint");
    assert_eq!(
        ctx.get_card(card.id)?.unwrap().sprint_id,
        Some(sprint.id),
        "sprint_id restored"
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_inverse_activate_sprint_reverts_to_planning() -> KanbanResult<()> {
    let mut ctx = make_ctx().await;
    let board = ctx.create_board("B".into(), None)?;
    let sprint = ctx.create_sprint(board.id, None, None)?;
    ctx.clear_history()?;

    let before = ctx.get_sprint(sprint.id)?.unwrap();
    assert_eq!(before.status, SprintStatus::Planning);
    assert!(before.start_date.is_none());

    ctx.execute(vec![Command::Sprint(SprintCommand::Activate(
        ActivateSprint {
            sprint_id: sprint.id,
            duration_days: 14,
        },
    ))])?;
    let after = ctx.get_sprint(sprint.id)?.unwrap();
    assert_eq!(after.status, SprintStatus::Active);
    assert!(after.start_date.is_some());
    assert!(after.end_date.is_some());

    assert!(ctx.undo()?);
    let restored = ctx.get_sprint(sprint.id)?.unwrap();
    assert_eq!(
        restored.status,
        SprintStatus::Planning,
        "status reverted to Planning"
    );
    assert!(restored.start_date.is_none(), "start_date cleared");
    assert!(restored.end_date.is_none(), "end_date cleared");
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_inverse_complete_sprint_reverts_to_active() -> KanbanResult<()> {
    let mut ctx = make_ctx().await;
    let board = ctx.create_board("B".into(), None)?;
    let sprint = ctx.create_sprint(board.id, None, None)?;
    ctx.execute(vec![Command::Sprint(SprintCommand::Activate(
        ActivateSprint {
            sprint_id: sprint.id,
            duration_days: 14,
        },
    ))])?;
    ctx.clear_history()?;

    ctx.execute(vec![Command::Sprint(SprintCommand::Complete(
        CompleteSprint {
            sprint_id: sprint.id,
        },
    ))])?;
    assert_eq!(
        ctx.get_sprint(sprint.id)?.unwrap().status,
        SprintStatus::Completed
    );

    assert!(ctx.undo()?);
    assert_eq!(
        ctx.get_sprint(sprint.id)?.unwrap().status,
        SprintStatus::Active,
        "Complete undo reverts to Active"
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_inverse_cancel_sprint_reverts_to_prior_status() -> KanbanResult<()> {
    let mut ctx = make_ctx().await;
    let board = ctx.create_board("B".into(), None)?;
    let sprint = ctx.create_sprint(board.id, None, None)?;
    ctx.clear_history()?;

    ctx.execute(vec![Command::Sprint(SprintCommand::Cancel(CancelSprint {
        sprint_id: sprint.id,
    }))])?;
    assert_eq!(
        ctx.get_sprint(sprint.id)?.unwrap().status,
        SprintStatus::Cancelled
    );

    assert!(ctx.undo()?);
    assert_eq!(
        ctx.get_sprint(sprint.id)?.unwrap().status,
        SprintStatus::Planning,
        "Cancel undo reverts to Planning"
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_inverse_add_blocks_removes_edge() -> KanbanResult<()> {
    let mut ctx = make_ctx().await;
    let board = ctx.create_board("B".into(), None)?;
    let col = ctx.create_column(board.id, "C".into(), None)?;
    let a = ctx.create_card(board.id, col.id, "A".into(), Default::default())?;
    let b = ctx.create_card(board.id, col.id, "B".into(), Default::default())?;
    ctx.clear_history()?;

    ctx.execute(vec![Command::Dependency(DependencyCommand::AddBlocks(
        AddBlocks {
            source: a.id,
            target: b.id,
            severity: Default::default(),
            as_archived: false,
        },
    ))])?;
    assert!(ctx.graph()?.contains(a.id, b.id), "edge added by forward");

    assert!(ctx.undo()?);
    assert!(
        !ctx.graph()?.contains(a.id, b.id),
        "edge removed by inverse"
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_inverse_add_relates_to_removes_edge() -> KanbanResult<()> {
    let mut ctx = make_ctx().await;
    let board = ctx.create_board("B".into(), None)?;
    let col = ctx.create_column(board.id, "C".into(), None)?;
    let a = ctx.create_card(board.id, col.id, "A".into(), Default::default())?;
    let b = ctx.create_card(board.id, col.id, "B".into(), Default::default())?;
    ctx.clear_history()?;

    ctx.execute(vec![Command::Dependency(DependencyCommand::AddRelates(
        AddRelates {
            source: a.id,
            target: b.id,
            kind: Default::default(),
            as_archived: false,
        },
    ))])?;
    assert!(
        ctx.graph()?.contains(a.id, b.id),
        "relates edge added by forward"
    );

    assert!(ctx.undo()?);
    assert!(
        !ctx.graph()?.contains(a.id, b.id),
        "relates edge removed by inverse"
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_inverse_remove_parent_reestablishes_relation() -> KanbanResult<()> {
    let mut ctx = make_ctx().await;
    let board = ctx.create_board("B".into(), None)?;
    let col = ctx.create_column(board.id, "C".into(), None)?;
    let parent = ctx.create_card(board.id, col.id, "Parent".into(), Default::default())?;
    let child = ctx.create_card(board.id, col.id, "Child".into(), Default::default())?;
    ctx.execute(vec![Command::Dependency(DependencyCommand::AddSpawns(
        AddSpawns {
            source: parent.id,
            target: child.id,
            as_archived: false,
        },
    ))])?;
    ctx.clear_history()?;

    ctx.execute(vec![Command::Dependency(DependencyCommand::RemoveSpawns(
        RemoveSpawns {
            source: parent.id,
            target: child.id,
            tolerate_missing: false,
        },
    ))])?;
    assert!(
        !ctx.graph()?.contains(parent.id, child.id),
        "parent edge removed by forward"
    );

    assert!(ctx.undo()?);
    assert!(
        ctx.graph()?.contains(parent.id, child.id),
        "parent edge re-established by inverse"
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_inverse_delete_column_recreates_with_fields() -> KanbanResult<()> {
    let mut ctx = make_ctx().await;
    let board = ctx.create_board("B".into(), None)?;
    let col = ctx.create_column(board.id, "Reborn".into(), None)?;
    // Set a wip_limit so the inverse needs to chain CreateColumn + UpdateColumn.
    ctx.execute(vec![Command::Column(ColumnCommand::Update(UpdateColumn {
        column_id: col.id,
        updates: ColumnUpdate {
            wip_limit: FieldUpdate::Set(7),
            ..Default::default()
        },
    }))])?;
    let original_pos = ctx.columns()?[0].position;
    ctx.clear_history()?;

    ctx.execute(vec![Command::Column(ColumnCommand::Delete(DeleteColumn {
        column_id: col.id,
    }))])?;
    assert_eq!(ctx.columns()?.len(), 0, "column deleted by forward");

    assert!(ctx.undo()?);
    let restored = &ctx.columns()?[0];
    assert_eq!(restored.id, col.id, "id restored");
    assert_eq!(restored.name, "Reborn", "name restored");
    assert_eq!(restored.position, original_pos, "position restored");
    assert_eq!(restored.wip_limit, Some(7), "wip_limit restored");
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_inverse_update_board_restores_fields() -> KanbanResult<()> {
    let mut ctx = make_ctx().await;
    let board = ctx.create_board("Original".into(), None)?;
    ctx.clear_history()?;

    ctx.execute(vec![Command::Board(BoardCommand::Update(UpdateBoard {
        board_id: board.id,
        updates: BoardUpdate {
            name: Some("Renamed".into()),
            description: FieldUpdate::Set("A new description".into()),
            ..Default::default()
        },
    }))])?;
    let after = ctx.boards()?.into_iter().next().unwrap();
    assert_eq!(after.name, "Renamed");
    assert_eq!(after.description, Some("A new description".into()));

    assert!(ctx.undo()?);
    let restored = ctx.boards()?.into_iter().next().unwrap();
    assert_eq!(restored.name, "Original");
    assert_eq!(restored.description, None);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_inverse_set_board_task_sort_reverts() -> KanbanResult<()> {
    let mut ctx = make_ctx().await;
    let board = ctx.create_board("B".into(), None)?;
    let before = ctx.boards()?.into_iter().next().unwrap();
    let original_field = before.task_sort_field;
    let original_order = before.task_sort_order;
    ctx.clear_history()?;

    ctx.execute(vec![Command::Board(BoardCommand::SetTaskSort(
        SetBoardTaskSort {
            board_id: board.id,
            field: SortField::Priority,
            order: SortOrder::Descending,
        },
    ))])?;
    let after = ctx.boards()?.into_iter().next().unwrap();
    assert_eq!(after.task_sort_field, SortField::Priority);

    assert!(ctx.undo()?);
    let restored = ctx.boards()?.into_iter().next().unwrap();
    assert_eq!(restored.task_sort_field, original_field);
    assert_eq!(restored.task_sort_order, original_order);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_inverse_set_board_task_list_view_reverts() -> KanbanResult<()> {
    let mut ctx = make_ctx().await;
    let board = ctx.create_board("B".into(), None)?;
    let original = ctx.boards()?.into_iter().next().unwrap().task_list_view;
    ctx.clear_history()?;

    let target = if original == TaskListView::Flat {
        TaskListView::ColumnView
    } else {
        TaskListView::Flat
    };
    ctx.execute(vec![Command::Board(BoardCommand::SetTaskListView(
        SetBoardTaskListView {
            board_id: board.id,
            view: target,
        },
    ))])?;
    assert_eq!(
        ctx.boards()?.into_iter().next().unwrap().task_list_view,
        target
    );

    assert!(ctx.undo()?);
    assert_eq!(
        ctx.boards()?.into_iter().next().unwrap().task_list_view,
        original
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_inverse_apply_board_settings_restores_prior_settings() -> KanbanResult<()> {
    use kanban_domain::editable::BoardSettingsDto;
    let mut ctx = make_ctx().await;
    let board = ctx.create_board("B".into(), Some("KAN".into()))?;
    ctx.clear_history()?;

    ctx.execute(vec![Command::Board(BoardCommand::ApplySettings(
        ApplyBoardSettings {
            board_id: board.id,
            dto: BoardSettingsDto {
                sprint_prefix: Some("SP".into()),
                card_prefix: Some("KAN".into()),
                sprint_duration_days: Some(21),
                sprint_names: vec!["alpha".into(), "beta".into()],
                completion_column_ids: Vec::new(),
            },
        },
    ))])?;
    let after = ctx.boards()?.into_iter().next().unwrap();
    assert_eq!(after.sprint_prefix, Some("SP".into()));
    assert_eq!(after.sprint_duration_days, Some(21));
    assert_eq!(after.sprint_names, vec!["alpha", "beta"]);

    assert!(ctx.undo()?);
    let restored = ctx.boards()?.into_iter().next().unwrap();
    assert_eq!(
        restored.sprint_prefix, None,
        "sprint_prefix restored to initial None"
    );
    assert_eq!(
        restored.sprint_duration_days, None,
        "sprint_duration_days restored"
    );
    assert!(
        restored.sprint_names.is_empty(),
        "sprint_names restored to initial empty"
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_inverse_update_sprint_restores_prefix() -> KanbanResult<()> {
    let mut ctx = make_ctx().await;
    let board = ctx.create_board("B".into(), None)?;
    let sprint = ctx.create_sprint(board.id, None, None)?;
    ctx.clear_history()?;
    let before_prefix = sprint.prefix.clone();

    ctx.execute(vec![Command::Sprint(SprintCommand::Update(UpdateSprint {
        sprint_id: sprint.id,
        updates: SprintUpdate {
            prefix: FieldUpdate::Set("SPR".into()),
            ..Default::default()
        },
    }))])?;
    let after = ctx.get_sprint(sprint.id)?.unwrap();
    assert_eq!(after.prefix, Some("SPR".into()));

    assert!(ctx.undo()?, "undo via inverse-command path");
    let restored = ctx.get_sprint(sprint.id)?.unwrap();
    assert_eq!(restored.prefix, before_prefix, "prefix restored");
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_inverse_set_parent_removes_edge() -> KanbanResult<()> {
    let mut ctx = make_ctx().await;
    let board = ctx.create_board("B".into(), None)?;
    let col = ctx.create_column(board.id, "C".into(), None)?;
    let parent = ctx.create_card(board.id, col.id, "P".into(), Default::default())?;
    let child = ctx.create_card(board.id, col.id, "C".into(), Default::default())?;
    ctx.clear_history()?;

    ctx.execute(vec![Command::Dependency(DependencyCommand::AddSpawns(
        AddSpawns {
            source: parent.id,
            target: child.id,
            as_archived: false,
        },
    ))])?;
    assert!(
        ctx.graph()?.contains(parent.id, child.id),
        "parent edge added"
    );

    assert!(ctx.undo()?);
    assert!(
        !ctx.graph()?.contains(parent.id, child.id),
        "parent edge removed by inverse"
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_inverse_create_subcard_removes_card_and_archive_trail() -> KanbanResult<()> {
    let mut ctx = make_ctx().await;
    let board = ctx.create_board("B".into(), None)?;
    let col = ctx.create_column(board.id, "C".into(), None)?;
    let parent = ctx.create_card(board.id, col.id, "Parent".into(), Default::default())?;
    ctx.clear_history()?;

    let subcard_id = Uuid::new_v4();
    ctx.execute(vec![Command::Dependency(DependencyCommand::CreateSubcard(
        CreateSubcardCommand {
            id: subcard_id,
            parent_id: parent.id,
            board_id: board.id,
            column_id: col.id,
            title: "Subcard".into(),
            description: None,
            position: 1,
        },
    ))])?;
    assert_eq!(ctx.cards()?.len(), 2);
    assert!(ctx.graph()?.contains(parent.id, subcard_id));

    assert!(ctx.undo()?);
    assert_eq!(ctx.cards()?.len(), 1, "subcard gone");
    assert!(
        ctx.archived_cards()?.is_empty(),
        "no archive trail — undoing CreateSubcard fully removes the card"
    );
    assert!(
        !ctx.graph()?.contains(parent.id, subcard_id),
        "parent edge cleaned up by the archive step in the inverse batch"
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_inverse_apply_card_metadata_restores_fields() -> KanbanResult<()> {
    use kanban_domain::editable::CardMetadataDto;
    let mut ctx = make_ctx().await;
    let board = ctx.create_board("B".into(), None)?;
    let col = ctx.create_column(board.id, "C".into(), None)?;
    let card = ctx.create_card(board.id, col.id, "C".into(), Default::default())?;
    ctx.clear_history()?;

    ctx.execute(vec![Command::Card(CardCommand::ApplyMetadata(
        ApplyCardMetadata {
            card_id: card.id,
            dto: CardMetadataDto {
                priority: "High".into(),
                status: "InProgress".into(),
                points: Some(8),
                due_date: None,
            },
        },
    ))])?;
    let after = ctx.get_card(card.id)?.unwrap();
    assert_eq!(after.priority, CardPriority::High);
    assert_eq!(after.points, Some(8));

    assert!(ctx.undo()?);
    let restored = ctx.get_card(card.id)?.unwrap();
    assert_eq!(restored.priority, CardPriority::Medium);
    assert_eq!(restored.points, None, "points cleared back to None");
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_inverse_archive_cards_restores_each() -> KanbanResult<()> {
    let mut ctx = make_ctx().await;
    let board = ctx.create_board("B".into(), None)?;
    let col = ctx.create_column(board.id, "C".into(), None)?;
    let a = ctx.create_card(board.id, col.id, "A".into(), Default::default())?;
    let b = ctx.create_card(board.id, col.id, "B".into(), Default::default())?;
    let pre_pos_a = a.position;
    let pre_pos_b = b.position;
    ctx.clear_history()?;

    ctx.execute(vec![Command::Card(CardCommand::Archive(ArchiveCards {
        ids: vec![a.id, b.id],
    }))])?;
    assert_eq!(ctx.cards()?.len(), 0);
    assert_eq!(ctx.archived_cards()?.len(), 2);

    assert!(ctx.undo()?);
    assert_eq!(ctx.cards()?.len(), 2, "both cards restored");
    assert_eq!(ctx.archived_cards()?.len(), 0);
    let restored_a = ctx.get_card(a.id)?.unwrap();
    let restored_b = ctx.get_card(b.id)?.unwrap();
    assert_eq!(restored_a.column_id, col.id);
    assert_eq!(restored_a.position, pre_pos_a);
    assert_eq!(restored_b.position, pre_pos_b);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_inverse_assign_cards_to_sprint_restores_prior_bindings() -> KanbanResult<()> {
    let mut ctx = make_ctx().await;
    let board = ctx.create_board("B".into(), None)?;
    let col = ctx.create_column(board.id, "C".into(), None)?;
    let card_unassigned = ctx.create_card(board.id, col.id, "U".into(), Default::default())?;
    let card_other = ctx.create_card(board.id, col.id, "O".into(), Default::default())?;
    let s1 = ctx.create_sprint(board.id, None, None)?;
    let s2 = ctx.create_sprint(board.id, None, None)?;
    // Put card_other into s1 first; card_unassigned has no sprint.
    ctx.execute(vec![Command::Card(CardCommand::AssignToSprint(
        AssignCardsToSprint {
            ids: vec![card_other.id],
            sprint_id: s1.id,
        },
    ))])?;
    ctx.clear_history()?;

    // Now assign both to s2.
    ctx.execute(vec![Command::Card(CardCommand::AssignToSprint(
        AssignCardsToSprint {
            ids: vec![card_unassigned.id, card_other.id],
            sprint_id: s2.id,
        },
    ))])?;
    assert_eq!(
        ctx.get_card(card_unassigned.id)?.unwrap().sprint_id,
        Some(s2.id)
    );
    assert_eq!(ctx.get_card(card_other.id)?.unwrap().sprint_id, Some(s2.id));

    assert!(ctx.undo()?);
    assert!(
        ctx.get_card(card_unassigned.id)?
            .unwrap()
            .sprint_id
            .is_none(),
        "unassigned card returns to no sprint"
    );
    assert_eq!(
        ctx.get_card(card_other.id)?.unwrap().sprint_id,
        Some(s1.id),
        "other card returns to its prior sprint s1"
    );
    Ok(())
}

/// Undo of AssignCardsToSprint must restore `sprint_logs` to its
/// exact pre-forward contents — pushing more entries (the previous
/// behaviour) would bloat the card's sprint history on every
/// undo/redo cycle.
#[tokio::test(flavor = "multi_thread")]
async fn test_inverse_assign_cards_to_sprint_restores_sprint_logs() -> KanbanResult<()> {
    let mut ctx = make_ctx().await;
    let board = ctx.create_board("B".into(), None)?;
    let col = ctx.create_column(board.id, "C".into(), None)?;
    let card = ctx.create_card(board.id, col.id, "C".into(), Default::default())?;
    let s1 = ctx.create_sprint(board.id, None, None)?;
    let s2 = ctx.create_sprint(board.id, None, None)?;
    ctx.execute(vec![Command::Card(CardCommand::AssignToSprint(
        AssignCardsToSprint {
            ids: vec![card.id],
            sprint_id: s1.id,
        },
    ))])?;
    let baseline_logs = ctx.get_card(card.id)?.unwrap().sprint_logs.clone();
    assert_eq!(baseline_logs.len(), 1, "fixture: one log entry on s1");

    ctx.execute(vec![Command::Card(CardCommand::AssignToSprint(
        AssignCardsToSprint {
            ids: vec![card.id],
            sprint_id: s2.id,
        },
    ))])?;
    assert_eq!(
        ctx.get_card(card.id)?.unwrap().sprint_logs.len(),
        2,
        "forward appends a log for s2"
    );

    assert!(ctx.undo()?);
    let restored = ctx.get_card(card.id)?.unwrap();
    assert_eq!(restored.sprint_id, Some(s1.id), "sprint_id restored to s1");
    assert_eq!(
        restored.sprint_logs, baseline_logs,
        "sprint_logs restored verbatim — no closing entry, no new entry"
    );

    // Redo + undo again must be idempotent.
    assert!(ctx.redo()?);
    assert!(ctx.undo()?);
    let restored = ctx.get_card(card.id)?.unwrap();
    assert_eq!(
        restored.sprint_logs, baseline_logs,
        "sprint_logs stays clean through a full redo + undo cycle"
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_inverse_unassign_card_from_sprint_restores_sprint_logs() -> KanbanResult<()> {
    let mut ctx = make_ctx().await;
    let board = ctx.create_board("B".into(), None)?;
    let col = ctx.create_column(board.id, "C".into(), None)?;
    let card = ctx.create_card(board.id, col.id, "C".into(), Default::default())?;
    let sprint = ctx.create_sprint(board.id, None, None)?;
    ctx.assign_card_to_sprint(card.id, sprint.id)?;
    let baseline_logs = ctx.get_card(card.id)?.unwrap().sprint_logs.clone();
    assert_eq!(baseline_logs.len(), 1);
    ctx.clear_history()?;

    ctx.execute(vec![Command::Card(CardCommand::UnassignFromSprint(
        UnassignCardFromSprint {
            card_id: card.id,
            timestamp: chrono::Utc::now(),
        },
    ))])?;
    let after_forward = ctx.get_card(card.id)?.unwrap();
    assert!(after_forward.sprint_id.is_none());
    assert!(
        after_forward.sprint_logs[0].ended_at.is_some(),
        "forward closes the current log"
    );

    assert!(ctx.undo()?);
    let restored = ctx.get_card(card.id)?.unwrap();
    assert_eq!(restored.sprint_id, Some(sprint.id));
    assert_eq!(
        restored.sprint_logs, baseline_logs,
        "sprint_logs restored verbatim — closing entry undone, not appended"
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_inverse_compact_column_positions_restores_gaps() -> KanbanResult<()> {
    let mut ctx = make_ctx().await;
    let board = ctx.create_board("B".into(), None)?;
    let col = ctx.create_column(board.id, "C".into(), None)?;
    let a = ctx.create_card(board.id, col.id, "A".into(), Default::default())?;
    let b = ctx.create_card(board.id, col.id, "B".into(), Default::default())?;
    let c = ctx.create_card(board.id, col.id, "C".into(), Default::default())?;
    // Create a gap by moving b out then back at a non-sequential pos.
    ctx.execute(vec![Command::Card(CardCommand::Move(MoveCard {
        card_id: b.id,
        new_column_id: col.id,
        new_position: 100,
    }))])?;
    ctx.execute(vec![Command::Card(CardCommand::Move(MoveCard {
        card_id: c.id,
        new_column_id: col.id,
        new_position: 200,
    }))])?;
    let pre_pos_a = ctx.get_card(a.id)?.unwrap().position;
    let pre_pos_b = ctx.get_card(b.id)?.unwrap().position;
    let pre_pos_c = ctx.get_card(c.id)?.unwrap().position;
    ctx.clear_history()?;

    ctx.execute(vec![Command::Card(CardCommand::CompactPositions(
        CompactColumnPositions { column_id: col.id },
    ))])?;
    // After compact, positions are 0, 1, 2.
    let mut cards: Vec<_> = ctx
        .cards()?
        .into_iter()
        .filter(|c| c.column_id == col.id)
        .collect();
    cards.sort_by_key(|c| c.position);
    assert_eq!(
        cards.iter().map(|c| c.position).collect::<Vec<_>>(),
        vec![0, 1, 2]
    );

    assert!(ctx.undo()?);
    assert_eq!(ctx.get_card(a.id)?.unwrap().position, pre_pos_a);
    assert_eq!(ctx.get_card(b.id)?.unwrap().position, pre_pos_b);
    assert_eq!(ctx.get_card(c.id)?.unwrap().position, pre_pos_c);
    Ok(())
}

/// End-to-end execute → undo round-trip for a user-initiated blocks
/// remove. The per-kind `RemoveBlocks::capture_inverse` reads the
/// pre-remove graph to recover the severity, so undo re-adds the
/// edge with the original metadata intact.
#[tokio::test(flavor = "multi_thread")]
async fn test_inverse_remove_blocks_restores_blocks_edge() -> KanbanResult<()> {
    let mut ctx = make_ctx().await;
    let board = ctx.create_board("B".into(), None)?;
    let col = ctx.create_column(board.id, "C".into(), None)?;
    let a = ctx.create_card(board.id, col.id, "A".into(), Default::default())?;
    let b = ctx.create_card(board.id, col.id, "B".into(), Default::default())?;
    ctx.execute(vec![Command::Dependency(DependencyCommand::AddBlocks(
        AddBlocks {
            source: a.id,
            target: b.id,
            severity: Default::default(),
            as_archived: false,
        },
    ))])?;
    ctx.clear_history()?;

    ctx.execute(vec![Command::Dependency(DependencyCommand::RemoveBlocks(
        RemoveBlocks {
            source: a.id,
            target: b.id,
            tolerate_missing: false,
        },
    ))])?;
    assert!(!ctx.graph()?.contains(a.id, b.id));

    assert!(ctx.undo()?);
    assert!(
        ctx.graph()?.contains(a.id, b.id),
        "edge restored by inverse"
    );
    Ok(())
}

/// Pin the end-to-end tolerate_missing replay path: after an
/// `AddSpawns`, the captured inverse is a `RemoveSpawns` with
/// `tolerate_missing = true`. If a later command independently removes
/// the same edge (here a user-initiated detach), the undo of the
/// original AddSpawns must still succeed — the captured inverse's
/// tolerant-Remove path swallows the EdgeNotFound on replay.
///
/// Unit-level tolerance is already pinned in `dependency_commands.rs`
/// (`test_remove_spawns_tolerant_succeeds_on_missing_edge`); this test
/// exercises the same branch through the full execute / undo machinery
/// so a regression that bypasses tolerate_missing somewhere in the
/// service layer (e.g. accidentally constructing the inverse with
/// strict semantics) would fail here.
#[tokio::test(flavor = "multi_thread")]
async fn test_inverse_attach_replays_tolerant_remove_after_independent_detach() -> KanbanResult<()>
{
    use kanban_domain::GraphOperations;
    let mut ctx = make_ctx().await;
    let board = ctx.create_board("B".into(), None)?;
    let col = ctx.create_column(board.id, "C".into(), None)?;
    let parent = ctx.create_card(board.id, col.id, "P".into(), Default::default())?;
    let child = ctx.create_card(board.id, col.id, "C".into(), Default::default())?;
    ctx.clear_history()?;

    // Forward 1: attach. Captured inverse = RemoveSpawns(tolerate_missing=true).
    ctx.attach_child(parent.id, child.id)?;
    assert!(ctx.graph()?.contains(parent.id, child.id));

    // Independent removal: the edge is gone from the graph by the time
    // we undo the original attach. The captured inverse should now
    // execute against an absent edge and succeed via tolerate_missing.
    ctx.detach_child(parent.id, child.id)?;
    assert!(!ctx.graph()?.contains(parent.id, child.id));

    // Undo the most recent command (the detach), then undo the attach.
    // The second undo replays the tolerant RemoveSpawns inverse against
    // an edge that no longer exists; without tolerate_missing this
    // would fail with EdgeNotFound.
    assert!(ctx.undo()?, "undo the detach");
    assert!(
        ctx.graph()?.contains(parent.id, child.id),
        "undo of detach restores the edge"
    );
    assert!(ctx.undo()?, "undo the attach (tolerant remove inverse)");
    assert!(
        !ctx.graph()?.contains(parent.id, child.id),
        "undo of attach removes the edge"
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_inverse_create_card_removes_card_and_archive_trail() -> KanbanResult<()> {
    use kanban_domain::commands::CreateCard;
    let mut ctx = make_ctx().await;
    let board = ctx.create_board("B".into(), Some("KAN".into()))?;
    let col = ctx.create_column(board.id, "C".into(), None)?;
    ctx.clear_history()?;

    let card_id = Uuid::new_v4();
    ctx.execute(vec![Command::Card(CardCommand::Create(CreateCard {
        id: card_id,
        card_number: 1,
        board_id: board.id,
        column_id: col.id,
        title: "Foo".into(),
        position: 0,
        options: Default::default(),
        timestamp: chrono::Utc::now(),
    }))])?;
    assert_eq!(ctx.cards()?.len(), 1);

    assert!(ctx.undo()?);
    assert_eq!(ctx.cards()?.len(), 0, "live card gone after undo");
    assert!(
        ctx.archived_cards()?.is_empty(),
        "no archive trail left behind — undoing a create must allow the \
         enclosing column to be deleted afterwards"
    );

    // Verify the no-trail invariant: deleting the column now succeeds.
    ctx.execute(vec![Command::Column(ColumnCommand::Delete(DeleteColumn {
        column_id: col.id,
    }))])?;
    assert_eq!(ctx.columns()?.len(), 0);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_inverse_restore_card_archives_it() -> KanbanResult<()> {
    use kanban_domain::commands::RestoreCard;
    let mut ctx = make_ctx().await;
    let board = ctx.create_board("B".into(), None)?;
    let col = ctx.create_column(board.id, "C".into(), None)?;
    let card = ctx.create_card(board.id, col.id, "Reborn".into(), Default::default())?;
    ctx.execute(vec![Command::Card(CardCommand::Archive(ArchiveCards {
        ids: vec![card.id],
    }))])?;
    assert_eq!(ctx.cards()?.len(), 0);
    assert_eq!(ctx.archived_cards()?.len(), 1);
    ctx.clear_history()?;

    ctx.execute(vec![Command::Card(CardCommand::Restore(RestoreCard {
        card_id: card.id,
        column_id: col.id,
        position: 0,
        timestamp: chrono::Utc::now(),
    }))])?;
    assert_eq!(ctx.cards()?.len(), 1);
    assert_eq!(ctx.archived_cards()?.len(), 0);

    assert!(ctx.undo()?);
    assert_eq!(
        ctx.cards()?.len(),
        0,
        "RestoreCard undo re-archives the card"
    );
    assert_eq!(ctx.archived_cards()?.len(), 1);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_inverse_create_board_redo_round_trip() -> KanbanResult<()> {
    let mut ctx = make_ctx().await;
    let id = Uuid::new_v4();

    ctx.execute(vec![Command::Board(BoardCommand::Create(CreateBoard {
        id,
        name: "Round-trip".into(),
        card_prefix: None,
        position: 7,
    }))])?;
    ctx.undo()?;
    assert!(ctx.redo()?, "redo must succeed");

    let boards = ctx.boards()?;
    assert_eq!(boards.len(), 1);
    assert_eq!(boards[0].id, id, "redo replays the original id");
    assert_eq!(boards[0].name, "Round-trip");
    assert_eq!(boards[0].position, 7);
    Ok(())
}

/// Composite end-to-end inverse-command round-trip.
///
/// The per-command tests above exercise each `capture_inverse` in
/// isolation. This test composes them: a varied sequence of operations
/// touches the same cards, moves them between columns, swaps sprint
/// bindings, archives and restores. Verifies that undoing every step
/// drives state all the way back to the pre-execute fixture — catching
/// ordering bugs across batches and between commands that share state
/// (e.g. an UpdateCard's inverse seeing the state left by a prior
/// MoveCard) that single-command tests miss.
///
/// The fixture (board + columns + cards) is created up-front and not
/// part of the round-trip. `CreateCard`'s inverse archives rather than
/// deletes (so card-counter ids aren't recycled), which combines with
/// `DeleteColumn`'s archived-cards-protection to make undo-to-empty
/// non-symmetric — see KAN-191 review for the orthogonal followup.
#[tokio::test(flavor = "multi_thread")]
async fn test_composite_round_trip_undo_returns_to_baseline() -> KanbanResult<()> {
    let mut ctx = make_ctx().await;

    // Fixture: not part of the round-trip.
    let board = ctx.create_board("Composite".into(), Some("CMP".into()))?;
    let todo = ctx.create_column(board.id, "TODO".into(), None)?;
    let done = ctx.create_column(board.id, "Done".into(), None)?;
    let c1 = ctx.create_card(board.id, todo.id, "Card 1".into(), Default::default())?;
    let c2 = ctx.create_card(board.id, todo.id, "Card 2".into(), Default::default())?;
    let c3 = ctx.create_card(board.id, todo.id, "Card 3".into(), Default::default())?;
    let sprint_a = ctx.create_sprint(board.id, None, None)?;
    let sprint_b = ctx.create_sprint(board.id, None, None)?;

    ctx.clear_history()?;

    // Baseline snapshot — what we expect to see after undoing the
    // round-trip sequence below.
    let baseline = ctx.snapshot()?;
    let baseline_card_count = baseline.cards.len();
    let baseline_archived = baseline.archived_cards.len();
    let baseline_card_columns: std::collections::HashMap<_, _> =
        baseline.cards.iter().map(|c| (c.id, c.column_id)).collect();
    let baseline_card_sprints: std::collections::HashMap<_, _> =
        baseline.cards.iter().map(|c| (c.id, c.sprint_id)).collect();
    let baseline_card_priorities: std::collections::HashMap<_, _> =
        baseline.cards.iter().map(|c| (c.id, c.priority)).collect();

    // Round-trip sequence — varied operations against fixture entities.
    ctx.move_card(c1.id, done.id, Some(0))?;
    ctx.assign_card_to_sprint(c1.id, sprint_a.id)?;
    ctx.assign_card_to_sprint(c2.id, sprint_a.id)?;
    ctx.update_card(
        c3.id,
        CardUpdate {
            priority: Some(CardPriority::High),
            ..Default::default()
        },
    )?;
    ctx.move_card(c2.id, done.id, Some(1))?;
    ctx.assign_card_to_sprint(c1.id, sprint_b.id)?; // re-assign to a different sprint
    ctx.archive_cards(vec![c3.id])?;
    ctx.update_card(
        c1.id,
        CardUpdate {
            priority: Some(CardPriority::Critical),
            ..Default::default()
        },
    )?;

    const ROUND_TRIP_STEPS: usize = 8;
    assert_eq!(ctx.undo_depth(), ROUND_TRIP_STEPS);

    // Mid-sequence state visibly differs from baseline.
    assert_eq!(ctx.cards()?.len(), baseline_card_count - 1, "c3 archived");
    assert_eq!(ctx.archived_cards()?.len(), baseline_archived + 1);

    // Undo every step.
    for step in (1..=ROUND_TRIP_STEPS).rev() {
        assert!(
            ctx.undo()?,
            "undo step {step} must succeed; undo_depth = {}",
            ctx.undo_depth()
        );
    }

    // Structural assertions: counts, column membership, sprint
    // bindings, priorities. updated_at drift is out of scope (see the
    // module docstring on command_replay.rs).
    let restored = ctx.snapshot()?;
    assert_eq!(restored.boards.len(), baseline.boards.len());
    assert_eq!(restored.columns.len(), baseline.columns.len());
    assert_eq!(restored.cards.len(), baseline_card_count);
    assert_eq!(restored.archived_cards.len(), baseline_archived);
    assert_eq!(restored.sprints.len(), baseline.sprints.len());

    for card in &restored.cards {
        assert_eq!(
            card.column_id, baseline_card_columns[&card.id],
            "card {} column must match baseline after undo",
            card.id
        );
        assert_eq!(
            card.sprint_id, baseline_card_sprints[&card.id],
            "card {} sprint must match baseline after undo",
            card.id
        );
        assert_eq!(
            card.priority, baseline_card_priorities[&card.id],
            "card {} priority must match baseline after undo",
            card.id
        );
    }

    assert!(!ctx.can_undo(), "round-trip exhausted the undo stack");
    assert_eq!(ctx.redo_depth(), ROUND_TRIP_STEPS);
    Ok(())
}

/// After a full undo, redoing every step should return state to the
/// same shape as the original forward sequence produced. Pins the
/// symmetry of the inverse-command model.
#[tokio::test(flavor = "multi_thread")]
async fn test_composite_round_trip_redo_restores_forward_state() -> KanbanResult<()> {
    let mut ctx = make_ctx().await;

    let board = ctx.create_board("Composite".into(), Some("CMP".into()))?;
    let todo = ctx.create_column(board.id, "TODO".into(), None)?;
    let done = ctx.create_column(board.id, "Done".into(), None)?;
    let c1 = ctx.create_card(board.id, todo.id, "Card 1".into(), Default::default())?;
    ctx.clear_history()?;

    // Round-trip-able sequence on the existing card.
    ctx.move_card(c1.id, done.id, Some(0))?;
    ctx.update_card(
        c1.id,
        CardUpdate {
            priority: Some(CardPriority::High),
            ..Default::default()
        },
    )?;

    let forward_snapshot = ctx.snapshot()?;

    // Undo all
    while ctx.can_undo() {
        ctx.undo()?;
    }
    // Redo all
    while ctx.can_redo() {
        assert!(ctx.redo()?, "redo must succeed during full replay");
    }

    let replayed = ctx.snapshot()?;
    assert_eq!(replayed.cards.len(), forward_snapshot.cards.len());
    let replayed_c1 = replayed.cards.iter().find(|c| c.id == c1.id).unwrap();
    let forward_c1 = forward_snapshot
        .cards
        .iter()
        .find(|c| c.id == c1.id)
        .unwrap();
    assert_eq!(replayed_c1.column_id, forward_c1.column_id);
    assert_eq!(replayed_c1.priority, forward_c1.priority);
    assert!(!ctx.can_redo(), "redo stack exhausted after full replay");
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_set_archived_cards_sprint_rejects_top_level_execute() {
    use kanban_domain::commands::{CascadeCommand, SetArchivedCardsSprint};

    let mut ctx = make_ctx().await;
    let cmd = Command::Cascade(CascadeCommand::SetArchivedCardsSprint(
        SetArchivedCardsSprint {
            archived_card_ids: vec![Uuid::new_v4()],
            sprint_id: Uuid::new_v4(),
        },
    ));

    let result = ctx.execute(vec![cmd]);
    let err = result.expect_err(
        "SetArchivedCardsSprint is synthetic-only and must not accept a top-level execute",
    );
    let msg = format!("{err}");
    assert!(
        msg.contains("SetArchivedCardsSprint"),
        "error must name the offending command, got: {msg}"
    );
}

/// A failed undo (inverse rejected by the backend) must leave the
/// UndoStack pinned on the same entry — the user's next attempt should
/// see the same work to do. Engineered by directly deleting the
/// targeted card out from under the command pipeline so the inverse
/// `MoveCard` lookup fails with `NotFound`.
#[tokio::test(flavor = "multi_thread")]
async fn test_failed_undo_leaves_undo_stack_pinned_for_retry() -> KanbanResult<()> {
    let mut ctx = make_ctx().await;
    let board = ctx.create_board("B".into(), None)?;
    let col_a = ctx.create_column(board.id, "A".into(), None)?;
    let col_b = ctx.create_column(board.id, "B".into(), None)?;
    let card = ctx.create_card(board.id, col_a.id, "C".into(), Default::default())?;
    ctx.clear_history()?;

    ctx.execute(vec![Command::Card(CardCommand::Move(MoveCard {
        card_id: card.id,
        new_column_id: col_b.id,
        new_position: 0,
    }))])?;
    assert_eq!(ctx.undo_depth(), 1);

    // Side-channel mutation: delete the card directly so the inverse
    // MoveCard's `get_card` lookup fails inside with_transaction.
    ctx.backend().delete_card(card.id)?;

    let result = ctx.undo();
    assert!(
        result.is_err(),
        "undo should fail when the inverse target is missing"
    );

    assert_eq!(
        ctx.undo_depth(),
        1,
        "failed undo must leave the cursor where it was"
    );
    assert!(
        ctx.can_undo(),
        "retry path must still see the same entry available"
    );
    Ok(())
}

/// Same invariant for redo: a failed redo leaves the cursor pinned.
#[tokio::test(flavor = "multi_thread")]
async fn test_failed_redo_leaves_undo_stack_pinned_for_retry() -> KanbanResult<()> {
    let mut ctx = make_ctx().await;
    let board = ctx.create_board("B".into(), None)?;
    let col_a = ctx.create_column(board.id, "A".into(), None)?;
    let col_b = ctx.create_column(board.id, "B".into(), None)?;
    let card = ctx.create_card(board.id, col_a.id, "C".into(), Default::default())?;
    ctx.clear_history()?;

    ctx.execute(vec![Command::Card(CardCommand::Move(MoveCard {
        card_id: card.id,
        new_column_id: col_b.id,
        new_position: 0,
    }))])?;
    ctx.undo()?;
    assert_eq!(ctx.redo_depth(), 1);

    // Side-channel: delete the card so the forward MoveCard fails on redo.
    ctx.backend().delete_card(card.id)?;

    let result = ctx.redo();
    assert!(
        result.is_err(),
        "redo should fail when the forward target is missing"
    );

    assert_eq!(
        ctx.redo_depth(),
        1,
        "failed redo must leave the cursor where it was"
    );
    assert!(ctx.can_redo(), "retry path must still see the same entry");
    Ok(())
}

/// Regression: create board, create card, archive card, then undo
/// three times. The third undo (of the create-card) used to leave an
/// archived card stranded, which then blocked the column-delete
/// inverse. The fixed CreateCard inverse archives + deletes so no
/// archive trail remains.
#[tokio::test(flavor = "multi_thread")]
async fn test_undo_chain_through_archive_create_create_column_succeeds() -> KanbanResult<()> {
    let mut ctx = make_ctx().await;
    let board = ctx.create_board("B".into(), None)?;
    let col = ctx.create_column(board.id, "TODO".into(), None)?;
    let card = ctx.create_card(board.id, col.id, "C1".into(), Default::default())?;
    ctx.archive_cards(vec![card.id])?;
    assert_eq!(ctx.archived_cards()?.len(), 1);

    // Undo archive — card is live again.
    assert!(ctx.undo()?);
    assert_eq!(ctx.cards()?.len(), 1);
    assert_eq!(ctx.archived_cards()?.len(), 0);

    // Undo create_card — card is gone with no archive trail.
    assert!(ctx.undo()?);
    assert_eq!(ctx.cards()?.len(), 0);
    assert_eq!(
        ctx.archived_cards()?.len(),
        0,
        "CreateCard inverse must leave no archive trail"
    );

    // Undo create_column — column delete should not be blocked by a
    // stale archive entry.
    assert!(ctx.undo()?);
    assert_eq!(ctx.columns()?.len(), 0);

    // Undo create_board.
    assert!(ctx.undo()?);
    assert_eq!(ctx.boards()?.len(), 0);

    assert!(!ctx.can_undo());
    Ok(())
}

/// Undo of `KanbanContext::delete_board` must restore the full cascade:
/// the board itself, its columns, live and archived cards, sprints,
/// and dependency-graph edges that lived among those cards. Pins the
/// composition of the 6-command DeleteBoard cascade through its
/// reverse-order inverse batch.
#[tokio::test(flavor = "multi_thread")]
async fn test_inverse_delete_board_restores_full_cascade() -> KanbanResult<()> {
    let mut ctx = make_ctx().await;
    let board = ctx.create_board("Cascade".into(), Some("CSC".into()))?;
    let todo = ctx.create_column(board.id, "TODO".into(), None)?;
    let done = ctx.create_column(board.id, "Done".into(), None)?;
    let c1 = ctx.create_card(board.id, todo.id, "C1".into(), Default::default())?;
    let c2 = ctx.create_card(board.id, todo.id, "C2".into(), Default::default())?;
    let c3 = ctx.create_card(board.id, done.id, "C3".into(), Default::default())?;
    let sprint_a = ctx.create_sprint(board.id, None, None)?;
    let _sprint_b = ctx.create_sprint(board.id, None, None)?;
    ctx.assign_card_to_sprint(c1.id, sprint_a.id)?;
    ctx.archive_cards(vec![c2.id])?;

    // Add a graph edge so the cascade has something to clean up.
    let mut graph = ctx.graph()?;
    graph.set_block(c1.id, c3.id).unwrap();
    ctx.backend().set_graph(graph)?;
    ctx.clear_history()?;

    let baseline = ctx.snapshot()?;
    let baseline_board_ids: std::collections::HashSet<_> =
        baseline.boards.iter().map(|b| b.id).collect();
    let baseline_column_ids: std::collections::HashSet<_> =
        baseline.columns.iter().map(|c| c.id).collect();
    let baseline_card_ids: std::collections::HashSet<_> =
        baseline.cards.iter().map(|c| c.id).collect();
    let baseline_archived_ids: std::collections::HashSet<_> = baseline
        .archived_cards
        .iter()
        .map(|ac| ac.entity_id)
        .collect();
    let baseline_sprint_ids: std::collections::HashSet<_> =
        baseline.sprints.iter().map(|s| s.id).collect();
    let baseline_edge_count = baseline.graph.len();
    assert!(baseline_edge_count > 0, "fixture must include graph edges");

    ctx.delete_board(board.id)?;

    assert!(ctx.boards()?.is_empty(), "board deleted");
    assert!(ctx.columns()?.is_empty(), "columns deleted");
    assert!(ctx.cards()?.is_empty(), "live cards deleted");
    assert!(ctx.archived_cards()?.is_empty(), "archived cards deleted");
    assert!(ctx.sprints()?.is_empty(), "sprints deleted");
    assert_eq!(ctx.graph()?.len(), 0, "card graph edges deleted");

    assert!(ctx.undo()?, "cascade undo must succeed");

    let restored = ctx.snapshot()?;
    let restored_board_ids: std::collections::HashSet<_> =
        restored.boards.iter().map(|b| b.id).collect();
    let restored_column_ids: std::collections::HashSet<_> =
        restored.columns.iter().map(|c| c.id).collect();
    let restored_card_ids: std::collections::HashSet<_> =
        restored.cards.iter().map(|c| c.id).collect();
    let restored_archived_ids: std::collections::HashSet<_> = restored
        .archived_cards
        .iter()
        .map(|ac| ac.entity_id)
        .collect();
    let restored_sprint_ids: std::collections::HashSet<_> =
        restored.sprints.iter().map(|s| s.id).collect();

    assert_eq!(restored_board_ids, baseline_board_ids, "board restored");
    assert_eq!(restored_column_ids, baseline_column_ids, "columns restored");
    assert_eq!(restored_card_ids, baseline_card_ids, "live cards restored");
    assert_eq!(
        restored_archived_ids, baseline_archived_ids,
        "archived cards restored"
    );
    assert_eq!(restored_sprint_ids, baseline_sprint_ids, "sprints restored");
    assert_eq!(
        restored.graph.len(),
        baseline_edge_count,
        "graph edges restored"
    );

    // Verify a per-card invariant beyond just id presence: c1's sprint
    // binding must be intact.
    let c1_restored = restored.cards.iter().find(|c| c.id == c1.id).unwrap();
    assert_eq!(
        c1_restored.sprint_id,
        Some(sprint_a.id),
        "card sprint binding survives cascade undo"
    );
    Ok(())
}

/// Cascade-undo must preserve the archive state of incident edges.
/// Pre-fix, `edges_to_undo_commands` issued `Add*` commands that
/// always landed active, so an archived edge incident to a deleted
/// card would silently revive on undo — losing the soft-delete state
/// the user had recorded. Post-fix, the helper sets `as_archived`
/// from `!e.is_active()` so archived edges restore as archived and
/// active edges restore as active.
#[tokio::test(flavor = "multi_thread")]
async fn test_inverse_cascade_preserves_archived_incident_edge_state() -> KanbanResult<()> {
    use kanban_core::Edge as _;
    let mut ctx = make_ctx().await;
    let board = ctx.create_board("B".into(), None)?;
    let col = ctx.create_column(board.id, "C".into(), None)?;
    let a = ctx.create_card(board.id, col.id, "A".into(), Default::default())?;
    let b = ctx.create_card(board.id, col.id, "B".into(), Default::default())?;
    let c = ctx.create_card(board.id, col.id, "C".into(), Default::default())?;
    // Two edges incident to A: one to B (will be archived), one to C
    // (will stay active).
    let mut graph = ctx.graph()?;
    graph.set_block(a.id, b.id).unwrap();
    graph.set_block(a.id, c.id).unwrap();
    // Archive node B; the cascade archives the A->B edge but leaves
    // A->C active (B is not incident to A->C).
    graph.archive_node(b.id);
    ctx.backend().set_graph(graph)?;
    ctx.clear_history()?;

    // Baseline counts (active = 1, archived = 1).
    let baseline = ctx.graph()?;
    assert_eq!(baseline.len(), 2, "two incident edges in history");
    assert_eq!(baseline.active_len(), 1, "one active");
    assert!(
        baseline.contains_archived(a.id, b.id),
        "A->B archived in baseline"
    );
    assert!(baseline.contains(a.id, c.id), "A->C active in baseline");

    // Delete the board (cascade removes A, B, C and all their edges).
    ctx.delete_board(board.id)?;
    assert_eq!(ctx.graph()?.len(), 0, "graph cleared by cascade");

    // Undo restores everything.
    assert!(ctx.undo()?);

    let restored = ctx.graph()?;
    assert_eq!(restored.len(), 2, "both edges restored in history");
    assert_eq!(
        restored.active_len(),
        1,
        "active count preserved (A->C stays active, A->B stays archived)"
    );
    assert!(
        restored.contains_archived(a.id, b.id) && !restored.contains(a.id, b.id),
        "A->B restored as archived"
    );
    assert!(restored.contains(a.id, c.id), "A->C restored as active");

    // Per-edge active flag round-trip.
    let blocks: Vec<_> = restored.blocks_edges().iter().collect();
    let ab = blocks
        .iter()
        .find(|e| e.source() == a.id && e.target() == b.id)
        .expect("A->B restored");
    let ac = blocks
        .iter()
        .find(|e| e.source() == a.id && e.target() == c.id)
        .expect("A->C restored");
    assert!(!ab.is_active(), "A->B archived state preserved");
    assert!(ac.is_active(), "A->C active state preserved");
    Ok(())
}
