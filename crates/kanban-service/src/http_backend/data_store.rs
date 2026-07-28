use kanban_domain::data_store::DataStore;
use kanban_domain::{
    ArchivedBoard, ArchivedCard, Board, Card, Column, DependencyGraph, KanbanResult, Snapshot,
    Sprint,
};
use uuid::Uuid;

use super::HttpBackend;

impl DataStore for HttpBackend {
    fn get_board(&self, _id: Uuid) -> KanbanResult<Option<Board>> {
        Err(kanban_domain::KanbanError::unsupported("get_board"))
    }

    fn list_boards(&self) -> KanbanResult<Vec<Board>> {
        Err(kanban_domain::KanbanError::unsupported("list_boards"))
    }

    fn upsert_board(&self, _board: Board) -> KanbanResult<()> {
        Err(kanban_domain::KanbanError::unsupported("upsert_board"))
    }

    fn delete_board(&self, _id: Uuid) -> KanbanResult<()> {
        Err(kanban_domain::KanbanError::unsupported("delete_board"))
    }

    fn get_column(&self, _id: Uuid) -> KanbanResult<Option<Column>> {
        Err(kanban_domain::KanbanError::unsupported("get_column"))
    }

    fn list_columns_by_board(&self, _board_id: Uuid) -> KanbanResult<Vec<Column>> {
        Err(kanban_domain::KanbanError::unsupported(
            "list_columns_by_board",
        ))
    }

    fn list_all_columns(&self) -> KanbanResult<Vec<Column>> {
        Err(kanban_domain::KanbanError::unsupported("list_all_columns"))
    }

    fn upsert_column(&self, _column: Column) -> KanbanResult<()> {
        Err(kanban_domain::KanbanError::unsupported("upsert_column"))
    }

    fn delete_column(&self, _id: Uuid) -> KanbanResult<()> {
        Err(kanban_domain::KanbanError::unsupported("delete_column"))
    }

    fn delete_columns_by_board(&self, _board_id: Uuid) -> KanbanResult<()> {
        Err(kanban_domain::KanbanError::unsupported(
            "delete_columns_by_board",
        ))
    }

    fn get_card(&self, _id: Uuid) -> KanbanResult<Option<Card>> {
        Err(kanban_domain::KanbanError::unsupported("get_card"))
    }

    fn list_all_cards(&self) -> KanbanResult<Vec<Card>> {
        Err(kanban_domain::KanbanError::unsupported("list_all_cards"))
    }

    fn list_cards_by_column(&self, _column_id: Uuid) -> KanbanResult<Vec<Card>> {
        Err(kanban_domain::KanbanError::unsupported(
            "list_cards_by_column",
        ))
    }

    fn list_cards_by_sprint(&self, _sprint_id: Uuid) -> KanbanResult<Vec<Card>> {
        Err(kanban_domain::KanbanError::unsupported(
            "list_cards_by_sprint",
        ))
    }

    fn count_cards_in_column(&self, _column_id: Uuid) -> KanbanResult<usize> {
        Err(kanban_domain::KanbanError::unsupported(
            "count_cards_in_column",
        ))
    }

    fn count_cards_in_column_excluding(
        &self,
        _column_id: Uuid,
        _exclude: &[Uuid],
    ) -> KanbanResult<usize> {
        Err(kanban_domain::KanbanError::unsupported(
            "count_cards_in_column_excluding",
        ))
    }

    fn upsert_card(&self, _card: Card) -> KanbanResult<()> {
        Err(kanban_domain::KanbanError::unsupported("upsert_card"))
    }

    fn delete_card(&self, _id: Uuid) -> KanbanResult<()> {
        Err(kanban_domain::KanbanError::unsupported("delete_card"))
    }

    fn delete_cards_by_columns(&self, _column_ids: &[Uuid]) -> KanbanResult<()> {
        Err(kanban_domain::KanbanError::unsupported(
            "delete_cards_by_columns",
        ))
    }

    fn clear_sprint_from_cards(
        &self,
        _sprint_id: Uuid,
        _timestamp: chrono::DateTime<chrono::Utc>,
    ) -> KanbanResult<()> {
        Err(kanban_domain::KanbanError::unsupported(
            "clear_sprint_from_cards",
        ))
    }

    fn get_archived_card(&self, _card_id: Uuid) -> KanbanResult<Option<ArchivedCard>> {
        Err(kanban_domain::KanbanError::unsupported("get_archived_card"))
    }

    fn list_archived_cards(&self) -> KanbanResult<Vec<ArchivedCard>> {
        Err(kanban_domain::KanbanError::unsupported(
            "list_archived_cards",
        ))
    }

    fn insert_archived_card(&self, _ac: ArchivedCard) -> KanbanResult<()> {
        Err(kanban_domain::KanbanError::unsupported(
            "insert_archived_card",
        ))
    }

    fn delete_archived_card(&self, _card_id: Uuid) -> KanbanResult<()> {
        Err(kanban_domain::KanbanError::unsupported(
            "delete_archived_card",
        ))
    }

    fn get_archived_board(&self, _board_id: Uuid) -> KanbanResult<Option<ArchivedBoard>> {
        Err(kanban_domain::KanbanError::unsupported(
            "get_archived_board",
        ))
    }

    fn list_archived_boards(&self) -> KanbanResult<Vec<ArchivedBoard>> {
        Err(kanban_domain::KanbanError::unsupported(
            "list_archived_boards",
        ))
    }

    fn insert_archived_board(&self, _ab: ArchivedBoard) -> KanbanResult<()> {
        Err(kanban_domain::KanbanError::unsupported(
            "insert_archived_board",
        ))
    }

    fn delete_archived_board(&self, _board_id: Uuid) -> KanbanResult<()> {
        Err(kanban_domain::KanbanError::unsupported(
            "delete_archived_board",
        ))
    }

    fn unarchive_board(&self, _board_id: Uuid) -> KanbanResult<()> {
        Err(kanban_domain::KanbanError::unsupported("unarchive_board"))
    }

    fn get_sprint(&self, _id: Uuid) -> KanbanResult<Option<Sprint>> {
        Err(kanban_domain::KanbanError::unsupported("get_sprint"))
    }

    fn list_sprints_by_board(&self, _board_id: Uuid) -> KanbanResult<Vec<Sprint>> {
        Err(kanban_domain::KanbanError::unsupported(
            "list_sprints_by_board",
        ))
    }

    fn list_all_sprints(&self) -> KanbanResult<Vec<Sprint>> {
        Err(kanban_domain::KanbanError::unsupported("list_all_sprints"))
    }

    fn upsert_sprint(&self, _sprint: Sprint) -> KanbanResult<()> {
        Err(kanban_domain::KanbanError::unsupported("upsert_sprint"))
    }

    fn delete_sprint(&self, _id: Uuid) -> KanbanResult<()> {
        Err(kanban_domain::KanbanError::unsupported("delete_sprint"))
    }

    fn delete_sprints_by_board(&self, _board_id: Uuid) -> KanbanResult<()> {
        Err(kanban_domain::KanbanError::unsupported(
            "delete_sprints_by_board",
        ))
    }

    fn get_graph(&self) -> KanbanResult<DependencyGraph> {
        Err(kanban_domain::KanbanError::unsupported("get_graph"))
    }

    fn set_graph(&self, _graph: DependencyGraph) -> KanbanResult<()> {
        Err(kanban_domain::KanbanError::unsupported("set_graph"))
    }

    fn snapshot(&self) -> KanbanResult<Snapshot> {
        Err(kanban_domain::KanbanError::unsupported("snapshot"))
    }

    fn apply_snapshot(&self, _snapshot: Snapshot) -> KanbanResult<()> {
        Err(kanban_domain::KanbanError::unsupported("apply_snapshot"))
    }
}
