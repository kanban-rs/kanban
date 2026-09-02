use crate::{
    ArchivedBoard, ArchivedCard, Board, Card, Column, DependencyGraph, LoadState, Snapshot, Sprint,
};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

/// The unified, per-Model view of every entity kind's flat, per-id and
/// parent-scoped tiers, chained by precedence in accessors like
/// `card_by_id_state`. This differs from [`crate::resolved::Collection`],
/// whose three tiers stay mutually independent so that a resolve pass can
/// touch one tier without silently inferring another: a `Model` accessor
/// answers "what do we know about this id, from any source", while a
/// `Collection` answers "what did this specific resolve pass say about this
/// tier", and conflating the two would let an archived-excluding tier
/// silently mark an id `Missing` that another tier still holds.
pub struct Model {
    boards: LoadState<Vec<Board>>,
    columns: LoadState<Vec<Column>>,
    cards: LoadState<Vec<Card>>,
    card_index: HashMap<Uuid, usize>,
    board_index: HashMap<Uuid, usize>,
    sprints: LoadState<Vec<Sprint>>,
    archived_cards: Option<Vec<ArchivedCard>>,
    archived_card_ids: HashSet<Uuid>,
    archived_boards: Option<Vec<ArchivedBoard>>,
    archived_board_ids: HashSet<Uuid>,
    graph: LoadState<DependencyGraph>,
    /// The per-id tier beside each unified collection. Not a second row
    /// store: a `Loaded` entry here can name an entity even while the
    /// matching flat collection is `NotLoaded`, and it is the only tier that
    /// can ever hold `LoadState::Missing` for a single id. Nothing writes
    /// `boards_by_id` today; it exists for symmetry of the mutators.
    boards_by_id: HashMap<Uuid, LoadState<Board>>,
    columns_by_id: HashMap<Uuid, LoadState<Column>>,
    cards_by_id: HashMap<Uuid, LoadState<Card>>,
    sprints_by_id: HashMap<Uuid, LoadState<Sprint>>,
    /// The parent-scoped tier, keyed by the fixed parent id per kind
    /// (`columns_by_board`/`sprints_by_board`/`archived_cards_by_board` by
    /// board id, `cards_by_column` by column id). A scoped result never
    /// mutates the flat collection or the per-id tier, and vice versa.
    columns_by_board: HashMap<Uuid, LoadState<Vec<Column>>>,
    cards_by_column: HashMap<Uuid, LoadState<Vec<Card>>>,
    sprints_by_board: HashMap<Uuid, LoadState<Vec<Sprint>>>,
    archived_cards_by_board: HashMap<Uuid, LoadState<Vec<ArchivedCard>>>,
    /// Reverse index from card id to the column whose `cards_by_column`
    /// entry currently holds it. Maintained only by `set_cards_of_column`;
    /// without it, resolving a card by id through the scoped tier would scan
    /// every column's bucket on every rendered row.
    scoped_card_index: HashMap<Uuid, Uuid>,
}

impl Default for Model {
    fn default() -> Self {
        Self {
            boards: LoadState::NotLoaded,
            columns: LoadState::NotLoaded,
            cards: LoadState::NotLoaded,
            card_index: HashMap::new(),
            board_index: HashMap::new(),
            sprints: LoadState::NotLoaded,
            archived_cards: None,
            archived_card_ids: HashSet::new(),
            archived_boards: None,
            archived_board_ids: HashSet::new(),
            graph: LoadState::NotLoaded,
            boards_by_id: HashMap::new(),
            columns_by_id: HashMap::new(),
            cards_by_id: HashMap::new(),
            sprints_by_id: HashMap::new(),
            columns_by_board: HashMap::new(),
            cards_by_column: HashMap::new(),
            sprints_by_board: HashMap::new(),
            archived_cards_by_board: HashMap::new(),
            scoped_card_index: HashMap::new(),
        }
    }
}

fn scoped_state<T>(map: &HashMap<Uuid, LoadState<Vec<T>>>, parent: Uuid) -> LoadState<&[T]> {
    map.get(&parent)
        .map(|s| s.as_ref().map(|v| v.as_slice()))
        .unwrap_or(LoadState::NotLoaded)
}

impl Model {
    /// Returns a [`ModelChanged`] receipt: whatever derives from this
    /// `Model` is stale until a [`DerivedProjections`] implementor consumes
    /// it.
    pub fn load_from_snapshot(&mut self, snapshot: Snapshot) -> ModelChanged {
        // Reference-marker model: `snapshot.cards`/`snapshot.boards` each carry
        // EVERY row — live AND archived — with archival recorded by markers
        // keyed by `entity_id`. One collection holds all rows and an id set
        // records which are archived; the live/archived split is a consumption
        // decision the view layer applies on top.
        self.boards = LoadState::Loaded(snapshot.boards);
        self.columns = LoadState::Loaded(snapshot.columns);
        self.sprints = LoadState::Loaded(snapshot.sprints);
        self.cards = LoadState::Loaded(snapshot.cards);
        self.graph = LoadState::Loaded(snapshot.graph);

        self.boards_by_id.clear();
        self.columns_by_id.clear();
        self.cards_by_id.clear();
        self.sprints_by_id.clear();
        self.cards_by_column.clear();
        self.scoped_card_index.clear();
        self.archived_cards_by_board.clear();

        self.absorb_archival_markers(
            Some(snapshot.archived_cards),
            Some(snapshot.archived_boards),
        );

        self.rebuild_card_index();
        self.rebuild_board_index();
        self.rebuild_board_scoped_tiers();

        ModelChanged::new()
    }

    fn rebuild_board_scoped_tiers(&mut self) {
        self.columns_by_board.clear();
        self.sprints_by_board.clear();

        if let LoadState::Loaded(boards) = &self.boards {
            for b in boards {
                self.columns_by_board
                    .insert(b.id, LoadState::Loaded(Vec::new()));
                self.sprints_by_board
                    .insert(b.id, LoadState::Loaded(Vec::new()));
            }
        }
        if let LoadState::Loaded(columns) = &self.columns {
            for c in columns {
                let LoadState::Loaded(bucket) = self
                    .columns_by_board
                    .entry(c.board_id)
                    .or_insert_with(|| LoadState::Loaded(Vec::new()))
                else {
                    unreachable!("entry is always seeded as Loaded above")
                };
                bucket.push(c.clone());
            }
        }
        if let LoadState::Loaded(sprints) = &self.sprints {
            for s in sprints {
                let LoadState::Loaded(bucket) = self
                    .sprints_by_board
                    .entry(s.board_id)
                    .or_insert_with(|| LoadState::Loaded(Vec::new()))
                else {
                    unreachable!("entry is always seeded as Loaded above")
                };
                bucket.push(s.clone());
            }
        }
    }

    fn absorb_archival_markers(
        &mut self,
        cards: Option<Vec<ArchivedCard>>,
        boards: Option<Vec<ArchivedBoard>>,
    ) {
        self.archived_card_ids = cards.iter().flatten().map(|ac| ac.entity_id).collect();
        self.archived_board_ids = boards.iter().flatten().map(|ab| ab.entity_id).collect();
        self.archived_cards = cards;
        self.archived_boards = boards;
    }

    fn rebuild_card_index(&mut self) {
        self.card_index = self
            .cards
            .loaded_or_empty()
            .iter()
            .enumerate()
            .map(|(i, c)| (c.id, i))
            .collect();
    }

    fn rebuild_board_index(&mut self) {
        self.board_index = self
            .boards
            .loaded_or_empty()
            .iter()
            .enumerate()
            .map(|(i, b)| (b.id, i))
            .collect();
    }
}

mod apply;
mod boards;
mod cards;
mod changed;
mod collections;
mod graph;
mod invalidate;

pub use changed::{DerivedProjections, ModelChanged, NoProjections};

#[cfg(any(test, feature = "test-helpers"))]
mod test_helpers;
#[cfg(any(test, feature = "test-helpers"))]
pub use test_helpers::ModelLoadStates;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Board, Card, Column, Snapshot, Sprint};

    fn make_card(board: &Board, column_id: Uuid) -> Card {
        Card::new(board.id, column_id, "task", 0)
    }

    #[test]
    fn test_default_model_returns_empty_slices() {
        let m = Model::default();
        assert!(m.boards_state().loaded_or_empty().is_empty());
        assert!(m.columns().is_empty());
        assert!(m.cards_state().loaded_or_empty().is_empty());
        assert!(m.sprints().is_empty());
        assert!(m.archived_card_markers().is_empty());
        assert!(m.archived_card_ids().is_empty());
    }

    #[test]
    fn test_load_from_snapshot_populates_boards_and_columns() {
        let mut m = Model::default();
        let board = Board::new("B", None::<String>);
        let col = Column::new(board.id, "Col", 0);
        let _ = m.load_from_snapshot(Snapshot {
            archived_boards: Vec::new(),
            boards: vec![board.clone()],
            columns: vec![col.clone()],
            ..Default::default()
        });
        assert_eq!(m.boards_state().loaded_or_empty().len(), 1);
        assert_eq!(m.boards_state().loaded_or_empty()[0].id, board.id);
        assert_eq!(m.columns().len(), 1);
        assert_eq!(m.columns()[0].id, col.id);
    }

    #[test]
    fn test_load_from_snapshot_overwrites_previous_state() {
        let mut m = Model::default();
        let board_a = Board::new("A", None::<String>);
        let _ = m.load_from_snapshot(Snapshot {
            archived_boards: Vec::new(),
            boards: vec![board_a],
            ..Default::default()
        });
        assert_eq!(m.boards_state().loaded_or_empty().len(), 1);

        let board_b = Board::new("B", None::<String>);
        let board_c = Board::new("C", None::<String>);
        let _ = m.load_from_snapshot(Snapshot {
            archived_boards: Vec::new(),
            boards: vec![board_b, board_c],
            ..Default::default()
        });
        assert_eq!(m.boards_state().loaded_or_empty().len(), 2);
        assert_eq!(m.boards_state().loaded_or_empty()[0].name, "B");
    }

    #[test]
    fn test_load_from_snapshot_clears_stale_card_index() {
        let mut m = Model::default();
        let board = Board::new("B", None::<String>);
        let col_id = Uuid::new_v4();
        let card = make_card(&board, col_id);
        let old_id = card.id;
        let _ = m.load_from_snapshot(Snapshot {
            archived_boards: Vec::new(),
            cards: vec![card],
            ..Default::default()
        });
        assert!(m.card_by_id_state(old_id).loaded().copied().is_some());

        // Reload with no cards — stale index entry must be gone
        let _ = m.load_from_snapshot(Snapshot::default());
        assert!(m.card_by_id_state(old_id).loaded().copied().is_none());
    }

    #[test]
    fn test_load_from_snapshot_populates_board_scoped_column_and_sprint_tiers() {
        let mut m = Model::default();
        let board_a = Board::new("A", None::<String>);
        let board_b = Board::new("B", None::<String>);
        let col_a = Column::new(board_a.id, "Col A", 0);
        let col_b = Column::new(board_b.id, "Col B", 0);
        let sprint_a = Sprint::new(board_a.id, 1, None, None::<String>);
        let sprint_b = Sprint::new(board_b.id, 1, None, None::<String>);
        let _ = m.load_from_snapshot(Snapshot {
            boards: vec![board_a.clone(), board_b.clone()],
            columns: vec![col_a.clone(), col_b.clone()],
            sprints: vec![sprint_a.clone(), sprint_b.clone()],
            ..Default::default()
        });

        let cols_a_state = m.board_columns_state(board_a.id);
        let cols_a = cols_a_state.loaded().unwrap();
        assert_eq!(cols_a.len(), 1);
        assert_eq!(cols_a[0].id, col_a.id);

        let cols_b_state = m.board_columns_state(board_b.id);
        let cols_b = cols_b_state.loaded().unwrap();
        assert_eq!(cols_b.len(), 1);
        assert_eq!(cols_b[0].id, col_b.id);

        let sprints_a_state = m.board_sprints_state(board_a.id);
        let sprints_a = sprints_a_state.loaded().unwrap();
        assert_eq!(sprints_a.len(), 1);
        assert_eq!(sprints_a[0].id, sprint_a.id);

        let sprints_b_state = m.board_sprints_state(board_b.id);
        let sprints_b = sprints_b_state.loaded().unwrap();
        assert_eq!(sprints_b.len(), 1);
        assert_eq!(sprints_b[0].id, sprint_b.id);
    }

    #[test]
    fn test_load_from_snapshot_marks_a_board_with_no_columns_loaded_empty() {
        let mut m = Model::default();
        let board = Board::new("B", None::<String>);
        let _ = m.load_from_snapshot(Snapshot {
            boards: vec![board.clone()],
            ..Default::default()
        });

        let cols_state = m.board_columns_state(board.id);
        assert!(cols_state.loaded().unwrap().is_empty());
        let sprints_state = m.board_sprints_state(board.id);
        assert!(sprints_state.loaded().unwrap().is_empty());
    }

    #[test]
    fn test_board_scoped_tiers_stay_not_loaded_for_a_board_absent_from_the_snapshot() {
        let mut m = Model::default();
        let board_a = Board::new("A", None::<String>);
        let _ = m.load_from_snapshot(Snapshot {
            boards: vec![board_a],
            ..Default::default()
        });

        let other = Uuid::new_v4();
        assert!(m.board_columns_state(other).is_not_loaded());
        assert!(m.board_sprints_state(other).is_not_loaded());
    }

    #[test]
    fn test_load_from_snapshot_replaces_stale_board_scoped_tiers() {
        let mut m = Model::default();
        let board_a = Board::new("A", None::<String>);
        let col_a1 = Column::new(board_a.id, "Col A1", 0);
        let _ = m.load_from_snapshot(Snapshot {
            boards: vec![board_a.clone()],
            columns: vec![col_a1.clone()],
            ..Default::default()
        });
        let loaded_state = m.board_columns_state(board_a.id);
        let loaded = loaded_state.loaded().unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].id, col_a1.id);

        let board_b = Board::new("B", None::<String>);
        let col_a2 = Column::new(board_a.id, "Col A2", 0);
        let col_b1 = Column::new(board_b.id, "Col B1", 0);
        let _ = m.load_from_snapshot(Snapshot {
            boards: vec![board_a.clone(), board_b.clone()],
            columns: vec![col_a2.clone(), col_b1.clone()],
            ..Default::default()
        });

        let loaded_a_state = m.board_columns_state(board_a.id);
        let loaded_a = loaded_a_state.loaded().unwrap();
        assert_eq!(loaded_a.len(), 1);
        assert_eq!(loaded_a[0].id, col_a2.id);
        assert!(!loaded_a.iter().any(|c| c.id == col_a1.id));

        let loaded_b_state = m.board_columns_state(board_b.id);
        let loaded_b = loaded_b_state.loaded().unwrap();
        assert_eq!(loaded_b.len(), 1);
        assert_eq!(loaded_b[0].id, col_b1.id);
    }

    #[test]
    fn test_load_from_snapshot_returns_a_model_changed_receipt() {
        let mut m = Model::default();
        let changed: ModelChanged = m.load_from_snapshot(Snapshot::default());
        assert!(m.cards_state().is_loaded());
        NoProjections.resync(&m, changed);
    }

    #[test]
    fn test_model_has_no_collapsing_boards_or_graph_accessors() {
        let boards_src = include_str!("boards.rs");
        let graph_src = include_str!("graph.rs");
        assert!(
            !boards_src.contains("pub fn boards(&self)"),
            "Model::boards must be deleted; callers should use boards_state().loaded_or_empty()"
        );
        assert!(
            !boards_src.contains("pub fn board_by_id(&self,"),
            "Model::board_by_id must be deleted; callers should use board_by_id_state(id).loaded().copied()"
        );
        assert!(
            !graph_src.contains("pub fn graph(&self)"),
            "Model::graph must be deleted; callers should use graph_state().loaded().unwrap_or_else(|| Model::empty_graph())"
        );
    }

    #[test]
    fn test_model_has_no_collapsing_card_accessors() {
        let cards_src = include_str!("cards.rs");
        assert!(
            !cards_src.contains("pub fn all_cards(&self)"),
            "Model::all_cards must be deleted; callers should use cards_state().loaded_or_empty()"
        );
        assert!(
            !cards_src.contains("pub fn card_by_id(&self,"),
            "Model::card_by_id must be deleted; callers should use card_by_id_state(id).loaded().copied()"
        );
    }

    #[test]
    fn test_model_has_no_collapsing_column_or_sprint_accessors() {
        let collections_src = include_str!("collections.rs");
        assert!(
            !collections_src.contains("pub fn columns(&self)"),
            "Model::columns must be deleted; callers should use columns_state().loaded_or_empty()"
        );
        assert!(
            !collections_src.contains("pub fn sprints(&self)"),
            "Model::sprints must be deleted; callers should use sprints_state().loaded_or_empty()"
        );
    }
}
