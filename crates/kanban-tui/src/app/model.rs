use chrono::{DateTime, Utc};
use kanban_domain::{
    sort_boards_in_place, ArchivedBoard, ArchivedCard, Board, BoardSortField, Card, Column,
    DependencyGraph, Snapshot, SortOrder, Sprint,
};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

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
    // archived_at timestamps keyed by board id, REBUILT from the archival
    // markers on every `load_from_snapshot`. The board head does NOT carry
    // archived_at (it stays live under the reference-marker model), so recency
    // sorting needs this side map. Because it is rebuilt from the markers each
    // load — and every archive/restore/permanent-delete handler calls
    // `prepare_frame` (→ `load_from_snapshot`) after mutating the set — it
    // cannot go stale relative to the archived partition it sorts.
    archived_board_at: HashMap<Uuid, DateTime<Utc>>,
    // Sort dimension for the ARCHIVED projects panel: the board-specific
    // `BoardSortField` (NOT the card `SortField`) paired with the shared
    // `SortOrder` toggle. Defaults to archived_at DESC — the conventional
    // trash/history "newest first" UX the #428 review flagged as lost. The LIVE
    // panel is always position order.
    archived_boards_sort_field: BoardSortField,
    archived_boards_sort_order: SortOrder,
    graph: DependencyGraph,
}

impl Default for Model {
    fn default() -> Self {
        Self {
            boards: None,
            columns: None,
            cards: None,
            card_index: HashMap::new(),
            board_index: HashMap::new(),
            sprints: None,
            archived_cards: None,
            archived_card_ids: HashSet::new(),
            archived_boards: None,
            archived_board_ids: HashSet::new(),
            displayed_cards_live: Vec::new(),
            displayed_cards_archived: Vec::new(),
            displayed_boards_live: Vec::new(),
            displayed_boards_archived: Vec::new(),
            archived_board_at: HashMap::new(),
            archived_boards_sort_field: BoardSortField::ArchivedAt,
            archived_boards_sort_order: SortOrder::Descending,
            graph: DependencyGraph::default(),
        }
    }
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

    /// The ARCHIVED heads in the CONFIGURED archived-boards order (default
    /// archived_at DESC — newest first). This is what the ArchivedBoardsView
    /// renders AND what its restore / permanent-delete affordances index into:
    /// both read this same cached, sorted partition so the rendered row and the
    /// selected id stay consistent under any sort. Independent of the transient
    /// `AppMode` (a confirm dialog opened over the archived view still resolves
    /// the archived head), because it reads the cached partition, not the mode.
    pub fn archived_boards_view(&self) -> impl Iterator<Item = &Board> {
        self.displayed_boards_archived.iter()
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
    /// markers). The live/archived partition is precomputed on load and served by
    /// [`displayed_cards`](Self::displayed_cards); this set backs that split.
    pub fn archived_card_ids(&self) -> &std::collections::HashSet<Uuid> {
        &self.archived_card_ids
    }

    pub fn archived_boards(&self) -> &[ArchivedBoard] {
        self.archived_boards.as_deref().unwrap_or(&[])
    }

    /// Ids of the archived boards. The heads themselves live in the unified
    /// `boards()` collection; this set records which of them are archived (built
    /// from the markers). The live/archived partition is precomputed on load and
    /// served by [`displayed_boards`](Self::displayed_boards); this set backs that
    /// split.
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

    /// The current archived-boards sort dimension (`BoardSortField`/`SortOrder`).
    pub fn archived_boards_sort(&self) -> (BoardSortField, SortOrder) {
        (
            self.archived_boards_sort_field,
            self.archived_boards_sort_order,
        )
    }

    /// Set the archived-boards sort field/order and re-sort the cached archived
    /// partition in place. The live partition is untouched (stays position order).
    pub fn set_archived_boards_sort(&mut self, field: BoardSortField, order: SortOrder) {
        self.archived_boards_sort_field = field;
        self.archived_boards_sort_order = order;
        self.sort_archived_partition();
    }

    /// Flip the archived-boards sort ORDER via the shared `SortOrder::toggled`
    /// (the same asc↔desc flip the card list uses), keeping the current field.
    pub fn toggle_archived_boards_sort_order(&mut self) {
        let next = self.archived_boards_sort_order.toggled();
        self.set_archived_boards_sort(self.archived_boards_sort_field, next);
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
        // Harvest archived_at per board id so the archived-boards panel can sort
        // by recency (the head does not carry it; the marker does).
        self.archived_board_at = snapshot
            .archived_boards
            .iter()
            .map(|ab| (ab.entity_id, ab.metadata.archived_at))
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
        // Live panel stays in board (position) order — unchanged. The archived
        // panel is sorted by the configured dimension (default archived_at DESC).
        self.displayed_boards_live = live_boards;
        self.displayed_boards_archived = archived_boards;
        self.sort_archived_partition();
    }

    /// Sort the cached archived-boards partition by the configured
    /// `BoardSortField`/`SortOrder`, via the shared board sort primitive. Called
    /// on load and whenever the sort dimension changes, so the rendered list and
    /// the selection resolver (both read this partition) stay consistent.
    fn sort_archived_partition(&mut self) {
        sort_boards_in_place(
            &mut self.displayed_boards_archived,
            self.archived_boards_sort_field,
            self.archived_boards_sort_order,
            &self.archived_board_at,
        );
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
        // `archived_card_ids` records the archived subset of the unified `cards()`
        // collection (the same set that backs `displayed_cards`). Assert an
        // archived card is reachable by filtering `cards()` through that set.
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

    fn seed_two_archived_boards(m: &mut Model) -> (Uuid, Uuid) {
        // `first` sits at position 0 but was archived EARLIER; `second` sits at
        // position 1 but was archived LATER. Position order and recency order
        // therefore disagree, so the two orderings are distinguishable.
        use kanban_domain::Archived;
        let mut first = Board::new("First", None::<String>);
        first.position = 0;
        let mut second = Board::new("Second", None::<String>);
        second.position = 1;
        let first_id = first.id;
        let second_id = second.id;
        let t_old = chrono::DateTime::parse_from_rfc3339("2026-01-01T00:00:00Z")
            .unwrap()
            .with_timezone(&chrono::Utc);
        let t_new = chrono::DateTime::parse_from_rfc3339("2026-06-01T00:00:00Z")
            .unwrap()
            .with_timezone(&chrono::Utc);
        m.load_from_snapshot(Snapshot {
            boards: vec![first, second],
            archived_boards: vec![
                Archived::at(first_id, t_old),
                Archived::at(second_id, t_new),
            ],
            ..Default::default()
        });
        (first_id, second_id)
    }

    #[test]
    fn test_archived_boards_default_order_is_recency() {
        // Default archived-boards order is archived_at DESC (newest first),
        // the conventional trash/history UX — NOT position order.
        let mut m = Model::default();
        let (first_id, second_id) = seed_two_archived_boards(&mut m);
        let order: Vec<Uuid> = m.displayed_boards(true).iter().map(|b| b.id).collect();
        assert_eq!(
            order,
            vec![second_id, first_id],
            "newest-archived (second) must come first by default"
        );
    }

    #[test]
    fn test_archived_boards_sort_by_position_matches_board_order() {
        let mut m = Model::default();
        let (first_id, second_id) = seed_two_archived_boards(&mut m);
        m.set_archived_boards_sort(
            kanban_domain::BoardSortField::Position,
            kanban_domain::SortOrder::Ascending,
        );
        let order: Vec<Uuid> = m.displayed_boards(true).iter().map(|b| b.id).collect();
        assert_eq!(
            order,
            vec![first_id, second_id],
            "position order restores board order (first at pos 0)"
        );
    }

    #[test]
    fn test_toggle_reverses_archived_boards_order() {
        // The shared SortOrder toggle flips the archived-boards order. From the
        // default (recency DESC) a toggle yields recency ASC (oldest first).
        let mut m = Model::default();
        let (first_id, second_id) = seed_two_archived_boards(&mut m);
        let before: Vec<Uuid> = m.displayed_boards(true).iter().map(|b| b.id).collect();
        assert_eq!(before, vec![second_id, first_id]);

        m.toggle_archived_boards_sort_order();
        let after: Vec<Uuid> = m.displayed_boards(true).iter().map(|b| b.id).collect();
        assert_eq!(
            after,
            vec![first_id, second_id],
            "toggle reverses to oldest-archived first"
        );
    }

    #[test]
    fn test_live_projects_panel_order_is_position_unaffected_by_archived_sort() {
        // Guard: the LIVE panel stays in position order regardless of the
        // archived-boards sort dimension/direction.
        use kanban_domain::Archived;
        let mut m = Model::default();
        let mut live_a = Board::new("LiveA", None::<String>);
        live_a.position = 0;
        let mut live_b = Board::new("LiveB", None::<String>);
        live_b.position = 1;
        let mut archived = Board::new("Archived", None::<String>);
        archived.position = 2;
        let a_id = live_a.id;
        let b_id = live_b.id;
        let arch_id = archived.id;
        m.load_from_snapshot(Snapshot {
            boards: vec![live_a, live_b, archived],
            archived_boards: vec![Archived::now(arch_id)],
            ..Default::default()
        });

        // Toggle / re-sort the archived dimension; live order must not move.
        m.toggle_archived_boards_sort_order();
        m.set_archived_boards_sort(
            kanban_domain::BoardSortField::ArchivedAt,
            kanban_domain::SortOrder::Ascending,
        );
        let live: Vec<Uuid> = m.displayed_boards(false).iter().map(|b| b.id).collect();
        assert_eq!(live, vec![a_id, b_id], "live panel stays in position order");
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
