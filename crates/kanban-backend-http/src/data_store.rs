use crate::HttpBackend;
use kanban_domain::{
    ArchivedBoard, ArchivedCard, Board, Card, Column, DataStore, DependencyGraph, KanbanError,
    KanbanResult, Prefix, Snapshot, Sprint,
};
use uuid::Uuid;

impl DataStore for HttpBackend {
    fn get_prefix(&self, _name: &str) -> KanbanResult<Option<Prefix>> {
        Err(KanbanError::unsupported("get_prefix"))
    }

    fn list_prefixes(&self) -> KanbanResult<Vec<Prefix>> {
        Err(KanbanError::unsupported("list_prefixes"))
    }

    fn upsert_prefix(&self, _prefix: Prefix) -> KanbanResult<()> {
        Err(KanbanError::unsupported("upsert_prefix"))
    }

    fn get_board(&self, _id: Uuid) -> KanbanResult<Option<Board>> {
        Err(KanbanError::unsupported("get_board"))
    }

    fn list_boards(&self) -> KanbanResult<Vec<Board>> {
        Err(KanbanError::unsupported("list_boards"))
    }

    fn upsert_board(&self, _board: Board) -> KanbanResult<()> {
        Err(KanbanError::unsupported("upsert_board"))
    }

    fn delete_board(&self, _id: Uuid) -> KanbanResult<()> {
        Err(KanbanError::unsupported("delete_board"))
    }

    fn get_column(&self, _id: Uuid) -> KanbanResult<Option<Column>> {
        Err(KanbanError::unsupported("get_column"))
    }

    fn list_columns_by_board(&self, _board_id: Uuid) -> KanbanResult<Vec<Column>> {
        Err(KanbanError::unsupported("list_columns_by_board"))
    }

    fn list_all_columns(&self) -> KanbanResult<Vec<Column>> {
        Err(KanbanError::unsupported("list_all_columns"))
    }

    fn upsert_column(&self, _column: Column) -> KanbanResult<()> {
        Err(KanbanError::unsupported("upsert_column"))
    }

    fn delete_column(&self, _id: Uuid) -> KanbanResult<()> {
        Err(KanbanError::unsupported("delete_column"))
    }

    fn delete_columns_by_board(&self, _board_id: Uuid) -> KanbanResult<()> {
        Err(KanbanError::unsupported("delete_columns_by_board"))
    }

    fn get_card(&self, _id: Uuid) -> KanbanResult<Option<Card>> {
        Err(KanbanError::unsupported("get_card"))
    }

    fn list_all_cards(&self) -> KanbanResult<Vec<Card>> {
        Err(KanbanError::unsupported("list_all_cards"))
    }

    fn list_cards_by_column(&self, _column_id: Uuid) -> KanbanResult<Vec<Card>> {
        Err(KanbanError::unsupported("list_cards_by_column"))
    }

    fn list_cards_by_sprint(&self, _sprint_id: Uuid) -> KanbanResult<Vec<Card>> {
        Err(KanbanError::unsupported("list_cards_by_sprint"))
    }

    fn count_cards_in_column(&self, _column_id: Uuid) -> KanbanResult<usize> {
        Err(KanbanError::unsupported("count_cards_in_column"))
    }

    fn count_cards_in_column_excluding(
        &self,
        _column_id: Uuid,
        _exclude_ids: &[Uuid],
    ) -> KanbanResult<usize> {
        Err(KanbanError::unsupported("count_cards_in_column_excluding"))
    }

    fn upsert_card(&self, _card: Card) -> KanbanResult<()> {
        Err(KanbanError::unsupported("upsert_card"))
    }

    fn delete_card(&self, _id: Uuid) -> KanbanResult<()> {
        Err(KanbanError::unsupported("delete_card"))
    }

    fn delete_cards_by_columns(&self, _column_ids: &[Uuid]) -> KanbanResult<()> {
        Err(KanbanError::unsupported("delete_cards_by_columns"))
    }

    fn clear_sprint_from_cards(
        &self,
        _sprint_id: Uuid,
        _timestamp: chrono::DateTime<chrono::Utc>,
    ) -> KanbanResult<()> {
        Err(KanbanError::unsupported("clear_sprint_from_cards"))
    }

    fn get_archived_card(&self, _card_id: Uuid) -> KanbanResult<Option<ArchivedCard>> {
        Err(KanbanError::unsupported("get_archived_card"))
    }

    fn list_archived_cards(&self) -> KanbanResult<Vec<ArchivedCard>> {
        Err(KanbanError::unsupported("list_archived_cards"))
    }

    fn insert_archived_card(&self, _ac: ArchivedCard) -> KanbanResult<()> {
        Err(KanbanError::unsupported("insert_archived_card"))
    }

    fn delete_archived_card(&self, _card_id: Uuid) -> KanbanResult<()> {
        Err(KanbanError::unsupported("delete_archived_card"))
    }

    fn get_archived_board(&self, _board_id: Uuid) -> KanbanResult<Option<ArchivedBoard>> {
        Err(KanbanError::unsupported("get_archived_board"))
    }

    fn list_archived_boards(&self) -> KanbanResult<Vec<ArchivedBoard>> {
        Err(KanbanError::unsupported("list_archived_boards"))
    }

    fn insert_archived_board(&self, _ab: ArchivedBoard) -> KanbanResult<()> {
        Err(KanbanError::unsupported("insert_archived_board"))
    }

    fn delete_archived_board(&self, _board_id: Uuid) -> KanbanResult<()> {
        Err(KanbanError::unsupported("delete_archived_board"))
    }

    fn unarchive_board(&self, _board_id: Uuid) -> KanbanResult<()> {
        Err(KanbanError::unsupported("unarchive_board"))
    }

    fn get_sprint(&self, _id: Uuid) -> KanbanResult<Option<Sprint>> {
        Err(KanbanError::unsupported("get_sprint"))
    }

    fn list_sprints_by_board(&self, _board_id: Uuid) -> KanbanResult<Vec<Sprint>> {
        Err(KanbanError::unsupported("list_sprints_by_board"))
    }

    fn list_all_sprints(&self) -> KanbanResult<Vec<Sprint>> {
        Err(KanbanError::unsupported("list_all_sprints"))
    }

    fn upsert_sprint(&self, _sprint: Sprint) -> KanbanResult<()> {
        Err(KanbanError::unsupported("upsert_sprint"))
    }

    fn delete_sprint(&self, _id: Uuid) -> KanbanResult<()> {
        Err(KanbanError::unsupported("delete_sprint"))
    }

    fn delete_sprints_by_board(&self, _board_id: Uuid) -> KanbanResult<()> {
        Err(KanbanError::unsupported("delete_sprints_by_board"))
    }

    fn get_graph(&self) -> KanbanResult<DependencyGraph> {
        Err(KanbanError::unsupported("get_graph"))
    }

    fn set_graph(&self, _graph: DependencyGraph) -> KanbanResult<()> {
        Err(KanbanError::unsupported("set_graph"))
    }

    fn snapshot(&self) -> KanbanResult<Snapshot> {
        Err(KanbanError::unsupported("snapshot"))
    }

    fn apply_snapshot(&self, _snapshot: Snapshot) -> KanbanResult<()> {
        Err(KanbanError::unsupported("apply_snapshot"))
    }
}
