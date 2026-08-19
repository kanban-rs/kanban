use uuid::Uuid;

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
    fn from(_state: &LoadState<T>) -> Self {
        todo!()
    }
}

/// `Loaded` and `Missing` are terminal and never re-requested; `Failed` may
/// be retried.
pub fn requestable(_status: FetchStatus) -> bool {
    todo!()
}

/// Whole-collection accessors and per-id accessors are independent: a
/// `card_list()` of `NotLoaded` alongside a `card(id)` of `Loaded` means one
/// card was fetched by id and the collection as a whole has never been read.
pub trait LoadedState {
    fn board_list(&self) -> FetchStatus;
    fn column_list(&self) -> FetchStatus;
    fn card_list(&self) -> FetchStatus;
    fn sprint_list(&self) -> FetchStatus;
    fn graph(&self) -> FetchStatus;
    fn column(&self, id: Uuid) -> FetchStatus;
    fn card(&self, id: Uuid) -> FetchStatus;
    fn sprint(&self, id: Uuid) -> FetchStatus;
}

/// The `*_list` flags request a whole collection, the id vectors request
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
}

impl FetchRound {
    pub fn is_empty(&self) -> bool {
        todo!()
    }
}

pub trait FetchPlan {
    fn next_round(&self, loaded: &dyn LoadedState) -> FetchRound;
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::error::KanbanError;

    struct StubLoaded {
        board_list: FetchStatus,
        card_list: FetchStatus,
        card: FetchStatus,
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
    }

    struct StubPlan;

    impl FetchPlan for StubPlan {
        fn next_round(&self, loaded: &dyn LoadedState) -> FetchRound {
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
            FetchStatus::from(&LoadState::<u32>::Failed(Arc::new(KanbanError::unsupported(
                "x"
            )))),
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
    }

    #[test]
    fn test_loaded_state_distinguishes_whole_collection_status_from_per_id_status() {
        let loaded = StubLoaded {
            board_list: FetchStatus::NotLoaded,
            card_list: FetchStatus::NotLoaded,
            card: FetchStatus::Loaded,
        };

        assert!(requestable(loaded.card_list()));
        assert!(!requestable(loaded.card(Uuid::new_v4())));
    }

    #[test]
    fn test_next_round_on_stub_plan_requests_board_list_when_not_loaded() {
        let loaded = StubLoaded {
            board_list: FetchStatus::NotLoaded,
            card_list: FetchStatus::NotLoaded,
            card: FetchStatus::NotLoaded,
        };
        let plan = StubPlan;

        let round = plan.next_round(&loaded);

        assert!(round.board_list);
        assert!(!round.is_empty());
    }

    #[test]
    fn test_next_round_on_stub_plan_returns_empty_round_once_board_list_loaded() {
        let loaded = StubLoaded {
            board_list: FetchStatus::Loaded,
            card_list: FetchStatus::NotLoaded,
            card: FetchStatus::NotLoaded,
        };
        let plan = StubPlan;

        assert!(plan.next_round(&loaded).is_empty());
    }
}
