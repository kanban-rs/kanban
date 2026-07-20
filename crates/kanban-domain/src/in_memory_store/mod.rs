mod archived_boards;
mod archived_cards;
mod boards;
mod cards;
mod columns;
mod command_log;
mod graph;
mod ordering;
mod snapshot;
mod sprints;
mod state;

#[cfg(test)]
mod test_support {
    use crate::{Board, Card, Column};
    use uuid::Uuid;

    pub(super) fn make_board(name: &str) -> Board {
        Board::new(name.to_string(), None::<String>)
    }

    pub(super) fn make_column(board_id: Uuid, name: &str, pos: i32) -> Column {
        Column::new(board_id, name.to_string(), pos)
    }

    pub(super) fn make_card(board: &mut Board, column_id: Uuid, title: &str, pos: i32) -> Card {
        Card::new(board, column_id, title.to_string(), pos)
    }
}

#[cfg(test)]
mod tests;

use std::sync::RwLock;

use uuid::Uuid;

use crate::command_batch::CommandBatch;
use crate::data_store::DataStore;
use crate::{
    ArchivedCard, Board, Card, Column, DependencyGraph, KanbanError, KanbanResult, Snapshot, Sprint,
};

use state::StoreState;

pub struct InMemoryStore {
    state: RwLock<StoreState>,
    command_log: RwLock<Vec<CommandBatch>>,
}

impl InMemoryStore {
    pub fn new() -> Self {
        Self {
            state: RwLock::new(StoreState::new()),
            command_log: RwLock::new(Vec::new()),
        }
    }

    fn read_state(&self) -> KanbanResult<std::sync::RwLockReadGuard<'_, StoreState>> {
        self.state
            .read()
            .map_err(|e| KanbanError::Internal(format!("State RwLock poisoned (read): {e}")))
    }

    fn write_state(&self) -> KanbanResult<std::sync::RwLockWriteGuard<'_, StoreState>> {
        self.state
            .write()
            .map_err(|e| KanbanError::Internal(format!("State RwLock poisoned (write): {e}")))
    }

    fn read_log(&self) -> KanbanResult<std::sync::RwLockReadGuard<'_, Vec<CommandBatch>>> {
        self.command_log
            .read()
            .map_err(|e| KanbanError::Internal(format!("Command log RwLock poisoned (read): {e}")))
    }

    fn write_log(&self) -> KanbanResult<std::sync::RwLockWriteGuard<'_, Vec<CommandBatch>>> {
        self.command_log
            .write()
            .map_err(|e| KanbanError::Internal(format!("Command log RwLock poisoned (write): {e}")))
    }
}

impl Default for InMemoryStore {
    fn default() -> Self {
        Self::new()
    }
}

impl DataStore for InMemoryStore {
    // Board

    fn get_board(&self, id: Uuid) -> KanbanResult<Option<Board>> {
        self.get_board_impl(id)
    }

    fn list_boards(&self) -> KanbanResult<Vec<Board>> {
        self.list_boards_impl()
    }

    fn upsert_board(&self, board: Board) -> KanbanResult<()> {
        self.upsert_board_impl(board)
    }

    fn delete_board(&self, id: Uuid) -> KanbanResult<()> {
        self.delete_board_impl(id)
    }

    // Column

    fn get_column(&self, id: Uuid) -> KanbanResult<Option<Column>> {
        self.get_column_impl(id)
    }

    fn list_columns_by_board(&self, board_id: Uuid) -> KanbanResult<Vec<Column>> {
        self.list_columns_by_board_impl(board_id)
    }

    fn list_all_columns(&self) -> KanbanResult<Vec<Column>> {
        self.list_all_columns_impl()
    }

    fn upsert_column(&self, column: Column) -> KanbanResult<()> {
        self.upsert_column_impl(column)
    }

    fn delete_column(&self, id: Uuid) -> KanbanResult<()> {
        self.delete_column_impl(id)
    }

    fn delete_columns_by_board(&self, board_id: Uuid) -> KanbanResult<()> {
        self.delete_columns_by_board_impl(board_id)
    }

    // Card

    fn get_card(&self, id: Uuid) -> KanbanResult<Option<Card>> {
        self.get_card_impl(id)
    }

    fn list_all_cards(&self) -> KanbanResult<Vec<Card>> {
        self.list_all_cards_impl()
    }

    fn list_cards_by_column(&self, column_id: Uuid) -> KanbanResult<Vec<Card>> {
        self.list_cards_by_column_impl(column_id)
    }

    fn list_cards_by_sprint(&self, sprint_id: Uuid) -> KanbanResult<Vec<Card>> {
        self.list_cards_by_sprint_impl(sprint_id)
    }

    fn count_cards_in_column(&self, column_id: Uuid) -> KanbanResult<usize> {
        self.count_cards_in_column_impl(column_id)
    }

    fn count_cards_in_column_excluding(
        &self,
        column_id: Uuid,
        exclude: &[Uuid],
    ) -> KanbanResult<usize> {
        self.count_cards_in_column_excluding_impl(column_id, exclude)
    }

    fn upsert_card(&self, card: Card) -> KanbanResult<()> {
        self.upsert_card_impl(card)
    }

    fn delete_card(&self, id: Uuid) -> KanbanResult<()> {
        self.delete_card_impl(id)
    }

    fn delete_cards_by_columns(&self, column_ids: &[Uuid]) -> KanbanResult<()> {
        self.delete_cards_by_columns_impl(column_ids)
    }

    fn clear_sprint_from_cards(
        &self,
        sprint_id: Uuid,
        timestamp: chrono::DateTime<chrono::Utc>,
    ) -> KanbanResult<()> {
        self.clear_sprint_from_cards_impl(sprint_id, timestamp)
    }

    // Archived card

    fn get_archived_card(&self, card_id: Uuid) -> KanbanResult<Option<ArchivedCard>> {
        self.get_archived_card_impl(card_id)
    }

    fn list_archived_cards(&self) -> KanbanResult<Vec<ArchivedCard>> {
        self.list_archived_cards_impl()
    }

    fn insert_archived_card(&self, ac: ArchivedCard) -> KanbanResult<()> {
        self.insert_archived_card_impl(ac)
    }

    fn list_archived_cards_by_board(&self, board_id: Uuid) -> KanbanResult<Vec<ArchivedCard>> {
        self.list_archived_cards_by_board_impl(board_id)
    }

    fn clear_sprint_from_archived_cards(
        &self,
        sprint_id: Uuid,
        timestamp: chrono::DateTime<chrono::Utc>,
    ) -> KanbanResult<()> {
        self.clear_sprint_from_archived_cards_impl(sprint_id, timestamp)
    }

    fn delete_archived_card(&self, card_id: Uuid) -> KanbanResult<()> {
        self.delete_archived_card_impl(card_id)
    }

    // Archived board

    fn get_archived_board(&self, board_id: Uuid) -> KanbanResult<Option<crate::ArchivedBoard>> {
        self.get_archived_board_impl(board_id)
    }

    fn list_archived_boards(&self) -> KanbanResult<Vec<crate::ArchivedBoard>> {
        self.list_archived_boards_impl()
    }

    fn insert_archived_board(&self, ab: crate::ArchivedBoard) -> KanbanResult<()> {
        self.insert_archived_board_impl(ab)
    }

    fn delete_archived_board(&self, board_id: Uuid) -> KanbanResult<()> {
        self.delete_archived_board_impl(board_id)
    }

    fn unarchive_board(&self, board_id: Uuid) -> KanbanResult<()> {
        self.unarchive_board_impl(board_id)
    }

    // Sprint

    fn get_sprint(&self, id: Uuid) -> KanbanResult<Option<Sprint>> {
        self.get_sprint_impl(id)
    }

    fn list_sprints_by_board(&self, board_id: Uuid) -> KanbanResult<Vec<Sprint>> {
        self.list_sprints_by_board_impl(board_id)
    }

    fn list_all_sprints(&self) -> KanbanResult<Vec<Sprint>> {
        self.list_all_sprints_impl()
    }

    fn upsert_sprint(&self, sprint: Sprint) -> KanbanResult<()> {
        self.upsert_sprint_impl(sprint)
    }

    fn delete_sprint(&self, id: Uuid) -> KanbanResult<()> {
        self.delete_sprint_impl(id)
    }

    fn delete_sprints_by_board(&self, board_id: Uuid) -> KanbanResult<()> {
        self.delete_sprints_by_board_impl(board_id)
    }

    // Graph

    fn get_graph(&self) -> KanbanResult<DependencyGraph> {
        self.get_graph_impl()
    }

    fn set_graph(&self, graph: DependencyGraph) -> KanbanResult<()> {
        self.set_graph_impl(graph)
    }

    fn modify_graph(&self, f: crate::data_store::GraphMutFn) -> KanbanResult<()> {
        self.modify_graph_impl(f)
    }

    // Snapshot

    fn snapshot(&self) -> KanbanResult<Snapshot> {
        self.snapshot_impl()
    }

    fn apply_snapshot(&self, snapshot: Snapshot) -> KanbanResult<()> {
        self.apply_snapshot_impl(snapshot)
    }
}
