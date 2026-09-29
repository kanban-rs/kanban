use kanban_backend::RemoteGraphWrites;
use kanban_backend_http::HttpBackend;
use kanban_domain::{
    CardPriority, CardStatus, CardUpdate, Column, ColumnUpdate, FieldUpdate, GraphOperations,
    Invalidation, KanbanOperations, NewBoard, NewCard, NewColumn, RelatesKind, Severity,
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
    assert!(backend.remote_sprint_writes().is_none());
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
    assert!(err.to_string().contains("NOT_FOUND"), "got: {err}");

    let graph = ctx.data_store().get_graph().unwrap();
    assert!(
        graph.contains(parent.id, child.id),
        "the real edge must survive an all-or-nothing failed batch"
    );

    server.shutdown().await;
}
