use kanban_domain::{
    ArchivedBoard, ArchivedCard, Board, Card, Column, DependencyGraph, Snapshot, Sprint,
};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

#[derive(Default)]
pub struct Model {
    boards: Option<Vec<Board>>,
    columns: Option<Vec<Column>>,
    cards: Option<Vec<Card>>,
    card_index: HashMap<Uuid, usize>,
    board_index: HashMap<Uuid, usize>,
    sprints: Option<Vec<Sprint>>,
    archived_cards: Option<Vec<ArchivedCard>>,
    archived_card_ids: HashSet<Uuid>,
    archived_boards: Option<Vec<ArchivedBoard>>,
    archived_board_ids: HashSet<Uuid>,
    // Live/archived partitions of the unified `cards`/`boards` collections,
    // computed ONCE in `load_from_snapshot` (snapshot-on-open) and served as a
    // borrow by `displayed_cards`/`displayed_boards`. This is the concrete
    // no-per-frame-recompute fix (KAN-933): the projects/tasks panels borrow
    // the cached subset every redraw instead of re-filtering+cloning per frame.
    displayed_cards_live: Vec<Card>,
    displayed_cards_archived: Vec<Card>,
    displayed_boards_live: Vec<Board>,
    displayed_boards_archived: Vec<Board>,
    graph: DependencyGraph,
}

impl Model {
    /// The single unified board collection (live AND archived heads). Which of
    /// these are archived is recorded in `archived_board_ids`; consumers that
    /// want only one subset filter this collection by that set (the projects
    /// panel does so via `displayed_boards`). Mirrors the unified `cards()`.
    pub fn boards(&self) -> &[Board] {
        self.boards.as_deref().unwrap_or(&[])
    }

    /// The LIVE boards (unified collection minus the archived heads), in board
    /// order. The live projects panel and every live-only quantity (first-board
    /// default selection, new-board position, live counts) resolve through this,
    /// so broadening `boards()` to the unified collection cannot leak archived
    /// heads into live semantics.
    pub fn live_boards(&self) -> impl Iterator<Item = &Board> {
        self.boards()
            .iter()
            .filter(|b| !self.archived_board_ids.contains(&b.id))
    }

    /// The ARCHIVED heads (unified collection filtered to the archived-id set),
    /// in board order. This is what the ArchivedBoardsView renders and what its
    /// restore / permanent-delete affordances index into — resolved directly
    /// from the id set so it is independent of the transient `AppMode` (a confirm
    /// dialog opened over the archived view must still resolve the archived head).
    pub fn archived_boards_view(&self) -> impl Iterator<Item = &Board> {
        self.boards()
            .iter()
            .filter(|b| self.archived_board_ids.contains(&b.id))
    }

    pub fn columns(&self) -> &[Column] {
        self.columns.as_deref().unwrap_or(&[])
    }

    pub fn cards(&self) -> &[Card] {
        self.cards.as_deref().unwrap_or(&[])
    }

    /// Resolve a card by id from the single unified collection (live AND
    /// archived rows). One index lookup — no live/archived re-join. A card is
    /// a card regardless of whether its head is archived.
    pub fn card_by_id(&self, id: Uuid) -> Option<&Card> {
        let &idx = self.card_index.get(&id)?;
        self.cards.as_ref()?.get(idx)
    }

    pub fn sprints(&self) -> &[Sprint] {
        self.sprints.as_deref().unwrap_or(&[])
    }

    pub fn archived_cards(&self) -> &[ArchivedCard] {
        self.archived_cards.as_deref().unwrap_or(&[])
    }

    /// Ids of the archived cards. Rows themselves live in the unified `cards()`
    /// collection; this set records which of them are archived (built from the
    /// markers). Consumers that need the archived subset filter `cards()` by
    /// this set. (T1c introduces a single `displayed_cards()` accessor.)
    pub fn archived_card_ids(&self) -> &std::collections::HashSet<Uuid> {
        &self.archived_card_ids
    }

    pub fn archived_boards(&self) -> &[ArchivedBoard] {
        self.archived_boards.as_deref().unwrap_or(&[])
    }

    /// Ids of the archived boards. The heads themselves live in the unified
    /// `boards()` collection; this set records which of them are archived (built
    /// from the markers). Consumers that need the archived subset filter
    /// `boards()` by this set. (T1c introduces a single `displayed_boards()`
    /// accessor that subsumes the inline filter.)
    pub fn archived_board_ids(&self) -> &HashSet<Uuid> {
        &self.archived_board_ids
    }

    /// The cards the tasks panel should display, selected by `want_archived`:
    /// the archived subset when a confirm dialog / the archived-cards view is
    /// active, the live subset otherwise. Returns a BORROW of the partition
    /// cached on the last `load_from_snapshot` — no per-frame filter or clone.
    pub fn displayed_cards(&self, want_archived: bool) -> &[Card] {
        if want_archived {
            &self.displayed_cards_archived
        } else {
            &self.displayed_cards_live
        }
    }

    /// The boards the projects panel should display, selected by `want_archived`.
    /// Borrow of the partition cached on `load_from_snapshot`; the mode decision
    /// (live vs archived) lives at the `App` accessor, which passes the
    /// stack-aware base mode in.
    pub fn displayed_boards(&self, want_archived: bool) -> &[Board] {
        if want_archived {
            &self.displayed_boards_archived
        } else {
            &self.displayed_boards_live
        }
    }

    /// Resolve a board by id from the single unified collection (live AND
    /// archived heads). One index lookup — no live/archived re-join. It is
    /// deliberately archival-agnostic: a board is a board regardless of whether
    /// its head is archived.
    pub fn board_by_id(&self, id: Uuid) -> Option<&Board> {
        let &idx = self.board_index.get(&id)?;
        self.boards.as_ref()?.get(idx)
    }

    pub fn graph(&self) -> &DependencyGraph {
        &self.graph
    }

    pub fn load_from_snapshot(&mut self, snapshot: Snapshot) {
        // Reference-marker model: `snapshot.cards` carries EVERY card — live AND
        // archived — with archival recorded by markers in `snapshot.archived_cards`
        // (keyed by `entity_id`). Unify: one collection holds all rows, and an
        // id set records which are archived. The live/archived distinction is a
        // consumption decision applied by filtering `cards()` on this set.
        let archived_card_ids: HashSet<Uuid> = snapshot
            .archived_cards
            .iter()
            .map(|ac| ac.entity_id)
            .collect();

        let cards = snapshot.cards;
        self.card_index.clear();
        for (i, card) in cards.iter().enumerate() {
            self.card_index.insert(card.id, i);
        }
        self.archived_card_ids = archived_card_ids;

        // Boards unify exactly like cards: `snapshot.boards` carries EVERY board
        // head (live + archived) in one collection; `snapshot.archived_boards`
        // are markers keyed by `entity_id`, and the id set records which heads
        // are archived. The live/archived distinction is a consumption decision
        // applied by filtering `boards()` on this set (the projects panel does so
        // via `displayed_boards`).
        let archived_board_ids: HashSet<Uuid> = snapshot
            .archived_boards
            .iter()
            .map(|ab| ab.entity_id)
            .collect();

        let boards = snapshot.boards;
        self.board_index.clear();
        for (i, board) in boards.iter().enumerate() {
            self.board_index.insert(board.id, i);
        }
        self.archived_board_ids = archived_board_ids;

        self.boards = Some(boards);
        self.columns = Some(snapshot.columns);
        self.sprints = Some(snapshot.sprints);
        self.cards = Some(cards);
        self.archived_cards = Some(snapshot.archived_cards);
        self.archived_boards = Some(snapshot.archived_boards);
        self.graph = snapshot.graph;

        // Partition the unified collections into live/archived subsets ONCE, here
        // on load (snapshot-on-open), so `displayed_cards`/`displayed_boards` can
        // serve a borrow every redraw instead of re-filtering per frame. Order is
        // preserved so index-based selection into the displayed set is stable.
        self.rebuild_displayed_partitions();
    }

    fn rebuild_displayed_partitions(&mut self) {
        let (archived_cards, live_cards): (Vec<Card>, Vec<Card>) = self
            .cards()
            .iter()
            .cloned()
            .partition(|c| self.archived_card_ids.contains(&c.id));
        self.displayed_cards_live = live_cards;
        self.displayed_cards_archived = archived_cards;

        let (archived_boards, live_boards): (Vec<Board>, Vec<Board>) = self
            .boards()
            .iter()
            .cloned()
            .partition(|b| self.archived_board_ids.contains(&b.id));
        self.displayed_boards_live = live_boards;
        self.displayed_boards_archived = archived_boards;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kanban_domain::{ArchivedCard, Board, Card, Column, Snapshot};

    fn make_card(board: &mut Board, column_id: Uuid) -> Card {
        Card::new(board, column_id, "task", 0)
    }

    #[test]
    fn test_default_model_returns_empty_slices() {
        let m = Model::default();
        assert!(m.boards().is_empty());
        assert!(m.columns().is_empty());
        assert!(m.cards().is_empty());
        assert!(m.sprints().is_empty());
        assert!(m.archived_cards().is_empty());
        assert!(m.archived_card_ids().is_empty());
    }

    #[test]
    fn test_load_from_snapshot_populates_boards_and_columns() {
        let mut m = Model::default();
        let board = Board::new("B", None::<String>);
        let col = Column::new(board.id, "Col", 0);
        m.load_from_snapshot(Snapshot {
            archived_boards: Vec::new(),
            boards: vec![board.clone()],
            columns: vec![col.clone()],
            ..Default::default()
        });
        assert_eq!(m.boards().len(), 1);
        assert_eq!(m.boards()[0].id, board.id);
        assert_eq!(m.columns().len(), 1);
        assert_eq!(m.columns()[0].id, col.id);
    }

    #[test]
    fn test_card_lookup_by_id_returns_correct_card() {
        let mut m = Model::default();
        let mut board = Board::new("B", None::<String>);
        let col_id = Uuid::new_v4();
        let card_a = make_card(&mut board, col_id);
        let card_b = make_card(&mut board, col_id);
        let card_b_id = card_b.id;
        m.load_from_snapshot(Snapshot {
            archived_boards: Vec::new(),
            cards: vec![card_a, card_b],
            ..Default::default()
        });
        let found = m.card_by_id(card_b_id).unwrap();
        assert_eq!(found.id, card_b_id);
    }

    #[test]
    fn test_card_by_id_resolves_live_and_archived_from_one_collection() {
        // After unification `cards()` holds live AND archived rows, and
        // `card_by_id` resolves either from the single collection — no
        // `or_else(archived_card())` re-join.
        let mut m = Model::default();
        let mut board = Board::new("B", None::<String>);
        let col_id = Uuid::new_v4();
        let live = make_card(&mut board, col_id);
        let archived = make_card(&mut board, col_id);
        let live_id = live.id;
        let archived_id = archived.id;
        m.load_from_snapshot(Snapshot {
            archived_boards: Vec::new(),
            cards: vec![live, archived],
            archived_cards: vec![ArchivedCard::new(archived_id, uuid::Uuid::nil())],
            ..Default::default()
        });

        // Both live and archived rows live in the single unified collection.
        assert_eq!(m.cards().len(), 2);

        // The single index resolves both.
        assert_eq!(m.card_by_id(live_id).map(|c| c.id), Some(live_id));
        assert_eq!(m.card_by_id(archived_id).map(|c| c.id), Some(archived_id));

        // The archived-id set records which rows are archived.
        assert!(!m.archived_card_ids().contains(&live_id));
        assert!(m.archived_card_ids().contains(&archived_id));
    }

    #[test]
    fn test_archived_view_filter_shows_archived_card_from_unified_collection() {
        // The archived-cards view (pending T1c's `displayed_cards()`) filters
        // the unified collection by `archived_card_ids`. Assert an archived
        // card is present through that path.
        let mut m = Model::default();
        let mut board = Board::new("B", None::<String>);
        let col_id = Uuid::new_v4();
        let live = make_card(&mut board, col_id);
        let archived = make_card(&mut board, col_id);
        let archived_id = archived.id;
        m.load_from_snapshot(Snapshot {
            archived_boards: Vec::new(),
            cards: vec![live, archived],
            archived_cards: vec![ArchivedCard::new(archived_id, uuid::Uuid::nil())],
            ..Default::default()
        });

        let displayed: Vec<Uuid> = m
            .cards()
            .iter()
            .filter(|c| m.archived_card_ids().contains(&c.id))
            .map(|c| c.id)
            .collect();
        assert_eq!(displayed, vec![archived_id]);
    }

    #[test]
    fn test_card_by_id_missing_id_returns_none() {
        let m = Model::default();
        assert!(m.card_by_id(Uuid::new_v4()).is_none());
    }

    #[test]
    fn test_displayed_cards_partition_cached_on_load() {
        // Cache-on-load guard: `load_from_snapshot` partitions the unified card
        // collection into live/archived subsets ONCE, and `displayed_cards`
        // returns the cached slice by `want_archived` — no per-frame filter.
        let mut m = Model::default();
        let mut board = Board::new("B", None::<String>);
        let col_id = Uuid::new_v4();
        let live = make_card(&mut board, col_id);
        let archived = make_card(&mut board, col_id);
        let live_id = live.id;
        let archived_id = archived.id;
        m.load_from_snapshot(Snapshot {
            archived_boards: Vec::new(),
            cards: vec![live, archived],
            archived_cards: vec![ArchivedCard::new(archived_id, uuid::Uuid::nil())],
            ..Default::default()
        });

        let live_ids: Vec<Uuid> = m.displayed_cards(false).iter().map(|c| c.id).collect();
        let archived_ids: Vec<Uuid> = m.displayed_cards(true).iter().map(|c| c.id).collect();
        assert_eq!(live_ids, vec![live_id]);
        assert_eq!(archived_ids, vec![archived_id]);
    }

    #[test]
    fn test_displayed_boards_partition_cached_on_load() {
        use kanban_domain::Archived;
        let mut m = Model::default();
        let live = Board::new("Live", None::<String>);
        let archived = Board::new("Archived", None::<String>);
        let live_id = live.id;
        let archived_id = archived.id;
        m.load_from_snapshot(Snapshot {
            boards: vec![live, archived],
            archived_boards: vec![Archived::now(archived_id)],
            ..Default::default()
        });

        let live_ids: Vec<Uuid> = m.displayed_boards(false).iter().map(|b| b.id).collect();
        let archived_ids: Vec<Uuid> = m.displayed_boards(true).iter().map(|b| b.id).collect();
        assert_eq!(live_ids, vec![live_id]);
        assert_eq!(archived_ids, vec![archived_id]);
    }

    #[test]
    fn test_default_model_returns_empty_archived_board_slices() {
        let m = Model::default();
        assert!(m.archived_boards().is_empty());
        assert!(m.archived_board_ids().is_empty());
        assert!(m.board_by_id(Uuid::new_v4()).is_none());
    }

    #[test]
    fn test_board_by_id_resolves_live_and_archived_from_one_collection() {
        // After unification `boards()` holds live AND archived heads, and
        // `board_by_id` resolves either from the single collection — no
        // `or_else(archived_board())` re-join.
        use kanban_domain::Archived;
        let mut m = Model::default();
        let live = Board::new("Live", None::<String>);
        let archived = Board::new("Archived", None::<String>);
        let live_id = live.id;
        let archived_id = archived.id;
        m.load_from_snapshot(Snapshot {
            // snapshot.boards carries BOTH heads; the marker names the archived one.
            boards: vec![live.clone(), archived.clone()],
            archived_boards: vec![Archived::now(archived_id)],
            ..Default::default()
        });

        // Both live and archived heads live in the single unified collection.
        assert_eq!(m.boards().len(), 2);

        // The single index resolves both.
        assert_eq!(m.board_by_id(live_id).map(|b| b.id), Some(live_id));
        assert_eq!(
            m.board_by_id(archived_id).map(|b| b.name.clone()),
            Some("Archived".to_string())
        );

        // The archived-id set records which heads are archived.
        assert!(!m.archived_board_ids().contains(&live_id));
        assert!(m.archived_board_ids().contains(&archived_id));
    }

    #[test]
    fn test_archived_boards_view_filter_shows_archived_board_from_unified_collection() {
        // The archived-boards view filters the unified collection by
        // `archived_board_ids`. Assert an archived board is present through that
        // path, and a live board is not.
        use kanban_domain::Archived;
        let mut m = Model::default();
        let live = Board::new("Live", None::<String>);
        let archived = Board::new("Archived", None::<String>);
        let live_id = live.id;
        let archived_id = archived.id;
        m.load_from_snapshot(Snapshot {
            boards: vec![live.clone(), archived.clone()],
            archived_boards: vec![Archived::now(archived_id)],
            ..Default::default()
        });

        let displayed: Vec<Uuid> = m
            .boards()
            .iter()
            .filter(|b| m.archived_board_ids().contains(&b.id))
            .map(|b| b.id)
            .collect();
        assert_eq!(displayed, vec![archived_id]);
        assert!(!displayed.contains(&live_id));
    }

    #[test]
    fn test_live_board_filter_excludes_archived_board_from_unified_collection() {
        // Guard: the LIVE projects panel filters archived heads OUT of the
        // unified collection (analogue of T1a's live-branch card fix).
        use kanban_domain::Archived;
        let mut m = Model::default();
        let live = Board::new("Live", None::<String>);
        let archived = Board::new("Archived", None::<String>);
        let live_id = live.id;
        let archived_id = archived.id;
        m.load_from_snapshot(Snapshot {
            boards: vec![live.clone(), archived.clone()],
            archived_boards: vec![Archived::now(archived_id)],
            ..Default::default()
        });

        let live_only: Vec<Uuid> = m
            .boards()
            .iter()
            .filter(|b| !m.archived_board_ids().contains(&b.id))
            .map(|b| b.id)
            .collect();
        assert_eq!(live_only, vec![live_id]);
        assert!(!live_only.contains(&archived_id));
    }

    #[test]
    fn test_board_by_id_missing_id_returns_none() {
        let mut m = Model::default();
        m.load_from_snapshot(Snapshot::default());
        assert!(m.board_by_id(Uuid::new_v4()).is_none());
    }

    #[test]
    fn test_load_from_snapshot_overwrites_previous_state() {
        let mut m = Model::default();
        let board_a = Board::new("A", None::<String>);
        m.load_from_snapshot(Snapshot {
            archived_boards: Vec::new(),
            boards: vec![board_a],
            ..Default::default()
        });
        assert_eq!(m.boards().len(), 1);

        let board_b = Board::new("B", None::<String>);
        let board_c = Board::new("C", None::<String>);
        m.load_from_snapshot(Snapshot {
            archived_boards: Vec::new(),
            boards: vec![board_b, board_c],
            ..Default::default()
        });
        assert_eq!(m.boards().len(), 2);
        assert_eq!(m.boards()[0].name, "B");
    }

    #[test]
    fn test_load_from_snapshot_clears_stale_card_index() {
        let mut m = Model::default();
        let mut board = Board::new("B", None::<String>);
        let col_id = Uuid::new_v4();
        let card = make_card(&mut board, col_id);
        let old_id = card.id;
        m.load_from_snapshot(Snapshot {
            archived_boards: Vec::new(),
            cards: vec![card],
            ..Default::default()
        });
        assert!(m.card_by_id(old_id).is_some());

        // Reload with no cards — stale index entry must be gone
        m.load_from_snapshot(Snapshot::default());
        assert!(m.card_by_id(old_id).is_none());
    }
}
