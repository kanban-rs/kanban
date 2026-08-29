use uuid::Uuid;

use crate::column::Column;
use crate::load_state::LoadState;

/// Payload-free mirror of `LoadState<T>`'s variants, usable across entity
/// kinds behind a `dyn` trait without a generic parameter per kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FetchStatus {
    NotLoaded,
    Loaded,
    Missing,
    Failed,
}

impl<T> From<&LoadState<T>> for FetchStatus {
    fn from(state: &LoadState<T>) -> Self {
        match state {
            LoadState::NotLoaded => FetchStatus::NotLoaded,
            LoadState::Loaded(_) => FetchStatus::Loaded,
            LoadState::Missing => FetchStatus::Missing,
            LoadState::Failed(_) => FetchStatus::Failed,
        }
    }
}

/// `Loaded` and `Missing` are terminal and never re-requested; `Failed` may
/// be retried.
pub fn requestable(status: FetchStatus) -> bool {
    matches!(status, FetchStatus::NotLoaded | FetchStatus::Failed)
}

/// Whole-collection accessors, parent-scoped accessors, and per-id accessors
/// are independent tiers: a `card_list()` of `NotLoaded` alongside a
/// `cards_of_column(c)` of `Loaded` alongside a `card(id)` of `Missing` is a
/// coherent state, not a contradiction.
///
/// Each parent-scoped accessor's parent kind is fixed and matches the
/// `DataStore` read that serves it: `columns_of_board` is served by
/// `list_columns_by_board`, `cards_of_column` by `list_cards_by_column`, and
/// `sprints_of_board` by `list_sprints_by_board`.
///
/// A parent-scoped accessor never returns `FetchStatus::Missing`: the scoped
/// reads answer an unknown parent with an empty vector, so implementors must
/// not synthesise `Missing` for them.
pub trait LoadedState {
    fn board_list(&self) -> FetchStatus;
    fn column_list(&self) -> FetchStatus;
    fn card_list(&self) -> FetchStatus;
    fn sprint_list(&self) -> FetchStatus;
    fn graph(&self) -> FetchStatus;
    fn column(&self, id: Uuid) -> FetchStatus;
    fn card(&self, id: Uuid) -> FetchStatus;
    fn sprint(&self, id: Uuid) -> FetchStatus;
    fn columns_of_board(&self, board_id: Uuid) -> FetchStatus;
    fn cards_of_column(&self, column_id: Uuid) -> FetchStatus;
    fn sprints_of_board(&self, board_id: Uuid) -> FetchStatus;
}

/// The `*_list` flags request a whole collection, the `*_by_*` vectors
/// request every child of a named parent, the bare id vectors request
/// individual entities, and an empty round is `resolve`'s halt signal.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FetchRound {
    pub board_list: bool,
    pub column_list: bool,
    pub card_list: bool,
    pub sprint_list: bool,
    pub graph: bool,
    pub columns: Vec<Uuid>,
    pub cards: Vec<Uuid>,
    pub sprints: Vec<Uuid>,
    /// Board ids whose columns are wanted.
    pub columns_by_board: Vec<Uuid>,
    /// Column ids whose cards are wanted.
    pub cards_by_column: Vec<Uuid>,
    /// Board ids whose sprints are wanted.
    pub sprints_by_board: Vec<Uuid>,
}

impl FetchRound {
    pub fn is_empty(&self) -> bool {
        !self.board_list
            && !self.column_list
            && !self.card_list
            && !self.sprint_list
            && !self.graph
            && self.columns.is_empty()
            && self.cards.is_empty()
            && self.sprints.is_empty()
            && self.columns_by_board.is_empty()
            && self.cards_by_column.is_empty()
            && self.sprints_by_board.is_empty()
    }
}

/// Payload projection over [`LoadedState`]. `Some` exactly when the
/// corresponding status accessor is `Loaded`. A genuinely column-less board
/// is `Some(&[])`, not `None`; `None` means never read. Only columns are
/// projected, because they are the only payload a plan needs in order to
/// name a later round's ids.
pub trait LoadedEntities: LoadedState {
    fn loaded_columns_of_board(&self, board_id: Uuid) -> Option<&[Column]>;
}

pub trait FetchPlan {
    fn next_round(&self, loaded: &dyn LoadedEntities) -> FetchRound;
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Arc;

    use super::*;
    use crate::error::KanbanError;

    struct StubLoaded {
        board_list: FetchStatus,
        card_list: FetchStatus,
        card: FetchStatus,
        cards_of_column: FetchStatus,
        columns_by_board: HashMap<Uuid, Vec<Column>>,
    }

    impl Default for StubLoaded {
        fn default() -> Self {
            StubLoaded {
                board_list: FetchStatus::NotLoaded,
                card_list: FetchStatus::NotLoaded,
                card: FetchStatus::NotLoaded,
                cards_of_column: FetchStatus::NotLoaded,
                columns_by_board: HashMap::new(),
            }
        }
    }

    impl LoadedState for StubLoaded {
        fn board_list(&self) -> FetchStatus {
            self.board_list
        }
        fn column_list(&self) -> FetchStatus {
            FetchStatus::NotLoaded
        }
        fn card_list(&self) -> FetchStatus {
            self.card_list
        }
        fn sprint_list(&self) -> FetchStatus {
            FetchStatus::NotLoaded
        }
        fn graph(&self) -> FetchStatus {
            FetchStatus::NotLoaded
        }
        fn column(&self, _id: Uuid) -> FetchStatus {
            FetchStatus::NotLoaded
        }
        fn card(&self, _id: Uuid) -> FetchStatus {
            self.card
        }
        fn sprint(&self, _id: Uuid) -> FetchStatus {
            FetchStatus::NotLoaded
        }
        fn columns_of_board(&self, board_id: Uuid) -> FetchStatus {
            if self.columns_by_board.contains_key(&board_id) {
                FetchStatus::Loaded
            } else {
                FetchStatus::NotLoaded
            }
        }
        fn cards_of_column(&self, _column_id: Uuid) -> FetchStatus {
            self.cards_of_column
        }
        fn sprints_of_board(&self, _board_id: Uuid) -> FetchStatus {
            FetchStatus::NotLoaded
        }
    }

    impl LoadedEntities for StubLoaded {
        fn loaded_columns_of_board(&self, board_id: Uuid) -> Option<&[Column]> {
            self.columns_by_board.get(&board_id).map(Vec::as_slice)
        }
    }

    struct StubPlan;

    impl FetchPlan for StubPlan {
        fn next_round(&self, loaded: &dyn LoadedEntities) -> FetchRound {
            FetchRound {
                board_list: requestable(loaded.board_list()),
                ..Default::default()
            }
        }
    }

    #[test]
    fn test_requestable_treats_not_loaded_and_failed_as_requestable() {
        assert!(requestable(FetchStatus::NotLoaded));
        assert!(requestable(FetchStatus::Failed));
    }

    #[test]
    fn test_requestable_treats_loaded_and_missing_as_terminal() {
        assert!(!requestable(FetchStatus::Loaded));
        assert!(!requestable(FetchStatus::Missing));
    }

    #[test]
    fn test_fetch_status_from_load_state_preserves_every_variant() {
        assert_eq!(
            FetchStatus::from(&LoadState::<u32>::NotLoaded),
            FetchStatus::NotLoaded
        );
        assert_eq!(
            FetchStatus::from(&LoadState::Loaded(1u32)),
            FetchStatus::Loaded
        );
        assert_eq!(
            FetchStatus::from(&LoadState::<u32>::Missing),
            FetchStatus::Missing
        );
        assert_eq!(
            FetchStatus::from(&LoadState::<u32>::Failed(Arc::new(
                KanbanError::unsupported("x")
            ))),
            FetchStatus::Failed
        );
    }

    #[test]
    fn test_fetch_round_default_is_empty() {
        assert!(FetchRound::default().is_empty());
    }

    #[test]
    fn test_fetch_round_with_any_single_field_set_is_not_empty() {
        let id = Uuid::new_v4();

        assert!(!FetchRound {
            board_list: true,
            ..Default::default()
        }
        .is_empty());
        assert!(!FetchRound {
            column_list: true,
            ..Default::default()
        }
        .is_empty());
        assert!(!FetchRound {
            card_list: true,
            ..Default::default()
        }
        .is_empty());
        assert!(!FetchRound {
            sprint_list: true,
            ..Default::default()
        }
        .is_empty());
        assert!(!FetchRound {
            graph: true,
            ..Default::default()
        }
        .is_empty());
        assert!(!FetchRound {
            columns: vec![id],
            ..Default::default()
        }
        .is_empty());
        assert!(!FetchRound {
            cards: vec![id],
            ..Default::default()
        }
        .is_empty());
        assert!(!FetchRound {
            sprints: vec![id],
            ..Default::default()
        }
        .is_empty());
        assert!(!FetchRound {
            columns_by_board: vec![id],
            ..Default::default()
        }
        .is_empty());
        assert!(!FetchRound {
            cards_by_column: vec![id],
            ..Default::default()
        }
        .is_empty());
        assert!(!FetchRound {
            sprints_by_board: vec![id],
            ..Default::default()
        }
        .is_empty());
    }

    #[test]
    fn test_a_scoped_only_round_is_not_empty() {
        let round = FetchRound {
            cards_by_column: vec![Uuid::new_v4()],
            ..Default::default()
        };

        assert!(!round.is_empty());
    }

    #[test]
    fn test_loaded_state_distinguishes_all_three_tiers() {
        let column_id = Uuid::new_v4();
        let card_id = Uuid::new_v4();
        let loaded = StubLoaded {
            card_list: FetchStatus::NotLoaded,
            cards_of_column: FetchStatus::Loaded,
            card: FetchStatus::Missing,
            ..Default::default()
        };

        assert!(requestable(loaded.card_list()));
        assert!(!requestable(loaded.cards_of_column(column_id)));
        assert!(!requestable(loaded.card(card_id)));
    }

    #[test]
    fn test_loaded_entities_projects_none_for_an_unfetched_scope() {
        let board_id = Uuid::new_v4();
        let loaded = StubLoaded::default();

        assert!(loaded.loaded_columns_of_board(board_id).is_none());
    }

    #[test]
    fn test_loaded_entities_distinguishes_an_empty_scope_from_an_unread_one() {
        let board_with_no_columns = Uuid::new_v4();
        let unread_board = Uuid::new_v4();
        let mut columns_by_board = HashMap::new();
        columns_by_board.insert(board_with_no_columns, Vec::new());
        let loaded = StubLoaded {
            columns_by_board,
            ..Default::default()
        };

        assert!(matches!(
            loaded.loaded_columns_of_board(board_with_no_columns),
            Some(s) if s.is_empty()
        ));
        assert!(loaded.loaded_columns_of_board(unread_board).is_none());
    }

    #[test]
    fn test_loaded_state_distinguishes_whole_collection_status_from_per_id_status() {
        let loaded = StubLoaded {
            card: FetchStatus::Loaded,
            ..Default::default()
        };

        assert!(requestable(loaded.card_list()));
        assert!(!requestable(loaded.card(Uuid::new_v4())));
    }

    #[test]
    fn test_next_round_on_stub_plan_requests_board_list_when_not_loaded() {
        let loaded = StubLoaded::default();
        let plan = StubPlan;

        let round = plan.next_round(&loaded);

        assert!(round.board_list);
        assert!(!round.is_empty());
    }

    #[test]
    fn test_next_round_on_stub_plan_returns_empty_round_once_board_list_loaded() {
        let loaded = StubLoaded {
            board_list: FetchStatus::Loaded,
            ..Default::default()
        };
        let plan = StubPlan;

        assert!(plan.next_round(&loaded).is_empty());
    }
}
