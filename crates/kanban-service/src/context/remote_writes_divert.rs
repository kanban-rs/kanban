#![cfg(test)]

use super::KanbanContext;
use crate::backend_test_support::MockBackend;
use kanban_backend::RemoteWrites;
use kanban_core::AppConfig;
use kanban_domain::{
    Board, BoardUpdate, Card, Column, ColumnUpdate, EntityIds, Invalidation, KanbanResult,
    NewBoard, NewCard, NewColumn, UndoOperations,
};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

const SENTINEL: Uuid = Uuid::from_u128(0x1571);

fn canned_inv() -> Invalidation {
    Invalidation::Entities(EntityIds::boards([SENTINEL]))
}

struct RecordingRemoteWrites {
    calls: Mutex<Vec<String>>,
    canned: Invalidation,
}

impl RecordingRemoteWrites {
    fn new(canned: Invalidation) -> Self {
        Self {
            calls: Mutex::new(Vec::new()),
            canned,
        }
    }

    fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }

    fn record(&self, call: String) {
        self.calls.lock().unwrap().push(call);
    }
}

impl RemoteWrites for RecordingRemoteWrites {
    fn create_board(
        &self,
        id: Option<Uuid>,
        _spec: &NewBoard,
    ) -> KanbanResult<(Board, Invalidation)> {
        self.record(format!("create_board:{id:?}"));
        Ok((Board::new("remote", None::<String>), self.canned.clone()))
    }

    fn update_board(
        &self,
        id: Uuid,
        _updates: &BoardUpdate,
    ) -> KanbanResult<(Board, Invalidation)> {
        self.record(format!("update_board:{id}"));
        let mut board = Board::new("remote", None::<String>);
        board.id = id;
        Ok((board, self.canned.clone()))
    }

    fn delete_board(&self, id: Uuid) -> KanbanResult<Invalidation> {
        self.record(format!("delete_board:{id}"));
        Ok(self.canned.clone())
    }

    fn create_column(
        &self,
        board_id: Uuid,
        _spec: &NewColumn,
    ) -> KanbanResult<(Column, Invalidation)> {
        self.record(format!("create_column:{board_id}"));
        Ok((Column::new(board_id, "remote", 0), self.canned.clone()))
    }

    fn update_column(
        &self,
        id: Uuid,
        _updates: &ColumnUpdate,
    ) -> KanbanResult<(Column, Invalidation)> {
        self.record(format!("update_column:{id}"));
        let mut column = Column::new(Uuid::nil(), "remote", 0);
        column.id = id;
        Ok((column, self.canned.clone()))
    }

    fn delete_column(&self, id: Uuid) -> KanbanResult<Invalidation> {
        self.record(format!("delete_column:{id}"));
        Ok(self.canned.clone())
    }

    fn create_card(&self, id: Option<Uuid>, _spec: &NewCard) -> KanbanResult<(Card, Invalidation)> {
        self.record(format!("create_card:{id:?}"));
        Ok((
            Card::new(Uuid::nil(), Uuid::nil(), "remote", 0),
            self.canned.clone(),
        ))
    }

    fn update_card(
        &self,
        id: Uuid,
        _updates: &kanban_domain::CardUpdate,
    ) -> KanbanResult<(Card, Invalidation)> {
        self.record(format!("update_card:{id}"));
        let mut card = Card::new(Uuid::nil(), Uuid::nil(), "remote", 0);
        card.id = id;
        Ok((card, self.canned.clone()))
    }

    fn delete_card(&self, id: Uuid) -> KanbanResult<Invalidation> {
        self.record(format!("delete_card:{id}"));
        Ok(self.canned.clone())
    }
}

async fn open_ctx(rw: Arc<RecordingRemoteWrites>) -> KanbanContext {
    let backend = Arc::new(MockBackend::with_remote_writes(rw));
    KanbanContext::open(backend, AppConfig::default())
        .await
        .unwrap()
}

fn new_board() -> NewBoard {
    NewBoard {
        name: "board".into(),
        description: None,
        sprint_prefix: None,
        card_prefix: None,
        task_sort_field: None,
        task_sort_order: None,
        sprint_duration_days: None,
        task_list_view: None,
    }
}

fn new_column(board_id: Uuid) -> NewColumn {
    NewColumn {
        board_id,
        name: "column".into(),
        wip_limit: None,
        default_status: None,
    }
}

fn new_card(column_id: Uuid) -> NewCard {
    NewCard {
        column_id,
        title: "card".into(),
        description: None,
        priority: kanban_domain::CardPriority::Medium,
        due_date: None,
        points: None,
        sprint_id: None,
    }
}

#[tokio::test]
async fn test_create_board_from_spec_with_remote_writes_diverts_before_local_prework() {
    let rw = Arc::new(RecordingRemoteWrites::new(canned_inv()));
    let mut ctx = open_ctx(rw.clone()).await;

    let (_, inv) = ctx.create_board_from_spec(None, new_board()).unwrap();

    assert_eq!(rw.calls(), vec!["create_board:None".to_string()]);
    assert_eq!(inv, canned_inv());
    assert!(ctx.data_store().list_boards().unwrap().is_empty());
}

#[tokio::test]
async fn test_update_board_with_remote_writes_diverts_and_returns_the_server_invalidation() {
    let rw = Arc::new(RecordingRemoteWrites::new(canned_inv()));
    let mut ctx = open_ctx(rw.clone()).await;
    let id = Uuid::new_v4();

    let (board, inv) = ctx.update_board_impl(id, BoardUpdate::default()).unwrap();

    assert_eq!(rw.calls(), vec![format!("update_board:{id}")]);
    assert_eq!(board.id, id);
    assert_eq!(inv, canned_inv());
}

#[tokio::test]
async fn test_delete_board_with_remote_writes_diverts_without_building_a_local_cascade() {
    let rw = Arc::new(RecordingRemoteWrites::new(canned_inv()));
    let mut ctx = open_ctx(rw.clone()).await;

    let board = Board::new("local", None::<String>);
    let board_id = board.id;
    let column = Column::new(board_id, "col", 0);
    let column_id = column.id;
    let card = Card::new(board_id, column_id, "card", 0);
    let card_id = card.id;
    ctx.data_store().upsert_board(board).unwrap();
    ctx.data_store().upsert_column(column).unwrap();
    ctx.data_store().upsert_card(card).unwrap();

    let inv = ctx.delete_board_impl(board_id).unwrap();

    assert_eq!(rw.calls(), vec![format!("delete_board:{board_id}")]);
    assert_eq!(inv, canned_inv());
    assert!(ctx.data_store().get_board(board_id).unwrap().is_some());
    assert!(ctx.data_store().get_column(column_id).unwrap().is_some());
    assert!(ctx.data_store().get_card(card_id).unwrap().is_some());
}

#[tokio::test]
async fn test_create_column_from_spec_with_remote_writes_diverts() {
    let rw = Arc::new(RecordingRemoteWrites::new(canned_inv()));
    let mut ctx = open_ctx(rw.clone()).await;
    let board_id = Uuid::new_v4();

    let (_, inv) = ctx
        .create_column_from_spec(None, new_column(board_id))
        .unwrap();

    assert_eq!(rw.calls(), vec![format!("create_column:{board_id}")]);
    assert_eq!(inv, canned_inv());
    assert!(ctx
        .data_store()
        .list_columns_by_board(board_id)
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn test_update_column_with_remote_writes_diverts() {
    let rw = Arc::new(RecordingRemoteWrites::new(canned_inv()));
    let mut ctx = open_ctx(rw.clone()).await;
    let id = Uuid::new_v4();

    let (column, inv) = ctx.update_column_impl(id, ColumnUpdate::default()).unwrap();

    assert_eq!(rw.calls(), vec![format!("update_column:{id}")]);
    assert_eq!(column.id, id);
    assert_eq!(inv, canned_inv());
}

#[tokio::test]
async fn test_delete_column_with_remote_writes_diverts() {
    let rw = Arc::new(RecordingRemoteWrites::new(canned_inv()));
    let mut ctx = open_ctx(rw.clone()).await;

    let board_id = Uuid::new_v4();
    let column = Column::new(board_id, "col", 0);
    let column_id = column.id;
    ctx.data_store().upsert_column(column).unwrap();

    let inv = ctx.delete_column_impl(column_id).unwrap();

    assert_eq!(rw.calls(), vec![format!("delete_column:{column_id}")]);
    assert_eq!(inv, canned_inv());
    assert!(ctx.data_store().get_column(column_id).unwrap().is_some());
}

#[tokio::test]
async fn test_create_card_from_spec_with_remote_writes_diverts_without_number_allocation() {
    let rw = Arc::new(RecordingRemoteWrites::new(canned_inv()));
    let mut ctx = open_ctx(rw.clone()).await;

    let board = Board::new("board", None::<String>);
    let board_id = board.id;
    let column = Column::new(board_id, "col", 0);
    let column_id = column.id;
    ctx.data_store().upsert_board(board.clone()).unwrap();
    ctx.data_store().upsert_column(column).unwrap();

    let (_, inv) = ctx
        .create_card_from_spec(None, new_card(column_id))
        .unwrap();

    assert_eq!(rw.calls(), vec!["create_card:None".to_string()]);
    assert_eq!(inv, canned_inv());
    assert!(ctx.data_store().list_all_cards().unwrap().is_empty());
    assert_eq!(ctx.data_store().get_board(board_id).unwrap(), Some(board));
}

#[tokio::test]
async fn test_update_card_with_remote_writes_diverts_and_returns_the_server_invalidation() {
    let rw = Arc::new(RecordingRemoteWrites::new(canned_inv()));
    let mut ctx = open_ctx(rw.clone()).await;
    let id = Uuid::new_v4();

    let (card, inv) = ctx
        .update_card_impl(id, kanban_domain::CardUpdate::default())
        .unwrap();

    assert_eq!(rw.calls(), vec![format!("update_card:{id}")]);
    assert_eq!(card.id, id);
    assert_eq!(inv, canned_inv());
    assert!(!UndoOperations::can_undo(&ctx));
}

#[tokio::test]
async fn test_delete_card_with_remote_writes_diverts() {
    let rw = Arc::new(RecordingRemoteWrites::new(canned_inv()));
    let mut ctx = open_ctx(rw.clone()).await;

    let board_id = Uuid::new_v4();
    let column_id = Uuid::new_v4();
    let card = Card::new(board_id, column_id, "card", 0);
    let card_id = card.id;
    ctx.data_store().upsert_card(card).unwrap();

    let inv = ctx.delete_card_impl(card_id).unwrap();

    assert_eq!(rw.calls(), vec![format!("delete_card:{card_id}")]);
    assert_eq!(inv, canned_inv());
    assert!(ctx.data_store().get_card(card_id).unwrap().is_some());
}

#[tokio::test]
async fn test_create_column_with_explicit_position_still_hits_the_fence() {
    let rw = Arc::new(RecordingRemoteWrites::new(canned_inv()));
    let mut ctx = open_ctx(rw.clone()).await;
    let board_id = Uuid::new_v4();

    let result = ctx.create_column_impl(board_id, "c".into(), Some(0));

    assert!(result.is_err());
    assert!(result.unwrap_err().is_unsupported());
    assert!(rw.calls().is_empty());
}
