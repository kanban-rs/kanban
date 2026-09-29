#![cfg(test)]

use super::KanbanContext;
use crate::backend_test_support::MockBackend;
use kanban_backend::{
    RemoteBatchOutcome, RemoteBatchWrites, RemoteBoardWrites, RemoteCardWrites, RemoteGraphWrites,
    RemoteWrites,
};
use kanban_core::AppConfig;
use kanban_domain::{
    Board, BoardUpdate, Card, Column, ColumnUpdate, EntityIds, Invalidation, KanbanResult,
    NewBoard, NewCard, NewColumn, RelatesKind, Severity, UndoOperations,
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

struct RecordingBoardWrites {
    calls: Mutex<Vec<String>>,
    canned: Invalidation,
}

impl RecordingBoardWrites {
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

impl RemoteBoardWrites for RecordingBoardWrites {
    fn archive_board(&self, id: Uuid) -> KanbanResult<Invalidation> {
        self.record(format!("archive_board:{id}"));
        Ok(self.canned.clone())
    }

    fn restore_board(&self, id: Uuid) -> KanbanResult<Invalidation> {
        self.record(format!("restore_board:{id}"));
        Ok(self.canned.clone())
    }
}

struct RecordingCardWrites {
    calls: Mutex<Vec<String>>,
    canned: Invalidation,
}

impl RecordingCardWrites {
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

impl RemoteCardWrites for RecordingCardWrites {
    fn archive_card(&self, id: Uuid) -> KanbanResult<Invalidation> {
        self.record(format!("archive_card:{id}"));
        Ok(self.canned.clone())
    }

    fn restore_card(
        &self,
        id: Uuid,
        column_id: Option<Uuid>,
    ) -> KanbanResult<(Card, Invalidation)> {
        self.record(format!("restore_card:{id}:{column_id:?}"));
        let mut card = Card::new(Uuid::nil(), Uuid::nil(), "remote", 0);
        card.id = id;
        Ok((card, self.canned.clone()))
    }

    fn move_card(
        &self,
        id: Uuid,
        column_id: Uuid,
        position: Option<i32>,
    ) -> KanbanResult<(Card, Invalidation)> {
        self.record(format!("move_card:{id}:{column_id}:{position:?}"));
        let mut card = Card::new(Uuid::nil(), Uuid::nil(), "remote", 0);
        card.id = id;
        Ok((card, self.canned.clone()))
    }

    fn assign_card_to_sprint(
        &self,
        id: Uuid,
        sprint_id: Uuid,
    ) -> KanbanResult<(Card, Invalidation)> {
        self.record(format!("assign_card_to_sprint:{id}:{sprint_id}"));
        let mut card = Card::new(Uuid::nil(), Uuid::nil(), "remote", 0);
        card.id = id;
        Ok((card, self.canned.clone()))
    }

    fn unassign_card_from_sprint(&self, id: Uuid) -> KanbanResult<(Card, Invalidation)> {
        self.record(format!("unassign_card_from_sprint:{id}"));
        let mut card = Card::new(Uuid::nil(), Uuid::nil(), "remote", 0);
        card.id = id;
        Ok((card, self.canned.clone()))
    }
}

struct RecordingGraphWrites {
    calls: Mutex<Vec<String>>,
    canned: Invalidation,
}

impl RecordingGraphWrites {
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

impl RemoteGraphWrites for RecordingGraphWrites {
    fn attach_children(&self, parent: Uuid, children: &[Uuid]) -> KanbanResult<Invalidation> {
        self.record(format!("attach_children:{parent}:{children:?}"));
        Ok(self.canned.clone())
    }

    fn detach_children(&self, parent: Uuid, children: &[Uuid]) -> KanbanResult<Invalidation> {
        self.record(format!("detach_children:{parent}:{children:?}"));
        Ok(self.canned.clone())
    }

    fn block(
        &self,
        blocker: Uuid,
        blocked: Uuid,
        severity: Severity,
    ) -> KanbanResult<Invalidation> {
        self.record(format!("block:{blocker}:{blocked}:{severity:?}"));
        Ok(self.canned.clone())
    }

    fn unblock(&self, blocker: Uuid, blocked: Uuid) -> KanbanResult<Invalidation> {
        self.record(format!("unblock:{blocker}:{blocked}"));
        Ok(self.canned.clone())
    }

    fn relate(&self, a: Uuid, b: Uuid, kind: RelatesKind) -> KanbanResult<Invalidation> {
        self.record(format!("relate:{a}:{b}:{kind:?}"));
        Ok(self.canned.clone())
    }

    fn dissociate(&self, a: Uuid, b: Uuid) -> KanbanResult<Invalidation> {
        self.record(format!("dissociate:{a}:{b}"));
        Ok(self.canned.clone())
    }
}

struct RecordingBatchWrites {
    calls: Mutex<Vec<String>>,
    canned: Invalidation,
    outcome: RemoteBatchOutcome,
}

impl RecordingBatchWrites {
    fn new(canned: Invalidation, outcome: RemoteBatchOutcome) -> Self {
        Self {
            calls: Mutex::new(Vec::new()),
            canned,
            outcome,
        }
    }

    fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }

    fn record(&self, call: String) {
        self.calls.lock().unwrap().push(call);
    }
}

impl RemoteBatchWrites for RecordingBatchWrites {
    fn archive_cards(&self, ids: &[Uuid]) -> KanbanResult<(RemoteBatchOutcome, Invalidation)> {
        self.record(format!("archive_cards:{ids:?}"));
        Ok((self.outcome.clone(), self.canned.clone()))
    }

    fn move_cards(
        &self,
        ids: &[Uuid],
        column_id: Uuid,
    ) -> KanbanResult<(RemoteBatchOutcome, Invalidation)> {
        self.record(format!("move_cards:{ids:?}:{column_id}"));
        Ok((self.outcome.clone(), self.canned.clone()))
    }

    fn assign_cards_to_sprint(
        &self,
        ids: &[Uuid],
        sprint_id: Uuid,
    ) -> KanbanResult<(RemoteBatchOutcome, Invalidation)> {
        self.record(format!("assign_cards_to_sprint:{ids:?}:{sprint_id}"));
        Ok((self.outcome.clone(), self.canned.clone()))
    }

    fn update_cards(
        &self,
        updates: &[(Uuid, kanban_domain::CardUpdate)],
    ) -> KanbanResult<(RemoteBatchOutcome, Invalidation)> {
        let ids: Vec<Uuid> = updates.iter().map(|(id, _)| *id).collect();
        self.record(format!("update_cards:{ids:?}"));
        Ok((self.outcome.clone(), self.canned.clone()))
    }
}

async fn open_ctx_with_batch_writes(
    rw: Arc<RecordingRemoteWrites>,
    batch_rw: Arc<RecordingBatchWrites>,
) -> KanbanContext {
    let backend = Arc::new(MockBackend::with_remote_batch_writes(rw, batch_rw));
    KanbanContext::open(backend, AppConfig::default())
        .await
        .unwrap()
}

async fn open_ctx(rw: Arc<RecordingRemoteWrites>) -> KanbanContext {
    let backend = Arc::new(MockBackend::with_remote_writes(rw));
    KanbanContext::open(backend, AppConfig::default())
        .await
        .unwrap()
}

async fn open_ctx_with_board_writes(
    rw: Arc<RecordingRemoteWrites>,
    board_rw: Arc<RecordingBoardWrites>,
) -> KanbanContext {
    let backend = Arc::new(MockBackend::with_remote_board_writes(rw, board_rw));
    KanbanContext::open(backend, AppConfig::default())
        .await
        .unwrap()
}

async fn open_ctx_with_card_writes(
    rw: Arc<RecordingRemoteWrites>,
    card_rw: Arc<RecordingCardWrites>,
) -> KanbanContext {
    let backend = Arc::new(MockBackend::with_remote_card_writes(rw, card_rw));
    KanbanContext::open(backend, AppConfig::default())
        .await
        .unwrap()
}

async fn open_ctx_with_graph_writes(
    rw: Arc<RecordingRemoteWrites>,
    graph_rw: Arc<RecordingGraphWrites>,
) -> KanbanContext {
    let backend = Arc::new(MockBackend::with_remote_graph_writes(rw, graph_rw));
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
async fn test_archive_board_with_remote_board_writes_diverts_before_local_prework() {
    let rw = Arc::new(RecordingRemoteWrites::new(canned_inv()));
    let board_rw = Arc::new(RecordingBoardWrites::new(canned_inv()));
    let mut ctx = open_ctx_with_board_writes(rw.clone(), board_rw.clone()).await;
    let id = Uuid::new_v4();

    let inv = ctx.archive_board_impl(id).unwrap();

    assert_eq!(board_rw.calls(), vec![format!("archive_board:{id}")]);
    assert!(rw.calls().is_empty());
    assert_eq!(inv, canned_inv());
}

#[tokio::test]
async fn test_restore_board_with_remote_board_writes_diverts_before_local_prework() {
    let rw = Arc::new(RecordingRemoteWrites::new(canned_inv()));
    let board_rw = Arc::new(RecordingBoardWrites::new(canned_inv()));
    let mut ctx = open_ctx_with_board_writes(rw.clone(), board_rw.clone()).await;
    let id = Uuid::new_v4();

    let inv = ctx.restore_board_impl(id).unwrap();

    assert_eq!(board_rw.calls(), vec![format!("restore_board:{id}")]);
    assert!(rw.calls().is_empty());
    assert_eq!(inv, canned_inv());
}

#[tokio::test]
async fn test_archive_board_with_remote_writes_but_no_board_writes_declines_with_a_per_op_message()
{
    let rw = Arc::new(RecordingRemoteWrites::new(canned_inv()));
    let mut ctx = open_ctx(rw.clone()).await;

    let err = ctx.archive_board_impl(Uuid::new_v4()).unwrap_err();

    assert!(err.is_unsupported(), "got: {err:?}");
    assert_eq!(
        err.to_string(),
        kanban_domain::KanbanError::unsupported("archive_board").to_string()
    );
    assert!(rw.calls().is_empty());
}

#[tokio::test]
async fn test_restore_board_with_remote_writes_but_no_board_writes_declines_with_a_per_op_message()
{
    let rw = Arc::new(RecordingRemoteWrites::new(canned_inv()));
    let mut ctx = open_ctx(rw.clone()).await;

    let err = ctx.restore_board_impl(Uuid::new_v4()).unwrap_err();

    assert!(err.is_unsupported(), "got: {err:?}");
    assert_eq!(
        err.to_string(),
        kanban_domain::KanbanError::unsupported("restore_board").to_string()
    );
    assert!(rw.calls().is_empty());
}

#[tokio::test]
async fn test_archive_card_with_remote_card_writes_diverts_before_local_prework() {
    let rw = Arc::new(RecordingRemoteWrites::new(canned_inv()));
    let card_rw = Arc::new(RecordingCardWrites::new(canned_inv()));
    let mut ctx = open_ctx_with_card_writes(rw.clone(), card_rw.clone()).await;
    let id = Uuid::new_v4();

    let ((), inv) = ctx.archive_card_impl(id).unwrap();

    assert_eq!(card_rw.calls(), vec![format!("archive_card:{id}")]);
    assert!(rw.calls().is_empty());
    assert_eq!(inv, canned_inv());
}

#[tokio::test]
async fn test_restore_card_with_remote_card_writes_diverts_before_local_prework() {
    let rw = Arc::new(RecordingRemoteWrites::new(canned_inv()));
    let card_rw = Arc::new(RecordingCardWrites::new(canned_inv()));
    let mut ctx = open_ctx_with_card_writes(rw.clone(), card_rw.clone()).await;
    let id = Uuid::new_v4();
    let column_id = Uuid::new_v4();

    let (card, inv) = ctx.restore_card_impl(id, Some(column_id)).unwrap();

    assert_eq!(
        card_rw.calls(),
        vec![format!("restore_card:{id}:{:?}", Some(column_id))]
    );
    assert!(rw.calls().is_empty());
    assert_eq!(card.id, id);
    assert_eq!(inv, canned_inv());
}

#[tokio::test]
async fn test_archive_card_with_remote_writes_but_no_card_writes_declines_with_a_per_op_message() {
    let rw = Arc::new(RecordingRemoteWrites::new(canned_inv()));
    let mut ctx = open_ctx(rw.clone()).await;

    let err = ctx.archive_card_impl(Uuid::new_v4()).unwrap_err();

    assert!(err.is_unsupported(), "got: {err:?}");
    assert_eq!(
        err.to_string(),
        kanban_domain::KanbanError::unsupported("archive_card").to_string()
    );
    assert!(rw.calls().is_empty());
}

#[tokio::test]
async fn test_restore_card_with_remote_writes_but_no_card_writes_declines_with_a_per_op_message() {
    let rw = Arc::new(RecordingRemoteWrites::new(canned_inv()));
    let mut ctx = open_ctx(rw.clone()).await;

    let err = ctx.restore_card_impl(Uuid::new_v4(), None).unwrap_err();

    assert!(err.is_unsupported(), "got: {err:?}");
    assert_eq!(
        err.to_string(),
        kanban_domain::KanbanError::unsupported("restore_card").to_string()
    );
    assert!(rw.calls().is_empty());
}

#[tokio::test]
async fn test_move_card_with_remote_card_writes_diverts_before_local_prework() {
    let rw = Arc::new(RecordingRemoteWrites::new(canned_inv()));
    let card_rw = Arc::new(RecordingCardWrites::new(canned_inv()));
    let mut ctx = open_ctx_with_card_writes(rw.clone(), card_rw.clone()).await;
    let id = Uuid::new_v4();
    let column_id = Uuid::new_v4();

    let (card, inv) = ctx.move_card_impl(id, column_id, None).unwrap();

    assert_eq!(
        card_rw.calls(),
        vec![format!("move_card:{id}:{column_id}:None")]
    );
    assert!(rw.calls().is_empty());
    assert_eq!(card.id, id);
    assert_eq!(inv, canned_inv());
}

#[tokio::test]
async fn test_assign_card_to_sprint_with_remote_card_writes_diverts_before_the_batch_path() {
    let rw = Arc::new(RecordingRemoteWrites::new(canned_inv()));
    let card_rw = Arc::new(RecordingCardWrites::new(canned_inv()));
    let mut ctx = open_ctx_with_card_writes(rw.clone(), card_rw.clone()).await;
    let card_id = Uuid::new_v4();
    let sprint_id = Uuid::new_v4();

    let (card, inv) = ctx.assign_card_to_sprint_impl(card_id, sprint_id).unwrap();

    assert_eq!(
        card_rw.calls(),
        vec![format!("assign_card_to_sprint:{card_id}:{sprint_id}")]
    );
    assert!(rw.calls().is_empty());
    assert_eq!(card.id, card_id);
    assert_eq!(inv, canned_inv());
}

#[tokio::test]
async fn test_unassign_card_from_sprint_with_remote_card_writes_diverts_before_local_prework() {
    let rw = Arc::new(RecordingRemoteWrites::new(canned_inv()));
    let card_rw = Arc::new(RecordingCardWrites::new(canned_inv()));
    let mut ctx = open_ctx_with_card_writes(rw.clone(), card_rw.clone()).await;
    let card_id = Uuid::new_v4();

    let (card, inv) = ctx.unassign_card_from_sprint_impl(card_id).unwrap();

    assert_eq!(
        card_rw.calls(),
        vec![format!("unassign_card_from_sprint:{card_id}")]
    );
    assert!(rw.calls().is_empty());
    assert_eq!(card.id, card_id);
    assert_eq!(inv, canned_inv());
}

#[tokio::test]
async fn test_move_card_with_remote_writes_but_no_card_writes_declines_with_a_per_op_message() {
    let rw = Arc::new(RecordingRemoteWrites::new(canned_inv()));
    let mut ctx = open_ctx(rw.clone()).await;

    let err = ctx
        .move_card_impl(Uuid::new_v4(), Uuid::new_v4(), None)
        .unwrap_err();

    assert!(err.is_unsupported(), "got: {err:?}");
    assert_eq!(
        err.to_string(),
        kanban_domain::KanbanError::unsupported("move_card").to_string()
    );
    assert!(rw.calls().is_empty());
}

#[tokio::test]
async fn test_assign_card_to_sprint_with_remote_writes_but_no_card_writes_declines_with_a_per_op_message(
) {
    let rw = Arc::new(RecordingRemoteWrites::new(canned_inv()));
    let mut ctx = open_ctx(rw.clone()).await;

    let err = ctx
        .assign_card_to_sprint_impl(Uuid::new_v4(), Uuid::new_v4())
        .unwrap_err();

    assert!(err.is_unsupported(), "got: {err:?}");
    assert_eq!(
        err.to_string(),
        kanban_domain::KanbanError::unsupported("assign_card_to_sprint").to_string()
    );
    assert!(rw.calls().is_empty());
}

#[tokio::test]
async fn test_unassign_card_from_sprint_with_remote_writes_but_no_card_writes_declines_with_a_per_op_message(
) {
    let rw = Arc::new(RecordingRemoteWrites::new(canned_inv()));
    let mut ctx = open_ctx(rw.clone()).await;

    let err = ctx
        .unassign_card_from_sprint_impl(Uuid::new_v4())
        .unwrap_err();

    assert!(err.is_unsupported(), "got: {err:?}");
    assert_eq!(
        err.to_string(),
        kanban_domain::KanbanError::unsupported("unassign_card_from_sprint").to_string()
    );
    assert!(rw.calls().is_empty());
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

#[tokio::test]
async fn test_attach_children_with_remote_graph_writes_diverts_before_any_card_read() {
    let rw = Arc::new(RecordingRemoteWrites::new(canned_inv()));
    let graph_rw = Arc::new(RecordingGraphWrites::new(canned_inv()));
    let mut ctx = open_ctx_with_graph_writes(rw.clone(), graph_rw.clone()).await;
    let parent = Uuid::new_v4();
    let children = vec![Uuid::new_v4()];

    let inv = ctx.attach_children_impl(parent, children.clone()).unwrap();

    assert_eq!(
        graph_rw.calls(),
        vec![format!("attach_children:{parent}:{children:?}")]
    );
    assert!(rw.calls().is_empty());
    assert_eq!(inv, canned_inv());
}

#[tokio::test]
async fn test_detach_children_with_remote_graph_writes_sends_every_child_in_one_call() {
    let rw = Arc::new(RecordingRemoteWrites::new(canned_inv()));
    let graph_rw = Arc::new(RecordingGraphWrites::new(canned_inv()));
    let mut ctx = open_ctx_with_graph_writes(rw.clone(), graph_rw.clone()).await;
    let parent = Uuid::new_v4();
    let children = vec![Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4()];

    let inv = ctx.detach_children_impl(parent, children.clone()).unwrap();

    assert_eq!(
        graph_rw.calls(),
        vec![format!("detach_children:{parent}:{children:?}")]
    );
    assert!(rw.calls().is_empty());
    assert_eq!(inv, canned_inv());
}

#[tokio::test]
async fn test_block_and_relate_with_remote_graph_writes_pass_severity_and_kind_through() {
    let rw = Arc::new(RecordingRemoteWrites::new(canned_inv()));
    let graph_rw = Arc::new(RecordingGraphWrites::new(canned_inv()));
    let mut ctx = open_ctx_with_graph_writes(rw.clone(), graph_rw.clone()).await;
    let a = Uuid::new_v4();
    let b = Uuid::new_v4();

    let block_inv = ctx.block_impl(a, b, Severity::High).unwrap();
    let relate_inv = ctx.relate_impl(a, b, RelatesKind::Duplicates).unwrap();

    assert_eq!(
        graph_rw.calls(),
        vec![
            format!("block:{a}:{b}:High"),
            format!("relate:{a}:{b}:Duplicates"),
        ]
    );
    assert!(rw.calls().is_empty());
    assert_eq!(block_inv, canned_inv());
    assert_eq!(relate_inv, canned_inv());
}

#[tokio::test]
async fn test_each_graph_op_with_remote_writes_but_no_graph_writes_declines_with_a_per_op_message()
{
    let rw = Arc::new(RecordingRemoteWrites::new(canned_inv()));
    let mut ctx = open_ctx(rw.clone()).await;
    let a = Uuid::new_v4();
    let b = Uuid::new_v4();

    let cases: Vec<(&str, kanban_domain::KanbanError)> = vec![
        (
            "attach_children",
            ctx.attach_children_impl(a, vec![b]).unwrap_err(),
        ),
        (
            "detach_children",
            ctx.detach_children_impl(a, vec![b]).unwrap_err(),
        ),
        ("block", ctx.block_impl(a, b, Severity::Medium).unwrap_err()),
        ("unblock", ctx.unblock_impl(a, b).unwrap_err()),
        (
            "relate",
            ctx.relate_impl(a, b, RelatesKind::Duplicates).unwrap_err(),
        ),
        ("dissociate", ctx.dissociate_impl(a, b).unwrap_err()),
    ];

    for (method, err) in cases {
        assert!(err.is_unsupported(), "{method} got: {err:?}");
        assert_eq!(
            err.to_string(),
            kanban_domain::KanbanError::unsupported(method).to_string(),
            "{method} message mismatch"
        );
    }
    assert!(rw.calls().is_empty());
}

#[tokio::test]
async fn test_archive_cards_with_remote_batch_writes_all_failed_returns_the_first_failure_as_validation(
) {
    let rw = Arc::new(RecordingRemoteWrites::new(canned_inv()));
    let a = Uuid::new_v4();
    let outcome = RemoteBatchOutcome {
        succeeded: vec![],
        failed: vec![(a, "Card a not found".to_string())],
    };
    let batch_rw = Arc::new(RecordingBatchWrites::new(canned_inv(), outcome));
    let mut ctx = open_ctx_with_batch_writes(rw.clone(), batch_rw.clone()).await;

    let err = ctx.archive_cards_impl(vec![a]).unwrap_err();

    assert!(err.to_string().contains("Card a not found"), "got: {err}");
    assert!(rw.calls().is_empty());
}

#[tokio::test]
async fn test_archive_cards_with_remote_batch_writes_returns_the_server_count_and_invalidation_verbatim(
) {
    let rw = Arc::new(RecordingRemoteWrites::new(canned_inv()));
    let a = Uuid::new_v4();
    let b = Uuid::new_v4();
    let outcome = RemoteBatchOutcome {
        succeeded: vec![a, b],
        failed: vec![],
    };
    let batch_rw = Arc::new(RecordingBatchWrites::new(canned_inv(), outcome));
    let mut ctx = open_ctx_with_batch_writes(rw.clone(), batch_rw.clone()).await;

    let (count, inv) = ctx.archive_cards_impl(vec![a, b]).unwrap();

    assert_eq!(count, 2);
    assert_eq!(inv, canned_inv());
    assert_eq!(
        batch_rw.calls(),
        vec![format!("archive_cards:{:?}", [a, b])]
    );
    assert!(rw.calls().is_empty());
}

#[tokio::test]
async fn test_move_cards_with_remote_batch_writes_diverts_before_the_local_dedup_and_reads() {
    let rw = Arc::new(RecordingRemoteWrites::new(canned_inv()));
    let a = Uuid::new_v4();
    let column_id = Uuid::new_v4();
    let ids = vec![a, a];
    let outcome = RemoteBatchOutcome {
        succeeded: vec![a],
        failed: vec![],
    };
    let batch_rw = Arc::new(RecordingBatchWrites::new(canned_inv(), outcome));
    let mut ctx = open_ctx_with_batch_writes(rw.clone(), batch_rw.clone()).await;

    let (count, inv) = ctx.move_cards_impl(ids.clone(), column_id).unwrap();

    assert_eq!(count, 1);
    assert_eq!(inv, canned_inv());
    assert_eq!(
        batch_rw.calls(),
        vec![format!("move_cards:{ids:?}:{column_id}")]
    );
    assert!(rw.calls().is_empty());
}

#[tokio::test]
async fn test_move_cards_with_remote_writes_but_no_batch_writes_declines_with_a_per_op_message() {
    let rw = Arc::new(RecordingRemoteWrites::new(canned_inv()));
    let mut ctx = open_ctx(rw.clone()).await;

    let err = ctx
        .move_cards_impl(vec![Uuid::new_v4()], Uuid::new_v4())
        .unwrap_err();

    assert!(err.is_unsupported(), "got: {err:?}");
    assert_eq!(
        err.to_string(),
        kanban_domain::KanbanError::unsupported("move_cards").to_string()
    );
    assert!(rw.calls().is_empty());
}

#[tokio::test]
async fn test_assign_cards_to_sprint_with_remote_batch_writes_diverts_before_local_reads() {
    let rw = Arc::new(RecordingRemoteWrites::new(canned_inv()));
    let a = Uuid::new_v4();
    let sprint_id = Uuid::new_v4();
    let outcome = RemoteBatchOutcome {
        succeeded: vec![a],
        failed: vec![],
    };
    let batch_rw = Arc::new(RecordingBatchWrites::new(canned_inv(), outcome));
    let mut ctx = open_ctx_with_batch_writes(rw.clone(), batch_rw.clone()).await;

    let (count, inv) = ctx.assign_cards_to_sprint_impl(vec![a], sprint_id).unwrap();

    assert_eq!(count, 1);
    assert_eq!(inv, canned_inv());
    assert_eq!(
        batch_rw.calls(),
        vec![format!("assign_cards_to_sprint:{:?}:{sprint_id}", [a])]
    );
    assert!(rw.calls().is_empty());
}

#[tokio::test]
async fn test_assign_cards_to_sprint_with_remote_writes_but_no_batch_writes_declines_with_a_per_op_message(
) {
    let rw = Arc::new(RecordingRemoteWrites::new(canned_inv()));
    let mut ctx = open_ctx(rw.clone()).await;

    let err = ctx
        .assign_cards_to_sprint_impl(vec![Uuid::new_v4()], Uuid::new_v4())
        .unwrap_err();

    assert!(err.is_unsupported(), "got: {err:?}");
    assert_eq!(
        err.to_string(),
        kanban_domain::KanbanError::unsupported("assign_cards_to_sprint").to_string()
    );
    assert!(rw.calls().is_empty());
}

#[tokio::test]
async fn test_update_cards_with_remote_batch_writes_diverts_before_local_reads() {
    let rw = Arc::new(RecordingRemoteWrites::new(canned_inv()));
    let a = Uuid::new_v4();
    let outcome = RemoteBatchOutcome {
        succeeded: vec![a],
        failed: vec![],
    };
    let batch_rw = Arc::new(RecordingBatchWrites::new(canned_inv(), outcome));
    let mut ctx = open_ctx_with_batch_writes(rw.clone(), batch_rw.clone()).await;

    let (count, inv) = ctx
        .update_cards_impl(vec![(a, kanban_domain::CardUpdate::default())])
        .unwrap();

    assert_eq!(count, 1);
    assert_eq!(inv, canned_inv());
    assert_eq!(batch_rw.calls(), vec![format!("update_cards:{:?}", [a])]);
    assert!(rw.calls().is_empty());
}

#[tokio::test]
async fn test_update_cards_with_remote_writes_but_no_batch_writes_declines_with_a_per_op_message() {
    let rw = Arc::new(RecordingRemoteWrites::new(canned_inv()));
    let mut ctx = open_ctx(rw.clone()).await;

    let err = ctx
        .update_cards_impl(vec![(Uuid::new_v4(), kanban_domain::CardUpdate::default())])
        .unwrap_err();

    assert!(err.is_unsupported(), "got: {err:?}");
    assert_eq!(
        err.to_string(),
        kanban_domain::KanbanError::unsupported("update_cards").to_string()
    );
    assert!(rw.calls().is_empty());
}
