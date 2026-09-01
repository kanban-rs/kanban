//! `ViewScope` is the TUI's `FetchPlan`: it names the tiers the current
//! screen needs. The renderer reads the flat `*_list` tiers and the
//! handlers read the by-parent tiers, so `next_round` requests both
//! populations together whenever a board subtree is in scope.

use uuid::Uuid;

use kanban_service::{FetchPlan, FetchRound, LoadedEntities};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ViewScope {
    pub board_list: bool,
    pub board: Option<Uuid>,
    pub board_columns: bool,
    pub board_cards: bool,
    pub board_sprints: bool,
    pub card: Option<Uuid>,
    pub sprint: Option<Uuid>,
    pub graph: bool,
}

impl FetchPlan for ViewScope {
    fn next_round(&self, _loaded: &dyn LoadedEntities) -> FetchRound {
        unimplemented!()
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::collections::HashMap;

    use kanban_domain::Column;
    use kanban_service::FetchStatus;

    use super::*;

    struct StubLoaded {
        board_list: FetchStatus,
        column_list: RefCell<FetchStatus>,
        card_list: RefCell<FetchStatus>,
        sprint_list: RefCell<FetchStatus>,
        graph: FetchStatus,
        card: FetchStatus,
        sprint: FetchStatus,
        columns_of_board: RefCell<HashMap<Uuid, FetchStatus>>,
        cards_of_column: RefCell<HashMap<Uuid, FetchStatus>>,
        sprints_of_board: FetchStatus,
        loaded_columns: RefCell<HashMap<Uuid, Vec<Column>>>,
        archived_card_list: FetchStatus,
        archived_cards_of_board: FetchStatus,
    }

    impl Default for StubLoaded {
        fn default() -> Self {
            StubLoaded {
                board_list: FetchStatus::NotLoaded,
                column_list: RefCell::new(FetchStatus::NotLoaded),
                card_list: RefCell::new(FetchStatus::NotLoaded),
                sprint_list: RefCell::new(FetchStatus::NotLoaded),
                graph: FetchStatus::NotLoaded,
                card: FetchStatus::NotLoaded,
                sprint: FetchStatus::NotLoaded,
                columns_of_board: RefCell::new(HashMap::new()),
                cards_of_column: RefCell::new(HashMap::new()),
                sprints_of_board: FetchStatus::NotLoaded,
                loaded_columns: RefCell::new(HashMap::new()),
                archived_card_list: FetchStatus::NotLoaded,
                archived_cards_of_board: FetchStatus::NotLoaded,
            }
        }
    }

    impl kanban_service::LoadedState for StubLoaded {
        fn board_list(&self) -> FetchStatus {
            self.board_list
        }
        fn column_list(&self) -> FetchStatus {
            *self.column_list.borrow()
        }
        fn card_list(&self) -> FetchStatus {
            *self.card_list.borrow()
        }
        fn sprint_list(&self) -> FetchStatus {
            *self.sprint_list.borrow()
        }
        fn graph(&self) -> FetchStatus {
            self.graph
        }
        fn column(&self, _id: Uuid) -> FetchStatus {
            FetchStatus::NotLoaded
        }
        fn card(&self, _id: Uuid) -> FetchStatus {
            self.card
        }
        fn sprint(&self, _id: Uuid) -> FetchStatus {
            self.sprint
        }
        fn columns_of_board(&self, board_id: Uuid) -> FetchStatus {
            self.columns_of_board
                .borrow()
                .get(&board_id)
                .copied()
                .unwrap_or(FetchStatus::NotLoaded)
        }
        fn cards_of_column(&self, column_id: Uuid) -> FetchStatus {
            self.cards_of_column
                .borrow()
                .get(&column_id)
                .copied()
                .unwrap_or(FetchStatus::NotLoaded)
        }
        fn sprints_of_board(&self, _board_id: Uuid) -> FetchStatus {
            self.sprints_of_board
        }
        fn archived_card_list(&self) -> FetchStatus {
            self.archived_card_list
        }
        fn archived_cards_of_board(&self, _board_id: Uuid) -> FetchStatus {
            self.archived_cards_of_board
        }
    }

    impl LoadedEntities for StubLoaded {
        fn loaded_columns_of_board(&self, board_id: Uuid) -> Option<&[Column]> {
            let borrow = self.loaded_columns.borrow();
            let cols = borrow.get(&board_id)?;
            let ptr = cols.as_slice() as *const [Column];
            Some(unsafe { &*ptr })
        }
    }

    fn column(id: Uuid) -> Column {
        let mut column = Column::new(Uuid::new_v4(), "Todo", 0);
        column.id = id;
        column
    }

    #[test]
    fn test_opening_a_board_converges_to_its_columns_and_their_cards_in_one_drive() {
        let board = Uuid::new_v4();
        let c1 = Uuid::new_v4();
        let c2 = Uuid::new_v4();
        let stub = StubLoaded::default();

        let scope = ViewScope {
            board_list: true,
            board: Some(board),
            board_columns: true,
            board_cards: true,
            ..Default::default()
        };

        let round1 = scope.next_round(&stub);
        assert!(round1.board_list);
        assert!(round1.column_list);
        assert!(round1.card_list);
        assert!(round1.sprint_list);
        assert_eq!(round1.columns_by_board, vec![board]);
        assert!(round1.cards_by_column.is_empty());

        *stub.column_list.borrow_mut() = FetchStatus::Loaded;
        *stub.card_list.borrow_mut() = FetchStatus::Loaded;
        *stub.sprint_list.borrow_mut() = FetchStatus::Loaded;
        stub.columns_of_board
            .borrow_mut()
            .insert(board, FetchStatus::Loaded);
        stub.loaded_columns
            .borrow_mut()
            .insert(board, vec![column(c1), column(c2)]);

        let stub2 = StubLoaded {
            board_list: FetchStatus::Loaded,
            column_list: RefCell::new(FetchStatus::Loaded),
            card_list: RefCell::new(FetchStatus::Loaded),
            sprint_list: RefCell::new(FetchStatus::Loaded),
            ..StubLoaded::default()
        };
        stub2
            .columns_of_board
            .borrow_mut()
            .insert(board, FetchStatus::Loaded);
        stub2
            .loaded_columns
            .borrow_mut()
            .insert(board, vec![column(c1), column(c2)]);

        let round2 = scope.next_round(&stub2);
        assert!(!round2.board_list);
        assert!(!round2.column_list);
        assert!(!round2.card_list);
        assert!(!round2.sprint_list);
        assert!(round2.columns_by_board.is_empty());
        let mut cards_by_column = round2.cards_by_column.clone();
        cards_by_column.sort();
        let mut expected = vec![c1, c2];
        expected.sort();
        assert_eq!(cards_by_column, expected);

        let stub3 = StubLoaded {
            board_list: FetchStatus::Loaded,
            column_list: RefCell::new(FetchStatus::Loaded),
            card_list: RefCell::new(FetchStatus::Loaded),
            sprint_list: RefCell::new(FetchStatus::Loaded),
            ..StubLoaded::default()
        };
        stub3
            .columns_of_board
            .borrow_mut()
            .insert(board, FetchStatus::Loaded);
        stub3
            .loaded_columns
            .borrow_mut()
            .insert(board, vec![column(c1), column(c2)]);
        stub3
            .cards_of_column
            .borrow_mut()
            .insert(c1, FetchStatus::Loaded);
        stub3
            .cards_of_column
            .borrow_mut()
            .insert(c2, FetchStatus::Loaded);

        let round3 = scope.next_round(&stub3);
        assert_eq!(round3, FetchRound::default());
        assert!(round3.is_empty());
    }

    #[test]
    fn test_a_loaded_tier_is_not_requested_again() {
        let board = Uuid::new_v4();
        let stub = StubLoaded {
            board_list: FetchStatus::Loaded,
            column_list: RefCell::new(FetchStatus::Loaded),
            card_list: RefCell::new(FetchStatus::Loaded),
            sprint_list: RefCell::new(FetchStatus::Loaded),
            graph: FetchStatus::Loaded,
            card: FetchStatus::Loaded,
            sprint: FetchStatus::Loaded,
            sprints_of_board: FetchStatus::Loaded,
            ..StubLoaded::default()
        };
        stub.columns_of_board
            .borrow_mut()
            .insert(board, FetchStatus::Loaded);
        stub.loaded_columns.borrow_mut().insert(board, vec![]);

        let scope = ViewScope {
            board_list: true,
            board: Some(board),
            board_columns: true,
            board_cards: true,
            board_sprints: true,
            card: Some(Uuid::new_v4()),
            sprint: Some(Uuid::new_v4()),
            graph: true,
        };

        let round = scope.next_round(&stub);
        assert_eq!(round, FetchRound::default());
        assert!(round.is_empty());
    }

    #[test]
    fn test_a_failed_tier_is_requested_again() {
        let board = Uuid::new_v4();
        let stub = StubLoaded {
            board_list: FetchStatus::Loaded,
            column_list: RefCell::new(FetchStatus::Loaded),
            card_list: RefCell::new(FetchStatus::Loaded),
            sprint_list: RefCell::new(FetchStatus::Loaded),
            ..StubLoaded::default()
        };
        stub.columns_of_board
            .borrow_mut()
            .insert(board, FetchStatus::Failed);

        let scope = ViewScope {
            board: Some(board),
            board_columns: true,
            ..Default::default()
        };

        let round = scope.next_round(&stub);
        assert_eq!(round.columns_by_board, vec![board]);
    }

    #[test]
    fn test_a_missing_card_is_not_requested_again() {
        let id = Uuid::new_v4();
        let stub = StubLoaded {
            card: FetchStatus::Missing,
            ..StubLoaded::default()
        };

        let scope = ViewScope {
            card: Some(id),
            ..Default::default()
        };

        let round = scope.next_round(&stub);
        assert!(round.cards.is_empty());
    }

    #[test]
    fn test_a_column_less_board_stops_after_one_round() {
        let board = Uuid::new_v4();
        let stub = StubLoaded {
            board_list: FetchStatus::Loaded,
            column_list: RefCell::new(FetchStatus::Loaded),
            card_list: RefCell::new(FetchStatus::Loaded),
            sprint_list: RefCell::new(FetchStatus::Loaded),
            ..StubLoaded::default()
        };
        stub.columns_of_board
            .borrow_mut()
            .insert(board, FetchStatus::Loaded);
        stub.loaded_columns.borrow_mut().insert(board, vec![]);

        let scope = ViewScope {
            board_list: true,
            board: Some(board),
            board_columns: true,
            board_cards: true,
            ..Default::default()
        };

        let round = scope.next_round(&stub);
        assert!(round.cards_by_column.is_empty());
        assert_eq!(round, FetchRound::default());
    }

    #[test]
    fn test_no_board_in_scope_requests_only_the_board_list() {
        let stub = StubLoaded::default();
        let scope = ViewScope {
            board_list: true,
            board: None,
            board_columns: true,
            board_cards: true,
            ..Default::default()
        };

        let round = scope.next_round(&stub);
        assert!(round.board_list);
        assert!(!round.column_list);
        assert!(!round.card_list);
        assert!(!round.sprint_list);
        assert!(round.columns_by_board.is_empty());
        assert!(round.cards_by_column.is_empty());
        assert!(round.sprints_by_board.is_empty());
    }

    #[test]
    fn test_board_cards_alone_still_requests_the_flat_and_by_parent_column_tiers() {
        let board = Uuid::new_v4();
        let stub = StubLoaded::default();
        let scope = ViewScope {
            board: Some(board),
            board_columns: false,
            board_cards: true,
            ..Default::default()
        };

        let round = scope.next_round(&stub);
        assert_eq!(round.columns_by_board, vec![board]);
        assert!(round.column_list);
        assert!(round.card_list);
        assert!(round.sprint_list);
    }

    #[test]
    fn test_settings_scope_requests_only_the_board_list_even_with_a_board_present() {
        let board = Uuid::new_v4();
        let stub = StubLoaded::default();
        let scope = ViewScope {
            board_list: true,
            board: Some(board),
            board_columns: false,
            board_cards: false,
            ..Default::default()
        };

        let round = scope.next_round(&stub);
        assert!(round.board_list);
        assert!(!round.column_list);
        assert!(!round.card_list);
        assert!(!round.sprint_list);
        assert!(round.columns_by_board.is_empty());
        assert!(round.cards_by_column.is_empty());
        assert!(round.sprints_by_board.is_empty());
    }

    #[test]
    fn test_archived_tiers_are_never_requested_regardless_of_status() {
        let board = Uuid::new_v4();
        let stub = StubLoaded::default();
        let scope = ViewScope {
            board_list: true,
            board: Some(board),
            board_columns: true,
            board_cards: true,
            board_sprints: true,
            card: Some(Uuid::new_v4()),
            sprint: Some(Uuid::new_v4()),
            graph: true,
        };

        let round = scope.next_round(&stub);
        assert!(!round.archived_card_list);
        assert!(round.archived_cards_by_board.is_empty());
    }
}
