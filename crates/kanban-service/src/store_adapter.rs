use kanban_domain::data_store::DataStore;
use kanban_domain::{KanbanError, KanbanResult, Snapshot};
use std::collections::HashSet;
use uuid::Uuid;

/// Reads a whole workspace through per-entity `DataStore` calls rather than
/// `DataStore::snapshot`.
///
/// Archived boards are absent from `list_boards`, so their heads are recovered
/// individually through the unfiltered `get_board`. Archived cards are likewise
/// absent from `list_all_cards` and are fetched by id.
pub(crate) fn read_full_snapshot(store: &dyn DataStore) -> KanbanResult<Snapshot> {
    // Flat, per-collection reads rather than a walk down boards -> columns ->
    // cards. Cards carry no foreign key on `column_id` or `board_id`, so a card
    // can outlive its column; a hierarchical read could only reach cards through
    // a column and would drop those rows. FK repair re-homes them downstream,
    // but only if they are carried across at all.
    let archived_boards = store.list_archived_boards()?;
    let archived_cards = store.list_archived_cards()?;

    // `list_boards` is live-scoped, so an archived board's head has to be
    // recovered individually through the unfiltered `get_board`; without it the
    // whole archived subtree lands headless.
    let mut boards = store.list_boards()?;
    let live_board_ids: HashSet<Uuid> = boards.iter().map(|b| b.id).collect();
    for ab in &archived_boards {
        if !live_board_ids.contains(&ab.entity_id) {
            if let Some(board) = store.get_board(ab.entity_id)? {
                boards.push(board);
            }
        }
    }

    // Likewise `list_all_cards` hides archived rows under the marker model, so
    // fetch those by id or the markers import orphaned.
    let mut cards = store.list_all_cards()?;
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
        columns: store.list_all_columns()?,
        cards,
        archived_cards,
        archived_boards,
        sprints: store.list_all_sprints()?,
        graph: store.get_graph()?,
        prefixes: store.list_prefixes()?,
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
    // Before the boards: these carry all card and sprint numbering, and a
    // snapshot written without them restarts every namespace at 1.
    for prefix in snapshot.prefixes {
        store.upsert_prefix(prefix)?;
    }
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

pub(crate) fn repair_fks(snapshot: &mut Snapshot) -> KanbanResult<()> {
    let valid_columns: HashSet<Uuid> = snapshot.columns.iter().map(|c| c.id).collect();
    let valid_sprints: HashSet<Uuid> = snapshot.sprints.iter().map(|s| s.id).collect();
    let fallback_column: Option<Uuid> = snapshot
        .columns
        .iter()
        .min_by_key(|c| c.position)
        .map(|c| c.id);

    for card in snapshot.cards.iter_mut() {
        if let Some(sprint_id) = card.sprint_id {
            if !valid_sprints.contains(&sprint_id) {
                card.sprint_id = None;
            }
        }
        if !valid_columns.contains(&card.column_id) {
            if let Some(fb) = fallback_column {
                card.column_id = fb;
            }
        }
    }

    let orphaned = snapshot
        .cards
        .iter()
        .filter(|card| !valid_columns.contains(&card.column_id))
        .count();
    if orphaned > 0 {
        return Err(KanbanError::validation(format!(
            "cannot migrate: {orphaned} live card(s) reference a column that does not \
             exist and there is no column to reassign them to"
        )));
    }

    Ok(())
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
        fn get_prefix(&self, name: &str) -> KanbanResult<Option<kanban_domain::Prefix>> {
            self.0.get_prefix(name)
        }
        fn list_prefixes(&self) -> KanbanResult<Vec<kanban_domain::Prefix>> {
            self.0.list_prefixes()
        }
        fn upsert_prefix(&self, prefix: kanban_domain::Prefix) -> KanbanResult<()> {
            self.0.upsert_prefix(prefix)
        }

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
        let live = Board::new("Live", None::<String>);
        let live_column = Column::new(live.id, "Todo", 0);
        let blocker = Card::new(live.id, live_column.id, "Blocker", 0);
        let blocked = Card::new(live.id, live_column.id, "Blocked", 1);
        let archived_card = Card::new(live.id, live_column.id, "Archived card", 2);
        let live_sprint = Sprint::new(live.id, 1, None, None::<String>);

        let arch = Board::new("Archived board", None::<String>);
        let arch_column = Column::new(arch.id, "Done", 0);
        let arch_card = Card::new(arch.id, arch_column.id, "Card on archived board", 0);
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

    /// A full-snapshot write that omits the prefix rows restarts every
    /// namespace at 1 on the far side, re-minting identifiers that already
    /// exist on the cards written alongside them.
    ///
    /// This is the `kanban migrate <json> sqlite` path. It was masked while
    /// SQLite still carried `boards.card_counter`: the 9 -> 10 migration
    /// re-seeded `prefixes` from that column on every open, repairing the loss
    /// invisibly. With the column dropped there is nothing left to repair it.
    #[test]
    fn test_write_full_snapshot_carries_the_prefix_rows() -> KanbanResult<()> {
        use kanban_domain::Prefix;

        let store = InMemoryStore::new();
        let mut snapshot = Snapshot::new();
        snapshot.prefixes = vec![
            Prefix {
                name: "kan".into(),
                card_counter: 1258,
                sprint_counter: 22,
            },
            Prefix {
                name: "auth".into(),
                card_counter: 4,
                sprint_counter: 1,
            },
        ];

        write_full_snapshot(&store, snapshot)?;

        let mut written = store.list_prefixes()?;
        written.sort_by(|a, b| a.name.cmp(&b.name));
        assert_eq!(
            written.len(),
            2,
            "the prefix rows must survive a full-snapshot write"
        );
        assert_eq!(written[1].name, "kan");
        assert_eq!(
            (written[1].card_counter, written[1].sprint_counter),
            (1258, 22),
            "the counters carry the numbering; a reset re-mints existing identifiers"
        );
        Ok(())
    }

    /// The read counterpart of the write above. `kanban migrate <sqlite>
    /// <anything>` reads through here, so dropping the prefix rows on the way
    /// out loses the numbering just as surely as dropping them on the way in.
    #[test]
    fn test_read_full_snapshot_carries_the_prefix_rows() -> KanbanResult<()> {
        use kanban_domain::Prefix;

        let store = InMemoryStore::new();
        store.upsert_prefix(Prefix {
            name: "kan".into(),
            card_counter: 1258,
            sprint_counter: 22,
        })?;

        let snapshot = read_full_snapshot(&store)?;

        assert_eq!(
            snapshot.prefixes.len(),
            1,
            "reading a full snapshot must carry the prefix rows"
        );
        assert_eq!(
            (
                snapshot.prefixes[0].card_counter,
                snapshot.prefixes[0].sprint_counter
            ),
            (1258, 22)
        );
        Ok(())
    }

    /// Records any card written while the namespace its prefix names has no row.
    struct PrefixWriteOrderProbe {
        inner: InMemoryStore,
        unbacked_at_write: std::sync::Mutex<Vec<(u32, String)>>,
    }

    impl PrefixWriteOrderProbe {
        fn new() -> Self {
            Self {
                inner: InMemoryStore::new(),
                unbacked_at_write: std::sync::Mutex::new(Vec::new()),
            }
        }

        fn unbacked_at_write(&self) -> Vec<(u32, String)> {
            self.unbacked_at_write.lock().unwrap().clone()
        }
    }

    impl DataStore for PrefixWriteOrderProbe {
        fn get_prefix(&self, name: &str) -> KanbanResult<Option<kanban_domain::Prefix>> {
            self.inner.get_prefix(name)
        }
        fn list_prefixes(&self) -> KanbanResult<Vec<kanban_domain::Prefix>> {
            self.inner.list_prefixes()
        }
        fn upsert_prefix(&self, prefix: kanban_domain::Prefix) -> KanbanResult<()> {
            self.inner.upsert_prefix(prefix)
        }
        fn get_board(&self, id: Uuid) -> KanbanResult<Option<Board>> {
            self.inner.get_board(id)
        }
        fn list_boards(&self) -> KanbanResult<Vec<Board>> {
            self.inner.list_boards()
        }
        fn upsert_board(&self, board: Board) -> KanbanResult<()> {
            self.inner.upsert_board(board)
        }
        fn delete_board(&self, id: Uuid) -> KanbanResult<()> {
            self.inner.delete_board(id)
        }
        fn get_column(&self, id: Uuid) -> KanbanResult<Option<Column>> {
            self.inner.get_column(id)
        }
        fn list_columns_by_board(&self, board_id: Uuid) -> KanbanResult<Vec<Column>> {
            self.inner.list_columns_by_board(board_id)
        }
        fn list_all_columns(&self) -> KanbanResult<Vec<Column>> {
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
            self.inner.get_card(id)
        }
        fn list_all_cards(&self) -> KanbanResult<Vec<Card>> {
            self.inner.list_all_cards()
        }
        fn list_cards_by_column(&self, column_id: Uuid) -> KanbanResult<Vec<Card>> {
            self.inner.list_cards_by_column(column_id)
        }
        fn list_cards_by_sprint(&self, sprint_id: Uuid) -> KanbanResult<Vec<Card>> {
            self.inner.list_cards_by_sprint(sprint_id)
        }
        fn count_cards_in_column(&self, column_id: Uuid) -> KanbanResult<usize> {
            self.inner.count_cards_in_column(column_id)
        }
        fn count_cards_in_column_excluding(
            &self,
            column_id: Uuid,
            exclude: &[Uuid],
        ) -> KanbanResult<usize> {
            self.inner
                .count_cards_in_column_excluding(column_id, exclude)
        }
        fn upsert_card(&self, card: Card) -> KanbanResult<()> {
            if !card.prefix.is_empty() {
                let normalized = kanban_domain::Prefix::normalize(&card.prefix);
                if self.inner.get_prefix(&normalized)?.is_none() {
                    self.unbacked_at_write
                        .lock()
                        .unwrap()
                        .push((card.card_number, card.prefix.clone()));
                }
            }
            self.inner.upsert_card(card)
        }
        fn delete_card(&self, id: Uuid) -> KanbanResult<()> {
            self.inner.delete_card(id)
        }
        fn delete_cards_by_columns(&self, column_ids: &[Uuid]) -> KanbanResult<()> {
            self.inner.delete_cards_by_columns(column_ids)
        }
        fn list_archived_cards(&self) -> KanbanResult<Vec<ArchivedCard>> {
            self.inner.list_archived_cards()
        }
        fn insert_archived_card(&self, ac: ArchivedCard) -> KanbanResult<()> {
            self.inner.insert_archived_card(ac)
        }
        fn get_archived_card(&self, card_id: Uuid) -> KanbanResult<Option<ArchivedCard>> {
            self.inner.get_archived_card(card_id)
        }
        fn delete_archived_card(&self, card_id: Uuid) -> KanbanResult<()> {
            self.inner.delete_archived_card(card_id)
        }
        fn get_archived_board(
            &self,
            board_id: Uuid,
        ) -> KanbanResult<Option<kanban_domain::ArchivedBoard>> {
            self.inner.get_archived_board(board_id)
        }
        fn list_archived_boards(&self) -> KanbanResult<Vec<kanban_domain::ArchivedBoard>> {
            self.inner.list_archived_boards()
        }
        fn insert_archived_board(&self, ab: kanban_domain::ArchivedBoard) -> KanbanResult<()> {
            self.inner.insert_archived_board(ab)
        }
        fn delete_archived_board(&self, board_id: Uuid) -> KanbanResult<()> {
            self.inner.delete_archived_board(board_id)
        }
        fn get_sprint(&self, id: Uuid) -> KanbanResult<Option<Sprint>> {
            self.inner.get_sprint(id)
        }
        fn list_sprints_by_board(&self, board_id: Uuid) -> KanbanResult<Vec<Sprint>> {
            self.inner.list_sprints_by_board(board_id)
        }
        fn list_all_sprints(&self) -> KanbanResult<Vec<Sprint>> {
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
            self.inner.get_graph()
        }
        fn set_graph(&self, graph: DependencyGraph) -> KanbanResult<()> {
            self.inner.set_graph(graph)
        }
        fn clear_sprint_from_cards(
            &self,
            sprint_id: Uuid,
            timestamp: chrono::DateTime<chrono::Utc>,
        ) -> KanbanResult<()> {
            self.inner.clear_sprint_from_cards(sprint_id, timestamp)
        }
        fn snapshot(&self) -> KanbanResult<Snapshot> {
            panic!("write_full_snapshot must compose per-entity writes, not call apply_snapshot()")
        }
        fn apply_snapshot(&self, _snapshot: Snapshot) -> KanbanResult<()> {
            panic!("write_full_snapshot must compose per-entity writes, not call apply_snapshot()")
        }
    }

    fn snapshot_with_one_card(prefix: &str, card_number: u32) -> Snapshot {
        let board = Board::new("B", Some(prefix));
        let column = Column::new(board.id, "Todo", 0);
        let mut card = Card::new(board.id, column.id, "C", 0);
        card.card_number = card_number;
        card.prefix = prefix.to_string();

        let mut snapshot = Snapshot::new();
        snapshot.boards = vec![board];
        snapshot.columns = vec![column];
        snapshot.cards = vec![card];
        snapshot
    }

    #[test]
    fn test_the_snapshot_probe_reports_a_card_whose_prefix_row_is_missing_from_the_snapshot() {
        let probe = PrefixWriteOrderProbe::new();
        let snapshot = snapshot_with_one_card("KAN", 7);

        let err = write_full_snapshot(&probe, snapshot).unwrap_err();

        assert!(matches!(
            err,
            kanban_domain::KanbanError::Domain(kanban_domain::DomainError::PrefixNotBacked {
                card_number: 7,
                ref prefix,
            }) if prefix == "KAN"
        ));
        assert_eq!(probe.unbacked_at_write(), vec![(7, "KAN".to_string())]);
    }

    #[test]
    fn test_repair_snapshot_fks_repairs_live_row_of_archived_card() -> KanbanResult<()> {
        let valid_col = Column::new(Uuid::new_v4(), "Todo", 0);
        let card_id = Uuid::new_v4();
        let board_id = Uuid::new_v4();
        let mut card = Card::new(board_id, Uuid::new_v4(), "Archived card", 0);
        card.id = card_id;

        let mut snapshot = Snapshot::new();
        snapshot.columns = vec![valid_col.clone()];
        snapshot.cards = vec![card];
        snapshot.archived_cards = vec![ArchivedCard::new(card_id, board_id)];

        repair_fks(&mut snapshot)?;

        assert_eq!(
            snapshot.cards[0].column_id, valid_col.id,
            "live card row must be reassigned to fallback column"
        );
        let marker = &snapshot.archived_cards[0];
        assert_eq!(marker.entity_id, card_id);
        assert_eq!(marker.context.board_id, board_id);
        Ok(())
    }

    #[test]
    fn test_repair_snapshot_fks_marker_archived_cards_pass_through_unchanged() -> KanbanResult<()> {
        let col = Column::new(Uuid::new_v4(), "Todo", 0);
        let card_id = Uuid::new_v4();
        let board_id = Uuid::new_v4();
        let mut card = Card::new(board_id, col.id, "C", 0);
        card.id = card_id;
        let marker = ArchivedCard::new(card_id, board_id);

        let mut snapshot = Snapshot::new();
        snapshot.columns = vec![col];
        snapshot.cards = vec![card];
        snapshot.archived_cards = vec![marker];

        repair_fks(&mut snapshot)?;

        let marker_after = &snapshot.archived_cards[0];
        assert_eq!(marker_after.entity_id, marker.entity_id);
        assert_eq!(marker_after.context.board_id, marker.context.board_id);
        Ok(())
    }

    #[test]
    fn test_repair_fks_fails_when_there_is_no_column_to_rehome_to() {
        let board_id = Uuid::new_v4();
        let mut card = Card::new(board_id, Uuid::new_v4(), "Orphan", 0);
        card.id = Uuid::new_v4();

        let mut snapshot = Snapshot::new();
        snapshot.cards = vec![card];

        let err = repair_fks(&mut snapshot).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("cannot migrate") && msg.contains("live card"),
            "unexpected error: {msg}"
        );
    }

    #[test]
    fn test_write_full_snapshot_writes_the_prefix_rows_before_the_cards_that_name_them(
    ) -> KanbanResult<()> {
        use kanban_domain::Prefix;

        let probe = PrefixWriteOrderProbe::new();
        let mut snapshot = snapshot_with_one_card("KAN", 7);
        snapshot.prefixes = vec![Prefix {
            name: "kan".into(),
            card_counter: 7,
            sprint_counter: 0,
        }];

        write_full_snapshot(&probe, snapshot)?;

        assert!(
            probe.unbacked_at_write().is_empty(),
            "no card should ever be recorded as unbacked when its prefix row \
             is present in the same snapshot"
        );
        Ok(())
    }
}
