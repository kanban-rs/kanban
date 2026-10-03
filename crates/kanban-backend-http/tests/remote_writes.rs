use kanban_backend::RemoteGraphWrites;
use kanban_backend_http::HttpBackend;
use kanban_domain::{
    CardPriority, CardStatus, CardUpdate, Column, ColumnUpdate, EntityIds, FieldUpdate,
    GraphOperations, Invalidation, KanbanOperations, KanbanResult, NewBoard, NewCard, NewColumn,
    RelatesKind, Severity, SprintStatus, SprintUpdate,
};
use kanban_server::test_helpers::TestServer;
use kanban_service::{AppConfig, KanbanContext};
use std::sync::Arc;
use uuid::Uuid;

async fn ctx_over(server: &TestServer) -> KanbanContext {
    let backend = Arc::new(HttpBackend::new(&server.base_url()).unwrap());
    KanbanContext::open(backend, AppConfig::default())
        .await
        .unwrap()
}

fn a_new_board() -> NewBoard {
    NewBoard {
        name: "Roadmap".to_string(),
        description: None,
        sprint_prefix: None,
        card_prefix: None,
        task_sort_field: None,
        task_sort_order: None,
        sprint_duration_days: None,
        task_list_view: None,
    }
}

fn a_new_column(board_id: Uuid) -> NewColumn {
    NewColumn {
        board_id,
        name: "To Do".to_string(),
        wip_limit: Some(3),
        default_status: Some(CardStatus::InProgress),
    }
}

fn a_new_card(column_id: Uuid) -> NewCard {
    NewCard {
        column_id,
        title: "Task".to_string(),
        description: None,
        priority: CardPriority::High,
        due_date: None,
        points: Some(3),
        sprint_id: None,
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn test_create_board_over_http_hits_the_board_route_and_returns_server_state() {
    let server = TestServer::start().await;
    let mut ctx = ctx_over(&server).await;
    let id = Uuid::new_v4();
    let spec = a_new_board();

    let (board, invalidation) = ctx
        .create_board_from_spec(Some(id), spec.clone())
        .expect("create_board_from_spec should succeed over http");

    assert_eq!(board.id, id);
    assert_eq!(board.name, spec.name);
    assert_eq!(board.card_prefix, spec.card_prefix);
    let readback = ctx.data_store().get_board(id).unwrap();
    assert!(readback.is_some());
    match invalidation {
        Invalidation::Entities(ids) => assert!(ids.boards.contains(&id)),
        Invalidation::All => panic!("expected a scoped invalidation"),
    }

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_update_board_over_http_hits_the_board_route_and_returns_server_state() {
    let server = TestServer::start().await;
    let mut ctx = ctx_over(&server).await;
    let (board, _) = ctx
        .create_board_from_spec(None, a_new_board())
        .expect("seed create should succeed");

    let updates = kanban_domain::BoardUpdate {
        name: Some("Renamed".to_string()),
        description: FieldUpdate::Set("d".to_string()),
        ..Default::default()
    };
    let (updated, invalidation) = ctx
        .update_board_impl(board.id, updates)
        .expect("update_board_impl should succeed over http");

    assert_eq!(updated.name, "Renamed");
    assert_eq!(updated.description, Some("d".to_string()));
    let readback = ctx.data_store().get_board(board.id).unwrap().unwrap();
    assert_eq!(readback.name, "Renamed");
    match invalidation {
        Invalidation::Entities(ids) => assert!(ids.boards.contains(&board.id)),
        Invalidation::All => panic!("expected a scoped invalidation"),
    }

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_delete_board_over_http_hits_the_board_route_and_returns_server_state() {
    let server = TestServer::start().await;
    let mut ctx = ctx_over(&server).await;
    let (board, _) = ctx
        .create_board_from_spec(None, a_new_board())
        .expect("seed create should succeed");

    let invalidation = ctx
        .delete_board_impl(board.id)
        .expect("delete_board_impl should succeed over http");

    match invalidation {
        Invalidation::Entities(ids) => assert!(ids.boards.contains(&board.id)),
        Invalidation::All => panic!("expected a scoped invalidation"),
    }
    assert!(ctx.data_store().get_board(board.id).unwrap().is_none());

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_archive_board_over_http_hits_the_board_route_and_returns_server_state() {
    let server = TestServer::start().await;
    let mut ctx = ctx_over(&server).await;
    let (board, _) = ctx
        .create_board_from_spec(None, a_new_board())
        .expect("seed create should succeed");

    let invalidation = ctx
        .archive_board_impl(board.id)
        .expect("archive_board_impl should succeed over http");

    match invalidation {
        Invalidation::Entities(ids) => assert!(ids.boards.contains(&board.id)),
        Invalidation::All => panic!("expected a scoped invalidation"),
    }
    assert!(!ctx
        .data_store()
        .list_boards()
        .unwrap()
        .iter()
        .any(|b| b.id == board.id));
    assert!(ctx
        .data_store()
        .list_archived_boards()
        .unwrap()
        .iter()
        .any(|ab| ab.entity_id == board.id));

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_restore_board_over_http_hits_the_board_route_and_returns_server_state() {
    let server = TestServer::start().await;
    let mut ctx = ctx_over(&server).await;
    let (board, _) = ctx
        .create_board_from_spec(None, a_new_board())
        .expect("seed create should succeed");
    let _ = ctx
        .archive_board_impl(board.id)
        .expect("seed archive should succeed");

    let invalidation = ctx
        .restore_board_impl(board.id)
        .expect("restore_board_impl should succeed over http");

    match invalidation {
        Invalidation::Entities(ids) => assert!(ids.boards.contains(&board.id)),
        Invalidation::All => panic!("expected a scoped invalidation"),
    }
    assert!(ctx
        .data_store()
        .list_boards()
        .unwrap()
        .iter()
        .any(|b| b.id == board.id));
    assert!(!ctx
        .data_store()
        .list_archived_boards()
        .unwrap()
        .iter()
        .any(|ab| ab.entity_id == board.id));

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_create_column_over_http_hits_the_column_route_and_returns_server_state() {
    let server = TestServer::start().await;
    let mut ctx = ctx_over(&server).await;
    let (board, _) = ctx
        .create_board_from_spec(None, a_new_board())
        .expect("seed create should succeed");

    let (column, invalidation) = ctx
        .create_column_from_spec(None, a_new_column(board.id))
        .expect("create_column_from_spec should succeed over http");

    assert_eq!(column.board_id, board.id);
    assert_eq!(column.wip_limit, Some(3));
    assert_eq!(column.default_status, Some(CardStatus::InProgress));
    let readback: Column = ctx
        .data_store()
        .get_column(column.id)
        .unwrap()
        .expect("column should be readable back");
    assert_eq!(readback.position, column.position);
    match invalidation {
        Invalidation::Entities(ids) => assert!(ids.columns.contains(&column.id)),
        Invalidation::All => panic!("expected a scoped invalidation"),
    }

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_update_column_over_http_hits_the_flat_column_route_and_returns_server_state() {
    let server = TestServer::start().await;
    let mut ctx = ctx_over(&server).await;
    let (board, _) = ctx.create_board_from_spec(None, a_new_board()).unwrap();
    let (column, _) = ctx
        .create_column_from_spec(None, a_new_column(board.id))
        .unwrap();

    let updates = ColumnUpdate {
        name: Some("Done".to_string()),
        wip_limit: FieldUpdate::Clear,
        default_status: Some(Some(CardStatus::Done)),
        position: None,
    };
    let (updated, invalidation) = ctx
        .update_column_impl(column.id, updates)
        .expect("update_column_impl should succeed over http");

    assert_eq!(updated.name, "Done");
    assert_eq!(updated.wip_limit, None);
    assert_eq!(updated.default_status, Some(CardStatus::Done));
    match invalidation {
        Invalidation::Entities(ids) => assert!(ids.columns.contains(&column.id)),
        Invalidation::All => panic!("expected a scoped invalidation"),
    }

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_delete_column_over_http_hits_the_flat_column_route_and_returns_server_state() {
    let server = TestServer::start().await;
    let mut ctx = ctx_over(&server).await;
    let (board, _) = ctx.create_board_from_spec(None, a_new_board()).unwrap();
    let (column, _) = ctx
        .create_column_from_spec(None, a_new_column(board.id))
        .unwrap();

    let invalidation = ctx
        .delete_column_impl(column.id)
        .expect("delete_column_impl should succeed over http");

    match invalidation {
        Invalidation::Entities(ids) => assert!(ids.columns.contains(&column.id)),
        Invalidation::All => panic!("expected a scoped invalidation"),
    }
    assert!(ctx.data_store().get_column(column.id).unwrap().is_none());

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_create_card_over_http_hits_the_card_route_and_returns_server_state() {
    let server = TestServer::start().await;
    let mut ctx = ctx_over(&server).await;
    let (board, _) = ctx.create_board_from_spec(None, a_new_board()).unwrap();
    let (column, _) = ctx
        .create_column_from_spec(None, a_new_column(board.id))
        .unwrap();
    let id = Uuid::new_v4();

    let (card, invalidation) = ctx
        .create_card_from_spec(Some(id), a_new_card(column.id))
        .expect("create_card_from_spec should succeed over http");

    assert_eq!(card.id, id);
    assert_eq!(card.column_id, column.id);
    assert_eq!(card.board_id, board.id);
    assert_eq!(card.priority, CardPriority::High);
    assert_eq!(card.points, Some(3));
    assert!(card.card_number > 0, "server should have minted a number");
    match invalidation {
        Invalidation::Entities(ids) => assert!(ids.cards.contains(&card.id)),
        Invalidation::All => panic!("expected a scoped invalidation"),
    }

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_update_card_over_http_hits_the_flat_card_route_and_returns_server_state() {
    let server = TestServer::start().await;
    let mut ctx = ctx_over(&server).await;
    let (board, _) = ctx.create_board_from_spec(None, a_new_board()).unwrap();
    let (column, _) = ctx
        .create_column_from_spec(None, a_new_column(board.id))
        .unwrap();
    let (card, _) = ctx
        .create_card_from_spec(None, a_new_card(column.id))
        .unwrap();

    let updates = CardUpdate {
        title: Some("Updated".to_string()),
        status: Some(CardStatus::Done),
        description: FieldUpdate::Set("desc".to_string()),
        due_date: FieldUpdate::Clear,
        points: FieldUpdate::Set(5),
        ..Default::default()
    };
    let (updated, invalidation) = ctx
        .update_card_impl(card.id, updates)
        .expect("update_card_impl should succeed over http");

    assert_eq!(updated.title, "Updated");
    assert_eq!(updated.status, CardStatus::Done);
    assert_eq!(updated.description, Some("desc".to_string()));
    assert_eq!(updated.due_date, None);
    assert_eq!(updated.points, Some(5));
    let readback = ctx.data_store().get_card(card.id).unwrap().unwrap();
    assert_eq!(readback.title, "Updated");
    match invalidation {
        Invalidation::Entities(ids) => assert!(ids.cards.contains(&card.id)),
        Invalidation::All => panic!("expected a scoped invalidation"),
    }

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_delete_card_over_http_hits_the_flat_card_route_and_returns_server_state() {
    let server = TestServer::start().await;
    let mut ctx = ctx_over(&server).await;
    let (board, _) = ctx.create_board_from_spec(None, a_new_board()).unwrap();
    let (column, _) = ctx
        .create_column_from_spec(None, a_new_column(board.id))
        .unwrap();
    let (card, _) = ctx
        .create_card_from_spec(None, a_new_card(column.id))
        .unwrap();

    let invalidation = ctx
        .delete_card_impl(card.id)
        .expect("delete_card_impl should succeed over http");

    match invalidation {
        Invalidation::Entities(ids) => assert!(ids.cards.contains(&card.id)),
        Invalidation::All => panic!("expected a scoped invalidation"),
    }
    assert!(ctx.data_store().get_card(card.id).unwrap().is_none());

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_archive_card_over_http_hits_the_card_route_and_returns_server_state() {
    let server = TestServer::start().await;
    let mut ctx = ctx_over(&server).await;
    let (board, _) = ctx.create_board_from_spec(None, a_new_board()).unwrap();
    let (column, _) = ctx
        .create_column_from_spec(None, a_new_column(board.id))
        .unwrap();
    let (card, _) = ctx
        .create_card_from_spec(None, a_new_card(column.id))
        .unwrap();

    let ((), invalidation) = ctx
        .archive_card_impl(card.id)
        .expect("archive_card_impl should succeed over http");

    assert_eq!(
        invalidation,
        Invalidation::All,
        "RestoreCard::touched_entities() is None, so invalidation_from_inverse falls \
         back to All for the archive's inverse"
    );
    assert!(ctx.data_store().get_card(card.id).unwrap().is_some());
    assert!(ctx
        .data_store()
        .list_archived_cards()
        .unwrap()
        .iter()
        .any(|ac| ac.entity_id == card.id));

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_restore_card_over_http_hits_the_card_route_and_returns_server_state() {
    let server = TestServer::start().await;
    let mut ctx = ctx_over(&server).await;
    let (board, _) = ctx.create_board_from_spec(None, a_new_board()).unwrap();
    let (column, _) = ctx
        .create_column_from_spec(None, a_new_column(board.id))
        .unwrap();
    let (card, _) = ctx
        .create_card_from_spec(None, a_new_card(column.id))
        .unwrap();
    let _ = ctx
        .archive_card_impl(card.id)
        .expect("seed archive should succeed");

    let (restored, invalidation) = ctx
        .restore_card_impl(card.id, None)
        .expect("restore_card_impl should succeed over http");

    assert_eq!(restored.id, card.id);
    match invalidation {
        Invalidation::Entities(ids) => assert!(ids.cards.contains(&card.id)),
        Invalidation::All => panic!("expected a scoped invalidation"),
    }
    assert!(!ctx
        .data_store()
        .list_archived_cards()
        .unwrap()
        .iter()
        .any(|ac| ac.entity_id == card.id));

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_restore_card_to_column_over_http_sends_the_column_id_query_param() {
    let server = TestServer::start().await;
    let mut ctx = ctx_over(&server).await;
    let (board, _) = ctx.create_board_from_spec(None, a_new_board()).unwrap();
    let (column, _) = ctx
        .create_column_from_spec(None, a_new_column(board.id))
        .unwrap();
    let (other_column, _) = ctx
        .create_column_from_spec(None, a_new_column(board.id))
        .unwrap();
    let (card, _) = ctx
        .create_card_from_spec(None, a_new_card(column.id))
        .unwrap();
    let _ = ctx
        .archive_card_impl(card.id)
        .expect("seed archive should succeed");

    let (restored, invalidation) = ctx
        .restore_card_impl(card.id, Some(other_column.id))
        .expect("restore_card_impl should succeed over http");

    assert_eq!(restored.id, card.id);
    assert_eq!(restored.column_id, other_column.id);
    match invalidation {
        Invalidation::Entities(ids) => assert!(ids.cards.contains(&card.id)),
        Invalidation::All => panic!("expected a scoped invalidation"),
    }

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_delete_board_over_http_returns_the_cascade_invalidation() {
    struct SeededGraph {
        board_id: Uuid,
        column_id: Uuid,
        card_a: Uuid,
        card_b: Uuid,
        sprint_id: Uuid,
    }
    let seeded = std::sync::Arc::new(std::sync::Mutex::new(None::<SeededGraph>));
    let seeded_for_seed = std::sync::Arc::clone(&seeded);

    let server = TestServer::start_with(move |ctx| {
        let board_id = ctx
            .create_board("Cascade Board".to_string(), Some("KAN".to_string()))
            .unwrap()
            .id;
        let column_id = ctx
            .create_column(board_id, "To Do".to_string(), None)
            .unwrap()
            .id;
        let card_a = ctx
            .create_card(
                board_id,
                column_id,
                "Card A".to_string(),
                kanban_domain::CreateCardOptions::default(),
            )
            .unwrap()
            .id;
        let card_b = ctx
            .create_card(
                board_id,
                column_id,
                "Card B".to_string(),
                kanban_domain::CreateCardOptions::default(),
            )
            .unwrap()
            .id;
        let sprint_id = ctx.create_sprint(board_id, None, None).unwrap().id;
        ctx.attach_children(card_a, vec![card_b]).unwrap();
        *seeded_for_seed.lock().unwrap() = Some(SeededGraph {
            board_id,
            column_id,
            card_a,
            card_b,
            sprint_id,
        });
    })
    .await;

    let graph = seeded.lock().unwrap().take().unwrap();
    let mut ctx = ctx_over(&server).await;

    let invalidation = ctx
        .delete_board_impl(graph.board_id)
        .expect("delete_board_impl should succeed over http");

    match invalidation {
        Invalidation::Entities(ids) => {
            assert!(ids.boards.contains(&graph.board_id));
            assert!(ids.columns.contains(&graph.column_id));
            assert!(ids.cards.contains(&graph.card_a));
            assert!(ids.cards.contains(&graph.card_b));
            assert!(ids.sprints.contains(&graph.sprint_id));
            assert!(ids.graph);
        }
        Invalidation::All => panic!("expected an Entities invalidation naming the graph"),
    }

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_move_card_over_http_with_no_position_appends_past_archived_siblings() {
    let server = TestServer::start().await;
    let mut ctx = ctx_over(&server).await;
    let (board, _) = ctx.create_board_from_spec(None, a_new_board()).unwrap();
    let (source, _) = ctx
        .create_column_from_spec(None, a_new_column(board.id))
        .unwrap();
    let (target, _) = ctx
        .create_column_from_spec(None, a_new_column(board.id))
        .unwrap();
    let (archived, _) = ctx
        .create_card_from_spec(None, a_new_card(target.id))
        .unwrap();
    let _ = ctx
        .archive_card_impl(archived.id)
        .expect("seed archive should succeed");
    let (card, _) = ctx
        .create_card_from_spec(None, a_new_card(source.id))
        .unwrap();

    let (moved, invalidation) = ctx
        .move_card_impl(card.id, target.id, None)
        .expect("move_card_impl should succeed over http");

    assert_eq!(moved.column_id, target.id);
    assert_eq!(
        moved.position, 1,
        "position should land past the archived sibling via count(Include), not client-side count(LiveOnly)"
    );
    match invalidation {
        Invalidation::Entities(ids) => assert!(ids.cards.contains(&card.id)),
        Invalidation::All => panic!("expected a scoped invalidation"),
    }

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_move_cards_over_http_returns_a_scoped_entities_invalidation_naming_every_moved_id() {
    let server = TestServer::start().await;
    let mut ctx = ctx_over(&server).await;
    let (board, _) = ctx.create_board_from_spec(None, a_new_board()).unwrap();
    let (source, _) = ctx
        .create_column_from_spec(None, a_new_column(board.id))
        .unwrap();
    let (target, _) = ctx
        .create_column_from_spec(None, a_new_column(board.id))
        .unwrap();
    let (card_a, _) = ctx
        .create_card_from_spec(None, a_new_card(source.id))
        .unwrap();
    let (card_b, _) = ctx
        .create_card_from_spec(None, a_new_card(source.id))
        .unwrap();

    let (count, invalidation) = ctx
        .move_cards_impl(vec![card_a.id, card_b.id], target.id)
        .expect("move_cards_impl should succeed over http");

    assert_eq!(count, 2);
    match invalidation {
        Invalidation::Entities(ids) => {
            let moved: std::collections::HashSet<Uuid> =
                [card_a.id, card_b.id].into_iter().collect();
            assert_eq!(ids.cards, moved);
        }
        Invalidation::All => panic!("expected a scoped invalidation naming exactly the moved ids"),
    }

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_move_cards_over_http_with_one_unknown_id_moves_the_rest_and_counts_them() {
    let server = TestServer::start().await;
    let mut ctx = ctx_over(&server).await;
    let (board, _) = ctx.create_board_from_spec(None, a_new_board()).unwrap();
    let (source, _) = ctx
        .create_column_from_spec(None, a_new_column(board.id))
        .unwrap();
    let (target, _) = ctx
        .create_column_from_spec(None, a_new_column(board.id))
        .unwrap();
    let (card, _) = ctx
        .create_card_from_spec(None, a_new_card(source.id))
        .unwrap();
    let unknown = Uuid::new_v4();

    let (count, invalidation) = ctx
        .move_cards_impl(vec![card.id, unknown], target.id)
        .expect("move_cards_impl should apply the valid subset over http");

    assert_eq!(
        count, 1,
        "an unknown id must not fail the ids that do exist"
    );
    match invalidation {
        Invalidation::Entities(ids) => assert!(ids.cards.contains(&card.id)),
        Invalidation::All => panic!("expected a scoped invalidation"),
    }

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_move_cards_detailed_over_http_reports_per_id_failures() {
    let server = TestServer::start().await;
    let mut ctx = ctx_over(&server).await;
    let (board, _) = ctx.create_board_from_spec(None, a_new_board()).unwrap();
    let (source, _) = ctx
        .create_column_from_spec(None, a_new_column(board.id))
        .unwrap();
    let (target, _) = ctx
        .create_column_from_spec(None, a_new_column(board.id))
        .unwrap();
    let (card, _) = ctx
        .create_card_from_spec(None, a_new_card(source.id))
        .unwrap();
    let unknown = Uuid::new_v4();

    let (result, invalidation) = ctx.move_cards_detailed(vec![card.id, unknown], target.id);

    assert_eq!(result.succeeded, vec![card.id]);
    assert_eq!(result.failed.len(), 1);
    assert_eq!(result.failed[0].id, unknown);
    assert!(!result.failed[0].error.is_empty());
    match invalidation {
        Invalidation::Entities(ids) => assert!(ids.cards.contains(&card.id)),
        Invalidation::All => panic!("expected a scoped invalidation"),
    }

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_assign_cards_to_sprint_over_http_counts_a_card_already_in_the_sprint() {
    let server = TestServer::start().await;
    let mut ctx = ctx_over(&server).await;
    let (board, _) = ctx.create_board_from_spec(None, a_new_board()).unwrap();
    let (column, _) = ctx
        .create_column_from_spec(None, a_new_column(board.id))
        .unwrap();
    let (card, _) = ctx
        .create_card_from_spec(None, a_new_card(column.id))
        .unwrap();
    let sprint = ctx.create_sprint(board.id, None, None).unwrap();
    let _ = ctx.assign_card_to_sprint_impl(card.id, sprint.id).unwrap();

    let (count, invalidation) = ctx
        .assign_cards_to_sprint_impl(vec![card.id], sprint.id)
        .expect("assign_cards_to_sprint_impl should succeed over http");

    assert_eq!(
        count, 1,
        "a card already in the target sprint still counts as succeeded over http, \
         unlike the local before/after-count diff which would see no change"
    );
    assert!(
        matches!(invalidation, Invalidation::All),
        "reassigning a card already on the sprint produces no inverse entry, \
         so the empty inverse batch invalidates All, same as local; got {invalidation:?}"
    );

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_archive_cards_over_http_returns_the_same_invalidation_as_a_local_archive() {
    async fn archive_two_cards(ctx: &mut KanbanContext) -> (usize, Invalidation) {
        let (board, _) = ctx.create_board_from_spec(None, a_new_board()).unwrap();
        let (column, _) = ctx
            .create_column_from_spec(None, a_new_column(board.id))
            .unwrap();
        let (card_a, _) = ctx
            .create_card_from_spec(None, a_new_card(column.id))
            .unwrap();
        let (card_b, _) = ctx
            .create_card_from_spec(None, a_new_card(column.id))
            .unwrap();
        ctx.archive_cards_impl(vec![card_a.id, card_b.id])
            .expect("archive_cards_impl should succeed")
    }

    let server = TestServer::start().await;
    let mut remote = ctx_over(&server).await;
    let (remote_count, remote_invalidation) = archive_two_cards(&mut remote).await;
    server.shutdown().await;

    let dir = tempfile::tempdir().unwrap();
    let local_backend: Arc<dyn kanban_service::KanbanBackend> =
        Arc::new(kanban_persistence_json::JsonDataStore::new(Arc::new(
            kanban_persistence_json::JsonFileStore::new(dir.path().join("local.json")),
        )));
    let mut local = KanbanContext::open(local_backend, AppConfig::default())
        .await
        .unwrap();
    let (local_count, local_invalidation) = archive_two_cards(&mut local).await;

    assert_eq!(remote_count, local_count);
    assert_eq!(
        remote_invalidation, local_invalidation,
        "archiving cards over http must invalidate exactly what a local archive does"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_assign_card_to_sprint_over_http_returns_the_servers_invalidation() {
    let seeded = std::sync::Arc::new(std::sync::Mutex::new(None::<(Uuid, Uuid, Uuid)>));
    let seeded_for_seed = std::sync::Arc::clone(&seeded);

    let server = TestServer::start_with(move |ctx| {
        let board_id = ctx
            .create_board("Sprint Board".to_string(), Some("KAN".to_string()))
            .unwrap()
            .id;
        let column_id = ctx
            .create_column(board_id, "To Do".to_string(), None)
            .unwrap()
            .id;
        let card_id = ctx
            .create_card(
                board_id,
                column_id,
                "Card".to_string(),
                kanban_domain::CreateCardOptions::default(),
            )
            .unwrap()
            .id;
        let sprint_id = ctx.create_sprint(board_id, None, None).unwrap().id;
        *seeded_for_seed.lock().unwrap() = Some((card_id, column_id, sprint_id));
    })
    .await;

    let (card_id, _column_id, sprint_id) = seeded.lock().unwrap().take().unwrap();
    let mut ctx = ctx_over(&server).await;

    let (card, invalidation) = ctx
        .assign_card_to_sprint_impl(card_id, sprint_id)
        .expect("assign_card_to_sprint_impl should succeed over http");

    assert_eq!(card.sprint_id, Some(sprint_id));
    match invalidation {
        Invalidation::Entities(ids) => assert!(ids.cards.contains(&card_id)),
        Invalidation::All => panic!("expected a scoped invalidation"),
    }

    server.shutdown().await;
}

#[test]
fn test_http_backend_reports_remote_write_family_support() {
    let backend = HttpBackend::new("http://localhost:0").unwrap();
    let backend: &dyn kanban_backend::KanbanBackend = &backend;

    assert!(backend.remote_board_writes().is_some());
    assert!(backend.remote_card_writes().is_some());
    assert!(backend.remote_batch_writes().is_some());
    assert!(backend.remote_sprint_writes().is_some());
    assert!(backend.remote_graph_writes().is_some());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_attach_child_to_an_archived_card_over_http_is_born_archived() {
    let server = TestServer::start().await;
    let mut ctx = ctx_over(&server).await;
    let backend = HttpBackend::new(&server.base_url()).unwrap();
    let (board, _) = ctx.create_board_from_spec(None, a_new_board()).unwrap();
    let (column, _) = ctx
        .create_column_from_spec(None, a_new_column(board.id))
        .unwrap();
    let (parent, _) = ctx
        .create_card_from_spec(None, a_new_card(column.id))
        .unwrap();
    let (child, _) = ctx
        .create_card_from_spec(None, a_new_card(column.id))
        .unwrap();
    let _ = ctx
        .archive_card_impl(parent.id)
        .expect("seed archive should succeed");

    let _ = backend
        .attach_children(parent.id, &[child.id])
        .expect("attach_children over http should succeed");

    let graph = ctx.data_store().get_graph().unwrap();
    let edge = graph
        .spawns_edges()
        .iter()
        .find(|e| e.base.source == parent.id && e.base.target == child.id)
        .expect("spawns edge should be present in the server's graph");
    assert!(
        edge.base.archived_at.is_some(),
        "edge incident to an archived card must be born archived"
    );

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_graph_mutations_over_http_round_trip_every_edge_kind() {
    let server = TestServer::start().await;
    let mut ctx = ctx_over(&server).await;
    let backend = HttpBackend::new(&server.base_url()).unwrap();
    let (board, _) = ctx.create_board_from_spec(None, a_new_board()).unwrap();
    let (column_a, _) = ctx
        .create_column_from_spec(None, a_new_column(board.id))
        .unwrap();
    let (column_b, _) = ctx
        .create_column_from_spec(None, a_new_column(board.id))
        .unwrap();
    let (parent, _) = ctx
        .create_card_from_spec(None, a_new_card(column_a.id))
        .unwrap();
    let (child_a, _) = ctx
        .create_card_from_spec(None, a_new_card(column_a.id))
        .unwrap();
    let (child_b, _) = ctx
        .create_card_from_spec(None, a_new_card(column_a.id))
        .unwrap();
    let (blocked, _) = ctx
        .create_card_from_spec(None, a_new_card(column_b.id))
        .unwrap();
    let (related, _) = ctx
        .create_card_from_spec(None, a_new_card(column_b.id))
        .unwrap();

    let attach_inv = backend
        .attach_children(parent.id, &[child_a.id, child_b.id])
        .unwrap();
    let block_inv = backend
        .block(parent.id, blocked.id, Severity::High)
        .unwrap();
    let relate_inv = backend
        .relate(parent.id, related.id, RelatesKind::Duplicates)
        .unwrap();
    for inv in [&attach_inv, &block_inv, &relate_inv] {
        match inv {
            Invalidation::Entities(ids) => assert!(ids.cards.contains(&parent.id)),
            Invalidation::All => panic!("expected a scoped invalidation"),
        }
    }

    let graph = ctx.data_store().get_graph().unwrap();
    assert!(graph.contains(parent.id, child_a.id));
    assert!(graph.contains(parent.id, child_b.id));
    assert!(graph.contains(parent.id, blocked.id));
    assert!(graph.contains(parent.id, related.id));
    let block_edge = graph
        .blocks_edges()
        .iter()
        .find(|e| e.base.source == parent.id && e.base.target == blocked.id)
        .unwrap();
    assert_eq!(block_edge.severity, Severity::High);
    let relate_edge = graph
        .relates_edges()
        .iter()
        .find(|e| e.base.source == parent.id && e.base.target == related.id)
        .unwrap();
    assert_eq!(relate_edge.kind, RelatesKind::Duplicates);

    let detach_inv = backend.detach_children(parent.id, &[child_a.id]).unwrap();
    let unblock_inv = backend.unblock(parent.id, blocked.id).unwrap();
    let dissociate_inv = backend.dissociate(parent.id, related.id).unwrap();
    for inv in [&detach_inv, &unblock_inv, &dissociate_inv] {
        match inv {
            Invalidation::Entities(ids) => assert!(ids.cards.contains(&parent.id)),
            Invalidation::All => panic!("expected a scoped invalidation"),
        }
    }

    let graph = ctx.data_store().get_graph().unwrap();
    assert!(!graph.contains(parent.id, child_a.id));
    assert!(graph.contains(parent.id, child_b.id));
    assert!(!graph.contains(parent.id, blocked.id));
    assert!(!graph.contains(parent.id, related.id));

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_detach_children_over_http_with_one_missing_edge_removes_nothing() {
    let server = TestServer::start().await;
    let mut ctx = ctx_over(&server).await;
    let backend = HttpBackend::new(&server.base_url()).unwrap();
    let (board, _) = ctx.create_board_from_spec(None, a_new_board()).unwrap();
    let (column, _) = ctx
        .create_column_from_spec(None, a_new_column(board.id))
        .unwrap();
    let (parent, _) = ctx
        .create_card_from_spec(None, a_new_card(column.id))
        .unwrap();
    let (child, _) = ctx
        .create_card_from_spec(None, a_new_card(column.id))
        .unwrap();
    let (not_a_child, _) = ctx
        .create_card_from_spec(None, a_new_card(column.id))
        .unwrap();
    let _ = backend.attach_children(parent.id, &[child.id]).unwrap();

    let err = backend
        .detach_children(parent.id, &[child.id, not_a_child.id])
        .expect_err("detaching a missing edge alongside a real one should fail atomically");
    assert!(err.is_edge_not_found(), "got: {err}");

    let graph = ctx.data_store().get_graph().unwrap();
    assert!(
        graph.contains(parent.id, child.id),
        "the real edge must survive an all-or-nothing failed batch"
    );

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_attach_children_over_http_closing_a_cycle_returns_cycle_detected() {
    let server = TestServer::start().await;
    let mut ctx = ctx_over(&server).await;
    let (board, _) = ctx.create_board_from_spec(None, a_new_board()).unwrap();
    let (column, _) = ctx
        .create_column_from_spec(None, a_new_column(board.id))
        .unwrap();
    let (a, _) = ctx
        .create_card_from_spec(None, a_new_card(column.id))
        .unwrap();
    let a = a.id;
    let (b, _) = ctx
        .create_card_from_spec(None, a_new_card(column.id))
        .unwrap();
    let b = b.id;

    let _ = ctx.attach_children_impl(a, vec![b]).unwrap();
    let err = ctx.attach_children_impl(b, vec![a]).unwrap_err();
    assert!(err.is_cycle_detected(), "got: {err}");

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_block_self_over_http_returns_self_reference() {
    let server = TestServer::start().await;
    let mut ctx = ctx_over(&server).await;
    let (board, _) = ctx.create_board_from_spec(None, a_new_board()).unwrap();
    let (column, _) = ctx
        .create_column_from_spec(None, a_new_column(board.id))
        .unwrap();
    let (a, _) = ctx
        .create_card_from_spec(None, a_new_card(column.id))
        .unwrap();
    let a = a.id;

    let err = ctx.block_impl(a, a, Severity::Medium).unwrap_err();
    assert!(err.is_self_reference(), "got: {err}");

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_block_twice_over_http_returns_duplicate_edge() {
    let server = TestServer::start().await;
    let mut ctx = ctx_over(&server).await;
    let (board, _) = ctx.create_board_from_spec(None, a_new_board()).unwrap();
    let (column, _) = ctx
        .create_column_from_spec(None, a_new_column(board.id))
        .unwrap();
    let (a, _) = ctx
        .create_card_from_spec(None, a_new_card(column.id))
        .unwrap();
    let a = a.id;
    let (b, _) = ctx
        .create_card_from_spec(None, a_new_card(column.id))
        .unwrap();
    let b = b.id;

    let _ = ctx.block_impl(a, b, Severity::Medium).unwrap();
    let err = ctx.block_impl(a, b, Severity::Medium).unwrap_err();
    assert!(err.is_duplicate_edge(), "got: {err}");

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_unblock_missing_edge_over_http_returns_edge_not_found() {
    let server = TestServer::start().await;
    let mut ctx = ctx_over(&server).await;
    let (board, _) = ctx.create_board_from_spec(None, a_new_board()).unwrap();
    let (column, _) = ctx
        .create_column_from_spec(None, a_new_column(board.id))
        .unwrap();
    let (a, _) = ctx
        .create_card_from_spec(None, a_new_card(column.id))
        .unwrap();
    let a = a.id;
    let (b, _) = ctx
        .create_card_from_spec(None, a_new_card(column.id))
        .unwrap();
    let b = b.id;

    let err = ctx.unblock_impl(a, b).unwrap_err();
    assert!(err.is_edge_not_found(), "got: {err}");

    server.shutdown().await;
}

fn fixed_start() -> chrono::DateTime<chrono::Utc> {
    chrono::DateTime::parse_from_rfc3339("2030-06-01T12:00:00Z")
        .unwrap()
        .with_timezone(&chrono::Utc)
}

#[tokio::test(flavor = "multi_thread")]
async fn test_create_sprint_over_http_hits_the_sprint_route_and_returns_a_named_sprint() {
    let server = TestServer::start().await;
    let mut ctx = ctx_over(&server).await;
    let (board, _) = ctx.create_board_from_spec(None, a_new_board()).unwrap();

    let (sprint, _) = ctx
        .create_sprint_from_spec(board.id, None, Some("Alpha".into()), None, false)
        .unwrap();

    let board = ctx.get_board(board.id).unwrap().unwrap();
    assert_eq!(sprint.name_index, Some(0), "expected the pool's first slot");
    assert_eq!(sprint.get_name(&board), Some("Alpha"));

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_update_sprint_over_http_sets_dates() {
    let server = TestServer::start().await;
    let mut ctx = ctx_over(&server).await;
    let (board, _) = ctx.create_board_from_spec(None, a_new_board()).unwrap();
    let (sprint, _) = ctx
        .create_sprint_from_spec(board.id, None, Some("Alpha".into()), None, false)
        .unwrap();

    let (updated, _) = ctx
        .update_sprint_impl(
            sprint.id,
            SprintUpdate {
                start_date: FieldUpdate::Set(fixed_start()),
                ..Default::default()
            },
        )
        .unwrap();

    assert_eq!(updated.start_date, Some(fixed_start()));

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_delete_sprint_over_http_returns_the_server_invalidation() {
    let server = TestServer::start().await;
    let mut ctx = ctx_over(&server).await;
    let (board, _) = ctx.create_board_from_spec(None, a_new_board()).unwrap();
    let (sprint, _) = ctx
        .create_sprint_from_spec(board.id, None, Some("Alpha".into()), None, false)
        .unwrap();

    let invalidation = ctx.delete_sprint_impl(sprint.id).unwrap();
    match invalidation {
        Invalidation::Entities(ids) => assert!(ids.sprints.contains(&sprint.id)),
        Invalidation::All => panic!("expected a scoped invalidation"),
    }

    assert!(ctx.get_sprint(sprint.id).unwrap().is_none());

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_create_sprint_over_http_when_the_board_reread_fails_returns_the_committed_sprint_unnamed(
) -> KanbanResult<()> {
    let (server, fault) = TestServer::start_with_fault(|_| {}).await;
    let mut ctx = ctx_over(&server).await;
    let (board, _) = ctx.create_board_from_spec(None, a_new_board())?;

    *fault.lock().unwrap() = Some(("GET", format!("/v1/boards/{}", board.id)));

    let (sprint, invalidation) =
        ctx.create_sprint_from_spec(board.id, None, Some("Alpha".into()), None, false)?;

    assert_eq!(sprint.name_index, None);
    assert!(
        matches!(invalidation, Invalidation::All),
        "expected Invalidation::All, got {invalidation:?}"
    );

    *fault.lock().unwrap() = None;

    let sprints = ctx.data_store().list_sprints_by_board(board.id)?;
    assert_eq!(sprints.len(), 1);
    assert_eq!(sprints[0].id, sprint.id);
    let board = ctx.get_board(board.id)?.unwrap();
    assert_eq!(sprints[0].get_name(&board), Some("Alpha"));

    server.shutdown().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_update_sprint_over_http_when_the_board_reread_fails_returns_the_committed_update(
) -> KanbanResult<()> {
    let (server, fault) = TestServer::start_with_fault(|_| {}).await;
    let mut ctx = ctx_over(&server).await;
    let (board, _) = ctx.create_board_from_spec(None, a_new_board())?;
    let (sprint, _) =
        ctx.create_sprint_from_spec(board.id, None, Some("Alpha".into()), None, false)?;

    *fault.lock().unwrap() = Some(("GET", format!("/v1/boards/{}", board.id)));

    let (updated, invalidation) = ctx.update_sprint_impl(
        sprint.id,
        SprintUpdate {
            start_date: FieldUpdate::Set(fixed_start()),
            ..Default::default()
        },
    )?;

    assert_eq!(updated.start_date, Some(fixed_start()));
    match invalidation {
        Invalidation::Entities(ids) => assert_eq!(ids, EntityIds::sprints([sprint.id])),
        Invalidation::All => panic!("expected a scoped invalidation"),
    }

    *fault.lock().unwrap() = None;

    let sprints = ctx.data_store().list_sprints_by_board(board.id)?;
    assert_eq!(sprints[0].start_date, Some(fixed_start()));

    server.shutdown().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_activate_sprint_over_http_when_the_board_reread_fails_returns_the_active_sprint(
) -> KanbanResult<()> {
    let (server, fault) = TestServer::start_with_fault(|_| {}).await;
    let mut ctx = ctx_over(&server).await;
    let (board, _) = ctx.create_board_from_spec(None, a_new_board())?;
    let (sprint, _) =
        ctx.create_sprint_from_spec(board.id, None, Some("Alpha".into()), None, false)?;

    *fault.lock().unwrap() = Some(("GET", format!("/v1/boards/{}", board.id)));

    let (activated, invalidation) = ctx.activate_sprint_impl(sprint.id, None)?;

    assert_eq!(activated.status, SprintStatus::Active);
    match invalidation {
        Invalidation::Entities(ids) => assert_eq!(ids, EntityIds::sprints([sprint.id])),
        Invalidation::All => panic!("expected a scoped invalidation"),
    }

    *fault.lock().unwrap() = None;

    let sprints = ctx.data_store().list_sprints_by_board(board.id)?;
    assert_eq!(sprints[0].status, SprintStatus::Active);

    server.shutdown().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_complete_sprint_over_http_when_the_board_reread_fails_returns_the_completed_sprint(
) -> KanbanResult<()> {
    let (server, fault) = TestServer::start_with_fault(|_| {}).await;
    let mut ctx = ctx_over(&server).await;
    let (board, _) = ctx.create_board_from_spec(None, a_new_board())?;
    let (sprint, _) =
        ctx.create_sprint_from_spec(board.id, None, Some("Alpha".into()), None, false)?;
    let _ = ctx.activate_sprint_impl(sprint.id, None)?;

    *fault.lock().unwrap() = Some(("GET", format!("/v1/boards/{}", board.id)));

    let (completed, invalidation) = ctx.complete_sprint_impl(sprint.id)?;

    assert_eq!(completed.status, SprintStatus::Completed);
    match invalidation {
        Invalidation::Entities(ids) => assert_eq!(ids, EntityIds::sprints([sprint.id])),
        Invalidation::All => panic!("expected a scoped invalidation"),
    }

    *fault.lock().unwrap() = None;

    let sprints = ctx.data_store().list_sprints_by_board(board.id)?;
    assert_eq!(sprints[0].status, SprintStatus::Completed);

    server.shutdown().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_cancel_sprint_over_http_when_the_board_reread_fails_returns_the_cancelled_sprint(
) -> KanbanResult<()> {
    let (server, fault) = TestServer::start_with_fault(|_| {}).await;
    let mut ctx = ctx_over(&server).await;
    let (board, _) = ctx.create_board_from_spec(None, a_new_board())?;
    let (sprint, _) =
        ctx.create_sprint_from_spec(board.id, None, Some("Alpha".into()), None, false)?;
    let _ = ctx.activate_sprint_impl(sprint.id, None)?;

    *fault.lock().unwrap() = Some(("GET", format!("/v1/boards/{}", board.id)));

    let (cancelled, invalidation) = ctx.cancel_sprint_impl(sprint.id)?;

    assert_eq!(cancelled.status, SprintStatus::Cancelled);
    match invalidation {
        Invalidation::Entities(ids) => assert_eq!(ids, EntityIds::sprints([sprint.id])),
        Invalidation::All => panic!("expected a scoped invalidation"),
    }

    *fault.lock().unwrap() = None;

    let sprints = ctx.data_store().list_sprints_by_board(board.id)?;
    assert_eq!(sprints[0].status, SprintStatus::Cancelled);

    server.shutdown().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_sprint_over_http_when_the_board_read_fails_still_returns_an_error(
) -> KanbanResult<()> {
    let (server, fault) = TestServer::start_with_fault(|_| {}).await;
    let mut ctx = ctx_over(&server).await;
    let (board, _) = ctx.create_board_from_spec(None, a_new_board())?;
    let (sprint, _) =
        ctx.create_sprint_from_spec(board.id, None, Some("Alpha".into()), None, false)?;

    *fault.lock().unwrap() = Some(("GET", format!("/v1/boards/{}", board.id)));

    let err = ctx
        .data_store()
        .get_sprint(sprint.id)
        .expect_err("a read must still fail loudly when the board re-read fails");
    assert!(!err.to_string().is_empty());

    server.shutdown().await;
    Ok(())
}

fn scoped(invalidation: Invalidation) -> EntityIds {
    match invalidation {
        Invalidation::Entities(ids) => ids,
        Invalidation::All => panic!("expected a scoped invalidation"),
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn test_activate_complete_and_carry_over_over_http_return_named_sprints_and_move_the_cards(
) -> KanbanResult<()> {
    let server = TestServer::start().await;
    let mut ctx = ctx_over(&server).await;
    let (board, _) = ctx.create_board_from_spec(None, a_new_board())?;
    let (column, _) = ctx.create_column_from_spec(None, a_new_column(board.id))?;
    let (first, _) = ctx.create_card_from_spec(None, a_new_card(column.id))?;
    let (second, _) = ctx.create_card_from_spec(None, a_new_card(column.id))?;
    let (alpha, _) =
        ctx.create_sprint_from_spec(board.id, None, Some("Alpha".into()), None, false)?;
    let (beta, _) =
        ctx.create_sprint_from_spec(board.id, None, Some("Beta".into()), None, false)?;
    let _ = ctx.assign_card_to_sprint_impl(first.id, alpha.id)?;
    let _ = ctx.assign_card_to_sprint_impl(second.id, alpha.id)?;

    let (activated, activate_inv) = ctx.activate_sprint_impl(alpha.id, Some(7))?;
    let (completed, complete_inv) = ctx.complete_sprint_impl(alpha.id)?;
    let (moved, carry_inv) = ctx.carry_over_sprint_cards_impl(alpha.id, beta.id)?;

    let board = ctx.get_board(board.id)?.unwrap();
    assert_eq!(activated.status, SprintStatus::Active);
    assert_eq!(
        activated.get_name(&board),
        Some("Alpha"),
        "activate must resolve the name"
    );
    assert_eq!(
        activated.end_date.unwrap() - activated.start_date.unwrap(),
        chrono::Duration::days(7)
    );
    assert_eq!(completed.status, SprintStatus::Completed);
    assert_eq!(
        completed.get_name(&board),
        Some("Alpha"),
        "complete must resolve the name"
    );
    assert_eq!(scoped(activate_inv), EntityIds::sprints([alpha.id]));
    assert_eq!(scoped(complete_inv), EntityIds::sprints([alpha.id]));

    assert_eq!(moved, 2);
    let carried = scoped(carry_inv);
    assert_eq!(carried.cards.len(), 2);
    assert!(carried.cards.contains(&first.id) && carried.cards.contains(&second.id));
    for id in [first.id, second.id] {
        assert_eq!(
            ctx.data_store().get_card(id)?.unwrap().sprint_id,
            Some(beta.id)
        );
    }

    server.shutdown().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_cancel_sprint_over_http_returns_a_named_sprint_and_the_server_invalidation(
) -> KanbanResult<()> {
    let server = TestServer::start().await;
    let mut ctx = ctx_over(&server).await;
    let (board, _) = ctx.create_board_from_spec(None, a_new_board())?;
    let (sprint, _) =
        ctx.create_sprint_from_spec(board.id, None, Some("Alpha".into()), None, false)?;

    let (cancelled, invalidation) = ctx.cancel_sprint_impl(sprint.id)?;

    let board = ctx.get_board(board.id)?.unwrap();
    assert_eq!(cancelled.status, SprintStatus::Cancelled);
    assert_eq!(
        cancelled.get_name(&board),
        Some("Alpha"),
        "cancel must resolve the name"
    );
    assert_eq!(scoped(invalidation), EntityIds::sprints([sprint.id]));
    assert_eq!(
        ctx.data_store().list_sprints_by_board(board.id)?[0].status,
        SprintStatus::Cancelled
    );

    server.shutdown().await;
    Ok(())
}
