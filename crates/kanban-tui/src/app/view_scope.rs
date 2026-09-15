//! `ViewScope` is the TUI's `FetchPlan`: handlers and renderer read the
//! board-scoped tiers, so `next_round` requests only those.

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
    /// Wants the global archived-card marker tier, without walking it into
    /// per-card body fetches.
    pub archived_card_markers: bool,
    /// Wants the archived-card marker tier AND every marker's card body.
    pub archived_card_bodies: bool,
    pub archived_board_markers: bool,
    pub archived_board_bodies: bool,
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

            if self.graph {
                if let Some(neighbours) = loaded.loaded_graph_neighbours(card_id) {
                    let mut ids: Vec<Uuid> = neighbours
                        .into_iter()
                        .filter(|&id| requestable(loaded.card(id)))
                        .collect();
                    ids.sort_unstable();
                    round.cards.extend(ids);
                }
            }
        }

        if let Some(sprint_id) = self.sprint {
            if requestable(loaded.sprint(sprint_id)) {
                round.sprints.push(sprint_id);
            }
        }

        if let Some(board_id) = self.board {
            if (self.board_columns || self.board_cards)
                && requestable(loaded.columns_of_board(board_id))
            {
                round.columns_by_board.push(board_id);
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

            if self.archived_card_markers || self.archived_card_bodies {
                if requestable(loaded.archived_cards_of_board(board_id)) {
                    round.archived_cards_by_board.push(board_id);
                } else if self.archived_card_bodies {
                    if let Some(markers) = loaded.loaded_archived_cards_of_board(board_id) {
                        let mut ids: Vec<Uuid> = markers
                            .iter()
                            .map(|m| m.entity_id)
                            .filter(|&id| requestable(loaded.card(id)))
                            .collect();
                        ids.sort_unstable();
                        round.cards.extend(ids);
                    }
                }
            }
        }

        if self.archived_board_markers || self.archived_board_bodies {
            if requestable(loaded.archived_board_list()) {
                round.archived_board_list = true;
            } else if self.archived_board_bodies {
                if !round.board_list && requestable(loaded.board_list()) {
                    round.board_list = true;
                }
                if let Some(markers) = loaded.loaded_archived_board_markers() {
                    let mut ids: Vec<Uuid> = markers
                        .iter()
                        .map(|m| m.entity_id)
                        .filter(|&id| requestable(loaded.board_in_collection(id)))
                        .collect();
                    ids.sort_unstable();
                    round.boards.extend(ids);
                }
            }
        }

        round
    }
}

impl App {
    pub fn view_scope(&self) -> ViewScope {
        let board = self.scope_board_id();

        let mut scope = ViewScope {
            board_list: true,
            board,
            board_columns: true,
            board_cards: true,
            board_sprints: true,
            ..Default::default()
        };

        let mut base = self.get_base_mode();
        while let AppMode::Help(inner) = base {
            base = inner.as_ref();
        }

        match base {
            AppMode::CardDetail => {
                scope.card = self.selection.active_card_id;
                scope.graph = true;
            }
            AppMode::SprintDetail => {
                scope.sprint = self.selection.active_sprint_id;
            }
            AppMode::Settings => {
                scope.board_columns = false;
                scope.board_cards = false;
                scope.board_sprints = false;
            }
            AppMode::ArchivedCardsView => {
                scope.archived_card_markers = true;
                scope.archived_card_bodies = true;
            }
            AppMode::ArchivedBoardsView => {
                scope.archived_board_markers = true;
                scope.archived_board_bodies = true;
                scope.archived_card_markers = true;
            }
            // Card search filters the board already in scope (`board_cards`
            // covers it) and board search filters the board list
            // (`board_list` covers it); `CardQueryBuilder::execute` cannot
            // express an unscoped search, so there is no wider set to fetch.
            _ => {}
        }

        let mut current = &self.mode;
        while let AppMode::Help(inner) = current {
            current = inner.as_ref();
        }

        if matches!(
            current,
            AppMode::Dialog(DialogMode::ManageParents | DialogMode::ManageChildren)
        ) {
            scope.graph = true;
        }

        if matches!(current, AppMode::Dialog(DialogMode::DeleteBoardConfirm)) {
            scope.archived_card_markers = true;
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
        graph: FetchStatus,
        card: FetchStatus,
        sprint: FetchStatus,
        columns_of_board: HashMap<Uuid, FetchStatus>,
        cards_of_column: HashMap<Uuid, FetchStatus>,
        sprints_of_board: FetchStatus,
        loaded_columns: HashMap<Uuid, Vec<Column>>,
        archived_card_list: FetchStatus,
        archived_cards_of_board: HashMap<Uuid, FetchStatus>,
        archived_board_list: FetchStatus,
        card_status_by_id: HashMap<Uuid, FetchStatus>,
        board_in_collection: HashMap<Uuid, FetchStatus>,
        archived_card_markers: Option<Vec<kanban_domain::ArchivedCard>>,
        archived_cards_by_board_markers: HashMap<Uuid, Vec<kanban_domain::ArchivedCard>>,
        archived_board_markers: Option<Vec<kanban_domain::ArchivedBoard>>,
        graph_neighbours: HashMap<Uuid, Vec<Uuid>>,
    }

    impl Default for StubLoaded {
        fn default() -> Self {
            StubLoaded {
                board_list: FetchStatus::NotLoaded,
                graph: FetchStatus::NotLoaded,
                card: FetchStatus::NotLoaded,
                sprint: FetchStatus::NotLoaded,
                columns_of_board: HashMap::new(),
                cards_of_column: HashMap::new(),
                sprints_of_board: FetchStatus::NotLoaded,
                loaded_columns: HashMap::new(),
                archived_card_list: FetchStatus::NotLoaded,
                archived_cards_of_board: HashMap::new(),
                archived_board_list: FetchStatus::NotLoaded,
                card_status_by_id: HashMap::new(),
                board_in_collection: HashMap::new(),
                archived_card_markers: None,
                archived_cards_by_board_markers: HashMap::new(),
                archived_board_markers: None,
                graph_neighbours: HashMap::new(),
            }
        }
    }

    impl kanban_service::LoadedState for StubLoaded {
        fn board_list(&self) -> FetchStatus {
            self.board_list
        }
        fn graph(&self) -> FetchStatus {
            self.graph
        }
        fn board(&self, _id: Uuid) -> FetchStatus {
            FetchStatus::NotLoaded
        }
        fn column(&self, _id: Uuid) -> FetchStatus {
            FetchStatus::NotLoaded
        }
        fn card(&self, id: Uuid) -> FetchStatus {
            self.card_status_by_id
                .get(&id)
                .copied()
                .unwrap_or(self.card)
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
        fn archived_cards_of_board(&self, board_id: Uuid) -> FetchStatus {
            self.archived_cards_of_board
                .get(&board_id)
                .copied()
                .unwrap_or(FetchStatus::NotLoaded)
        }
        fn archived_board_list(&self) -> FetchStatus {
            self.archived_board_list
        }
        fn board_in_collection(&self, id: Uuid) -> FetchStatus {
            self.board_in_collection
                .get(&id)
                .copied()
                .unwrap_or(FetchStatus::NotLoaded)
        }
    }

    impl LoadedEntities for StubLoaded {
        fn loaded_columns_of_board(&self, board_id: Uuid) -> Option<&[Column]> {
            self.loaded_columns.get(&board_id).map(Vec::as_slice)
        }
        fn loaded_archived_card_markers(&self) -> Option<&[kanban_domain::ArchivedCard]> {
            self.archived_card_markers.as_deref()
        }
        fn loaded_archived_board_markers(&self) -> Option<&[kanban_domain::ArchivedBoard]> {
            self.archived_board_markers.as_deref()
        }
        fn loaded_archived_cards_of_board(
            &self,
            board_id: Uuid,
        ) -> Option<&[kanban_domain::ArchivedCard]> {
            self.archived_cards_by_board_markers
                .get(&board_id)
                .map(Vec::as_slice)
        }
        fn loaded_graph_neighbours(&self, card_id: Uuid) -> Option<Vec<Uuid>> {
            if self.graph == FetchStatus::Loaded {
                Some(
                    self.graph_neighbours
                        .get(&card_id)
                        .cloned()
                        .unwrap_or_default(),
                )
            } else {
                None
            }
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
            board_sprints: true,
            ..Default::default()
        };

        let round1 = scope.next_round(&stub);
        assert!(round1.board_list);
        assert_eq!(round1.columns_by_board, vec![board]);
        assert_eq!(round1.sprints_by_board, vec![board]);
        assert!(round1.cards_by_column.is_empty());

        let stub2 = StubLoaded {
            board_list: FetchStatus::Loaded,
            columns_of_board: HashMap::from([(board, FetchStatus::Loaded)]),
            sprints_of_board: FetchStatus::Loaded,
            loaded_columns: HashMap::from([(board, vec![column(c1), column(c2)])]),
            ..StubLoaded::default()
        };

        let round2 = scope.next_round(&stub2);
        assert!(!round2.board_list);
        assert!(round2.columns_by_board.is_empty());
        assert!(round2.sprints_by_board.is_empty());
        let mut cards_by_column = round2.cards_by_column.clone();
        cards_by_column.sort();
        let mut expected = vec![c1, c2];
        expected.sort();
        assert_eq!(cards_by_column, expected);

        let stub3 = StubLoaded {
            board_list: FetchStatus::Loaded,
            columns_of_board: HashMap::from([(board, FetchStatus::Loaded)]),
            sprints_of_board: FetchStatus::Loaded,
            loaded_columns: HashMap::from([(board, vec![column(c1), column(c2)])]),
            cards_of_column: HashMap::from([(c1, FetchStatus::Loaded), (c2, FetchStatus::Loaded)]),
            ..StubLoaded::default()
        };

        let round3 = scope.next_round(&stub3);
        assert_eq!(round3, FetchRound::default());
        assert!(round3.is_empty());
    }

    #[test]
    fn test_a_board_screen_with_failed_flat_tiers_plans_an_empty_round() {
        let board = Uuid::new_v4();
        let c1 = Uuid::new_v4();
        let stub = StubLoaded {
            board_list: FetchStatus::Loaded,
            columns_of_board: HashMap::from([(board, FetchStatus::Loaded)]),
            sprints_of_board: FetchStatus::Loaded,
            loaded_columns: HashMap::from([(board, vec![column(c1)])]),
            cards_of_column: HashMap::from([(c1, FetchStatus::Loaded)]),
            ..StubLoaded::default()
        };

        let scope = ViewScope {
            board_list: true,
            board: Some(board),
            board_columns: true,
            board_cards: true,
            board_sprints: true,
            ..Default::default()
        };

        assert!(scope.next_round(&stub).is_empty());
    }

    #[test]
    fn test_a_loaded_tier_is_not_requested_again() {
        let board = Uuid::new_v4();
        let stub = StubLoaded {
            board_list: FetchStatus::Loaded,
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
            archived_card_markers: false,
            archived_card_bodies: false,
            archived_board_markers: false,
            archived_board_bodies: false,
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
            columns_of_board: HashMap::from([(board, FetchStatus::Loaded)]),
            sprints_of_board: FetchStatus::Loaded,
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
        assert!(round.columns_by_board.is_empty());
        assert!(round.cards_by_column.is_empty());
        assert!(round.sprints_by_board.is_empty());
    }

    #[test]
    fn test_board_cards_alone_requests_only_the_by_parent_tiers() {
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
        assert!(round.columns_by_board.is_empty());
        assert!(round.cards_by_column.is_empty());
        assert!(round.sprints_by_board.is_empty());
    }

    #[test]
    fn test_a_non_archived_scope_never_requests_archived_tiers_or_board_bodies() {
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
            archived_card_markers: false,
            archived_card_bodies: false,
            archived_board_markers: false,
            archived_board_bodies: false,
        };

        let round = scope.next_round(&stub);
        assert!(!round.archived_card_list);
        assert!(round.archived_cards_by_board.is_empty());
        assert!(!round.archived_board_list);
        assert!(round.boards.is_empty());
    }

    fn archived_card_marker(entity_id: Uuid) -> kanban_domain::ArchivedCard {
        kanban_domain::ArchivedCard::new(entity_id, Uuid::new_v4())
    }

    fn archived_board_marker(entity_id: Uuid) -> kanban_domain::ArchivedBoard {
        kanban_domain::Archived::now(entity_id)
    }

    #[test]
    fn test_archived_cards_view_requests_the_board_scoped_marker_tier() {
        let board = Uuid::new_v4();
        let stub = StubLoaded::default();
        let scope = ViewScope {
            board: Some(board),
            archived_card_markers: true,
            ..Default::default()
        };

        let round = scope.next_round(&stub);
        assert_eq!(round.archived_cards_by_board, vec![board]);
        assert!(!round.archived_card_list);
    }

    #[test]
    fn test_archived_body_walk_rides_the_by_board_markers_into_per_id_fetches() {
        let board = Uuid::new_v4();
        let m1 = Uuid::new_v4();
        let m2 = Uuid::new_v4();
        let stub = StubLoaded {
            archived_cards_of_board: HashMap::from([(board, FetchStatus::Loaded)]),
            archived_cards_by_board_markers: HashMap::from([(
                board,
                vec![archived_card_marker(m1), archived_card_marker(m2)],
            )]),
            card_status_by_id: HashMap::from([(m2, FetchStatus::Loaded)]),
            ..StubLoaded::default()
        };
        let scope = ViewScope {
            board: Some(board),
            archived_card_bodies: true,
            ..Default::default()
        };

        let round = scope.next_round(&stub);
        assert_eq!(round.cards, vec![m1]);
        assert!(!round.archived_card_list);
    }

    #[test]
    fn test_archived_boards_view_requests_the_marker_tier_then_the_heads() {
        let stub1 = StubLoaded::default();
        let scope = ViewScope {
            archived_board_bodies: true,
            ..Default::default()
        };

        let round1 = scope.next_round(&stub1);
        assert!(round1.archived_board_list);
        assert!(round1.boards.is_empty());

        let m1 = Uuid::new_v4();
        let m2 = Uuid::new_v4();
        let stub2 = StubLoaded {
            archived_board_list: FetchStatus::Loaded,
            archived_board_markers: Some(vec![
                archived_board_marker(m1),
                archived_board_marker(m2),
            ]),
            ..StubLoaded::default()
        };

        let round2 = scope.next_round(&stub2);
        let mut expected = vec![m1, m2];
        expected.sort();
        assert_eq!(round2.boards, expected);
    }

    #[test]
    fn test_delete_board_confirm_requests_by_board_markers() {
        let board = Uuid::new_v4();
        let stub = StubLoaded::default();
        let scope = ViewScope {
            board: Some(board),
            archived_card_markers: true,
            ..Default::default()
        };

        let round = scope.next_round(&stub);
        assert_eq!(round.archived_cards_by_board, vec![board]);
        assert!(!round.archived_card_list);
        assert!(round.cards.is_empty());
    }

    #[test]
    fn test_no_board_in_scope_requests_no_archived_card_tier() {
        let stub = StubLoaded::default();
        let scope = ViewScope {
            board: None,
            archived_card_markers: true,
            archived_card_bodies: true,
            ..Default::default()
        };

        let round = scope.next_round(&stub);
        assert!(!round.archived_card_list);
        assert!(round.archived_cards_by_board.is_empty());
        assert!(round.cards.is_empty());
    }
}
