use super::super::BackendFactory;
use super::sprint_board::{assert_graph_unchanged, graph_state, seed_two_boards};
use crate::KanbanContext;
use kanban_core::AppConfig;
use kanban_domain::{
    CardStatus, CardUpdate, CreateCardOptions, Invalidation, KanbanOperations, UndoOperations,
};
use std::collections::HashSet;
use tempfile::TempDir;

pub async fn test_move_card_between_columns_roundtrip(factory: &BackendFactory) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let mut ctx = KanbanContext::open(factory(&path), AppConfig::default())
        .await
        .unwrap();

    let board = ctx.create_board("Board".into(), None).unwrap();
    let col1 = ctx.create_column(board.id, "Todo".into(), Some(0)).unwrap();
    let col2 = ctx.create_column(board.id, "Done".into(), Some(1)).unwrap();

    let card = ctx
        .create_card(
            board.id,
            col1.id,
            "Moving Card".into(),
            CreateCardOptions::default(),
        )
        .unwrap();

    ctx.move_card(card.id, col2.id, Some(0)).unwrap();

    ctx.save().await.unwrap();
    let ctx = KanbanContext::open_deferred(factory(&path), AppConfig::default());

    let c = ctx.get_card(card.id).unwrap().unwrap();
    assert_eq!(c.column_id, col2.id);
    assert_eq!(c.position, 0);
}

pub async fn test_update_with_status_and_a_cross_board_column_moves_the_card_and_undo_restores_the_whole_graph(
    factory: &BackendFactory,
) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let mut ctx = KanbanContext::open(factory(&path), AppConfig::default())
        .await
        .unwrap();
    let fx = seed_two_boards(&mut ctx).await.unwrap();

    let mut before = graph_state(&ctx, fx.a1).unwrap();

    let updated = ctx
        .update_card(
            fx.a1,
            CardUpdate {
                status: Some(CardStatus::InProgress),
                column_id: Some(fx.b_col),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(updated.column_id, fx.b_col);
    assert_eq!(updated.board_id, fx.board_b);
    assert_eq!(updated.position, 1);
    assert_eq!(updated.status, CardStatus::InProgress);
    assert_eq!(updated.sprint_id, None);
    assert!(updated
        .sprint_logs
        .iter()
        .find(|l| l.sprint_id == fx.sa)
        .expect("sa log entry")
        .ended_at
        .is_some());

    ctx.undo().unwrap();
    let a1 = ctx.get_card(fx.a1).unwrap().unwrap();
    assert_eq!(a1.board_id, fx.board_a);
    assert_eq!(a1.column_id, fx.a_col);
    assert_eq!(a1.position, 0);
    assert_eq!(a1.sprint_id, Some(fx.sa));
    assert!(a1
        .sprint_logs
        .iter()
        .find(|l| l.sprint_id == fx.sa)
        .expect("sa log entry")
        .ended_at
        .is_none());

    ctx.redo().unwrap();
    let a1 = ctx.get_card(fx.a1).unwrap().unwrap();
    assert_eq!(a1.board_id, fx.board_b);
    assert_eq!(a1.sprint_id, None);

    ctx.undo().unwrap();
    ctx.save().await.unwrap();
    let ctx = KanbanContext::open_deferred(factory(&path), AppConfig::default());
    let after = graph_state(&ctx, fx.a1).unwrap();
    before
        .cards
        .iter_mut()
        .find(|c| c.id == fx.a1)
        .unwrap()
        .updated_at = after
        .cards
        .iter()
        .find(|c| c.id == fx.a1)
        .unwrap()
        .updated_at;
    assert_graph_unchanged(&before, &after);
}

pub async fn test_update_with_status_and_a_same_board_column_appends_like_a_move(
    factory: &BackendFactory,
) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let mut ctx = KanbanContext::open(factory(&path), AppConfig::default())
        .await
        .unwrap();
    let fx = seed_two_boards(&mut ctx).await.unwrap();

    let a_col2 = ctx.create_column(fx.board_a, "Col 2".into(), None).unwrap();
    let a3 = ctx
        .create_card(
            fx.board_a,
            a_col2.id,
            "A3".into(),
            CreateCardOptions::default(),
        )
        .unwrap();

    let updated = ctx
        .update_card(
            fx.a1,
            CardUpdate {
                status: Some(CardStatus::InProgress),
                column_id: Some(a_col2.id),
                ..Default::default()
            },
        )
        .unwrap();

    assert_eq!(updated.column_id, a_col2.id);
    assert_eq!(updated.position, 1);
    assert_eq!(updated.board_id, fx.board_a);
    assert_eq!(updated.sprint_id, Some(fx.sa));
    assert!(updated
        .sprint_logs
        .iter()
        .find(|l| l.sprint_id == fx.sa)
        .expect("sa log entry")
        .ended_at
        .is_none());
    assert_eq!(updated.status, CardStatus::InProgress);

    let a3 = ctx.get_card(a3.id).unwrap().unwrap();
    assert_eq!(a3.position, 0);
}

pub async fn test_update_with_status_and_a_cross_board_column_invalidates_the_source_and_destination_column(
    factory: &BackendFactory,
) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let mut ctx = KanbanContext::open(factory(&path), AppConfig::default())
        .await
        .unwrap();
    let fx = seed_two_boards(&mut ctx).await.unwrap();

    let (_, inv) = ctx
        .update_card_impl(
            fx.a1,
            CardUpdate {
                status: Some(CardStatus::InProgress),
                column_id: Some(fx.b_col),
                ..Default::default()
            },
        )
        .unwrap();

    match inv {
        Invalidation::Entities(ids) => {
            assert_eq!(
                ids.card_columns.get(&fx.a1),
                Some(&HashSet::from([fx.a_col, fx.b_col]))
            );
        }
        other => panic!("expected Invalidation::Entities, got {other:?}"),
    }
}

pub async fn test_update_cards_naming_one_card_twice_across_boards_is_refused_and_writes_nothing(
    factory: &BackendFactory,
) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let mut ctx = KanbanContext::open(factory(&path), AppConfig::default())
        .await
        .unwrap();
    let fx = seed_two_boards(&mut ctx).await.unwrap();

    let before = graph_state(&ctx, fx.a1).unwrap();

    let err = ctx
        .update_cards(vec![
            (
                fx.a1,
                CardUpdate {
                    column_id: Some(fx.b_col),
                    ..Default::default()
                },
            ),
            (
                fx.a1,
                CardUpdate {
                    column_id: Some(fx.a_col),
                    ..Default::default()
                },
            ),
        ])
        .unwrap_err();
    assert!(err.is_validation());

    ctx.save().await.unwrap();
    let ctx = KanbanContext::open_deferred(factory(&path), AppConfig::default());
    let after = graph_state(&ctx, fx.a1).unwrap();
    assert_graph_unchanged(&before, &after);
}
