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
    // The board the card partitions are scoped to. A scope change is a
    // staleness source independent of `ModelChanged`: `set_scope_board`
    // rebuilds the card partitions itself rather than waiting for the next
    // `resync`, which may otherwise skip the rebuild on an unchanged receipt.
    scope_board: Option<Uuid>,
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
            scope_board: None,
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
    fn resync(&mut self, model: &Model, changed: ModelChanged) {
        if !changed.any() {
            return;
        }
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

    /// Sets the board the card partitions are scoped to and rebuilds them
    /// immediately against `model`. A no-op when `board_id` already matches
    /// the current scope.
    pub fn set_scope_board(&mut self, board_id: Option<Uuid>, model: &Model) {
        if self.scope_board == board_id {
            return;
        }
        self.scope_board = board_id;
        self.rebuild_card_partitions(model);
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
        let board_id = board.id;
        let column = Column::new(board.id, "Col", 0);
        let live = Card::new(board.id, column.id, "live", 0);
        let archived = Card::new(board.id, column.id, "archived", 1);
        let (live_id, archived_id) = (live.id, archived.id);
        let mut model = Model::default();
        let changed = model.load_from_snapshot(Snapshot {
            boards: vec![board],
            columns: vec![column],
            cards: vec![live, archived],
            archived_cards: vec![ArchivedCard::new(archived_id, board_id)],
            archived_boards: Vec::new(),
            ..Default::default()
        });
        let mut controller = Controller::default();
        controller.set_scope_board(Some(board_id), &model);
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
        let board_id = board.id;
        let column = Column::new(board.id, "Col", 0);
        let column_id = column.id;
        let live = Card::new(board.id, column.id, "live", 0);
        let archived = Card::new(board.id, column.id, "archived", 1);
        let live_id = live.id;
        let archived_id = archived.id;
        let mut model = Model::default();
        let changed = model.load_from_snapshot(Snapshot {
            boards: vec![board.clone()],
            columns: vec![column.clone()],
            cards: vec![live, archived],
            archived_cards: vec![ArchivedCard::new(archived_id, board_id)],
            archived_boards: Vec::new(),
            ..Default::default()
        });
        let mut controller = Controller::default();
        controller.set_scope_board(Some(board_id), &model);
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

        let mut cards_by_id = std::collections::HashMap::new();
        cards_by_id.insert(archived_id, LoadState::Loaded(archived_edited));
        let mut cards_by_parent = std::collections::HashMap::new();
        cards_by_parent.insert(column_id, LoadState::Loaded(vec![live_edited, extra]));
        let changed = model.apply_resolved(Resolved {
            cards: Collection {
                by_id: cards_by_id,
                by_parent: cards_by_parent,
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

    fn seed_non_empty_model() -> (Model, Controller, Uuid, Uuid) {
        let board = seed_board("B", 0);
        let board_id = board.id;
        let column = Column::new(board.id, "Col", 0);
        let column_id = column.id;
        let live = Card::new(board.id, column.id, "live", 0);
        let archived = Card::new(board.id, column.id, "archived", 1);
        let archived_id = archived.id;
        let mut model = Model::default();
        let changed = model.load_from_snapshot(Snapshot {
            boards: vec![board],
            columns: vec![column],
            cards: vec![live, archived],
            archived_cards: vec![ArchivedCard::new(archived_id, board_id)],
            archived_boards: Vec::new(),
            ..Default::default()
        });
        let mut controller = Controller::default();
        controller.set_scope_board(Some(board_id), &model);
        controller.resync(&model, changed);
        (model, controller, board_id, column_id)
    }

    #[test]
    fn test_resync_with_an_unchanged_receipt_leaves_the_cached_partitions_untouched() {
        let (mut model, mut controller, _board_id, _column_id) = seed_non_empty_model();
        let cards_before = controller
            .displayed_cards(false)
            .loaded()
            .copied()
            .unwrap()
            .as_ptr();
        let boards_before = controller
            .displayed_boards(false)
            .loaded()
            .copied()
            .unwrap()
            .as_ptr();

        let unchanged = model.apply_resolved(Resolved::default());
        assert!(!unchanged.any());
        controller.resync(&model, unchanged);

        assert_eq!(
            controller
                .displayed_cards(false)
                .loaded()
                .copied()
                .unwrap()
                .as_ptr(),
            cards_before
        );
        assert_eq!(
            controller
                .displayed_boards(false)
                .loaded()
                .copied()
                .unwrap()
                .as_ptr(),
            boards_before
        );
    }

    #[test]
    fn test_resync_with_a_changed_receipt_rebuilds_the_partitions() {
        let (mut model, mut controller, board_id, column_id) = seed_non_empty_model();
        let cards_before = controller
            .displayed_cards(false)
            .loaded()
            .copied()
            .unwrap()
            .as_ptr();

        let live_id = model.board_cards_state(board_id).loaded().unwrap()[0].id;
        let mut live_edited = Card::new(board_id, column_id, "live edited", 0);
        live_edited.id = live_id;
        let mut cards_by_parent = std::collections::HashMap::new();
        cards_by_parent.insert(column_id, LoadState::Loaded(vec![live_edited.clone()]));
        let changed = model.apply_resolved(Resolved {
            cards: Collection {
                by_parent: cards_by_parent,
                ..Default::default()
            },
            ..Default::default()
        });
        assert!(changed.any());
        controller.resync(&model, changed);

        let cards_after = controller.displayed_cards(false).loaded().copied().unwrap();
        assert_ne!(cards_after.as_ptr(), cards_before);
        assert_eq!(cards_after[0].title, "live edited");
    }

    #[test]
    fn test_resync_after_a_whole_model_replacement_rebuilds_the_partitions() {
        let board_a1 = seed_board("A1", 0);
        let board_a2 = seed_board("A2", 1);
        let mut model_a = Model::default();
        let changed = model_a.load_from_snapshot(Snapshot {
            boards: vec![board_a1, board_a2],
            archived_boards: Vec::new(),
            ..Default::default()
        });
        let mut controller = Controller::default();
        controller.resync(&model_a, changed);
        assert_eq!(
            controller
                .displayed_boards(false)
                .loaded()
                .copied()
                .unwrap()
                .len(),
            2
        );

        let board_b = seed_board("B", 0);
        let board_b_id = board_b.id;
        let mut model_b = Model::default();
        let _ = model_b.load_from_snapshot(Snapshot {
            boards: vec![board_b],
            archived_boards: Vec::new(),
            ..Default::default()
        });

        let changed = model_a.replace_with(model_b);
        assert!(changed.any());
        controller.resync(&model_a, changed);

        let boards: Vec<Uuid> = controller
            .displayed_boards(false)
            .loaded()
            .copied()
            .unwrap()
            .iter()
            .map(|b| b.id)
            .collect();
        assert_eq!(boards, vec![board_b_id]);
    }

    #[test]
    fn test_board_sort_change_after_a_skipped_resync_still_reorders() {
        let mut zed = Board::new("Zed", None::<String>);
        zed.position = 0;
        let mut alpha = Board::new("Alpha", None::<String>);
        alpha.position = 1;
        let zed_id = zed.id;
        let alpha_id = alpha.id;
        let mut model = Model::default();
        let changed = model.load_from_snapshot(Snapshot {
            boards: vec![zed, alpha],
            archived_boards: Vec::new(),
            ..Default::default()
        });
        let mut controller = Controller::default();
        controller.resync(&model, changed);

        let unchanged = model.apply_resolved(Resolved::default());
        assert!(!unchanged.any());
        controller.resync(&model, unchanged);

        controller.set_board_sort(false, BoardSortField::Name, SortOrder::Descending);
        let order: Vec<Uuid> = controller
            .displayed_boards(false)
            .loaded()
            .copied()
            .unwrap()
            .iter()
            .map(|b| b.id)
            .collect();
        assert_eq!(order, vec![zed_id, alpha_id]);
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
        assert!(prod.contains("pub(crate) fn unchanged()"));
        assert!(
            !prod.contains("pub fn unchanged("),
            "a pub unchanged() would let an external caller mint a resync-skipping receipt"
        );
        assert!(!model_changed_block.contains("derive(Debug, Default)"));
    }

    #[test]
    fn test_resync_consumes_the_receipt_from_apply_resolved() {
        let board = seed_board("B", 0);
        let board_id = board.id;
        let column = Column::new(board.id, "Col", 0);
        let column_id = column.id;
        let live = Card::new(board.id, column.id, "live", 0);
        let archived = Card::new(board.id, column.id, "archived", 1);
        let live_id = live.id;
        let archived_id = archived.id;
        let mut model = Model::default();
        let changed = model.load_from_snapshot(Snapshot {
            boards: vec![board.clone()],
            columns: vec![column.clone()],
            cards: vec![live, archived],
            archived_cards: vec![ArchivedCard::new(archived_id, board_id)],
            archived_boards: Vec::new(),
            ..Default::default()
        });
        let mut controller = Controller::default();
        controller.set_scope_board(Some(board_id), &model);
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

        let mut cards_by_id = std::collections::HashMap::new();
        cards_by_id.insert(archived_id, LoadState::Loaded(archived_edited));
        let mut cards_by_parent = std::collections::HashMap::new();
        cards_by_parent.insert(column_id, LoadState::Loaded(vec![live_edited, extra]));
        let changed = model.apply_resolved(Resolved {
            cards: Collection {
                by_id: cards_by_id,
                by_parent: cards_by_parent,
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
    fn test_a_freshly_defaulted_controller_reports_live_archived_shorthands_not_loaded() {
        let controller = Controller::default();
        assert!(controller.live_cards().is_not_loaded());
        assert!(controller.archived_cards().is_not_loaded());
        assert!(controller.archived_boards_view().is_not_loaded());
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
        let board_id = Uuid::new_v4();
        let mut model = Model::default();
        let mut columns_by_parent = std::collections::HashMap::new();
        columns_by_parent.insert(board_id, LoadState::Loaded(Vec::new()));
        let mut archived_by_parent = std::collections::HashMap::new();
        archived_by_parent.insert(board_id, LoadState::Loaded(Vec::new()));
        let changed = model.apply_resolved(Resolved {
            columns: Collection {
                by_parent: columns_by_parent,
                ..Default::default()
            },
            boards: Collection {
                all: LoadState::Loaded(Vec::new()),
                ..Default::default()
            },
            archived_cards: Collection {
                by_parent: archived_by_parent,
                ..Default::default()
            },
            archived_boards: Collection {
                all: LoadState::Loaded(Vec::new()),
                ..Default::default()
            },
            ..Default::default()
        });
        let mut controller = Controller::default();
        controller.set_scope_board(Some(board_id), &model);
        controller.resync(&model, changed);

        assert!(matches!(controller.displayed_cards(false), LoadState::Loaded(v) if v.is_empty()));
        assert!(matches!(controller.displayed_cards(true), LoadState::Loaded(v) if v.is_empty()));
        assert!(matches!(controller.displayed_boards(false), LoadState::Loaded(v) if v.is_empty()));
        assert!(matches!(controller.displayed_boards(true), LoadState::Loaded(v) if v.is_empty()));
    }

    #[test]
    fn test_card_partitions_report_not_loaded_when_the_by_board_marker_tier_is_not_loaded() {
        let board = seed_board("B", 0);
        let board_id = board.id;
        let column = Column::new(board.id, "Col", 0);
        let column_id = column.id;
        let card = Card::new(board.id, column.id, "card", 0);
        let card_id = card.id;
        let mut model = Model::default();
        let mut columns_by_parent = std::collections::HashMap::new();
        columns_by_parent.insert(board_id, LoadState::Loaded(vec![column.clone()]));
        let mut cards_by_parent = std::collections::HashMap::new();
        cards_by_parent.insert(column_id, LoadState::Loaded(vec![card.clone()]));
        let changed = model.apply_resolved(Resolved {
            columns: Collection {
                by_parent: columns_by_parent,
                ..Default::default()
            },
            cards: Collection {
                by_parent: cards_by_parent,
                ..Default::default()
            },
            ..Default::default()
        });
        let mut controller = Controller::default();
        controller.set_scope_board(Some(board_id), &model);
        controller.resync(&model, changed);

        assert!(!controller.displayed_cards(true).is_loaded());
        assert!(controller.displayed_cards(true).is_not_loaded());
        assert!(controller.displayed_cards(false).is_loaded());
        let live_ids: Vec<Uuid> = controller
            .displayed_cards(false)
            .loaded()
            .copied()
            .unwrap_or(&[])
            .iter()
            .map(|c| c.id)
            .collect();
        assert_eq!(live_ids, vec![card_id]);

        let changed = model.load_from_snapshot(Snapshot {
            boards: vec![board],
            columns: vec![column],
            cards: vec![card],
            archived_cards: Vec::new(),
            archived_boards: Vec::new(),
            ..Default::default()
        });
        controller.set_scope_board(Some(board_id), &model);
        controller.resync(&model, changed);
        assert!(controller.displayed_cards(true).is_loaded());
    }

    #[test]
    fn test_board_partitions_report_not_loaded_when_markers_not_loaded_and_boards_loaded() {
        let board = seed_board("B", 0);
        let board_id = board.id;
        let mut model = Model::default();
        let changed = model.apply_resolved(Resolved {
            boards: Collection {
                all: LoadState::Loaded(vec![board]),
                ..Default::default()
            },
            ..Default::default()
        });
        let mut controller = Controller::default();
        controller.resync(&model, changed);

        assert!(controller.displayed_boards(true).is_not_loaded());
        assert!(controller.displayed_boards(false).is_loaded());
        let live_ids: Vec<Uuid> = controller
            .displayed_boards(false)
            .loaded()
            .copied()
            .unwrap_or(&[])
            .iter()
            .map(|b| b.id)
            .collect();
        assert_eq!(live_ids, vec![board_id]);
    }

    #[test]
    fn test_card_partitions_report_failed_when_the_by_board_marker_tier_failed() {
        let board = seed_board("B", 0);
        let board_id = board.id;
        let column = Column::new(board.id, "Col", 0);
        let column_id = column.id;
        let card = Card::new(board.id, column.id, "card", 0);
        let mut model = Model::default();
        let mut columns_by_parent = std::collections::HashMap::new();
        columns_by_parent.insert(board_id, LoadState::Loaded(vec![column]));
        let mut cards_by_parent = std::collections::HashMap::new();
        cards_by_parent.insert(column_id, LoadState::Loaded(vec![card]));
        let mut archived_by_parent = std::collections::HashMap::new();
        archived_by_parent.insert(
            board_id,
            LoadState::Failed(std::sync::Arc::new(
                kanban_domain::KanbanError::unsupported("boom"),
            )),
        );
        let changed = model.apply_resolved(Resolved {
            columns: Collection {
                by_parent: columns_by_parent,
                ..Default::default()
            },
            cards: Collection {
                by_parent: cards_by_parent,
                ..Default::default()
            },
            archived_cards: Collection {
                by_parent: archived_by_parent,
                ..Default::default()
            },
            ..Default::default()
        });
        let mut controller = Controller::default();
        controller.set_scope_board(Some(board_id), &model);
        controller.resync(&model, changed);

        assert!(controller.displayed_cards(true).is_failed());
        assert!(controller.displayed_cards(false).is_loaded());
    }

    #[test]
    fn test_board_partitions_report_failed_when_marker_tier_failed() {
        let board = seed_board("B", 0);
        let mut model = Model::default();
        let changed = model.apply_resolved(Resolved {
            boards: Collection {
                all: LoadState::Loaded(vec![board]),
                ..Default::default()
            },
            archived_boards: Collection {
                all: LoadState::Failed(std::sync::Arc::new(
                    kanban_domain::KanbanError::unsupported("boom"),
                )),
                ..Default::default()
            },
            ..Default::default()
        });
        let mut controller = Controller::default();
        controller.resync(&model, changed);

        assert!(controller.displayed_boards(true).is_failed());
        assert!(controller.displayed_boards(false).is_loaded());
    }

    #[test]
    fn test_resync_propagates_failed_from_the_model_into_both_partitions() {
        let board_id = Uuid::new_v4();
        let mut model = Model::default();
        let mut columns_by_parent = std::collections::HashMap::new();
        columns_by_parent.insert(
            board_id,
            LoadState::Failed(std::sync::Arc::new(
                kanban_domain::KanbanError::unsupported("x"),
            )),
        );
        let mut archived_by_parent = std::collections::HashMap::new();
        archived_by_parent.insert(
            board_id,
            LoadState::Failed(std::sync::Arc::new(
                kanban_domain::KanbanError::unsupported("x"),
            )),
        );
        let changed = model.apply_resolved(Resolved {
            columns: Collection {
                by_parent: columns_by_parent,
                ..Default::default()
            },
            archived_cards: Collection {
                by_parent: archived_by_parent,
                ..Default::default()
            },
            ..Default::default()
        });
        let mut controller = Controller::default();
        controller.set_scope_board(Some(board_id), &model);
        controller.resync(&model, changed);

        assert!(controller.displayed_cards(false).is_failed());
        assert!(controller.displayed_cards(true).is_failed());
    }

    #[test]
    fn test_sync_rebuilds_the_partitions_after_the_model_changes() {
        let board = seed_board("B", 0);
        let board_id = board.id;
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
        controller.set_scope_board(Some(board_id), &model);
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

    #[test]
    fn test_scoped_loaded_model_renders_live_partition() {
        let mut model = Model::default();

        let board = seed_board("B", 0);
        let board_id = board.id;
        let column = Column::new(board_id, "Col", 0);
        let column_id = column.id;
        let card = Card::new(board_id, column_id, "card", 0);

        let mut columns_by_parent = std::collections::HashMap::new();
        columns_by_parent.insert(board_id, LoadState::Loaded(vec![column]));
        let mut cards_by_parent = std::collections::HashMap::new();
        cards_by_parent.insert(column_id, LoadState::Loaded(vec![card.clone()]));
        let mut archived_by_parent = std::collections::HashMap::new();
        archived_by_parent.insert(board_id, LoadState::Loaded(Vec::new()));
        let changed = model.apply_resolved(Resolved {
            columns: Collection {
                by_parent: columns_by_parent,
                ..Default::default()
            },
            cards: Collection {
                by_parent: cards_by_parent,
                ..Default::default()
            },
            archived_cards: Collection {
                by_parent: archived_by_parent,
                ..Default::default()
            },
            ..Default::default()
        });

        let mut controller = Controller::default();
        controller.set_scope_board(Some(board_id), &model);
        controller.resync(&model, changed);

        let live_ids: Vec<Uuid> = controller
            .displayed_cards(false)
            .loaded()
            .copied()
            .unwrap_or(&[])
            .iter()
            .map(|c| c.id)
            .collect();
        assert_eq!(live_ids, vec![card.id]);
        assert!(matches!(controller.displayed_cards(true), LoadState::Loaded(v) if v.is_empty()));
    }

    #[test]
    fn test_no_scope_board_leaves_card_partitions_not_loaded() {
        let board = seed_board("B", 0);
        let mut model = Model::default();
        let changed = model.load_from_snapshot(Snapshot {
            boards: vec![board],
            archived_boards: Vec::new(),
            ..Default::default()
        });
        let mut controller = Controller::default();
        controller.resync(&model, changed);

        assert!(controller.displayed_cards(false).is_not_loaded());
        assert!(controller.displayed_cards(true).is_not_loaded());
        assert!(controller.displayed_boards(false).is_loaded());
    }

    #[test]
    fn test_set_scope_board_change_rebuilds_partitions_without_a_model_change() {
        let board_1 = seed_board("B1", 0);
        let board_1_id = board_1.id;
        let column_1 = Column::new(board_1_id, "Col", 0);
        let card_1 = Card::new(board_1_id, column_1.id, "card1", 0);
        let card_1_id = card_1.id;

        let board_2 = seed_board("B2", 1);
        let board_2_id = board_2.id;
        let column_2 = Column::new(board_2_id, "Col", 0);
        let card_2 = Card::new(board_2_id, column_2.id, "card2", 0);
        let card_2_id = card_2.id;

        let mut model = Model::default();
        let _ = model.load_from_snapshot(Snapshot {
            boards: vec![board_1, board_2],
            columns: vec![column_1, column_2],
            cards: vec![card_1, card_2],
            archived_boards: Vec::new(),
            ..Default::default()
        });

        let mut controller = Controller::default();
        controller.set_scope_board(Some(board_1_id), &model);
        let ids_1: Vec<Uuid> = controller
            .displayed_cards(false)
            .loaded()
            .copied()
            .unwrap_or(&[])
            .iter()
            .map(|c| c.id)
            .collect();
        assert_eq!(ids_1, vec![card_1_id]);

        controller.set_scope_board(Some(board_2_id), &model);
        let ids_2: Vec<Uuid> = controller
            .displayed_cards(false)
            .loaded()
            .copied()
            .unwrap_or(&[])
            .iter()
            .map(|c| c.id)
            .collect();
        assert_eq!(ids_2, vec![card_2_id]);
    }

    #[test]
    fn test_archived_partition_joins_by_board_markers_with_per_id_bodies() {
        let board = seed_board("B", 0);
        let board_id = board.id;
        let column = Column::new(board_id, "Col", 0);
        let column_id = column.id;
        let card_a = Card::new(board_id, column_id, "a", 0);
        let card_b = Card::new(board_id, column_id, "b", 1);
        let card_a_id = card_a.id;
        let card_b_id = card_b.id;

        let mut model = Model::default();
        let mut columns_by_parent = std::collections::HashMap::new();
        columns_by_parent.insert(board_id, LoadState::Loaded(vec![column]));
        let mut archived_by_parent = std::collections::HashMap::new();
        archived_by_parent.insert(
            board_id,
            LoadState::Loaded(vec![
                ArchivedCard::new(card_a_id, board_id),
                ArchivedCard::new(card_b_id, board_id),
            ]),
        );
        let mut cards_by_id = std::collections::HashMap::new();
        cards_by_id.insert(card_a_id, LoadState::Loaded(card_a));
        cards_by_id.insert(card_b_id, LoadState::Loaded(card_b));
        let changed = model.apply_resolved(Resolved {
            columns: Collection {
                by_parent: columns_by_parent,
                ..Default::default()
            },
            archived_cards: Collection {
                by_parent: archived_by_parent,
                ..Default::default()
            },
            cards: Collection {
                by_id: cards_by_id,
                ..Default::default()
            },
            ..Default::default()
        });

        let mut controller = Controller::default();
        controller.set_scope_board(Some(board_id), &model);
        controller.resync(&model, changed);

        let archived_ids: Vec<Uuid> = controller
            .displayed_cards(true)
            .loaded()
            .copied()
            .unwrap_or(&[])
            .iter()
            .map(|c| c.id)
            .collect();
        assert_eq!(archived_ids, vec![card_a_id, card_b_id]);
    }

    #[test]
    fn test_archived_partition_not_loaded_while_a_body_is_in_flight() {
        let board = seed_board("B", 0);
        let board_id = board.id;
        let column = Column::new(board_id, "Col", 0);
        let column_id = column.id;
        let card_a = Card::new(board_id, column_id, "a", 0);
        let card_a_id = card_a.id;
        let card_b_id = Uuid::new_v4();

        let mut model = Model::default();
        let mut columns_by_parent = std::collections::HashMap::new();
        columns_by_parent.insert(board_id, LoadState::Loaded(vec![column.clone()]));
        let mut cards_by_parent = std::collections::HashMap::new();
        cards_by_parent.insert(column_id, LoadState::Loaded(vec![card_a.clone()]));
        let mut archived_by_parent = std::collections::HashMap::new();
        archived_by_parent.insert(
            board_id,
            LoadState::Loaded(vec![ArchivedCard::new(card_b_id, board_id)]),
        );
        let mut cards_by_id = std::collections::HashMap::new();
        cards_by_id.insert(card_a_id, LoadState::Loaded(card_a));
        let changed = model.apply_resolved(Resolved {
            columns: Collection {
                by_parent: columns_by_parent,
                ..Default::default()
            },
            cards: Collection {
                by_parent: cards_by_parent,
                by_id: cards_by_id,
                ..Default::default()
            },
            archived_cards: Collection {
                by_parent: archived_by_parent,
                ..Default::default()
            },
            ..Default::default()
        });

        let mut controller = Controller::default();
        controller.set_scope_board(Some(board_id), &model);
        controller.resync(&model, changed);

        assert!(controller.displayed_cards(false).is_loaded());
        assert!(controller.displayed_cards(true).is_not_loaded());
    }

    #[test]
    fn test_archived_partition_skips_a_missing_body() {
        let board = seed_board("B", 0);
        let board_id = board.id;
        let column = Column::new(board_id, "Col", 0);
        let card_a = Card::new(board_id, column.id, "a", 0);
        let card_a_id = card_a.id;
        let card_b_id = Uuid::new_v4();

        let mut model = Model::default();
        let mut columns_by_parent = std::collections::HashMap::new();
        columns_by_parent.insert(board_id, LoadState::Loaded(vec![column]));
        let mut archived_by_parent = std::collections::HashMap::new();
        archived_by_parent.insert(
            board_id,
            LoadState::Loaded(vec![
                ArchivedCard::new(card_a_id, board_id),
                ArchivedCard::new(card_b_id, board_id),
            ]),
        );
        let mut cards_by_id = std::collections::HashMap::new();
        cards_by_id.insert(card_a_id, LoadState::Loaded(card_a));
        cards_by_id.insert(card_b_id, LoadState::Missing);
        let changed = model.apply_resolved(Resolved {
            columns: Collection {
                by_parent: columns_by_parent,
                ..Default::default()
            },
            archived_cards: Collection {
                by_parent: archived_by_parent,
                ..Default::default()
            },
            cards: Collection {
                by_id: cards_by_id,
                ..Default::default()
            },
            ..Default::default()
        });

        let mut controller = Controller::default();
        controller.set_scope_board(Some(board_id), &model);
        controller.resync(&model, changed);

        let archived_ids: Vec<Uuid> = controller
            .displayed_cards(true)
            .loaded()
            .copied()
            .unwrap_or(&[])
            .iter()
            .map(|c| c.id)
            .collect();
        assert_eq!(archived_ids, vec![card_a_id]);
    }

    #[test]
    fn test_archived_partition_reports_failed_when_a_failed_body_follows_a_not_loaded_one() {
        let board = seed_board("B", 0);
        let board_id = board.id;
        let column = Column::new(board_id, "Col", 0);
        let not_loaded_id = Uuid::new_v4();
        let failed_id = Uuid::new_v4();

        let mut model = Model::default();
        let mut columns_by_parent = std::collections::HashMap::new();
        columns_by_parent.insert(board_id, LoadState::Loaded(vec![column]));
        let mut archived_by_parent = std::collections::HashMap::new();
        archived_by_parent.insert(
            board_id,
            LoadState::Loaded(vec![
                ArchivedCard::new(not_loaded_id, board_id),
                ArchivedCard::new(failed_id, board_id),
            ]),
        );
        let mut cards_by_id = std::collections::HashMap::new();
        cards_by_id.insert(
            failed_id,
            LoadState::Failed(std::sync::Arc::new(
                kanban_domain::KanbanError::unsupported("boom"),
            )),
        );
        let changed = model.apply_resolved(Resolved {
            columns: Collection {
                by_parent: columns_by_parent,
                ..Default::default()
            },
            archived_cards: Collection {
                by_parent: archived_by_parent,
                ..Default::default()
            },
            cards: Collection {
                by_id: cards_by_id,
                ..Default::default()
            },
            ..Default::default()
        });

        let mut controller = Controller::default();
        controller.set_scope_board(Some(board_id), &model);
        controller.resync(&model, changed);

        assert!(
            controller.displayed_cards(true).is_failed(),
            "a Failed body must win over an earlier NotLoaded marker in the walk"
        );
    }

    #[test]
    fn test_archived_partition_reports_failed_when_a_failed_body_precedes_a_not_loaded_one() {
        let board = seed_board("B", 0);
        let board_id = board.id;
        let column = Column::new(board_id, "Col", 0);
        let not_loaded_id = Uuid::new_v4();
        let failed_id = Uuid::new_v4();

        let mut model = Model::default();
        let mut columns_by_parent = std::collections::HashMap::new();
        columns_by_parent.insert(board_id, LoadState::Loaded(vec![column]));
        let mut archived_by_parent = std::collections::HashMap::new();
        archived_by_parent.insert(
            board_id,
            LoadState::Loaded(vec![
                ArchivedCard::new(failed_id, board_id),
                ArchivedCard::new(not_loaded_id, board_id),
            ]),
        );
        let mut cards_by_id = std::collections::HashMap::new();
        cards_by_id.insert(
            failed_id,
            LoadState::Failed(std::sync::Arc::new(
                kanban_domain::KanbanError::unsupported("boom"),
            )),
        );
        let changed = model.apply_resolved(Resolved {
            columns: Collection {
                by_parent: columns_by_parent,
                ..Default::default()
            },
            archived_cards: Collection {
                by_parent: archived_by_parent,
                ..Default::default()
            },
            cards: Collection {
                by_id: cards_by_id,
                ..Default::default()
            },
            ..Default::default()
        });

        let mut controller = Controller::default();
        controller.set_scope_board(Some(board_id), &model);
        controller.resync(&model, changed);

        assert!(
            controller.displayed_cards(true).is_failed(),
            "a Failed body must win regardless of marker order"
        );
    }

    #[test]
    fn test_board_scoped_live_partition_excludes_another_boards_cards() {
        let board_1 = seed_board("B1", 0);
        let board_1_id = board_1.id;
        let column_1 = Column::new(board_1_id, "Col", 0);
        let card_1 = Card::new(board_1_id, column_1.id, "card1", 0);
        let card_1_id = card_1.id;

        let board_2 = seed_board("B2", 1);
        let board_2_id = board_2.id;
        let column_2 = Column::new(board_2_id, "Col", 0);
        let card_2 = Card::new(board_2_id, column_2.id, "card2", 0);

        let mut model = Model::default();
        let changed = model.load_from_snapshot(Snapshot {
            boards: vec![board_1, board_2],
            columns: vec![column_1, column_2],
            cards: vec![card_1, card_2],
            archived_boards: Vec::new(),
            ..Default::default()
        });

        let mut controller = Controller::default();
        controller.set_scope_board(Some(board_1_id), &model);
        controller.resync(&model, changed);

        let live_ids: Vec<Uuid> = controller
            .displayed_cards(false)
            .loaded()
            .copied()
            .unwrap_or(&[])
            .iter()
            .map(|c| c.id)
            .collect();
        assert_eq!(live_ids, vec![card_1_id]);
    }
}
