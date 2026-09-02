#![allow(dead_code)]

use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use kanban_backend_memory::InMemoryStore;
use kanban_domain::{
    ArchivedBoard, ArchivedCard, Board, Card, Column, DataStore, DependencyGraph, KanbanError,
    KanbanResult, Prefix, Snapshot, Sprint,
};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadOp {
    pub method: &'static str,
    pub ids: Vec<Uuid>,
}

pub type ReadOpLog = Arc<Mutex<Vec<ReadOp>>>;

pub fn assert_ops(log: &Mutex<Vec<ReadOp>>, expected: &[ReadOp]) {
    let actual = log.lock().unwrap().clone();
    assert_eq!(
        actual.as_slice(),
        expected,
        "read op log mismatch\n  expected: {expected:?}\n  actual:   {actual:?}"
    );
}

pub struct RecordingStore {
    inner: InMemoryStore,
    ops: ReadOpLog,
    fail_cards: Mutex<HashSet<Uuid>>,
    fail_columns: Mutex<HashSet<Uuid>>,
    fail_sprints: Mutex<HashSet<Uuid>>,
    fail_methods: Mutex<HashSet<&'static str>>,
}

impl RecordingStore {
    pub fn new() -> Self {
        Self {
            inner: InMemoryStore::new(),
            ops: Arc::new(Mutex::new(Vec::new())),
            fail_cards: Mutex::new(HashSet::new()),
            fail_columns: Mutex::new(HashSet::new()),
            fail_sprints: Mutex::new(HashSet::new()),
            fail_methods: Mutex::new(HashSet::new()),
        }
    }

    pub fn ops(&self) -> ReadOpLog {
        self.ops.clone()
    }

    pub fn clear_log(&self) {
        self.ops.lock().unwrap().clear();
    }

    pub fn fail_card(&self, id: Uuid) {
        self.fail_cards.lock().unwrap().insert(id);
    }

    pub fn fail_column(&self, id: Uuid) {
        self.fail_columns.lock().unwrap().insert(id);
    }

    pub fn fail_sprint(&self, id: Uuid) {
        self.fail_sprints.lock().unwrap().insert(id);
    }

    pub fn fail_method(&self, method: &'static str) {
        self.fail_methods.lock().unwrap().insert(method);
    }

    pub fn clear_failures(&self) {
        self.fail_cards.lock().unwrap().clear();
        self.fail_columns.lock().unwrap().clear();
        self.fail_sprints.lock().unwrap().clear();
        self.fail_methods.lock().unwrap().clear();
    }

    fn check(&self, method: &'static str) -> KanbanResult<()> {
        if self.fail_methods.lock().unwrap().contains(method) {
            return Err(KanbanError::unsupported("injected read failure"));
        }
        Ok(())
    }

    fn record(&self, method: &'static str, ids: Vec<Uuid>) {
        self.ops.lock().unwrap().push(ReadOp { method, ids });
    }
}

impl Default for RecordingStore {
    fn default() -> Self {
        Self::new()
    }
}

impl DataStore for RecordingStore {
    fn get_prefix(&self, name: &str) -> KanbanResult<Option<Prefix>> {
        self.record("get_prefix", vec![]);
        self.inner.get_prefix(name)
    }
    fn list_prefixes(&self) -> KanbanResult<Vec<Prefix>> {
        self.record("list_prefixes", vec![]);
        self.inner.list_prefixes()
    }
    fn upsert_prefix(&self, prefix: Prefix) -> KanbanResult<()> {
        self.inner.upsert_prefix(prefix)
    }
    fn get_board(&self, id: Uuid) -> KanbanResult<Option<Board>> {
        self.record("get_board", vec![id]);
        self.inner.get_board(id)
    }
    fn list_boards(&self) -> KanbanResult<Vec<Board>> {
        self.record("list_boards", vec![]);
        self.check("list_boards")?;
        self.inner.list_boards()
    }
    fn upsert_board(&self, board: Board) -> KanbanResult<()> {
        self.inner.upsert_board(board)
    }
    fn delete_board(&self, id: Uuid) -> KanbanResult<()> {
        self.inner.delete_board(id)
    }
    fn get_column(&self, id: Uuid) -> KanbanResult<Option<Column>> {
        self.record("get_column", vec![id]);
        if self.fail_columns.lock().unwrap().contains(&id) {
            return Err(KanbanError::unsupported("injected read failure"));
        }
        self.inner.get_column(id)
    }
    fn list_columns_by_board(&self, board_id: Uuid) -> KanbanResult<Vec<Column>> {
        self.record("list_columns_by_board", vec![board_id]);
        self.check("list_columns_by_board")?;
        self.inner.list_columns_by_board(board_id)
    }
    fn list_all_columns(&self) -> KanbanResult<Vec<Column>> {
        self.record("list_all_columns", vec![]);
        self.check("list_all_columns")?;
        self.inner.list_all_columns()
    }
    fn upsert_column(&self, column: Column) -> KanbanResult<()> {
        self.inner.upsert_column(column)
    }
    fn delete_column(&self, id: Uuid) -> KanbanResult<()> {
        self.inner.delete_column(id)
    }
    fn delete_columns_by_board(&self, board_id: Uuid) -> KanbanResult<()> {
        self.inner.delete_columns_by_board(board_id)
    }
    fn get_card(&self, id: Uuid) -> KanbanResult<Option<Card>> {
        self.record("get_card", vec![id]);
        if self.fail_cards.lock().unwrap().contains(&id) {
            return Err(KanbanError::unsupported("injected read failure"));
        }
        self.inner.get_card(id)
    }
    fn list_all_cards(&self) -> KanbanResult<Vec<Card>> {
        self.record("list_all_cards", vec![]);
        self.check("list_all_cards")?;
        self.inner.list_all_cards()
    }
    fn list_cards_by_column(&self, column_id: Uuid) -> KanbanResult<Vec<Card>> {
        self.record("list_cards_by_column", vec![column_id]);
        self.check("list_cards_by_column")?;
        self.inner.list_cards_by_column(column_id)
    }
    fn list_cards_by_sprint(&self, sprint_id: Uuid) -> KanbanResult<Vec<Card>> {
        self.record("list_cards_by_sprint", vec![sprint_id]);
        self.inner.list_cards_by_sprint(sprint_id)
    }
    fn count_cards_in_column(&self, column_id: Uuid) -> KanbanResult<usize> {
        self.record("count_cards_in_column", vec![column_id]);
        self.inner.count_cards_in_column(column_id)
    }
    fn count_cards_in_column_excluding(
        &self,
        column_id: Uuid,
        exclude_ids: &[Uuid],
    ) -> KanbanResult<usize> {
        self.record("count_cards_in_column_excluding", vec![column_id]);
        self.inner
            .count_cards_in_column_excluding(column_id, exclude_ids)
    }
    fn upsert_card(&self, card: Card) -> KanbanResult<()> {
        self.inner.upsert_card(card)
    }
    fn delete_card(&self, id: Uuid) -> KanbanResult<()> {
        self.inner.delete_card(id)
    }
    fn delete_cards_by_columns(&self, column_ids: &[Uuid]) -> KanbanResult<()> {
        self.inner.delete_cards_by_columns(column_ids)
    }
    fn clear_sprint_from_cards(
        &self,
        sprint_id: Uuid,
        cleared_at: chrono::DateTime<chrono::Utc>,
    ) -> KanbanResult<()> {
        self.inner.clear_sprint_from_cards(sprint_id, cleared_at)
    }
    fn get_archived_card(&self, card_id: Uuid) -> KanbanResult<Option<ArchivedCard>> {
        self.record("get_archived_card", vec![card_id]);
        self.inner.get_archived_card(card_id)
    }
    fn list_archived_cards(&self) -> KanbanResult<Vec<ArchivedCard>> {
        self.record("list_archived_cards", vec![]);
        self.check("list_archived_cards")?;
        self.inner.list_archived_cards()
    }
    fn list_archived_cards_by_board(&self, board_id: Uuid) -> KanbanResult<Vec<ArchivedCard>> {
        self.record("list_archived_cards_by_board", vec![board_id]);
        self.check("list_archived_cards_by_board")?;
        self.inner.list_archived_cards_by_board(board_id)
    }
    fn insert_archived_card(&self, ac: ArchivedCard) -> KanbanResult<()> {
        self.inner.insert_archived_card(ac)
    }
    fn delete_archived_card(&self, card_id: Uuid) -> KanbanResult<()> {
        self.inner.delete_archived_card(card_id)
    }
    fn get_archived_board(&self, board_id: Uuid) -> KanbanResult<Option<ArchivedBoard>> {
        self.inner.get_archived_board(board_id)
    }
    fn list_archived_boards(&self) -> KanbanResult<Vec<ArchivedBoard>> {
        self.record("list_archived_boards", vec![]);
        self.check("list_archived_boards")?;
        self.inner.list_archived_boards()
    }
    fn insert_archived_board(&self, ab: ArchivedBoard) -> KanbanResult<()> {
        self.inner.insert_archived_board(ab)
    }
    fn delete_archived_board(&self, board_id: Uuid) -> KanbanResult<()> {
        self.inner.delete_archived_board(board_id)
    }
    fn unarchive_board(&self, board_id: Uuid) -> KanbanResult<()> {
        self.inner.unarchive_board(board_id)
    }
    fn get_sprint(&self, id: Uuid) -> KanbanResult<Option<Sprint>> {
        self.record("get_sprint", vec![id]);
        if self.fail_sprints.lock().unwrap().contains(&id) {
            return Err(KanbanError::unsupported("injected read failure"));
        }
        self.inner.get_sprint(id)
    }
    fn list_sprints_by_board(&self, board_id: Uuid) -> KanbanResult<Vec<Sprint>> {
        self.record("list_sprints_by_board", vec![board_id]);
        self.check("list_sprints_by_board")?;
        self.inner.list_sprints_by_board(board_id)
    }
    fn list_all_sprints(&self) -> KanbanResult<Vec<Sprint>> {
        self.record("list_all_sprints", vec![]);
        self.check("list_all_sprints")?;
        self.inner.list_all_sprints()
    }
    fn upsert_sprint(&self, sprint: Sprint) -> KanbanResult<()> {
        self.inner.upsert_sprint(sprint)
    }
    fn delete_sprint(&self, id: Uuid) -> KanbanResult<()> {
        self.inner.delete_sprint(id)
    }
    fn delete_sprints_by_board(&self, board_id: Uuid) -> KanbanResult<()> {
        self.inner.delete_sprints_by_board(board_id)
    }
    fn get_graph(&self) -> KanbanResult<DependencyGraph> {
        self.record("get_graph", vec![]);
        self.check("get_graph")?;
        self.inner.get_graph()
    }
    fn set_graph(&self, graph: DependencyGraph) -> KanbanResult<()> {
        self.inner.set_graph(graph)
    }
    fn snapshot(&self) -> KanbanResult<Snapshot> {
        self.record("snapshot", vec![]);
        self.inner.snapshot()
    }
    fn apply_snapshot(&self, snapshot: Snapshot) -> KanbanResult<()> {
        self.inner.apply_snapshot(snapshot)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_recorder_logs_one_get_card_for_one_get_card_call() {
        let store = RecordingStore::new();
        let card = Card::new(Uuid::new_v4(), Uuid::new_v4(), "t", 0);
        let id = card.id;
        store.upsert_card(card).unwrap();
        store.clear_log();

        let fetched = store.get_card(id).unwrap();

        assert!(fetched.is_some());
        assert_ops(
            &store.ops(),
            &[ReadOp {
                method: "get_card",
                ids: vec![id],
            }],
        );
    }
}
