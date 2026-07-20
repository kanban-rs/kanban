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

    /// Board-scoped archived cards. Default filters the full list by the
    /// `board_id` field now carried on each `ArchivedCard` (B1). SQL backends
    /// override with a single `WHERE board_id = ?` query.
    fn list_archived_cards_by_board(&self, board_id: Uuid) -> KanbanResult<Vec<ArchivedCard>> {
        let all = self.list_archived_cards()?;
        Ok(all
            .into_iter()
            .filter(|ac| ac.context.board_id == board_id)
            .collect())
    }

    fn clear_sprint_from_archived_cards(
        &self,
        sprint_id: Uuid,
        timestamp: chrono::DateTime<chrono::Utc>,
    ) -> KanbanResult<()> {
        // Reference-marker model: an archived card is an ordinary LIVE card plus a
        // marker. Clear the sprint on the live card fetched by the marker's
        // `entity_id` (the card is the single source of truth).
        let markers = self.list_archived_cards()?;
        for marker in markers {
            if let Some(mut card) = self.get_card(marker.entity_id)? {
                if card.sprint_id == Some(sprint_id) {
                    card.sprint_id = None;
                    card.updated_at = timestamp;
                    self.upsert_card(card)?;
                }
            }
        }
        Ok(())
    }

    // Archived board (C2/C3a). A board is a scoping root: archive moves its head
    // out of the live `boards` set into a discrete archived collection as
    // `Archived<Board>`; the subtree stays in the flat collections. These four
    // ship FUNCTIONAL DEFAULTS so every backend stays green between C2 and the
    // persistence overrides (C4/C5). `InMemoryStore` overrides all four; the
    // JSON backend inherits them via its inner `InMemoryStore`; SQLite overrides
    // in C5. The defaults are chosen so no core path bricks on a not-yet-migrated
    // backend:
    //   - reads default empty (a backend with no archived collection has none);
    //   - `delete` defaults to a no-op — deleting from an absent collection is
    //     vacuously successful, and the collection-agnostic `DeleteBoard` calls
    //     it on EVERY board delete (incl. live boards), so it must not error;
    //   - `insert` defaults to `unsupported` and must stay loud: a silent drop
    //     would lose the board on archive (archive = insert-archived + delete-live).
    //     Archiving on a not-yet-migrated backend therefore fails loud until C5,
    //     which is correct — the feature genuinely isn't stored there yet.
    fn get_archived_board(&self, _board_id: Uuid) -> KanbanResult<Option<crate::ArchivedBoard>> {
        Ok(None)
    }
    fn list_archived_boards(&self) -> KanbanResult<Vec<crate::ArchivedBoard>> {
        Ok(Vec::new())
    }
    fn insert_archived_board(&self, _ab: crate::ArchivedBoard) -> KanbanResult<()> {
        Err(crate::KanbanError::unsupported("insert_archived_board"))
    }
    /// Remove a board from the archived collection **only**. On a marker-style
    /// backend (SQLite) this also removes the shared entity row, so it MUST be a
    /// no-op on a *live* (non-archived) board — matching the in-memory store,
    /// which only touches its archived map. `RestoreBoard` relies on this: it
    /// calls `delete_archived_board` BEFORE re-inserting the board as live
    /// (delete-then-upsert), so re-adding it doesn't collide with the archived
    /// row on a shared-row backend.
    fn delete_archived_board(&self, _board_id: Uuid) -> KanbanResult<()> {
        Ok(())
    }

    /// Remove a board from the archived collection while KEEPING its shared
    /// entity row and subtree (the RESTORE path). The default delegates to
    /// [`delete_archived_board`](Self::delete_archived_board), which is correct
    /// for map-style backends (in-memory/JSON) whose `delete_archived_board`
    /// only drops the head from the archived map and never touches the subtree.
    /// A shared-row backend (SQLite), whose `delete_archived_board` deletes the
    /// entity row (needed for permanent delete), MUST override this to drop only
    /// the archived marker — otherwise `RestoreBoard` (delete-archived then
    /// upsert-head) would CASCADE the still-present subtree away (KAN-863).
    fn unarchive_board(&self, board_id: Uuid) -> KanbanResult<()> {
        self.delete_archived_board(board_id)
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
    use crate::ArchivedFilter;
    use std::sync::Mutex;

    #[test]
    fn test_data_store_is_object_safe() {
        fn _assert_object_safe(_: &dyn DataStore) {}
    }

    /// Minimal stub implementing only the trait methods the filter-aware
    /// defaults delegate to, so it inherits the new defaults verbatim. Kept
    /// dedicated (not `InMemoryStore`) on purpose: once real backends override
    /// the filter-aware methods (F1b/F1c), an `InMemoryStore`-based floor
    /// assertion would evaporate and stop proving the loud floor. `FloorStore`
    /// never overrides them, so it pins the default behaviour forever.
    #[derive(Default)]
    struct FloorStore {
        cards: Mutex<Vec<Card>>,
    }

    impl FloorStore {
        fn with_card(card: Card) -> Self {
            Self {
                cards: Mutex::new(vec![card]),
            }
        }
    }

    impl DataStore for FloorStore {
        fn get_board(&self, _id: Uuid) -> KanbanResult<Option<Board>> {
            unimplemented!()
        }
        fn list_boards(&self) -> KanbanResult<Vec<Board>> {
            unimplemented!()
        }
        fn upsert_board(&self, _board: Board) -> KanbanResult<()> {
            unimplemented!()
        }
        fn delete_board(&self, _id: Uuid) -> KanbanResult<()> {
            unimplemented!()
        }
        fn get_column(&self, _id: Uuid) -> KanbanResult<Option<Column>> {
            unimplemented!()
        }
        fn list_columns_by_board(&self, _board_id: Uuid) -> KanbanResult<Vec<Column>> {
            unimplemented!()
        }
        fn list_all_columns(&self) -> KanbanResult<Vec<Column>> {
            unimplemented!()
        }
        fn upsert_column(&self, _column: Column) -> KanbanResult<()> {
            unimplemented!()
        }
        fn delete_column(&self, _id: Uuid) -> KanbanResult<()> {
            unimplemented!()
        }
        fn delete_columns_by_board(&self, _board_id: Uuid) -> KanbanResult<()> {
            unimplemented!()
        }
        fn get_card(&self, _id: Uuid) -> KanbanResult<Option<Card>> {
            unimplemented!()
        }
        fn list_all_cards(&self) -> KanbanResult<Vec<Card>> {
            unimplemented!()
        }
        fn list_cards_by_column(&self, column_id: Uuid) -> KanbanResult<Vec<Card>> {
            Ok(self
                .cards
                .lock()
                .unwrap()
                .iter()
                .filter(|c| c.column_id == column_id)
                .cloned()
                .collect())
        }
        fn list_cards_by_sprint(&self, _sprint_id: Uuid) -> KanbanResult<Vec<Card>> {
            unimplemented!()
        }
        fn count_cards_in_column(&self, column_id: Uuid) -> KanbanResult<usize> {
            Ok(self.list_cards_by_column(column_id)?.len())
        }
        fn count_cards_in_column_excluding(
            &self,
            _column_id: Uuid,
            _exclude: &[Uuid],
        ) -> KanbanResult<usize> {
            unimplemented!()
        }
        fn upsert_card(&self, _card: Card) -> KanbanResult<()> {
            unimplemented!()
        }
        fn delete_card(&self, _id: Uuid) -> KanbanResult<()> {
            unimplemented!()
        }
        fn delete_cards_by_columns(&self, _column_ids: &[Uuid]) -> KanbanResult<()> {
            unimplemented!()
        }
        fn clear_sprint_from_cards(
            &self,
            _sprint_id: Uuid,
            _timestamp: chrono::DateTime<chrono::Utc>,
        ) -> KanbanResult<()> {
            unimplemented!()
        }
        fn get_archived_card(&self, _card_id: Uuid) -> KanbanResult<Option<ArchivedCard>> {
            unimplemented!()
        }
        fn list_archived_cards(&self) -> KanbanResult<Vec<ArchivedCard>> {
            unimplemented!()
        }
        fn insert_archived_card(&self, _ac: ArchivedCard) -> KanbanResult<()> {
            unimplemented!()
        }
        fn delete_archived_card(&self, _card_id: Uuid) -> KanbanResult<()> {
            unimplemented!()
        }
        fn get_sprint(&self, _id: Uuid) -> KanbanResult<Option<Sprint>> {
            unimplemented!()
        }
        fn list_sprints_by_board(&self, _board_id: Uuid) -> KanbanResult<Vec<Sprint>> {
            unimplemented!()
        }
        fn list_all_sprints(&self) -> KanbanResult<Vec<Sprint>> {
            unimplemented!()
        }
        fn upsert_sprint(&self, _sprint: Sprint) -> KanbanResult<()> {
            unimplemented!()
        }
        fn delete_sprint(&self, _id: Uuid) -> KanbanResult<()> {
            unimplemented!()
        }
        fn delete_sprints_by_board(&self, _board_id: Uuid) -> KanbanResult<()> {
            unimplemented!()
        }
        fn get_graph(&self) -> KanbanResult<DependencyGraph> {
            unimplemented!()
        }
        fn set_graph(&self, _graph: DependencyGraph) -> KanbanResult<()> {
            unimplemented!()
        }
        fn snapshot(&self) -> KanbanResult<Snapshot> {
            unimplemented!()
        }
        fn apply_snapshot(&self, _snapshot: Snapshot) -> KanbanResult<()> {
            unimplemented!()
        }
    }

    fn seed_card(column_id: Uuid) -> Card {
        let mut board = Board::new("floor", None::<String>);
        Card::new(&mut board, column_id, "seed", 0)
    }

    #[test]
    fn test_filtered_read_liveonly_delegates_to_existing() -> KanbanResult<()> {
        let column_id = Uuid::new_v4();
        let store = FloorStore::with_card(seed_card(column_id));

        let via_filter = store.list_cards_by_column_filtered(column_id, ArchivedFilter::LiveOnly)?;
        let direct = store.list_cards_by_column(column_id)?;
        assert_eq!(via_filter, direct);

        let count_via_filter =
            store.count_cards_in_column_filtered(column_id, ArchivedFilter::LiveOnly)?;
        let count_direct = store.count_cards_in_column(column_id)?;
        assert_eq!(count_via_filter, count_direct);
        assert_eq!(count_via_filter, 1);

        Ok(())
    }

    #[test]
    fn test_filtered_read_non_live_is_unsupported_until_overridden() {
        let column_id = Uuid::new_v4();
        let store = FloorStore::default();

        for archived in [ArchivedFilter::ArchivedOnly, ArchivedFilter::Include] {
            let list_err = store
                .list_cards_by_column_filtered(column_id, archived)
                .expect_err("non-live list must fail loud, not fall back to live-only");
            assert!(
                list_err.is_unsupported(),
                "expected unsupported for list {archived:?}, got {list_err:?}"
            );

            let count_err = store
                .count_cards_in_column_filtered(column_id, archived)
                .expect_err("non-live count must fail loud, not fall back to live-only");
            assert!(
                count_err.is_unsupported(),
                "expected unsupported for count {archived:?}, got {count_err:?}"
            );
        }
    }
}
