use uuid::Uuid;

use crate::{ArchivedCard, Board, Card, Column, DependencyGraph, KanbanResult, Snapshot, Sprint};

pub type GraphMutFn = Box<dyn FnOnce(&mut DependencyGraph) -> KanbanResult<()>>;

pub trait DataStore: Send + Sync {
    // Board
    fn get_board(&self, id: Uuid) -> KanbanResult<Option<Board>>;
    fn list_boards(&self) -> KanbanResult<Vec<Board>>;
    fn upsert_board(&self, board: Board) -> KanbanResult<()>;
    fn delete_board(&self, id: Uuid) -> KanbanResult<()>;

    // Column
    fn get_column(&self, id: Uuid) -> KanbanResult<Option<Column>>;
    fn list_columns_by_board(&self, board_id: Uuid) -> KanbanResult<Vec<Column>>;
    fn list_all_columns(&self) -> KanbanResult<Vec<Column>>;
    fn upsert_column(&self, column: Column) -> KanbanResult<()>;
    fn delete_column(&self, id: Uuid) -> KanbanResult<()>;
    fn delete_columns_by_board(&self, board_id: Uuid) -> KanbanResult<()>;

    // Card
    fn get_card(&self, id: Uuid) -> KanbanResult<Option<Card>>;
    fn list_all_cards(&self) -> KanbanResult<Vec<Card>>;
    fn list_cards_by_column(&self, column_id: Uuid) -> KanbanResult<Vec<Card>>;
    fn list_cards_by_sprint(&self, sprint_id: Uuid) -> KanbanResult<Vec<Card>>;
    fn count_cards_in_column(&self, column_id: Uuid) -> KanbanResult<usize>;
    fn count_cards_in_column_excluding(
        &self,
        column_id: Uuid,
        exclude: &[Uuid],
    ) -> KanbanResult<usize>;
    fn upsert_card(&self, card: Card) -> KanbanResult<()>;
    fn delete_card(&self, id: Uuid) -> KanbanResult<()>;
    fn delete_cards_by_columns(&self, column_ids: &[Uuid]) -> KanbanResult<()>;

    /// Return all cards across the given columns in one call.
    ///
    /// Default impl loops over [`list_cards_by_column`](Self::list_cards_by_column);
    /// SQL-backed implementations should override with a single `IN (?, …)` query.
    fn list_cards_by_columns(&self, column_ids: &[Uuid]) -> KanbanResult<Vec<Card>> {
        let mut out = Vec::new();
        for col_id in column_ids {
            out.extend(self.list_cards_by_column(*col_id)?);
        }
        Ok(out)
    }
    fn clear_sprint_from_cards(
        &self,
        sprint_id: Uuid,
        timestamp: chrono::DateTime<chrono::Utc>,
    ) -> KanbanResult<()>;

    // Archived card
    fn get_archived_card(&self, card_id: Uuid) -> KanbanResult<Option<ArchivedCard>>;
    fn list_archived_cards(&self) -> KanbanResult<Vec<ArchivedCard>>;
    fn insert_archived_card(&self, ac: ArchivedCard) -> KanbanResult<()>;
    fn delete_archived_card(&self, card_id: Uuid) -> KanbanResult<()>;

    fn list_archived_cards_by_columns(
        &self,
        column_ids: &[Uuid],
    ) -> KanbanResult<Vec<ArchivedCard>> {
        let all = self.list_archived_cards()?;
        Ok(all
            .into_iter()
            .filter(|ac| column_ids.contains(&ac.original_column_id))
            .collect())
    }

    /// Board-scoped archived cards. Default filters the full list by the
    /// `board_id` field now carried on each `ArchivedCard` (B1). SQL backends
    /// override with a single `WHERE board_id = ?` query.
    fn list_archived_cards_by_board(&self, board_id: Uuid) -> KanbanResult<Vec<ArchivedCard>> {
        let all = self.list_archived_cards()?;
        Ok(all
            .into_iter()
            .filter(|ac| ac.board_id == board_id)
            .collect())
    }

    fn clear_sprint_from_archived_cards(
        &self,
        sprint_id: Uuid,
        timestamp: chrono::DateTime<chrono::Utc>,
    ) -> KanbanResult<()> {
        let all = self.list_archived_cards()?;
        for mut ac in all {
            if ac.card.sprint_id == Some(sprint_id) {
                ac.card.sprint_id = None;
                ac.card.updated_at = timestamp;
                self.insert_archived_card(ac)?;
            }
        }
        Ok(())
    }

    // Sprint
    fn get_sprint(&self, id: Uuid) -> KanbanResult<Option<Sprint>>;
    fn list_sprints_by_board(&self, board_id: Uuid) -> KanbanResult<Vec<Sprint>>;
    fn list_all_sprints(&self) -> KanbanResult<Vec<Sprint>>;
    fn upsert_sprint(&self, sprint: Sprint) -> KanbanResult<()>;
    fn delete_sprint(&self, id: Uuid) -> KanbanResult<()>;
    fn delete_sprints_by_board(&self, board_id: Uuid) -> KanbanResult<()>;

    // Graph
    fn get_graph(&self) -> KanbanResult<DependencyGraph>;
    fn set_graph(&self, graph: DependencyGraph) -> KanbanResult<()>;

    /// Atomically read-modify-write the dependency graph.
    ///
    /// # TOCTOU warning for implementors
    ///
    /// The default implementation calls `get_graph()` and `set_graph()` as two
    /// separate operations. Any concurrent writer that runs between the two calls
    /// will have its changes silently overwritten. Implementors that wrap interior
    /// locking (e.g. `RwLock`, database transactions) **must** override this method
    /// to perform the read and write within a single lock span, as `InMemoryStore`
    /// already does.
    fn modify_graph(&self, f: GraphMutFn) -> KanbanResult<()> {
        let mut graph = self.get_graph()?;
        f(&mut graph)?;
        self.set_graph(graph)
    }

    // Snapshot (import/export, JSON file I/O, migration)
    fn snapshot(&self) -> KanbanResult<Snapshot>;
    fn apply_snapshot(&self, snapshot: Snapshot) -> KanbanResult<()>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_data_store_is_object_safe() {
        fn _assert_object_safe(_: &dyn DataStore) {}
    }
}
