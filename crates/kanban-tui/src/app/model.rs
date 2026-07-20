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
    sprints: Option<Vec<Sprint>>,
    archived_cards: Option<Vec<ArchivedCard>>,
    archived_card_ids: HashSet<Uuid>,
    archived_boards: Option<Vec<ArchivedBoard>>,
    archived_boards_flat: Option<Vec<Board>>,
    archived_board_index: HashMap<Uuid, usize>,
    graph: DependencyGraph,
}

impl Model {
    /// LIVE boards only (archived heads are split out into `archived_boards_flat`).
    pub fn boards(&self) -> &[Board] {
        self.boards.as_deref().unwrap_or(&[])
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

    /// The resolved live board heads for the archived boards, in marker order —
    /// what the ArchivedBoardsView renders. Built once on load, so rendering is
    /// zero-cost (no per-frame resolution).
    pub fn archived_boards_flat(&self) -> &[Board] {
        self.archived_boards_flat.as_deref().unwrap_or(&[])
    }

    pub fn archived_board(&self, id: Uuid) -> Option<&Board> {
        let &idx = self.archived_board_index.get(&id)?;
        self.archived_boards_flat.as_ref()?.get(idx)
    }

    /// Resolve a board by id across BOTH the live set and the archived heads.
    /// This is the single uniform resolver for "the board with this id" — it is
    /// deliberately archival-agnostic: a board is a board regardless of whether
    /// its head is archived. Live boards take precedence (an id is only ever in
    /// one set, but the ordering makes the common case a direct hit).
    pub fn board_by_id(&self, id: Uuid) -> Option<&Board> {
        self.boards()
            .iter()
            .find(|b| b.id == id)
            .or_else(|| self.archived_board(id))
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

        // Boards split exactly like cards: `snapshot.boards` carries EVERY board
        // head (live + archived); `snapshot.archived_boards` are markers keyed by
        // `entity_id`. The live board views see only live boards; the archived
        // view sees the archived heads resolved from the same rows.
        let archived_board_ids: std::collections::HashSet<Uuid> = snapshot
            .archived_boards
            .iter()
            .map(|ab| ab.entity_id)
            .collect();

        let board_by_id: std::collections::HashMap<Uuid, Board> =
            snapshot.boards.iter().map(|b| (b.id, b.clone())).collect();

        let live_boards: Vec<Board> = snapshot
            .boards
            .into_iter()
            .filter(|b| !archived_board_ids.contains(&b.id))
            .collect();

        self.archived_board_index.clear();
        let mut board_flat = Vec::with_capacity(snapshot.archived_boards.len());
        for ab in snapshot.archived_boards.iter() {
            if let Some(board) = board_by_id.get(&ab.entity_id) {
                self.archived_board_index
                    .insert(ab.entity_id, board_flat.len());
                board_flat.push(board.clone());
            }
        }

        self.boards = Some(live_boards);
        self.columns = Some(snapshot.columns);
        self.sprints = Some(snapshot.sprints);
        self.cards = Some(cards);
        self.archived_cards = Some(snapshot.archived_cards);
        self.archived_boards = Some(snapshot.archived_boards);
        self.archived_boards_flat = Some(board_flat);
        self.graph = snapshot.graph;
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
    fn test_default_model_returns_empty_archived_board_slices() {
        let m = Model::default();
        assert!(m.archived_boards().is_empty());
        assert!(m.archived_boards_flat().is_empty());
        assert!(m.archived_board(Uuid::new_v4()).is_none());
    }

    #[test]
    fn test_load_from_snapshot_splits_live_and_archived_boards() {
        use kanban_domain::Archived;
        let mut m = Model::default();
        let live = Board::new("Live", None::<String>);
        let archived = Board::new("Archived", None::<String>);
        let archived_id = archived.id;
        m.load_from_snapshot(Snapshot {
            // snapshot.boards carries BOTH heads; the marker names the archived one.
            boards: vec![live.clone(), archived.clone()],
            archived_boards: vec![Archived::now(archived_id)],
            ..Default::default()
        });

        // Live board view excludes the archived head.
        assert_eq!(m.boards().len(), 1);
        assert_eq!(m.boards()[0].id, live.id);

        // The archived view resolves the archived head.
        assert_eq!(m.archived_boards().len(), 1);
        assert_eq!(m.archived_boards()[0].entity_id, archived_id);
        assert_eq!(m.archived_boards_flat().len(), 1);
        assert_eq!(m.archived_boards_flat()[0].id, archived_id);
        let found = m.archived_board(archived_id).unwrap();
        assert_eq!(found.name, "Archived");
    }

    #[test]
    fn test_archived_board_lookup_missing_id_returns_none() {
        let mut m = Model::default();
        m.load_from_snapshot(Snapshot::default());
        assert!(m.archived_board(Uuid::new_v4()).is_none());
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
