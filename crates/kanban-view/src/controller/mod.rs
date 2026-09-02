use chrono::{DateTime, Utc};
use kanban_domain::{
    Board, BoardSortField, Card, DerivedProjections, LoadState, Model, ModelChanged, SortOrder,
    DEFAULT_ARCHIVED_BOARD_SORT, DEFAULT_BOARD_SORT_LIVE,
};
use std::collections::HashMap;
use uuid::Uuid;

mod board_sort;
mod partitions;

/// Presentation state derived from a [`Model`] plus session-scoped choices the
/// backend never supplied. The `Model` holds entities and load state only;
/// sort, partitions and the archived-at side map are the `Controller`'s.
#[derive(Debug)]
pub struct Controller {
    // Live/archived partitions of the Model's unified `cards`/`boards`
    // collections, computed ONCE in `resync` and served as a borrow by
    // `displayed_cards`/`displayed_boards`. This is the concrete
    // no-per-frame-recompute fix: the projects/tasks panels borrow the cached
    // subset every redraw instead of re-filtering+cloning per frame.
    displayed_cards_live: LoadState<Vec<Card>>,
    displayed_cards_archived: LoadState<Vec<Card>>,
    displayed_boards_live: LoadState<Vec<Board>>,
    displayed_boards_archived: LoadState<Vec<Board>>,
    // archived_at timestamps keyed by board id, REBUILT from the Model's
    // archival markers on every `resync`. The board head does NOT carry
    // archived_at (it stays live under the reference-marker model), so recency
    // sorting needs this side map.
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
}

impl Default for Controller {
    fn default() -> Self {
        Self {
            displayed_cards_live: LoadState::NotLoaded,
            displayed_cards_archived: LoadState::NotLoaded,
            displayed_boards_live: LoadState::NotLoaded,
            displayed_boards_archived: LoadState::NotLoaded,
            archived_board_at: HashMap::new(),
            live_board_sort_field: DEFAULT_BOARD_SORT_LIVE.0,
            live_board_sort_order: DEFAULT_BOARD_SORT_LIVE.1,
            archived_board_sort_field: DEFAULT_ARCHIVED_BOARD_SORT.0,
            archived_board_sort_order: DEFAULT_ARCHIVED_BOARD_SORT.1,
        }
    }
}

impl DerivedProjections for Controller {
    fn resync(&mut self, model: &Model, _changed: ModelChanged) {
        self.archived_board_at = model
            .archived_boards()
            .iter()
            .map(|ab| (ab.entity_id, ab.metadata.archived_at))
            .collect();
        self.rebuild_card_partitions(model);
        self.rebuild_board_partitions(model);
    }
}

impl Controller {
    /// The cards the tasks panel should display, selected by `want_archived`:
    /// the archived subset when a confirm dialog / the archived-cards view is
    /// active, the live subset otherwise. Returns a BORROW of the partition
    /// cached on the last [`resync`](kanban_domain::DerivedProjections::resync)
    /// — no per-frame filter or clone.
    pub fn displayed_cards(&self, want_archived: bool) -> LoadState<&[Card]> {
        if want_archived {
            self.displayed_cards_archived.as_ref().map(Vec::as_slice)
        } else {
            self.displayed_cards_live.as_ref().map(Vec::as_slice)
        }
    }

    /// The boards the projects panel should display, selected by
    /// `want_archived`. Borrow of the partition cached on
    /// [`resync`](kanban_domain::DerivedProjections::resync); the mode decision (live vs archived) lives at the
    /// `App` accessor, which passes the stack-aware base mode in.
    pub fn displayed_boards(&self, want_archived: bool) -> LoadState<&[Board]> {
        if want_archived {
            self.displayed_boards_archived.as_ref().map(Vec::as_slice)
        } else {
            self.displayed_boards_live.as_ref().map(Vec::as_slice)
        }
    }

    /// The live cards — the common case for anything rendering to the user.
    /// Thin wrapper over the cached live/archived partition.
    pub fn live_cards(&self) -> LoadState<&[Card]> {
        self.displayed_cards(false)
    }

    /// The archived cards, as full `Card` entities (not the marker records —
    /// see `Model::archived_card_markers` for those).
    pub fn archived_cards(&self) -> LoadState<&[Card]> {
        self.displayed_cards(true)
    }

    /// The ARCHIVED heads in the CONFIGURED archived-boards order (default
    /// archived_at DESC — newest first). This is what the ArchivedBoardsView
    /// renders AND what its restore / permanent-delete affordances index into:
    /// both read this same cached, sorted partition so the rendered row and the
    /// selected id stay consistent under any sort.
    pub fn archived_boards_view(&self) -> LoadState<&[Board]> {
        self.displayed_boards(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kanban_domain::{
        resolved::Collection, ArchiveMetadata, ArchivedBoard, ArchivedCard, Column, LoadState,
        NoContext, Resolved, Snapshot,
    };

    fn seed_board(name: &str, position: i32) -> Board {
        let mut b = Board::new(name, None::<String>);
        b.position = position;
        b
    }

    fn archived_board_marker(id: Uuid, at: &str) -> ArchivedBoard {
        ArchivedBoard {
            entity_id: id,
            metadata: ArchiveMetadata {
                archived_at: chrono::DateTime::parse_from_rfc3339(at)
                    .unwrap()
                    .with_timezone(&Utc),
            },
            context: NoContext {},
        }
    }

    #[test]
    fn test_sync_partitions_cards_by_the_archived_markers() {
        let board = seed_board("B", 0);
        let column = Column::new(board.id, "Col", 0);
        let live = Card::new(board.id, column.id, "live", 0);
        let archived = Card::new(board.id, column.id, "archived", 1);
        let (live_id, archived_id) = (live.id, archived.id);
        let mut model = Model::default();
        let changed = model.load_from_snapshot(Snapshot {
            boards: vec![board],
            columns: vec![column],
            cards: vec![live, archived],
            archived_cards: vec![ArchivedCard::new(archived_id, Uuid::nil())],
            archived_boards: Vec::new(),
            ..Default::default()
        });
        let mut controller = Controller::default();
        controller.resync(&model, changed);

        let live_ids: Vec<Uuid> = controller
            .displayed_cards(false)
            .loaded()
            .copied()
            .unwrap_or(&[])
            .iter()
            .map(|c| c.id)
            .collect();
        let archived_ids: Vec<Uuid> = controller
            .displayed_cards(true)
            .loaded()
            .copied()
            .unwrap_or(&[])
            .iter()
            .map(|c| c.id)
            .collect();
        assert_eq!(live_ids, vec![live_id]);
        assert_eq!(archived_ids, vec![archived_id]);
    }

    #[test]
    fn test_sync_partitions_boards_by_the_archived_markers() {
        let live = seed_board("Live", 0);
        let archived = seed_board("Archived", 1);
        let (live_id, archived_id) = (live.id, archived.id);
        let mut model = Model::default();
        let changed = model.load_from_snapshot(Snapshot {
            boards: vec![live, archived],
            archived_boards: vec![archived_board_marker(archived_id, "2026-01-01T00:00:00Z")],
            ..Default::default()
        });
        let mut controller = Controller::default();
        controller.resync(&model, changed);

        let live_ids: Vec<Uuid> = controller
            .displayed_boards(false)
            .loaded()
            .copied()
            .unwrap_or(&[])
            .iter()
            .map(|b| b.id)
            .collect();
        let archived_ids: Vec<Uuid> = controller
            .displayed_boards(true)
            .loaded()
            .copied()
            .unwrap_or(&[])
            .iter()
            .map(|b| b.id)
            .collect();
        assert_eq!(live_ids, vec![live_id]);
        assert_eq!(archived_ids, vec![archived_id]);
    }

    #[test]
    fn test_sync_rebuilds_the_partitions_after_apply_resolved() {
        let board = seed_board("B", 0);
        let column = Column::new(board.id, "Col", 0);
        let live = Card::new(board.id, column.id, "live", 0);
        let archived = Card::new(board.id, column.id, "archived", 1);
        let live_id = live.id;
        let archived_id = archived.id;
        let mut model = Model::default();
        let changed = model.load_from_snapshot(Snapshot {
            boards: vec![board.clone()],
            columns: vec![column.clone()],
            cards: vec![live, archived],
            archived_cards: vec![ArchivedCard::new(archived_id, Uuid::nil())],
            archived_boards: Vec::new(),
            ..Default::default()
        });
        let mut controller = Controller::default();
        controller.resync(&model, changed);
        assert_eq!(
            controller
                .displayed_cards(false)
                .loaded()
                .copied()
                .unwrap_or(&[])
                .len(),
            1
        );

        let mut live_edited = Card::new(board.id, column.id, "live edited", 0);
        live_edited.id = live_id;
        let mut archived_edited = Card::new(board.id, column.id, "archived edited", 1);
        archived_edited.id = archived_id;
        let extra = Card::new(board.id, column.id, "extra", 2);
        let extra_id = extra.id;

        let changed = model.apply_resolved(Resolved {
            cards: Collection {
                all: LoadState::Loaded(vec![live_edited, archived_edited, extra]),
                ..Default::default()
            },
            ..Default::default()
        });
        controller.resync(&model, changed);

        let live_ids: Vec<Uuid> = controller
            .displayed_cards(false)
            .loaded()
            .copied()
            .unwrap_or(&[])
            .iter()
            .map(|c| c.id)
            .collect();
        let archived_ids: Vec<Uuid> = controller
            .displayed_cards(true)
            .loaded()
            .copied()
            .unwrap_or(&[])
            .iter()
            .map(|c| c.id)
            .collect();
        assert_eq!(live_ids, vec![live_id, extra_id]);
        assert_eq!(archived_ids, vec![archived_id]);
        assert_eq!(
            controller
                .displayed_cards(false)
                .loaded()
                .copied()
                .unwrap_or(&[])[0]
                .title,
            "live edited"
        );
    }

    #[test]
    fn test_a_projections_implementor_cannot_mint_a_receipt() {
        let src = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../kanban-domain/src/model/changed.rs"
        ))
        .expect("kanban-domain changed.rs must be readable");
        let prod = src.split("#[cfg(test)]").next().unwrap();
        let model_changed_block = prod.split("pub struct ModelChanged").next().unwrap();
        assert!(prod.contains("pub(crate) fn new()"));
        assert!(!prod.contains("pub fn new("));
        assert!(!model_changed_block.contains("derive(Debug, Default)"));
    }

    #[test]
    fn test_resync_consumes_the_receipt_from_apply_resolved() {
        let board = seed_board("B", 0);
        let column = Column::new(board.id, "Col", 0);
        let live = Card::new(board.id, column.id, "live", 0);
        let archived = Card::new(board.id, column.id, "archived", 1);
        let live_id = live.id;
        let archived_id = archived.id;
        let mut model = Model::default();
        let changed = model.load_from_snapshot(Snapshot {
            boards: vec![board.clone()],
            columns: vec![column.clone()],
            cards: vec![live, archived],
            archived_cards: vec![ArchivedCard::new(archived_id, Uuid::nil())],
            archived_boards: Vec::new(),
            ..Default::default()
        });
        let mut controller = Controller::default();
        controller.resync(&model, changed);
        assert_eq!(
            controller
                .displayed_cards(false)
                .loaded()
                .copied()
                .unwrap_or(&[])
                .len(),
            1
        );

        let mut live_edited = Card::new(board.id, column.id, "live edited", 0);
        live_edited.id = live_id;
        let mut archived_edited = Card::new(board.id, column.id, "archived edited", 1);
        archived_edited.id = archived_id;
        let extra = Card::new(board.id, column.id, "extra", 2);
        let extra_id = extra.id;

        let changed = model.apply_resolved(Resolved {
            cards: Collection {
                all: LoadState::Loaded(vec![live_edited, archived_edited, extra]),
                ..Default::default()
            },
            ..Default::default()
        });
        controller.resync(&model, changed);

        let live_ids: Vec<Uuid> = controller
            .displayed_cards(false)
            .loaded()
            .copied()
            .unwrap_or(&[])
            .iter()
            .map(|c| c.id)
            .collect();
        let archived_ids: Vec<Uuid> = controller
            .displayed_cards(true)
            .loaded()
            .copied()
            .unwrap_or(&[])
            .iter()
            .map(|c| c.id)
            .collect();
        assert_eq!(live_ids, vec![live_id, extra_id]);
        assert_eq!(archived_ids, vec![archived_id]);
    }

    #[test]
    fn test_a_freshly_defaulted_controller_reports_its_partitions_not_loaded() {
        let controller = Controller::default();
        assert!(controller.displayed_cards(true).is_not_loaded());
        assert!(controller.displayed_cards(false).is_not_loaded());
        assert!(controller.displayed_boards(true).is_not_loaded());
        assert!(controller.displayed_boards(false).is_not_loaded());
    }

    #[test]
    fn test_resync_over_a_not_loaded_model_leaves_the_partitions_not_loaded() {
        let mut model = Model::default();
        let changed = model.apply_resolved(Resolved::default());
        let mut controller = Controller::default();
        controller.resync(&model, changed);

        assert!(controller.displayed_cards(false).is_not_loaded());
        assert!(controller.displayed_cards(true).is_not_loaded());
        assert!(controller.displayed_boards(false).is_not_loaded());
        assert!(controller.displayed_boards(true).is_not_loaded());
    }

    #[test]
    fn test_resync_over_a_loaded_empty_model_reports_loaded_empty() {
        let mut model = Model::default();
        let changed = model.apply_resolved(Resolved {
            cards: Collection {
                all: LoadState::Loaded(Vec::new()),
                ..Default::default()
            },
            boards: Collection {
                all: LoadState::Loaded(Vec::new()),
                ..Default::default()
            },
            ..Default::default()
        });
        let mut controller = Controller::default();
        controller.resync(&model, changed);

        assert!(matches!(controller.displayed_cards(false), LoadState::Loaded(v) if v.is_empty()));
        assert!(matches!(controller.displayed_cards(true), LoadState::Loaded(v) if v.is_empty()));
        assert!(matches!(controller.displayed_boards(false), LoadState::Loaded(v) if v.is_empty()));
        assert!(matches!(controller.displayed_boards(true), LoadState::Loaded(v) if v.is_empty()));
    }

    #[test]
    fn test_resync_propagates_failed_from_the_model_into_both_partitions() {
        let mut model = Model::default();
        let changed = model.apply_resolved(Resolved {
            cards: Collection {
                all: LoadState::Failed(std::sync::Arc::new(
                    kanban_domain::KanbanError::unsupported("x"),
                )),
                ..Default::default()
            },
            ..Default::default()
        });
        let mut controller = Controller::default();
        controller.resync(&model, changed);

        assert!(controller.displayed_cards(false).is_failed());
        assert!(controller.displayed_cards(true).is_failed());
    }

    #[test]
    fn test_sync_rebuilds_the_partitions_after_the_model_changes() {
        let board = seed_board("B", 0);
        let column = Column::new(board.id, "Col", 0);
        let card_a = Card::new(board.id, column.id, "a", 0);
        let card_b = Card::new(board.id, column.id, "b", 1);
        let mut model = Model::default();
        let changed = model.load_from_snapshot(Snapshot {
            boards: vec![board.clone()],
            columns: vec![column.clone()],
            cards: vec![card_a.clone()],
            archived_boards: Vec::new(),
            ..Default::default()
        });
        let mut controller = Controller::default();
        controller.resync(&model, changed);
        assert_eq!(
            controller
                .displayed_cards(false)
                .loaded()
                .copied()
                .unwrap_or(&[])
                .len(),
            1
        );

        let changed = model.load_from_snapshot(Snapshot {
            boards: vec![board],
            columns: vec![column],
            cards: vec![card_a, card_b],
            archived_boards: Vec::new(),
            ..Default::default()
        });
        controller.resync(&model, changed);
        assert_eq!(
            controller
                .displayed_cards(false)
                .loaded()
                .copied()
                .unwrap_or(&[])
                .len(),
            2
        );
    }

    #[test]
    fn test_sync_derives_archived_board_at_from_the_markers() {
        let first = seed_board("First", 0);
        let second = seed_board("Second", 1);
        let (first_id, second_id) = (first.id, second.id);
        let mut model = Model::default();
        let changed = model.load_from_snapshot(Snapshot {
            boards: vec![first, second],
            archived_boards: vec![
                archived_board_marker(first_id, "2026-01-01T00:00:00Z"),
                archived_board_marker(second_id, "2026-06-01T00:00:00Z"),
            ],
            ..Default::default()
        });
        let mut controller = Controller::default();
        controller.resync(&model, changed);

        let order: Vec<Uuid> = controller
            .archived_boards_view()
            .loaded()
            .copied()
            .unwrap_or(&[])
            .iter()
            .map(|b| b.id)
            .collect();
        assert_eq!(order, vec![second_id, first_id]);
    }
}
