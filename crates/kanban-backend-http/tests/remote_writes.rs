use kanban_backend_http::HttpBackend;
use kanban_domain::{
    CardPriority, CardStatus, CardUpdate, Column, ColumnUpdate, FieldUpdate, GraphOperations,
    Invalidation, KanbanOperations, NewBoard, NewCard, NewColumn,
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
