use super::super::BackendFactory;
use super::assert_card_eq;
use crate::KanbanContext;
use kanban_core::AppConfig;
use kanban_domain::dependencies::edge_meta::Severity;
use kanban_domain::{
    Card, CardUpdate, CreateCardOptions, FieldUpdate, GraphOperations, KanbanOperations,
    KanbanResult, UndoOperations,
};
use tempfile::TempDir;
use uuid::Uuid;

pub struct TwoBoards {
    pub board_a: Uuid,
    pub board_b: Uuid,
    pub a_col: Uuid,
    pub b_col: Uuid,
    pub a1: Uuid,
    pub a2: Uuid,
    pub b1: Uuid,
    pub sa: Uuid,
    pub sb: Uuid,
}

pub async fn seed_two_boards(ctx: &mut KanbanContext) -> KanbanResult<TwoBoards> {
    let board_a = ctx.create_board("A".into(), Some("AAA".into()))?;
    let a_col = ctx.create_column(board_a.id, "Col".into(), None)?;
    let a1 = ctx.create_card(
        board_a.id,
        a_col.id,
        "A1".into(),
        CreateCardOptions::default(),
    )?;
    let a2 = ctx.create_card(
        board_a.id,
        a_col.id,
        "A2".into(),
        CreateCardOptions::default(),
    )?;
    let sa = ctx.create_sprint(board_a.id, None, None)?;

    let board_b = ctx.create_board("B".into(), Some("BBB".into()))?;
    let b_col = ctx.create_column(board_b.id, "Col".into(), None)?;
    let b1 = ctx.create_card(
        board_b.id,
        b_col.id,
        "B1".into(),
        CreateCardOptions::default(),
    )?;
    let sb = ctx.create_sprint(board_b.id, None, None)?;

    ctx.block(a1.id, a2.id, Severity::High)?;
    ctx.assign_card_to_sprint(a1.id, sa.id)?;

    Ok(TwoBoards {
        board_a: board_a.id,
        board_b: board_b.id,
        a_col: a_col.id,
        b_col: b_col.id,
        a1: a1.id,
        a2: a2.id,
        b1: b1.id,
        sa: sa.id,
        sb: sb.id,
    })
}

/// Bind a card to a sprint by writing directly through the data store,
/// bypassing `AssignCardsToSprint`'s board check entirely. Used to simulate a
/// pre-existing cross-board binding that predates this invariant.
pub fn bind_raw(ctx: &KanbanContext, card_id: Uuid, sprint_id: Uuid) -> KanbanResult<()> {
    let mut card = ctx
        .data_store()
        .get_card(card_id)?
        .ok_or_else(|| kanban_domain::KanbanError::not_found("Card", card_id))?;
    let sprint = ctx
        .data_store()
        .get_sprint(sprint_id)?
        .ok_or_else(|| kanban_domain::KanbanError::not_found("Sprint", sprint_id))?;
    card.assign_to_sprint(
        sprint.id,
        sprint.sprint_number,
        None::<String>,
        "Planning",
        chrono::Utc::now(),
    );
    ctx.data_store().upsert_card(card)
}

pub struct GraphState {
    pub cards: Vec<Card>,
    pub sprints: Vec<(Uuid, Uuid)>,
    pub columns: Vec<(Uuid, Uuid)>,
    pub a1_blocks: Vec<Uuid>,
}

pub fn graph_state(ctx: &KanbanContext, a1: Uuid) -> KanbanResult<GraphState> {
    let mut cards = ctx.data_store().list_all_cards()?;
    cards.sort_by_key(|c| c.id);
    let mut sprints: Vec<(Uuid, Uuid)> = ctx
        .data_store()
        .list_all_sprints()?
        .into_iter()
        .map(|s| (s.id, s.board_id))
        .collect();
    sprints.sort();
    let mut columns: Vec<(Uuid, Uuid)> = ctx
        .data_store()
        .list_all_columns()?
        .into_iter()
        .map(|c| (c.id, c.board_id))
        .collect();
    columns.sort();
    let a1_blocks = ctx.list_blocked_by(a1)?;
    Ok(GraphState {
        cards,
        sprints,
        columns,
        a1_blocks,
    })
}

pub fn assert_graph_unchanged(before: &GraphState, after: &GraphState) {
    assert_eq!(before.cards.len(), after.cards.len(), "card count");
    for (b, a) in before.cards.iter().zip(after.cards.iter()) {
        super::assert_card_eq(b, a);
    }
    assert_eq!(before.sprints, after.sprints, "sprints");
    assert_eq!(before.columns, after.columns, "columns");
    assert_eq!(before.a1_blocks, after.a1_blocks, "a1 blocked-by edges");
}

pub async fn test_binding_a_card_to_a_sprint_on_another_board_is_refused_on_every_backend(
    factory: &BackendFactory,
) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let mut ctx = KanbanContext::open(factory(&path), AppConfig::default())
        .await
        .unwrap();
    let fx = seed_two_boards(&mut ctx).await.unwrap();

    let before = graph_state(&ctx, fx.a1).unwrap();

    assert!(ctx
        .assign_card_to_sprint(fx.a2, fx.sb)
        .unwrap_err()
        .is_sprint_board_mismatch());
    assert!(ctx
        .assign_cards_to_sprint(vec![fx.a1, fx.a2], fx.sb)
        .unwrap_err()
        .is_sprint_board_mismatch());
    assert!(ctx
        .update_card(
            fx.a2,
            CardUpdate {
                sprint_id: FieldUpdate::Set(fx.sb),
                ..Default::default()
            },
        )
        .unwrap_err()
        .is_sprint_board_mismatch());

    ctx.save().await.unwrap();
    let ctx = KanbanContext::open_deferred(factory(&path), AppConfig::default());
    let after = graph_state(&ctx, fx.a1).unwrap();
    assert_graph_unchanged(&before, &after);
}

pub async fn test_carry_over_to_a_sprint_on_another_board_is_refused_and_moves_nothing(
    factory: &BackendFactory,
) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let mut ctx = KanbanContext::open(factory(&path), AppConfig::default())
        .await
        .unwrap();
    let fx = seed_two_boards(&mut ctx).await.unwrap();

    ctx.activate_sprint(fx.sa, Some(14)).unwrap();
    ctx.complete_sprint(fx.sa).unwrap();

    let before = graph_state(&ctx, fx.a1).unwrap();
    assert!(ctx
        .carry_over_sprint_cards(fx.sa, fx.sb)
        .unwrap_err()
        .is_sprint_board_mismatch());

    ctx.save().await.unwrap();
    let ctx = KanbanContext::open_deferred(factory(&path), AppConfig::default());
    let after = graph_state(&ctx, fx.a1).unwrap();
    assert_graph_unchanged(&before, &after);

    let mut ctx = ctx;
    let empty_sprint = ctx.create_sprint(fx.board_a, None, None).unwrap();
    ctx.activate_sprint(empty_sprint.id, Some(14)).unwrap();
    ctx.complete_sprint(empty_sprint.id).unwrap();
    assert!(ctx
        .carry_over_sprint_cards(empty_sprint.id, fx.sb)
        .unwrap_err()
        .is_sprint_board_mismatch());
}

pub async fn test_carry_over_leaves_a_pre_existing_cross_board_card_on_the_source_sprint(
    factory: &BackendFactory,
) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let mut ctx = KanbanContext::open(factory(&path), AppConfig::default())
        .await
        .unwrap();
    let fx = seed_two_boards(&mut ctx).await.unwrap();

    bind_raw(&ctx, fx.b1, fx.sa).unwrap();
    ctx.activate_sprint(fx.sa, Some(14)).unwrap();
    ctx.complete_sprint(fx.sa).unwrap();
    let sa2 = ctx.create_sprint(fx.board_a, None, None).unwrap();

    let moved = ctx.carry_over_sprint_cards(fx.sa, sa2.id).unwrap();
    assert_eq!(moved, 1);

    ctx.save().await.unwrap();
    let ctx = KanbanContext::open_deferred(factory(&path), AppConfig::default());
    let b1 = ctx.get_card(fx.b1).unwrap().unwrap();
    assert_eq!(b1.sprint_id, Some(fx.sa));
    let a1 = ctx.get_card(fx.a1).unwrap().unwrap();
    assert_eq!(a1.sprint_id, Some(sa2.id));
}

pub async fn test_detailed_assign_fails_only_the_cards_on_another_board(factory: &BackendFactory) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let mut ctx = KanbanContext::open(factory(&path), AppConfig::default())
        .await
        .unwrap();
    let fx = seed_two_boards(&mut ctx).await.unwrap();

    let (result, _invalidation) = ctx.assign_cards_to_sprint_detailed(vec![fx.a2, fx.b1], fx.sb);
    assert_eq!(result.succeeded, vec![fx.b1]);
    assert_eq!(result.failed.len(), 1);
    assert_eq!(result.failed[0].id, fx.a2);
    assert!(
        result.failed[0].error.contains("belongs to board"),
        "error: {}",
        result.failed[0].error
    );

    ctx.save().await.unwrap();
    let ctx = KanbanContext::open_deferred(factory(&path), AppConfig::default());
    let b1 = ctx.get_card(fx.b1).unwrap().unwrap();
    assert_eq!(b1.sprint_id, Some(fx.sb));
    let a2 = ctx.get_card(fx.a2).unwrap().unwrap();
    assert_eq!(a2.sprint_id, None);
}

pub async fn test_update_moving_a_card_cross_board_with_a_sprint_on_the_target_board_succeeds(
    factory: &BackendFactory,
) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let mut ctx = KanbanContext::open(factory(&path), AppConfig::default())
        .await
        .unwrap();
    let fx = seed_two_boards(&mut ctx).await.unwrap();

    ctx.update_card(
        fx.a1,
        CardUpdate {
            column_id: Some(fx.b_col),
            sprint_id: FieldUpdate::Set(fx.sb),
            ..Default::default()
        },
    )
    .unwrap();

    ctx.save().await.unwrap();
    let ctx = KanbanContext::open_deferred(factory(&path), AppConfig::default());
    let a1 = ctx.get_card(fx.a1).unwrap().unwrap();
    assert_eq!(a1.board_id, fx.board_b);
    assert_eq!(a1.sprint_id, Some(fx.sb));
}

pub async fn test_update_with_status_and_a_cross_board_column_and_sprint_is_refused(
    factory: &BackendFactory,
) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let mut ctx = KanbanContext::open(factory(&path), AppConfig::default())
        .await
        .unwrap();
    let fx = seed_two_boards(&mut ctx).await.unwrap();

    let before = graph_state(&ctx, fx.a1).unwrap();
    assert!(ctx
        .update_card(
            fx.a2,
            CardUpdate {
                status: Some(kanban_domain::CardStatus::Todo),
                column_id: Some(fx.b_col),
                sprint_id: FieldUpdate::Set(fx.sb),
                ..Default::default()
            },
        )
        .unwrap_err()
        .is_sprint_board_mismatch());

    ctx.save().await.unwrap();
    let ctx = KanbanContext::open_deferred(factory(&path), AppConfig::default());
    let after = graph_state(&ctx, fx.a1).unwrap();
    assert_graph_unchanged(&before, &after);
}

pub async fn test_a_pre_existing_cross_board_binding_still_reads_and_survives_an_unrelated_edit(
    factory: &BackendFactory,
) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let mut ctx = KanbanContext::open(factory(&path), AppConfig::default())
        .await
        .unwrap();
    let fx = seed_two_boards(&mut ctx).await.unwrap();

    bind_raw(&ctx, fx.a2, fx.sb).unwrap();

    ctx.save().await.unwrap();
    let mut ctx = KanbanContext::open_deferred(factory(&path), AppConfig::default());
    let a2 = ctx.get_card(fx.a2).unwrap().unwrap();
    assert_eq!(a2.sprint_id, Some(fx.sb));

    ctx.update_card(
        fx.a2,
        CardUpdate {
            title: Some("Renamed".into()),
            ..Default::default()
        },
    )
    .unwrap();
    ctx.update_card(
        fx.a2,
        CardUpdate {
            sprint_id: FieldUpdate::Set(fx.sb),
            ..Default::default()
        },
    )
    .unwrap();
}

pub async fn test_undo_delete_sprint_restores_a_pre_existing_cross_board_binding_verbatim(
    factory: &BackendFactory,
) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let mut ctx = KanbanContext::open(factory(&path), AppConfig::default())
        .await
        .unwrap();
    let fx = seed_two_boards(&mut ctx).await.unwrap();

    bind_raw(&ctx, fx.a2, fx.sb).unwrap();

    let before = graph_state(&ctx, fx.a1).unwrap();
    ctx.delete_sprint(fx.sb).unwrap();
    ctx.undo().unwrap();

    ctx.save().await.unwrap();
    let ctx = KanbanContext::open_deferred(factory(&path), AppConfig::default());
    let after = graph_state(&ctx, fx.a1).unwrap();
    assert_graph_unchanged(&before, &after);
}

pub async fn test_moving_a_sprint_bound_card_cross_board_detaches_it_and_undo_restores_the_whole_graph(
    factory: &BackendFactory,
) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let mut ctx = KanbanContext::open(factory(&path), AppConfig::default())
        .await
        .unwrap();
    let fx = seed_two_boards(&mut ctx).await.unwrap();

    let before = graph_state(&ctx, fx.a1).unwrap();

    ctx.move_card(fx.a1, fx.b_col, None).unwrap();
    let a1 = ctx.get_card(fx.a1).unwrap().unwrap();
    assert_eq!(a1.sprint_id, None);
    assert_eq!(a1.board_id, fx.board_b);
    assert!(a1
        .sprint_logs
        .iter()
        .find(|l| l.sprint_id == fx.sa)
        .expect("sa log entry")
        .ended_at
        .is_some());

    ctx.undo().unwrap();
    let a1 = ctx.get_card(fx.a1).unwrap().unwrap();
    assert_eq!(a1.sprint_id, Some(fx.sa));
    ctx.redo().unwrap();
    let a1 = ctx.get_card(fx.a1).unwrap().unwrap();
    assert_eq!(a1.sprint_id, None);

    ctx.undo().unwrap();
    ctx.save().await.unwrap();
    let ctx = KanbanContext::open_deferred(factory(&path), AppConfig::default());
    let after = graph_state(&ctx, fx.a1).unwrap();
    assert_graph_unchanged(&before, &after);
}

pub async fn test_batch_moving_sprint_bound_cards_cross_board_detaches_each(
    factory: &BackendFactory,
) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let mut ctx = KanbanContext::open(factory(&path), AppConfig::default())
        .await
        .unwrap();
    let fx = seed_two_boards(&mut ctx).await.unwrap();

    ctx.assign_card_to_sprint(fx.a2, fx.sa).unwrap();

    let before = graph_state(&ctx, fx.a1).unwrap();
    ctx.move_cards(vec![fx.a1, fx.a2], fx.b_col).unwrap();

    let a1 = ctx.get_card(fx.a1).unwrap().unwrap();
    assert_eq!(a1.sprint_id, None);
    assert!(a1
        .sprint_logs
        .iter()
        .find(|l| l.sprint_id == fx.sa)
        .expect("sa log entry")
        .ended_at
        .is_some());
    let a2 = ctx.get_card(fx.a2).unwrap().unwrap();
    assert_eq!(a2.sprint_id, None);
    assert!(a2
        .sprint_logs
        .iter()
        .find(|l| l.sprint_id == fx.sa)
        .expect("sa log entry")
        .ended_at
        .is_some());

    ctx.undo().unwrap();
    ctx.save().await.unwrap();
    let ctx = KanbanContext::open_deferred(factory(&path), AppConfig::default());
    let after = graph_state(&ctx, fx.a1).unwrap();
    assert_graph_unchanged(&before, &after);
}

pub async fn test_update_moving_a_card_cross_board_closes_the_old_sprint_log(
    factory: &BackendFactory,
) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let mut ctx = KanbanContext::open(factory(&path), AppConfig::default())
        .await
        .unwrap();
    let fx = seed_two_boards(&mut ctx).await.unwrap();

    ctx.update_card(
        fx.a1,
        CardUpdate {
            column_id: Some(fx.b_col),
            sprint_id: FieldUpdate::Set(fx.sb),
            ..Default::default()
        },
    )
    .unwrap();

    ctx.assign_card_to_sprint(fx.a2, fx.sa).unwrap();
    ctx.update_card(
        fx.a2,
        CardUpdate {
            column_id: Some(fx.b_col),
            ..Default::default()
        },
    )
    .unwrap();

    ctx.save().await.unwrap();
    let ctx = KanbanContext::open_deferred(factory(&path), AppConfig::default());
    let a1 = ctx.get_card(fx.a1).unwrap().unwrap();
    assert_eq!(a1.sprint_id, Some(fx.sb));
    assert!(a1
        .sprint_logs
        .iter()
        .find(|l| l.sprint_id == fx.sa)
        .expect("sa log entry")
        .ended_at
        .is_some());
    let a2 = ctx.get_card(fx.a2).unwrap().unwrap();
    assert_eq!(a2.sprint_id, None);
}

pub async fn test_update_moving_a_card_cross_board_while_resubmitting_its_old_sprint_is_refused(
    factory: &BackendFactory,
) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let mut ctx = KanbanContext::open(factory(&path), AppConfig::default())
        .await
        .unwrap();
    let fx = seed_two_boards(&mut ctx).await.unwrap();

    let before = graph_state(&ctx, fx.a1).unwrap();
    assert!(ctx
        .update_card(
            fx.a1,
            CardUpdate {
                column_id: Some(fx.b_col),
                sprint_id: FieldUpdate::Set(fx.sa),
                ..Default::default()
            },
        )
        .unwrap_err()
        .is_sprint_board_mismatch());

    ctx.save().await.unwrap();
    let ctx = KanbanContext::open_deferred(factory(&path), AppConfig::default());
    let after = graph_state(&ctx, fx.a1).unwrap();
    assert_graph_unchanged(&before, &after);
}

pub async fn test_restoring_an_archived_card_into_another_boards_column_detaches_its_sprint(
    factory: &BackendFactory,
) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let mut ctx = KanbanContext::open(factory(&path), AppConfig::default())
        .await
        .unwrap();
    let fx = seed_two_boards(&mut ctx).await.unwrap();

    ctx.archive_card(fx.a1).unwrap();
    ctx.restore_card(fx.a1, Some(fx.b_col)).unwrap();

    ctx.save().await.unwrap();
    let ctx = KanbanContext::open_deferred(factory(&path), AppConfig::default());
    let a1 = ctx.get_card(fx.a1).unwrap().unwrap();
    assert_eq!(a1.sprint_id, None);
    assert_eq!(a1.board_id, fx.board_b);
    let a2 = ctx.get_card(fx.a2).unwrap().unwrap();
    assert_eq!(a2.sprint_id, None);
    let b1 = ctx.get_card(fx.b1).unwrap().unwrap();
    assert_eq!(b1.sprint_id, None);
    let sprints: std::collections::HashSet<Uuid> = ctx
        .data_store()
        .list_all_sprints()
        .unwrap()
        .into_iter()
        .map(|s| s.id)
        .collect();
    assert_eq!(sprints, std::collections::HashSet::from([fx.sa, fx.sb]));
    let columns: std::collections::HashSet<Uuid> = ctx
        .data_store()
        .list_all_columns()
        .unwrap()
        .into_iter()
        .map(|c| c.id)
        .collect();
    assert_eq!(
        columns,
        std::collections::HashSet::from([fx.a_col, fx.b_col])
    );
    assert_eq!(ctx.list_blocked_by(fx.a1).unwrap(), vec![fx.a2]);
}

pub async fn test_undo_restoring_a_bound_card_into_another_boards_column_restores_the_whole_graph(
    factory: &BackendFactory,
) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let mut ctx = KanbanContext::open(factory(&path), AppConfig::default())
        .await
        .unwrap();
    let fx = seed_two_boards(&mut ctx).await.unwrap();

    ctx.archive_card(fx.a1).unwrap();
    let before = graph_state(&ctx, fx.a2).unwrap();

    ctx.restore_card(fx.a1, Some(fx.b_col)).unwrap();
    let a1 = ctx.get_card(fx.a1).unwrap().unwrap();
    assert_eq!(a1.board_id, fx.board_b);
    assert_eq!(a1.sprint_id, None);
    let first_restore = a1;

    ctx.undo().unwrap();
    let a1 = ctx.get_card(fx.a1).unwrap().unwrap();
    assert_eq!(a1.board_id, fx.board_a);
    assert_eq!(a1.column_id, fx.a_col);
    assert_eq!(a1.sprint_id, Some(fx.sa));
    assert!(
        ctx.data_store().get_archived_card(fx.a1).unwrap().is_some(),
        "undo must leave the card archived again"
    );

    ctx.redo().unwrap();
    let a1 = ctx.get_card(fx.a1).unwrap().unwrap();
    assert_card_eq(&first_restore, &a1);

    ctx.undo().unwrap();
    ctx.save().await.unwrap();
    let ctx = KanbanContext::open_deferred(factory(&path), AppConfig::default());
    let after = graph_state(&ctx, fx.a2).unwrap();
    assert_graph_unchanged(&before, &after);
    assert!(
        ctx.data_store().get_archived_card(fx.a1).unwrap().is_some(),
        "undo must leave the card archived again after reload"
    );
}

pub async fn test_undo_moving_a_card_onto_its_cross_board_sprints_board_restores_the_binding(
    factory: &BackendFactory,
) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let mut ctx = KanbanContext::open(factory(&path), AppConfig::default())
        .await
        .unwrap();
    let fx = seed_two_boards(&mut ctx).await.unwrap();

    bind_raw(&ctx, fx.a2, fx.sb).unwrap();

    let before = graph_state(&ctx, fx.a1).unwrap();
    ctx.move_card(fx.a2, fx.b_col, None).unwrap();
    let a2 = ctx.get_card(fx.a2).unwrap().unwrap();
    assert_eq!(a2.sprint_id, Some(fx.sb));

    ctx.undo().unwrap();

    ctx.save().await.unwrap();
    let ctx = KanbanContext::open_deferred(factory(&path), AppConfig::default());
    let after = graph_state(&ctx, fx.a1).unwrap();
    assert_graph_unchanged(&before, &after);
}
