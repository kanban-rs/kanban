#![cfg(feature = "test-helpers")]

//! `Session` must implement `KanbanOperations` and `GraphOperations` itself,
//! with mutators routed through the `state::mutate`/`state::mutate_unit`
//! seam. These tests lock the impls in place: a compile-time check that the
//! traits are satisfied, plus behavioral checks that the delegation actually
//! reaches the store and the `sync` path sees the result.

use kanban_domain::{
    BoardUpdate, GraphOperations, KanbanOperations, LoadState, Model, NoProjections,
};
use kanban_server::scope::RouteScope;
use kanban_server::state::Session;
use kanban_server::test_helpers::{make_sqlite_state, make_state};
use tempfile::tempdir;

#[test]
fn test_session_implements_both_capability_traits() {
    fn assert_impls<T: KanbanOperations + GraphOperations>() {}
    assert_impls::<Session>();
}

#[tokio::test(flavor = "multi_thread")]
async fn test_session_trait_mutator_commits_through_the_seam() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let board_id = {
        let mut guard = state.lock_session().await;
        guard.ctx.create_board("Original".into(), None).unwrap().id
    };

    {
        let mut guard = state.lock_session().await;
        let updated = KanbanOperations::update_board(
            &mut *guard,
            board_id,
            BoardUpdate {
                name: Some("Renamed".into()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(updated.name, "Renamed");
    }

    {
        let guard = state.lock_session().await;
        let mut model = Model::default();
        guard.sync(&RouteScope::Board(board_id), &mut model, &mut NoProjections);
        match model.board_id_status(board_id) {
            LoadState::Loaded(b) => assert_eq!(b.name, "Renamed"),
            other => panic!("expected Loaded, got {other:?}"),
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn test_session_trait_mutator_commits_through_the_seam_on_sqlite() {
    let dir = tempdir().unwrap();
    let state = make_sqlite_state(&dir.path().join("s.sqlite")).await;

    let board_id = {
        let mut guard = state.lock_session().await;
        guard.ctx.create_board("Original".into(), None).unwrap().id
    };

    {
        let mut guard = state.lock_session().await;
        let updated = KanbanOperations::update_board(
            &mut *guard,
            board_id,
            BoardUpdate {
                name: Some("Renamed".into()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(updated.name, "Renamed");
    }

    {
        let guard = state.lock_session().await;
        let mut model = Model::default();
        guard.sync(&RouteScope::Board(board_id), &mut model, &mut NoProjections);
        match model.board_id_status(board_id) {
            LoadState::Loaded(b) => assert_eq!(b.name, "Renamed"),
            other => panic!("expected Loaded, got {other:?}"),
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn test_session_trait_read_matches_context_read() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let board_id = {
        let mut guard = state.lock_session().await;
        guard.ctx.create_board("Board1".into(), None).unwrap().id
    };

    let guard = state.lock_session().await;
    let via_trait = KanbanOperations::get_board(&*guard, board_id).unwrap();
    let via_ctx = guard.ctx.get_board(board_id).unwrap();

    let via_trait = via_trait.expect("board should be found via trait");
    let via_ctx = via_ctx.expect("board should be found via ctx");
    assert_eq!(via_trait.id, via_ctx.id);
    assert_eq!(via_trait.name, via_ctx.name);
    assert_eq!(via_trait.card_prefix, via_ctx.card_prefix);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_session_graph_mutator_commits_through_the_seam() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let (parent, child) = {
        let mut guard = state.lock_session().await;
        let board = guard
            .ctx
            .create_board("Board".into(), Some("KAN".into()))
            .unwrap();
        let column = guard
            .ctx
            .create_column(board.id, "Todo".into(), None)
            .unwrap();
        let parent = guard
            .ctx
            .create_card(board.id, column.id, "Parent".into(), Default::default())
            .unwrap()
            .id;
        let child = guard
            .ctx
            .create_card(board.id, column.id, "Child".into(), Default::default())
            .unwrap()
            .id;
        (parent, child)
    };

    {
        let mut guard = state.lock_session().await;
        GraphOperations::attach_children(&mut *guard, parent, vec![child]).unwrap();
    }

    {
        let guard = state.lock_session().await;
        let mut model = Model::default();
        guard.sync(
            &RouteScope::CardGraph(parent),
            &mut model,
            &mut NoProjections,
        );
        match model.graph_state() {
            LoadState::Loaded(g) => {
                assert_eq!(g.children(parent), vec![child]);
                assert!(g.children(child).is_empty());
            }
            other => panic!("expected Loaded, got {other:?}"),
        }
    }
}
