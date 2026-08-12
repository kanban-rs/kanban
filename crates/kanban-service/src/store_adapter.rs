use kanban_domain::data_store::DataStore;
use kanban_domain::{KanbanResult, Snapshot};
use std::collections::HashSet;
use uuid::Uuid;

/// Reads a whole workspace through per-entity `DataStore` calls rather than
/// `DataStore::snapshot`.
///
/// Archived boards are absent from `list_boards`, so their heads are recovered
/// individually through the unfiltered `get_board`. Archived cards are likewise
/// absent from `list_all_cards` and are fetched by id.
pub(crate) fn read_full_snapshot(store: &dyn DataStore) -> KanbanResult<Snapshot> {
    let archived_boards = store.list_archived_boards()?;

    let mut boards = store.list_boards()?;
    let live_board_ids: HashSet<Uuid> = boards.iter().map(|b| b.id).collect();
    for ab in &archived_boards {
        if !live_board_ids.contains(&ab.entity_id) {
            if let Some(board) = store.get_board(ab.entity_id)? {
                boards.push(board);
            }
        }
    }

    let mut columns = Vec::new();
    let mut cards = Vec::new();
    let mut archived_cards = Vec::new();
    let mut sprints = Vec::new();

    for board in &boards {
        let board_columns = store.list_columns_by_board(board.id)?;
        let board_archived = store.list_archived_cards_by_board(board.id)?;

        for column in &board_columns {
            cards.extend(store.list_cards_by_column(column.id)?);
        }
        columns.extend(board_columns);

        sprints.extend(store.list_sprints_by_board(board.id)?);
        archived_cards.extend(board_archived);
    }

    // `list_cards_by_column` is live-only under the marker model, so an archived
    // card's row is missing above. Fetch it unfiltered, or the marker imports
    // orphaned.
    let live_card_ids: HashSet<Uuid> = cards.iter().map(|c| c.id).collect();
    for ac in &archived_cards {
        if !live_card_ids.contains(&ac.entity_id) {
            if let Some(card) = store.get_card(ac.entity_id)? {
                cards.push(card);
            }
        }
    }

    Ok(Snapshot {
        boards,
        columns,
        cards,
        archived_cards,
        archived_boards,
        sprints,
        graph: store.get_graph()?,
    })
}

/// Writes a whole workspace through per-entity `DataStore` calls rather than
/// `DataStore::apply_snapshot`. The caller supplies the transaction.
///
/// Order is load-bearing on a relational backend, which checks foreign keys as
/// each row lands: sprints precede cards because `cards.sprint_id` references
/// them, and both follow boards. Archival markers reference the rows they mark,
/// so they come last.
pub(crate) fn write_full_snapshot(store: &dyn DataStore, snapshot: Snapshot) -> KanbanResult<()> {
    for board in snapshot.boards {
        store.upsert_board(board)?;
    }
    for column in snapshot.columns {
        store.upsert_column(column)?;
    }
    for sprint in snapshot.sprints {
        store.upsert_sprint(sprint)?;
    }
    for card in snapshot.cards {
        store.upsert_card(card)?;
    }
    for ac in snapshot.archived_cards {
        store.insert_archived_card(ac)?;
    }
    for ab in snapshot.archived_boards {
        store.insert_archived_board(ab)?;
    }
    store.set_graph(snapshot.graph)
}

#[cfg(test)]
mod tests {
    use super::*;
    use kanban_backend_memory::InMemoryStore;
    use kanban_domain::{Archived, ArchivedCard, Board, Card, Column, DependencyGraph, Sprint};
    use uuid::Uuid;

    /// Delegates everything to an inner store but refuses the two whole-store
    /// trait methods this card exists to stop using. Any accidental fallback to
    /// them fails the test loudly instead of silently producing a correct-looking
    /// result.
    struct NoWholeStoreReads(InMemoryStore);

    impl DataStore for NoWholeStoreReads {
        fn snapshot(&self) -> KanbanResult<Snapshot> {
            panic!("read_full_snapshot must compose per-entity reads, not call snapshot()")
        }
        fn apply_snapshot(&self, _snapshot: Snapshot) -> KanbanResult<()> {
            panic!("write_full_snapshot must compose per-entity writes, not call apply_snapshot()")
        }

        fn get_board(&self, id: Uuid) -> KanbanResult<Option<Board>> {
            self.0.get_board(id)
        }
        fn list_boards(&self) -> KanbanResult<Vec<Board>> {
            self.0.list_boards()
        }
        fn upsert_board(&self, board: Board) -> KanbanResult<()> {
            self.0.upsert_board(board)
        }
        fn delete_board(&self, id: Uuid) -> KanbanResult<()> {
            self.0.delete_board(id)
        }
        fn get_column(&self, id: Uuid) -> KanbanResult<Option<Column>> {
            self.0.get_column(id)
        }
        fn list_columns_by_board(&self, board_id: Uuid) -> KanbanResult<Vec<Column>> {
            self.0.list_columns_by_board(board_id)
        }
        fn list_all_columns(&self) -> KanbanResult<Vec<Column>> {
            self.0.list_all_columns()
        }
        fn upsert_column(&self, column: Column) -> KanbanResult<()> {
            self.0.upsert_column(column)
        }
        fn delete_column(&self, id: Uuid) -> KanbanResult<()> {
            self.0.delete_column(id)
        }
        fn delete_columns_by_board(&self, board_id: Uuid) -> KanbanResult<()> {
            self.0.delete_columns_by_board(board_id)
        }
        fn get_card(&self, id: Uuid) -> KanbanResult<Option<Card>> {
            self.0.get_card(id)
        }
        fn list_all_cards(&self) -> KanbanResult<Vec<Card>> {
            self.0.list_all_cards()
        }
        fn list_cards_by_column(&self, column_id: Uuid) -> KanbanResult<Vec<Card>> {
            self.0.list_cards_by_column(column_id)
        }
        fn list_cards_by_sprint(&self, sprint_id: Uuid) -> KanbanResult<Vec<Card>> {
            self.0.list_cards_by_sprint(sprint_id)
        }
        fn count_cards_in_column(&self, column_id: Uuid) -> KanbanResult<usize> {
            self.0.count_cards_in_column(column_id)
        }
        fn count_cards_in_column_excluding(
            &self,
            column_id: Uuid,
            exclude: &[Uuid],
        ) -> KanbanResult<usize> {
            self.0.count_cards_in_column_excluding(column_id, exclude)
        }
        fn upsert_card(&self, card: Card) -> KanbanResult<()> {
            self.0.upsert_card(card)
        }
        fn delete_card(&self, id: Uuid) -> KanbanResult<()> {
            self.0.delete_card(id)
        }
        fn delete_cards_by_columns(&self, column_ids: &[Uuid]) -> KanbanResult<()> {
            self.0.delete_cards_by_columns(column_ids)
        }
        fn list_archived_cards(&self) -> KanbanResult<Vec<ArchivedCard>> {
            self.0.list_archived_cards()
        }
        fn list_archived_cards_by_board(&self, board_id: Uuid) -> KanbanResult<Vec<ArchivedCard>> {
            self.0.list_archived_cards_by_board(board_id)
        }
        fn insert_archived_card(&self, ac: ArchivedCard) -> KanbanResult<()> {
            self.0.insert_archived_card(ac)
        }
        fn get_archived_card(&self, card_id: Uuid) -> KanbanResult<Option<ArchivedCard>> {
            self.0.get_archived_card(card_id)
        }
        fn delete_archived_card(&self, card_id: Uuid) -> KanbanResult<()> {
            self.0.delete_archived_card(card_id)
        }
        fn get_archived_board(
            &self,
            board_id: Uuid,
        ) -> KanbanResult<Option<kanban_domain::ArchivedBoard>> {
            self.0.get_archived_board(board_id)
        }
        fn list_archived_boards(&self) -> KanbanResult<Vec<kanban_domain::ArchivedBoard>> {
            self.0.list_archived_boards()
        }
        fn insert_archived_board(&self, ab: kanban_domain::ArchivedBoard) -> KanbanResult<()> {
            self.0.insert_archived_board(ab)
        }
        fn delete_archived_board(&self, board_id: Uuid) -> KanbanResult<()> {
            self.0.delete_archived_board(board_id)
        }
        fn get_sprint(&self, id: Uuid) -> KanbanResult<Option<Sprint>> {
            self.0.get_sprint(id)
        }
        fn list_sprints_by_board(&self, board_id: Uuid) -> KanbanResult<Vec<Sprint>> {
            self.0.list_sprints_by_board(board_id)
        }
        fn list_all_sprints(&self) -> KanbanResult<Vec<Sprint>> {
            self.0.list_all_sprints()
        }
        fn upsert_sprint(&self, sprint: Sprint) -> KanbanResult<()> {
            self.0.upsert_sprint(sprint)
        }
        fn delete_sprint(&self, id: Uuid) -> KanbanResult<()> {
            self.0.delete_sprint(id)
        }
        fn delete_sprints_by_board(&self, board_id: Uuid) -> KanbanResult<()> {
            self.0.delete_sprints_by_board(board_id)
        }
        fn clear_sprint_from_cards(
            &self,
            sprint_id: Uuid,
            timestamp: chrono::DateTime<chrono::Utc>,
        ) -> KanbanResult<()> {
            self.0.clear_sprint_from_cards(sprint_id, timestamp)
        }
        fn get_graph(&self) -> KanbanResult<DependencyGraph> {
            self.0.get_graph()
        }
        fn set_graph(&self, graph: DependencyGraph) -> KanbanResult<()> {
            self.0.set_graph(graph)
        }
    }

    struct Seeded {
        live_board: Uuid,
        live_column: Uuid,
        blocker: Uuid,
        blocked: Uuid,
        archived_card: Uuid,
        live_sprint: Uuid,
        archived_board: Uuid,
        archived_board_column: Uuid,
        archived_board_card: Uuid,
        archived_board_sprint: Uuid,
    }

    /// A live board with a column, two linked cards, a sprint and an archived
    /// card, PLUS a separately archived board carrying its own whole subtree.
    fn seed(store: &InMemoryStore) -> KanbanResult<Seeded> {
        let mut live = Board::new("Live", None::<String>);
        let live_column = Column::new(live.id, "Todo", 0);
        let blocker = Card::new(&mut live, live_column.id, "Blocker", 0);
        let blocked = Card::new(&mut live, live_column.id, "Blocked", 1);
        let archived_card = Card::new(&mut live, live_column.id, "Archived card", 2);
        let live_sprint = Sprint::new(live.id, 1, None, None::<String>);

        let mut arch = Board::new("Archived board", None::<String>);
        let arch_column = Column::new(arch.id, "Done", 0);
        let arch_card = Card::new(&mut arch, arch_column.id, "Card on archived board", 0);
        let arch_sprint = Sprint::new(arch.id, 1, None, None::<String>);

        let ids = Seeded {
            live_board: live.id,
            live_column: live_column.id,
            blocker: blocker.id,
            blocked: blocked.id,
            archived_card: archived_card.id,
            live_sprint: live_sprint.id,
            archived_board: arch.id,
            archived_board_column: arch_column.id,
            archived_board_card: arch_card.id,
            archived_board_sprint: arch_sprint.id,
        };

        store.upsert_board(live)?;
        store.upsert_column(live_column)?;
        store.upsert_card(blocker)?;
        store.upsert_card(blocked)?;
        store.upsert_card(archived_card)?;
        store.upsert_sprint(live_sprint)?;
        store.insert_archived_card(ArchivedCard::new(ids.archived_card, ids.live_board))?;

        store.upsert_board(arch)?;
        store.upsert_column(arch_column)?;
        store.upsert_card(arch_card)?;
        store.upsert_sprint(arch_sprint)?;
        store.insert_archived_board(Archived::now(ids.archived_board))?;

        store.modify_graph(Box::new({
            let (a, b) = (ids.blocker, ids.blocked);
            move |g| g.set_block(a, b)
        }))?;

        Ok(ids)
    }

    #[test]
    fn test_read_full_snapshot_includes_archived_board_subtree() -> KanbanResult<()> {
        let inner = InMemoryStore::new();
        let ids = seed(&inner)?;
        let store = NoWholeStoreReads(inner);

        let snap = read_full_snapshot(&store)?;

        assert!(
            snap.boards.iter().any(|b| b.id == ids.archived_board),
            "an archived board is absent from list_boards, so its head must be \
             recovered through the unfiltered get_board; missing this silently \
             drops the whole subtree"
        );
        assert!(
            snap.columns
                .iter()
                .any(|c| c.id == ids.archived_board_column),
            "the archived board's column must survive"
        );
        assert!(
            snap.cards.iter().any(|c| c.id == ids.archived_board_card),
            "the archived board's card must survive"
        );
        assert!(
            snap.sprints
                .iter()
                .any(|s| s.id == ids.archived_board_sprint),
            "the archived board's sprint must survive"
        );
        assert!(
            snap.archived_boards
                .iter()
                .any(|ab| ab.entity_id == ids.archived_board),
            "the archival marker itself must survive"
        );
        Ok(())
    }

    #[test]
    fn test_read_full_snapshot_carries_archived_cards_and_their_live_rows() -> KanbanResult<()> {
        let inner = InMemoryStore::new();
        let ids = seed(&inner)?;
        let store = NoWholeStoreReads(inner);

        let snap = read_full_snapshot(&store)?;

        assert!(
            snap.archived_cards
                .iter()
                .any(|ac| ac.entity_id == ids.archived_card),
            "the archived-card marker must survive"
        );
        assert!(
            snap.cards.iter().any(|c| c.id == ids.archived_card),
            "list_all_cards is live-only under the marker model, so the archived \
             card's row must be fetched by id or the marker lands orphaned"
        );
        Ok(())
    }

    #[test]
    fn test_read_full_snapshot_carries_the_live_graph() -> KanbanResult<()> {
        let inner = InMemoryStore::new();
        let ids = seed(&inner)?;
        let store = NoWholeStoreReads(inner);

        let snap = read_full_snapshot(&store)?;

        assert_eq!(
            snap.graph.blockers(ids.blocked),
            vec![ids.blocker],
            "the dependency edge lives in the workspace-global graph and must be \
             carried across"
        );
        Ok(())
    }

    #[test]
    fn test_write_full_snapshot_restores_the_whole_graph() -> KanbanResult<()> {
        let source = InMemoryStore::new();
        let ids = seed(&source)?;
        let snap = read_full_snapshot(&NoWholeStoreReads(source))?;

        let dest_inner = InMemoryStore::new();
        let dest = NoWholeStoreReads(dest_inner);
        write_full_snapshot(&dest, snap)?;

        assert!(dest.get_board(ids.live_board)?.is_some(), "live board");
        assert!(
            dest.get_board(ids.archived_board)?.is_some(),
            "archived board head"
        );
        assert!(dest.get_column(ids.live_column)?.is_some(), "live column");
        assert!(
            dest.get_column(ids.archived_board_column)?.is_some(),
            "archived board's column"
        );
        assert!(dest.get_card(ids.blocker)?.is_some(), "blocker card");
        assert!(dest.get_card(ids.blocked)?.is_some(), "blocked card");
        assert!(
            dest.get_card(ids.archived_card)?.is_some(),
            "archived card's live row"
        );
        assert!(
            dest.get_card(ids.archived_board_card)?.is_some(),
            "archived board's card"
        );
        assert!(dest.get_sprint(ids.live_sprint)?.is_some(), "live sprint");
        assert!(
            dest.get_sprint(ids.archived_board_sprint)?.is_some(),
            "archived board's sprint"
        );
        assert!(
            dest.get_archived_card(ids.archived_card)?.is_some(),
            "archived-card marker"
        );
        assert!(
            dest.get_archived_board(ids.archived_board)?.is_some(),
            "archived-board marker"
        );
        assert_eq!(
            dest.get_graph()?.blockers(ids.blocked),
            vec![ids.blocker],
            "dependency edge"
        );
        Ok(())
    }

    #[test]
    fn test_read_then_write_then_read_is_the_identity() -> KanbanResult<()> {
        let source = InMemoryStore::new();
        seed(&source)?;
        let first = read_full_snapshot(&NoWholeStoreReads(source))?;

        let dest = NoWholeStoreReads(InMemoryStore::new());
        write_full_snapshot(&dest, first.clone())?;
        let second = read_full_snapshot(&dest)?;

        // Compared as sorted id sets: neither read nor write promises an ordering,
        // so a Vec comparison would fail on a permutation that loses nothing.
        fn ids<T, F: Fn(&T) -> Uuid>(v: &[T], f: F) -> Vec<Uuid> {
            let mut out: Vec<Uuid> = v.iter().map(f).collect();
            out.sort();
            out
        }

        assert_eq!(ids(&first.boards, |b| b.id), ids(&second.boards, |b| b.id));
        assert_eq!(
            ids(&first.columns, |c| c.id),
            ids(&second.columns, |c| c.id)
        );
        assert_eq!(ids(&first.cards, |c| c.id), ids(&second.cards, |c| c.id));
        assert_eq!(
            ids(&first.sprints, |s| s.id),
            ids(&second.sprints, |s| s.id)
        );
        assert_eq!(
            ids(&first.archived_cards, |a| a.entity_id),
            ids(&second.archived_cards, |a| a.entity_id)
        );
        assert_eq!(
            ids(&first.archived_boards, |a| a.entity_id),
            ids(&second.archived_boards, |a| a.entity_id)
        );
        assert_eq!(
            first.graph, second.graph,
            "the dependency graph must survive"
        );
        Ok(())
    }
}
