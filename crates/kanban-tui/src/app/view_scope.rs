//! `ViewScope` is the TUI's `FetchPlan`: it names the tiers the current
//! screen needs. The renderer reads the flat `*_list` tiers and the
//! handlers read the by-parent tiers, so `next_round` requests both
//! populations together whenever a board subtree is in scope.

use uuid::Uuid;

use kanban_service::{requestable, FetchPlan, FetchRound, LoadedEntities};

use super::{App, AppMode, DialogMode};

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
    fn next_round(&self, loaded: &dyn LoadedEntities) -> FetchRound {
        let mut round = FetchRound {
            board_list: self.board_list && requestable(loaded.board_list()),
            graph: self.graph && requestable(loaded.graph()),
            ..Default::default()
        };

        if let Some(card_id) = self.card {
            if requestable(loaded.card(card_id)) {
                round.cards.push(card_id);
            }
        }

        if let Some(sprint_id) = self.sprint {
            if requestable(loaded.sprint(sprint_id)) {
                round.sprints.push(sprint_id);
            }
        }

        if let Some(board_id) = self.board {
            let board_subtree = self.board_columns || self.board_cards;

            if board_subtree {
                if requestable(loaded.column_list()) {
                    round.column_list = true;
                }
                if requestable(loaded.card_list()) {
                    round.card_list = true;
                }
                if requestable(loaded.sprint_list()) {
                    round.sprint_list = true;
                }
                if requestable(loaded.columns_of_board(board_id)) {
                    round.columns_by_board.push(board_id);
                }
            }

            if self.board_sprints && requestable(loaded.sprints_of_board(board_id)) {
                round.sprints_by_board.push(board_id);
            }

            if self.board_cards {
                if let Some(columns) = loaded.loaded_columns_of_board(board_id) {
                    for column in columns {
                        if requestable(loaded.cards_of_column(column.id)) {
                            round.cards_by_column.push(column.id);
                        }
                    }
                }
            }
        }

        round
    }
}

impl App {
    pub fn view_scope(&self) -> ViewScope {
        let board = self
            .selection
            .active_board_id
            .or_else(|| self.board_list.get_selected_board_id());

        let mut scope = ViewScope {
            board_list: true,
            board,
            board_columns: true,
            board_cards: true,
            ..Default::default()
        };

        let mut base = self.get_base_mode();
        while let AppMode::Help(inner) = base {
            base = inner.as_ref();
        }

        match base {
            AppMode::CardDetail => {
                scope.card = self.selection.active_card_id;
                scope.board_sprints = true;
                scope.graph = true;
            }
            AppMode::BoardDetail => {
                scope.board_sprints = true;
            }
            AppMode::SprintDetail => {
                scope.sprint = self.selection.active_sprint_id;
                scope.board_sprints = true;
            }
            AppMode::Settings => {
                scope.board_columns = false;
                scope.board_cards = false;
            }
            // Archived cards are snapshot-fed from the marker set populated on
            // load, not fetched through a round, so this view requests nothing
            // beyond the default board scope. The archived tier itself has no
            // producer here.
            AppMode::ArchivedCardsView => {}
            // Card search filters the board already in scope (`board_cards`
            // covers it) and board search filters the board list
            // (`board_list` covers it); `CardQueryBuilder::execute` cannot
            // express an unscoped search, so there is no wider set to fetch.
            _ => {}
        }

        match &self.mode {
            AppMode::Dialog(DialogMode::ManageParents | DialogMode::ManageChildren) => {
                scope.graph = true;
            }
            AppMode::Dialog(DialogMode::CarryOverSprint) => {
                scope.board_sprints = true;
            }
            _ => {}
        }

        if !self.filter.active_sprint_filters.is_empty() {
            scope.board_sprints = true;
        }

        scope
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use kanban_domain::Column;
    use kanban_service::FetchStatus;

    use super::*;

    struct StubLoaded {
        board_list: FetchStatus,
        column_list: FetchStatus,
        card_list: FetchStatus,
        sprint_list: FetchStatus,
        graph: FetchStatus,
        card: FetchStatus,
        sprint: FetchStatus,
        columns_of_board: HashMap<Uuid, FetchStatus>,
        cards_of_column: HashMap<Uuid, FetchStatus>,
        sprints_of_board: FetchStatus,
        loaded_columns: HashMap<Uuid, Vec<Column>>,
        archived_card_list: FetchStatus,
        archived_cards_of_board: FetchStatus,
    }

    impl Default for StubLoaded {
        fn default() -> Self {
            StubLoaded {
                board_list: FetchStatus::NotLoaded,
                column_list: FetchStatus::NotLoaded,
                card_list: FetchStatus::NotLoaded,
                sprint_list: FetchStatus::NotLoaded,
                graph: FetchStatus::NotLoaded,
                card: FetchStatus::NotLoaded,
                sprint: FetchStatus::NotLoaded,
                columns_of_board: HashMap::new(),
                cards_of_column: HashMap::new(),
                sprints_of_board: FetchStatus::NotLoaded,
                loaded_columns: HashMap::new(),
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
            self.column_list
        }
        fn card_list(&self) -> FetchStatus {
            self.card_list
        }
        fn sprint_list(&self) -> FetchStatus {
            self.sprint_list
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
                .get(&board_id)
                .copied()
                .unwrap_or(FetchStatus::NotLoaded)
        }
        fn cards_of_column(&self, column_id: Uuid) -> FetchStatus {
            self.cards_of_column
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
            self.loaded_columns.get(&board_id).map(Vec::as_slice)
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

        let stub2 = StubLoaded {
            board_list: FetchStatus::Loaded,
            column_list: FetchStatus::Loaded,
            card_list: FetchStatus::Loaded,
            sprint_list: FetchStatus::Loaded,
            columns_of_board: HashMap::from([(board, FetchStatus::Loaded)]),
            loaded_columns: HashMap::from([(board, vec![column(c1), column(c2)])]),
            ..StubLoaded::default()
        };

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
            column_list: FetchStatus::Loaded,
            card_list: FetchStatus::Loaded,
            sprint_list: FetchStatus::Loaded,
            columns_of_board: HashMap::from([(board, FetchStatus::Loaded)]),
            loaded_columns: HashMap::from([(board, vec![column(c1), column(c2)])]),
            cards_of_column: HashMap::from([(c1, FetchStatus::Loaded), (c2, FetchStatus::Loaded)]),
            ..StubLoaded::default()
        };

        let round3 = scope.next_round(&stub3);
        assert_eq!(round3, FetchRound::default());
        assert!(round3.is_empty());
    }

    #[test]
    fn test_a_loaded_tier_is_not_requested_again() {
        let board = Uuid::new_v4();
        let stub = StubLoaded {
            board_list: FetchStatus::Loaded,
            column_list: FetchStatus::Loaded,
            card_list: FetchStatus::Loaded,
            sprint_list: FetchStatus::Loaded,
            graph: FetchStatus::Loaded,
            card: FetchStatus::Loaded,
            sprint: FetchStatus::Loaded,
            sprints_of_board: FetchStatus::Loaded,
            columns_of_board: HashMap::from([(board, FetchStatus::Loaded)]),
            loaded_columns: HashMap::from([(board, vec![])]),
            ..StubLoaded::default()
        };

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
            column_list: FetchStatus::Loaded,
            card_list: FetchStatus::Loaded,
            sprint_list: FetchStatus::Loaded,
            columns_of_board: HashMap::from([(board, FetchStatus::Failed)]),
            ..StubLoaded::default()
        };

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
            column_list: FetchStatus::Loaded,
            card_list: FetchStatus::Loaded,
            sprint_list: FetchStatus::Loaded,
            columns_of_board: HashMap::from([(board, FetchStatus::Loaded)]),
            loaded_columns: HashMap::from([(board, vec![])]),
            ..StubLoaded::default()
        };

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
