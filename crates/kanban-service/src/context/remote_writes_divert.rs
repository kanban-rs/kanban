#![cfg(test)]

use super::{BatchOperationFailure, KanbanContext};
use crate::backend_test_support::MockBackend;
use kanban_api::{ApiError, ErrorCode};
use kanban_backend::{
    RemoteBatchOutcome, RemoteBatchWrites, RemoteBoardWrites, RemoteCardWrites, RemoteGraphWrites,
    RemoteSprintWrites, RemoteWrites,
};
use kanban_core::AppConfig;
use kanban_domain::{
    Board, BoardUpdate, Card, Column, ColumnUpdate, EntityIds, Invalidation, KanbanResult,
    NewBoard, NewCard, NewColumn, RelatesKind, Severity, Sprint, UndoOperations,
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

struct RecordingSprintWrites {
    calls: Mutex<Vec<String>>,
    canned: Invalidation,
}

impl RecordingSprintWrites {
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

impl RemoteSprintWrites for RecordingSprintWrites {
    fn create_sprint(
        &self,
        board_id: Uuid,
        id: Option<Uuid>,
        name: Option<&str>,
        prefix: Option<&str>,
    ) -> KanbanResult<(Sprint, Invalidation)> {
        self.record(format!(
            "create_sprint:{board_id}:{id:?}:{name:?}:{prefix:?}"
        ));
        let sprint = Sprint::new(board_id, 1, None, prefix.map(str::to_string));
        Ok((sprint, self.canned.clone()))
    }

    fn update_sprint(
        &self,
        id: Uuid,
        updates: &kanban_domain::SprintUpdate,
    ) -> KanbanResult<(Sprint, Invalidation)> {
        self.record(format!("update_sprint:{id}:{updates:?}"));
        let sprint = Sprint::new(Uuid::new_v4(), 1, Some(0), None::<String>);
        Ok((sprint, self.canned.clone()))
    }

    fn delete_sprint(&self, id: Uuid) -> KanbanResult<Invalidation> {
        self.record(format!("delete_sprint:{id}"));
        Ok(self.canned.clone())
    }

    fn activate_sprint(
        &self,
        id: Uuid,
        duration_days: Option<i32>,
    ) -> KanbanResult<(Sprint, Invalidation)> {
        self.record(format!("activate_sprint:{id}:{duration_days:?}"));
        let sprint = Sprint::new(Uuid::new_v4(), 1, Some(0), None::<String>);
        Ok((sprint, self.canned.clone()))
    }

    fn complete_sprint(&self, id: Uuid) -> KanbanResult<(Sprint, Invalidation)> {
        self.record(format!("complete_sprint:{id}"));
        let sprint = Sprint::new(Uuid::new_v4(), 1, Some(0), None::<String>);
        Ok((sprint, self.canned.clone()))
    }

    fn cancel_sprint(&self, id: Uuid) -> KanbanResult<(Sprint, Invalidation)> {
        self.record(format!("cancel_sprint:{id}"));
        let sprint = Sprint::new(Uuid::new_v4(), 1, Some(0), None::<String>);
        Ok((sprint, self.canned.clone()))
    }

    fn carry_over_sprint_cards(
        &self,
        from_sprint_id: Uuid,
        to_sprint_id: Uuid,
    ) -> KanbanResult<(usize, Invalidation)> {
        self.record(format!(
            "carry_over_sprint_cards:{from_sprint_id}:{to_sprint_id}"
        ));
        Ok((0, self.canned.clone()))
    }
}

struct RecordingBatchWrites {
    calls: Mutex<Vec<String>>,
    canned: Invalidation,
    outcome: RemoteBatchOutcome,
    transport_error: Option<String>,
}

impl RecordingBatchWrites {
    fn new(canned: Invalidation, outcome: RemoteBatchOutcome) -> Self {
        Self {
            calls: Mutex::new(Vec::new()),
            canned,
            outcome,
            transport_error: None,
        }
    }

    fn new_transport_error(message: impl Into<String>) -> Self {
        Self {
            calls: Mutex::new(Vec::new()),
            canned: canned_inv(),
            outcome: RemoteBatchOutcome {
                succeeded: vec![],
                failed: vec![],
            },
            transport_error: Some(message.into()),
        }
    }

    fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }

    fn record(&self, call: String) {
        self.calls.lock().unwrap().push(call);
    }

    fn result(&self) -> KanbanResult<(RemoteBatchOutcome, Invalidation)> {
        match &self.transport_error {
            Some(message) => Err(kanban_domain::KanbanError::Transport(message.clone())),
            None => Ok((self.outcome.clone(), self.canned.clone())),
        }
    }
}

impl RemoteBatchWrites for RecordingBatchWrites {
    fn archive_cards(&self, ids: &[Uuid]) -> KanbanResult<(RemoteBatchOutcome, Invalidation)> {
        self.record(format!("archive_cards:{ids:?}"));
        self.result()
    }

    fn move_cards(
        &self,
        ids: &[Uuid],
        column_id: Uuid,
    ) -> KanbanResult<(RemoteBatchOutcome, Invalidation)> {
        self.record(format!("move_cards:{ids:?}:{column_id}"));
        self.result()
    }

    fn assign_cards_to_sprint(
        &self,
        ids: &[Uuid],
        sprint_id: Uuid,
    ) -> KanbanResult<(RemoteBatchOutcome, Invalidation)> {
        self.record(format!("assign_cards_to_sprint:{ids:?}:{sprint_id}"));
        self.result()
    }

    fn update_cards(
        &self,
        updates: &[(Uuid, kanban_domain::CardUpdate)],
    ) -> KanbanResult<(RemoteBatchOutcome, Invalidation)> {
        let ids: Vec<Uuid> = updates.iter().map(|(id, _)| *id).collect();
        self.record(format!("update_cards:{ids:?}"));
        self.result()
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

async fn open_ctx_with_sprint_writes(
    rw: Arc<RecordingRemoteWrites>,
    sprint_rw: Arc<RecordingSprintWrites>,
) -> KanbanContext {
    let backend = Arc::new(MockBackend::with_remote_sprint_writes(rw, sprint_rw));
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
async fn test_archive_cards_with_remote_batch_writes_all_failed_rebuilds_the_error_through_from_api_error(
) {
    let rw = Arc::new(RecordingRemoteWrites::new(canned_inv()));
    let a = Uuid::new_v4();
    let api_error = ApiError::new(ErrorCode::NotFound, "Card a not found");
    let outcome = RemoteBatchOutcome {
        succeeded: vec![],
        failed: vec![(a, api_error.clone())],
    };
    let batch_rw = Arc::new(RecordingBatchWrites::new(canned_inv(), outcome));
    let mut ctx = open_ctx_with_batch_writes(rw.clone(), batch_rw.clone()).await;

    let err = ctx.archive_cards_impl(vec![a]).unwrap_err();

    assert_eq!(
        err.to_string(),
        kanban_domain::KanbanError::from(api_error).to_string()
    );
    assert!(rw.calls().is_empty());
}

#[tokio::test]
async fn test_archive_cards_with_remote_batch_writes_all_failed_with_a_server_fault_returns_internal(
) {
    let rw = Arc::new(RecordingRemoteWrites::new(canned_inv()));
    let a = Uuid::new_v4();
    let api_error = ApiError::new(ErrorCode::DatabaseError, "internal server error");
    let outcome = RemoteBatchOutcome {
        succeeded: vec![],
        failed: vec![(a, api_error)],
    };
    let batch_rw = Arc::new(RecordingBatchWrites::new(canned_inv(), outcome));
    let mut ctx = open_ctx_with_batch_writes(rw.clone(), batch_rw.clone()).await;

    let err = ctx.archive_cards_impl(vec![a]).unwrap_err();

    assert!(
        matches!(err, kanban_domain::KanbanError::Internal(_)),
        "got: {err:?}"
    );
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
async fn test_move_cards_with_remote_batch_writes_partial_failure_counts_only_the_succeeded_ids() {
    let rw = Arc::new(RecordingRemoteWrites::new(canned_inv()));
    let a = Uuid::new_v4();
    let b = Uuid::new_v4();
    let column_id = Uuid::new_v4();
    let outcome = RemoteBatchOutcome {
        succeeded: vec![a],
        failed: vec![(b, ApiError::new(ErrorCode::NotFound, "Card b not found"))],
    };
    let batch_rw = Arc::new(RecordingBatchWrites::new(canned_inv(), outcome));
    let mut ctx = open_ctx_with_batch_writes(rw.clone(), batch_rw.clone()).await;

    let (count, inv) = ctx.move_cards_impl(vec![a, b], column_id).unwrap();

    assert_eq!(count, 1);
    assert_eq!(inv, canned_inv());
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

#[tokio::test]
async fn test_archive_cards_detailed_transport_error_fails_every_id_and_invalidates_all() {
    let rw = Arc::new(RecordingRemoteWrites::new(canned_inv()));
    let a = Uuid::new_v4();
    let b = Uuid::new_v4();
    let batch_rw = Arc::new(RecordingBatchWrites::new_transport_error(
        "connection refused",
    ));
    let mut ctx = open_ctx_with_batch_writes(rw.clone(), batch_rw.clone()).await;

    let (result, inv) = ctx.archive_cards_detailed(vec![a, b]);

    assert!(result.succeeded.is_empty(), "got: {:?}", result.succeeded);
    let failed_ids: Vec<Uuid> = result.failed.iter().map(|f| f.id).collect();
    assert_eq!(failed_ids, vec![a, b]);
    assert!(
        result
            .failed
            .iter()
            .all(|f| f.error.contains("connection refused")),
        "got: {:?}",
        result.failed
    );
    assert_eq!(inv, Invalidation::All);
    assert!(rw.calls().is_empty());
}

#[tokio::test]
async fn test_move_cards_detailed_transport_error_fails_every_id_and_invalidates_all() {
    let rw = Arc::new(RecordingRemoteWrites::new(canned_inv()));
    let a = Uuid::new_v4();
    let b = Uuid::new_v4();
    let column_id = Uuid::new_v4();
    let batch_rw = Arc::new(RecordingBatchWrites::new_transport_error(
        "connection refused",
    ));
    let mut ctx = open_ctx_with_batch_writes(rw.clone(), batch_rw.clone()).await;

    let (result, inv) = ctx.move_cards_detailed(vec![a, b], column_id);

    assert!(result.succeeded.is_empty(), "got: {:?}", result.succeeded);
    let failed_ids: Vec<Uuid> = result.failed.iter().map(|f| f.id).collect();
    assert_eq!(failed_ids, vec![a, b]);
    assert!(
        result
            .failed
            .iter()
            .all(|f| f.error.contains("connection refused")),
        "got: {:?}",
        result.failed
    );
    assert_eq!(inv, Invalidation::All);
    assert!(rw.calls().is_empty());
}

#[tokio::test]
async fn test_assign_cards_to_sprint_detailed_transport_error_fails_every_id_and_invalidates_all() {
    let rw = Arc::new(RecordingRemoteWrites::new(canned_inv()));
    let a = Uuid::new_v4();
    let b = Uuid::new_v4();
    let sprint_id = Uuid::new_v4();
    let batch_rw = Arc::new(RecordingBatchWrites::new_transport_error(
        "connection refused",
    ));
    let mut ctx = open_ctx_with_batch_writes(rw.clone(), batch_rw.clone()).await;

    let (result, inv) = ctx.assign_cards_to_sprint_detailed(vec![a, b], sprint_id);

    assert!(result.succeeded.is_empty(), "got: {:?}", result.succeeded);
    let failed_ids: Vec<Uuid> = result.failed.iter().map(|f| f.id).collect();
    assert_eq!(failed_ids, vec![a, b]);
    assert!(
        result
            .failed
            .iter()
            .all(|f| f.error.contains("connection refused")),
        "got: {:?}",
        result.failed
    );
    assert_eq!(inv, Invalidation::All);
    assert!(rw.calls().is_empty());
}

#[tokio::test]
async fn test_move_cards_detailed_with_remote_batch_writes_returns_the_outcome_verbatim() {
    let rw = Arc::new(RecordingRemoteWrites::new(canned_inv()));
    let a = Uuid::new_v4();
    let b = Uuid::new_v4();
    let column_id = Uuid::new_v4();
    let outcome = RemoteBatchOutcome {
        succeeded: vec![a],
        failed: vec![(b, ApiError::new(ErrorCode::NotFound, "Card b not found"))],
    };
    let batch_rw = Arc::new(RecordingBatchWrites::new(canned_inv(), outcome));
    let mut ctx = open_ctx_with_batch_writes(rw.clone(), batch_rw.clone()).await;

    let (result, inv) = ctx.move_cards_detailed(vec![a, b], column_id);

    assert_eq!(result.succeeded, vec![a]);
    assert_eq!(result.failed.len(), 1);
    assert_eq!(result.failed[0].id, b);
    assert_eq!(result.failed[0].error, "Card b not found");
    assert_eq!(
        result.failed[0].api_error,
        ApiError::new(ErrorCode::NotFound, "Card b not found")
    );
    assert_eq!(inv, canned_inv());
    assert_eq!(
        batch_rw.calls(),
        vec![format!("move_cards:{:?}:{column_id}", [a, b])]
    );
    assert!(rw.calls().is_empty());
}

#[tokio::test]
async fn test_assign_cards_to_sprint_detailed_with_remote_batch_writes_diverts_before_the_sprint_read(
) {
    let rw = Arc::new(RecordingRemoteWrites::new(canned_inv()));
    let a = Uuid::new_v4();
    let sprint_id = Uuid::new_v4();
    let outcome = RemoteBatchOutcome {
        succeeded: vec![a],
        failed: vec![],
    };
    let batch_rw = Arc::new(RecordingBatchWrites::new(canned_inv(), outcome));
    let mut ctx = open_ctx_with_batch_writes(rw.clone(), batch_rw.clone()).await;

    let (result, inv) = ctx.assign_cards_to_sprint_detailed(vec![a], sprint_id);

    assert_eq!(result.succeeded, vec![a]);
    assert!(result.failed.is_empty());
    assert_eq!(inv, canned_inv());
    assert_eq!(
        batch_rw.calls(),
        vec![format!("assign_cards_to_sprint:{:?}:{sprint_id}", [a])]
    );
    assert!(rw.calls().is_empty());
}

#[tokio::test]
async fn test_archive_cards_detailed_with_remote_writes_but_no_batch_writes_fails_every_id_with_the_per_op_decline(
) {
    let rw = Arc::new(RecordingRemoteWrites::new(canned_inv()));
    let a = Uuid::new_v4();
    let b = Uuid::new_v4();
    let mut ctx = open_ctx(rw.clone()).await;

    let (result, inv) = ctx.archive_cards_detailed(vec![a, b]);

    assert!(result.succeeded.is_empty());
    let expected_error = kanban_domain::KanbanError::unsupported("archive_cards");
    let expected: Vec<BatchOperationFailure> = vec![a, b]
        .into_iter()
        .map(|id| BatchOperationFailure::new(id, &expected_error))
        .collect();
    for (got, want) in result.failed.iter().zip(expected.iter()) {
        assert_eq!(got.id, want.id);
        assert_eq!(got.error, want.error);
    }
    assert_eq!(result.failed.len(), 2);
    assert_eq!(inv, Invalidation::Entities(EntityIds::default()));
    assert!(rw.calls().is_empty());
}

#[tokio::test]
async fn test_create_sprint_from_spec_with_remote_sprint_writes_diverts_before_the_board_fk_read() {
    let rw = Arc::new(RecordingRemoteWrites::new(canned_inv()));
    let sprint_rw = Arc::new(RecordingSprintWrites::new(canned_inv()));
    let mut ctx = open_ctx_with_sprint_writes(rw.clone(), sprint_rw.clone()).await;
    let board_id = Uuid::new_v4();
    let id = Uuid::new_v4();

    let (_sprint, inv) = ctx
        .create_sprint_from_spec(
            board_id,
            Some(id),
            Some("Sprint 1".to_string()),
            Some("SPR".to_string()),
            false,
        )
        .unwrap();

    assert_eq!(
        sprint_rw.calls(),
        vec![format!(
            "create_sprint:{board_id}:{:?}:{:?}:{:?}",
            Some(id),
            Some("Sprint 1"),
            Some("SPR")
        )]
    );
    assert!(rw.calls().is_empty());
    assert_eq!(inv, canned_inv());
}

#[tokio::test]
async fn test_create_sprint_with_auto_consume_and_no_name_over_remote_declines_naming_the_flag() {
    let rw = Arc::new(RecordingRemoteWrites::new(canned_inv()));
    let sprint_rw = Arc::new(RecordingSprintWrites::new(canned_inv()));
    let mut ctx = open_ctx_with_sprint_writes(rw.clone(), sprint_rw.clone()).await;
    let board_id = Uuid::new_v4();

    let err = ctx
        .create_sprint_from_spec(board_id, None, None, None, true)
        .unwrap_err();

    assert!(err.is_unsupported(), "got: {err:?}");
    assert_eq!(
        err.to_string(),
        kanban_domain::KanbanError::unsupported("create_sprint.auto_consume_name over HTTP")
            .to_string()
    );
    assert!(sprint_rw.calls().is_empty());
    assert!(rw.calls().is_empty());
}

#[tokio::test]
async fn test_create_sprint_with_auto_consume_and_an_explicit_name_still_diverts() {
    let rw = Arc::new(RecordingRemoteWrites::new(canned_inv()));
    let sprint_rw = Arc::new(RecordingSprintWrites::new(canned_inv()));
    let mut ctx = open_ctx_with_sprint_writes(rw.clone(), sprint_rw.clone()).await;
    let board_id = Uuid::new_v4();

    let (_sprint, inv) = ctx
        .create_sprint_from_spec(board_id, None, Some("Sprint 1".to_string()), None, true)
        .unwrap();

    assert_eq!(
        sprint_rw.calls(),
        vec![format!(
            "create_sprint:{board_id}:{:?}:{:?}:{:?}",
            None::<Uuid>,
            Some("Sprint 1"),
            None::<&str>
        )]
    );
    assert!(rw.calls().is_empty());
    assert_eq!(inv, canned_inv());
}

#[tokio::test]
async fn test_create_or_replace_sprint_create_arm_diverts_transitively() {
    let rw = Arc::new(RecordingRemoteWrites::new(canned_inv()));
    let sprint_rw = Arc::new(RecordingSprintWrites::new(canned_inv()));
    let mut ctx = open_ctx_with_sprint_writes(rw.clone(), sprint_rw.clone()).await;
    let board_id = Uuid::new_v4();
    let id = Uuid::new_v4();

    let (outcome, inv) = ctx
        .create_or_replace_sprint(board_id, id, Some("Sprint 1".to_string()), None, false)
        .unwrap();

    assert!(outcome.created);
    assert_eq!(
        sprint_rw.calls(),
        vec![format!(
            "create_sprint:{board_id}:{:?}:{:?}:{:?}",
            Some(id),
            Some("Sprint 1"),
            None::<&str>
        )]
    );
    assert!(rw.calls().is_empty());
    assert_eq!(inv, canned_inv());
}

#[tokio::test]
async fn test_create_sprint_with_remote_writes_but_no_sprint_writes_declines_with_a_per_op_message()
{
    let rw = Arc::new(RecordingRemoteWrites::new(canned_inv()));
    let mut ctx = open_ctx(rw.clone()).await;

    let err = ctx
        .create_sprint_from_spec(Uuid::new_v4(), None, None, None, false)
        .unwrap_err();

    assert!(err.is_unsupported(), "got: {err:?}");
    assert_eq!(
        err.to_string(),
        kanban_domain::KanbanError::unsupported("create_sprint").to_string()
    );
    assert!(rw.calls().is_empty());
}

#[tokio::test]
async fn test_update_sprint_with_remote_sprint_writes_returns_the_server_sprint_not_a_local_reread()
{
    let rw = Arc::new(RecordingRemoteWrites::new(canned_inv()));
    let sprint_rw = Arc::new(RecordingSprintWrites::new(canned_inv()));
    let mut ctx = open_ctx_with_sprint_writes(rw.clone(), sprint_rw.clone()).await;
    let id = Uuid::new_v4();
    let updates = kanban_domain::SprintUpdate {
        name: Some("Renamed".to_string()),
        ..Default::default()
    };

    let (sprint, inv) = ctx.update_sprint_impl(id, updates.clone()).unwrap();

    assert_eq!(sprint.name_index, Some(0));
    assert_eq!(
        sprint_rw.calls(),
        vec![format!("update_sprint:{id}:{updates:?}")]
    );
    assert!(rw.calls().is_empty());
    assert_eq!(inv, canned_inv());
}

#[tokio::test]
async fn test_delete_sprint_with_remote_sprint_writes_diverts() {
    let rw = Arc::new(RecordingRemoteWrites::new(canned_inv()));
    let sprint_rw = Arc::new(RecordingSprintWrites::new(canned_inv()));
    let mut ctx = open_ctx_with_sprint_writes(rw.clone(), sprint_rw.clone()).await;
    let id = Uuid::new_v4();

    let inv = ctx.delete_sprint_impl(id).unwrap();

    assert_eq!(sprint_rw.calls(), vec![format!("delete_sprint:{id}")]);
    assert!(rw.calls().is_empty());
    assert_eq!(inv, canned_inv());
}

#[tokio::test]
async fn test_update_sprint_with_remote_writes_but_no_sprint_writes_declines_with_a_per_op_message()
{
    let rw = Arc::new(RecordingRemoteWrites::new(canned_inv()));
    let mut ctx = open_ctx(rw.clone()).await;

    let err = ctx
        .update_sprint_impl(Uuid::new_v4(), kanban_domain::SprintUpdate::default())
        .unwrap_err();

    assert!(err.is_unsupported(), "got: {err:?}");
    assert_eq!(
        err.to_string(),
        kanban_domain::KanbanError::unsupported("update_sprint").to_string()
    );
    assert!(rw.calls().is_empty());
}

#[tokio::test]
async fn test_delete_sprint_with_remote_writes_but_no_sprint_writes_declines_with_a_per_op_message()
{
    let rw = Arc::new(RecordingRemoteWrites::new(canned_inv()));
    let mut ctx = open_ctx(rw.clone()).await;

    let err = ctx.delete_sprint_impl(Uuid::new_v4()).unwrap_err();

    assert!(err.is_unsupported(), "got: {err:?}");
    assert_eq!(
        err.to_string(),
        kanban_domain::KanbanError::unsupported("delete_sprint").to_string()
    );
    assert!(rw.calls().is_empty());
}

#[tokio::test]
async fn test_carry_over_with_remote_sprint_writes_diverts_before_any_sprint_read() {
    let rw = Arc::new(RecordingRemoteWrites::new(canned_inv()));
    let sprint_rw = Arc::new(RecordingSprintWrites::new(canned_inv()));
    let mut ctx = open_ctx_with_sprint_writes(rw.clone(), sprint_rw.clone()).await;
    let from_id = Uuid::new_v4();
    let to_id = Uuid::new_v4();

    let (moved, inv) = ctx.carry_over_sprint_cards_impl(from_id, to_id).unwrap();

    assert_eq!(moved, 0);
    assert_eq!(
        sprint_rw.calls(),
        vec![format!("carry_over_sprint_cards:{from_id}:{to_id}")]
    );
    assert!(rw.calls().is_empty());
    assert_eq!(inv, canned_inv());
}

#[tokio::test]
async fn test_carry_over_with_remote_writes_but_no_sprint_writes_declines_with_a_per_op_message() {
    let rw = Arc::new(RecordingRemoteWrites::new(canned_inv()));
    let mut ctx = open_ctx(rw.clone()).await;

    let err = ctx
        .carry_over_sprint_cards_impl(Uuid::new_v4(), Uuid::new_v4())
        .unwrap_err();

    assert!(err.is_unsupported(), "got: {err:?}");
    assert_eq!(
        err.to_string(),
        kanban_domain::KanbanError::unsupported("carry_over_sprint_cards").to_string()
    );
    assert!(rw.calls().is_empty());
}

#[tokio::test]
async fn test_activate_sprint_with_remote_sprint_writes_passes_the_optional_duration_through_untouched(
) {
    let rw = Arc::new(RecordingRemoteWrites::new(canned_inv()));
    let sprint_rw = Arc::new(RecordingSprintWrites::new(canned_inv()));
    let mut ctx = open_ctx_with_sprint_writes(rw.clone(), sprint_rw.clone()).await;
    let id = Uuid::new_v4();

    let (_sprint, inv) = ctx.activate_sprint_impl(id, None).unwrap();
    assert_eq!(
        sprint_rw.calls(),
        vec![format!("activate_sprint:{id}:{:?}", None::<i32>)]
    );
    assert_eq!(inv, canned_inv());

    let id2 = Uuid::new_v4();
    let (_sprint, inv2) = ctx.activate_sprint_impl(id2, Some(7)).unwrap();
    assert_eq!(
        sprint_rw.calls(),
        vec![
            format!("activate_sprint:{id}:{:?}", None::<i32>),
            format!("activate_sprint:{id2}:{:?}", Some(7)),
        ]
    );
    assert!(rw.calls().is_empty());
    assert_eq!(inv2, canned_inv());
}

#[tokio::test]
async fn test_activate_sprint_with_remote_writes_but_no_sprint_writes_declines_with_a_per_op_message(
) {
    let rw = Arc::new(RecordingRemoteWrites::new(canned_inv()));
    let mut ctx = open_ctx(rw.clone()).await;

    let err = ctx.activate_sprint_impl(Uuid::new_v4(), None).unwrap_err();

    assert!(err.is_unsupported(), "got: {err:?}");
    assert_eq!(
        err.to_string(),
        kanban_domain::KanbanError::unsupported("activate_sprint").to_string()
    );
    assert!(rw.calls().is_empty());
}

#[tokio::test]
async fn test_complete_sprint_with_remote_sprint_writes_diverts() {
    let rw = Arc::new(RecordingRemoteWrites::new(canned_inv()));
    let sprint_rw = Arc::new(RecordingSprintWrites::new(canned_inv()));
    let mut ctx = open_ctx_with_sprint_writes(rw.clone(), sprint_rw.clone()).await;
    let id = Uuid::new_v4();

    let (_sprint, inv) = ctx.complete_sprint_impl(id).unwrap();

    assert_eq!(sprint_rw.calls(), vec![format!("complete_sprint:{id}")]);
    assert!(rw.calls().is_empty());
    assert_eq!(inv, canned_inv());
}

#[tokio::test]
async fn test_complete_sprint_with_remote_writes_but_no_sprint_writes_declines_with_a_per_op_message(
) {
    let rw = Arc::new(RecordingRemoteWrites::new(canned_inv()));
    let mut ctx = open_ctx(rw.clone()).await;

    let err = ctx.complete_sprint_impl(Uuid::new_v4()).unwrap_err();

    assert!(err.is_unsupported(), "got: {err:?}");
    assert_eq!(
        err.to_string(),
        kanban_domain::KanbanError::unsupported("complete_sprint").to_string()
    );
    assert!(rw.calls().is_empty());
}

#[tokio::test]
async fn test_cancel_sprint_with_remote_sprint_writes_diverts() {
    let rw = Arc::new(RecordingRemoteWrites::new(canned_inv()));
    let sprint_rw = Arc::new(RecordingSprintWrites::new(canned_inv()));
    let mut ctx = open_ctx_with_sprint_writes(rw.clone(), sprint_rw.clone()).await;
    let id = Uuid::new_v4();

    let (_sprint, inv) = ctx.cancel_sprint_impl(id).unwrap();

    assert_eq!(sprint_rw.calls(), vec![format!("cancel_sprint:{id}")]);
    assert!(rw.calls().is_empty());
    assert_eq!(inv, canned_inv());
}

#[tokio::test]
async fn test_cancel_sprint_with_remote_writes_but_no_sprint_writes_declines_with_a_per_op_message()
{
    let rw = Arc::new(RecordingRemoteWrites::new(canned_inv()));
    let mut ctx = open_ctx(rw.clone()).await;

    let err = ctx.cancel_sprint_impl(Uuid::new_v4()).unwrap_err();

    assert!(err.is_unsupported(), "got: {err:?}");
    assert_eq!(
        err.to_string(),
        kanban_domain::KanbanError::unsupported("cancel_sprint").to_string()
    );
    assert!(rw.calls().is_empty());
}
