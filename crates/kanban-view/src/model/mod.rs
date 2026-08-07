use chrono::{DateTime, Utc};
use kanban_core::AppConfig;
use kanban_domain::{
    sort_boards_in_place, ArchivedBoard, ArchivedCard, Board, BoardSortField, Card, Column,
    DependencyGraph, Snapshot, SortOrder, Sprint, DEFAULT_ARCHIVED_BOARD_SORT,
    DEFAULT_BOARD_SORT_LIVE,
};
use std::collections::{HashMap, HashSet};
use std::str::FromStr;
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
    // Sort dimension for the PROJECTS panel — the board-specific `BoardSortField`
    // (NOT the card `SortField`) paired with the shared `SortOrder` toggle. The
    // live and archived partitions each carry their own independent field/order
    // pair: the live pair is seeded from and persisted to `AppConfig.board_sort_*`,
    // while the archived pair is session-only (never persisted) and defaults to
    // recency (ArchivedAt DESC). Setting one pair never affects the other.
    live_board_sort_field: BoardSortField,
    live_board_sort_order: SortOrder,
    archived_board_sort_field: BoardSortField,
    archived_board_sort_order: SortOrder,
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
            live_board_sort_field: DEFAULT_BOARD_SORT_LIVE.0,
            live_board_sort_order: DEFAULT_BOARD_SORT_LIVE.1,
            archived_board_sort_field: DEFAULT_ARCHIVED_BOARD_SORT.0,
            archived_board_sort_order: DEFAULT_ARCHIVED_BOARD_SORT.1,
            graph: DependencyGraph::default(),
        }
    }
}

impl Model {
    pub fn columns(&self) -> &[Column] {
        self.columns.as_deref().unwrap_or(&[])
    }

    pub fn sprints(&self) -> &[Sprint] {
        self.sprints.as_deref().unwrap_or(&[])
    }

    pub fn graph(&self) -> &DependencyGraph {
        &self.graph
    }

    pub fn load_from_snapshot(&mut self, snapshot: Snapshot) {
        // Reference-marker model: `snapshot.cards` carries EVERY card — live AND
        // archived — with archival recorded by markers in `snapshot.archived_cards`
        // (keyed by `entity_id`). Unify: one collection holds all rows, and an
        // id set records which are archived. The live/archived distinction is a
        // consumption decision applied by filtering `all_cards()` on this set.
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
            .all_cards()
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
        self.sort_partitions();
    }

    /// Sort BOTH cached board partitions, each against its own independent
    /// field/order pair. Called on load and whenever either sort dimension
    /// changes, so the rendered lists and the selection resolvers (which read
    /// these partitions) stay consistent.
    fn sort_partitions(&mut self) {
        sort_boards_in_place(
            &mut self.displayed_boards_live,
            self.live_board_sort_field,
            self.live_board_sort_order,
            &self.archived_board_at,
        );
        sort_boards_in_place(
            &mut self.displayed_boards_archived,
            self.archived_board_sort_field,
            self.archived_board_sort_order,
            &self.archived_board_at,
        );
    }
}

mod board_sort;
mod boards;
mod cards;

#[cfg(test)]
mod tests {
    use super::*;
    use kanban_domain::{Board, Card, Column, Snapshot};

    fn make_card(board: &mut Board, column_id: Uuid) -> Card {
        Card::new(board, column_id, "task", 0)
    }

    #[test]
    fn test_default_model_returns_empty_slices() {
        let m = Model::default();
        assert!(m.boards().is_empty());
        assert!(m.columns().is_empty());
        assert!(m.all_cards().is_empty());
        assert!(m.sprints().is_empty());
        assert!(m.archived_card_markers().is_empty());
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
