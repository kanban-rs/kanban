//! A `RouteScope` is built from the matched route plus its path/query
//! params. The tiers a variant names are the tiers that route's response
//! body reads. Two routes have no variant and stay lock-only reads outside
//! this plan: `GET /v1/prefixes`, because no prefix tier exists in
//! `FetchRound` at any level, and `GET /v1/cards/lookup`, because it
//! dispatches straight to the indexed store reads behind
//! `find_cards_by_identifier` rather than through any `FetchRound` tier.

use kanban_domain::ArchivedFilter;
use kanban_service::{requestable, FetchPlan, FetchRound, LoadedEntities};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteScope {
    BoardList,
    ArchivedBoardList,
    Board(Uuid),
    BoardColumns(Uuid),
    BoardCards {
        board_id: Uuid,
        column_id: Option<Uuid>,
        archived: ArchivedFilter,
        /// Plans the board's sprint tier, which the search predicate reads
        /// to match a card's branch name.
        search: bool,
    },
    BoardArchivedCards(Uuid),
    BoardSprints(Uuid),
    Card(Uuid),
    Column(Uuid),
    /// `board_id` is `Some` only for the nested route, where the path
    /// already names the board whose name pool the response needs.
    Sprint {
        board_id: Option<Uuid>,
        sprint_id: Uuid,
    },
    CardGraph(Uuid),
    Graph,
}

fn want_board(round: &mut FetchRound, loaded: &dyn LoadedEntities, board_id: Uuid) {
    if requestable(loaded.board(board_id)) {
        round.boards.push(board_id);
    }
}

impl FetchPlan for RouteScope {
    fn next_round(&self, loaded: &dyn LoadedEntities) -> FetchRound {
        let mut round = FetchRound::default();

        match *self {
            RouteScope::BoardList => {
                round.board_list = requestable(loaded.board_list());
            }
            RouteScope::ArchivedBoardList => {
                round.archived_board_list = requestable(loaded.archived_board_list());
            }
            RouteScope::Board(id) => {
                want_board(&mut round, loaded, id);
                round.archived_board_list = requestable(loaded.archived_board_list());
            }
            RouteScope::BoardColumns(board_id) => {
                want_board(&mut round, loaded, board_id);
                if requestable(loaded.columns_of_board(board_id)) {
                    round.columns_by_board.push(board_id);
                }
            }
            RouteScope::BoardCards {
                board_id,
                column_id,
                archived,
                search,
            } => {
                want_board(&mut round, loaded, board_id);
                if requestable(loaded.columns_of_board(board_id)) {
                    round.columns_by_board.push(board_id);
                }
                match column_id {
                    Some(column_id) => {
                        if requestable(loaded.cards_of_column(column_id)) {
                            round.cards_by_column.push(column_id);
                        }
                    }
                    None => {
                        if let Some(columns) = loaded.loaded_columns_of_board(board_id) {
                            for column in columns {
                                if requestable(loaded.cards_of_column(column.id)) {
                                    round.cards_by_column.push(column.id);
                                }
                            }
                        }
                    }
                }
                if archived != ArchivedFilter::LiveOnly {
                    round.archived_card_list = requestable(loaded.archived_card_list());
                    if let Some(markers) = loaded.loaded_archived_card_markers() {
                        for marker in markers {
                            if marker.context.board_id == board_id
                                && requestable(loaded.card(marker.entity_id))
                            {
                                round.cards.push(marker.entity_id);
                            }
                        }
                    }
                }
                if search && requestable(loaded.sprints_of_board(board_id)) {
                    round.sprints_by_board.push(board_id);
                }
            }
            RouteScope::BoardArchivedCards(board_id) => {
                want_board(&mut round, loaded, board_id);
                if requestable(loaded.archived_cards_of_board(board_id)) {
                    round.archived_cards_by_board.push(board_id);
                }
            }
            RouteScope::BoardSprints(board_id) => {
                want_board(&mut round, loaded, board_id);
                if requestable(loaded.sprints_of_board(board_id)) {
                    round.sprints_by_board.push(board_id);
                }
            }
            RouteScope::Card(id) => {
                if requestable(loaded.card(id)) {
                    round.cards.push(id);
                }
            }
            RouteScope::Column(id) => {
                if requestable(loaded.column(id)) {
                    round.columns.push(id);
                }
            }
            RouteScope::Sprint {
                board_id,
                sprint_id,
            } => {
                if requestable(loaded.sprint(sprint_id)) {
                    round.sprints.push(sprint_id);
                }
                if let Some(board_id) = board_id {
                    want_board(&mut round, loaded, board_id);
                }
            }
            RouteScope::CardGraph(id) => {
                round.graph = requestable(loaded.graph());
                if requestable(loaded.card(id)) {
                    round.cards.push(id);
                }
            }
            RouteScope::Graph => {
                round.graph = requestable(loaded.graph());
            }
        }

        round
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use kanban_domain::resolved::Collection;
    use kanban_domain::{
        ArchivedFilter, Board, Card, Column, KanbanError, LoadState, Model, Resolved,
    };
    use uuid::Uuid;

    use super::*;
    use kanban_service::FetchPlan;

    #[test]
    fn test_route_scope_for_archived_board_list_requests_only_the_marker_tier() {
        let round = RouteScope::ArchivedBoardList.next_round(&Model::default());

        assert_eq!(
            round,
            FetchRound {
                archived_board_list: true,
                ..Default::default()
            }
        );
        assert!(!round.board_list);
        assert!(round.boards.is_empty());
    }

    #[test]
    fn test_route_scope_for_archived_board_list_halts_once_the_markers_are_loaded() {
        let mut model = Model::default();
        let _ = model.apply_resolved(Resolved {
            archived_boards: Collection {
                all: LoadState::Loaded(vec![]),
                ..Default::default()
            },
            ..Default::default()
        });

        let round = RouteScope::ArchivedBoardList.next_round(&model);
        assert!(round.is_empty());
    }

    #[test]
    fn test_route_scope_for_board_list_requests_only_the_board_list() {
        let round = RouteScope::BoardList.next_round(&Model::default());

        assert_eq!(
            round,
            FetchRound {
                board_list: true,
                ..Default::default()
            }
        );
        assert!(!round.archived_board_list);
        assert!(round.boards.is_empty());
    }

    #[test]
    fn test_route_scope_for_single_board_requests_the_board_by_id_and_archived_markers() {
        let id = Uuid::new_v4();
        let round = RouteScope::Board(id).next_round(&Model::default());

        assert_eq!(round.boards, vec![id]);
        assert!(round.archived_board_list);
        assert!(!round.board_list);
    }

    #[test]
    fn test_route_scope_for_board_cards_requests_only_board_scoped_tiers() {
        let board_id = Uuid::new_v4();
        let round = RouteScope::BoardCards {
            board_id,
            column_id: None,
            archived: ArchivedFilter::LiveOnly,
            search: false,
        }
        .next_round(&Model::default());

        assert_eq!(round.boards, vec![board_id]);
        assert_eq!(round.columns_by_board, vec![board_id]);
        assert!(round.archived_cards_by_board.is_empty());
        assert!(!round.board_list);
        assert!(round.cards_by_column.is_empty());
    }

    #[test]
    fn test_route_scope_for_board_cards_walks_loaded_columns_into_a_second_round_then_halts() {
        let board = Board::new("Kanban", None::<String>);
        let board_id = board.id;
        let column = Column::new(board_id, "TODO", 0);
        let column_id = column.id;

        let mut model = Model::default();
        let _ = model.apply_resolved(Resolved {
            boards: Collection {
                by_id: [(board_id, LoadState::Loaded(board))].into(),
                ..Default::default()
            },
            columns: Collection {
                by_parent: [(board_id, LoadState::Loaded(vec![column]))].into(),
                ..Default::default()
            },
            archived_cards: Collection {
                by_parent: [(board_id, LoadState::Loaded(vec![]))].into(),
                ..Default::default()
            },
            ..Default::default()
        });

        let scope = RouteScope::BoardCards {
            board_id,
            column_id: None,
            archived: ArchivedFilter::LiveOnly,
            search: false,
        };
        let round2 = scope.next_round(&model);
        assert_eq!(
            round2,
            FetchRound {
                cards_by_column: vec![column_id],
                ..Default::default()
            }
        );

        let mut model2 = model;
        let _ = model2.apply_resolved(Resolved {
            cards: Collection {
                by_parent: [(column_id, LoadState::Loaded(vec![]))].into(),
                ..Default::default()
            },
            ..Default::default()
        });
        let round3 = scope.next_round(&model2);
        assert!(round3.is_empty());
    }

    #[test]
    fn test_route_scope_for_board_cards_filtered_by_column_requests_that_column_in_the_first_round()
    {
        let board_id = Uuid::new_v4();
        let column_id = Uuid::new_v4();
        let round = RouteScope::BoardCards {
            board_id,
            column_id: Some(column_id),
            archived: ArchivedFilter::LiveOnly,
            search: false,
        }
        .next_round(&Model::default());

        assert_eq!(round.cards_by_column, vec![column_id]);
        assert_eq!(round.boards, vec![board_id]);
        assert_eq!(round.columns_by_board, vec![board_id]);
    }

    #[test]
    fn test_route_scope_for_board_cards_live_only_plans_no_archived_tier() {
        let board_id = Uuid::new_v4();
        let round = RouteScope::BoardCards {
            board_id,
            column_id: None,
            archived: ArchivedFilter::LiveOnly,
            search: false,
        }
        .next_round(&Model::default());

        assert_eq!(round.boards, vec![board_id]);
        assert_eq!(round.columns_by_board, vec![board_id]);
        assert!(round.archived_cards_by_board.is_empty());
        assert!(!round.archived_card_list);
        assert!(round.cards.is_empty());
        assert!(!round.board_list);
    }

    #[test]
    fn test_route_scope_for_board_cards_including_archived_walks_global_markers_of_that_board_into_a_card_round(
    ) {
        use kanban_domain::ArchivedCard;

        let board_id = Uuid::new_v4();
        let other_board_id = Uuid::new_v4();
        let column = Column::new(board_id, "TODO", 0);
        let column_id = column.id;
        let marker_for_this_board = ArchivedCard::new(Uuid::new_v4(), board_id);
        let marker_for_other_board = ArchivedCard::new(Uuid::new_v4(), other_board_id);
        let this_board_entity_id = marker_for_this_board.entity_id;

        let scope = RouteScope::BoardCards {
            board_id,
            column_id: None,
            archived: ArchivedFilter::Include,
            search: false,
        };

        let round1 = scope.next_round(&Model::default());
        assert!(round1.archived_card_list);
        assert!(round1.cards.is_empty());

        let mut model = Model::default();
        let _ = model.apply_resolved(Resolved {
            boards: Collection {
                by_id: [(
                    board_id,
                    LoadState::Loaded(Board::new("Kanban", None::<String>)),
                )]
                .into(),
                ..Default::default()
            },
            columns: Collection {
                by_parent: [(board_id, LoadState::Loaded(vec![column]))].into(),
                ..Default::default()
            },
            archived_cards: Collection {
                all: LoadState::Loaded(vec![marker_for_this_board, marker_for_other_board]),
                ..Default::default()
            },
            ..Default::default()
        });

        let round2 = scope.next_round(&model);
        assert_eq!(round2.cards, vec![this_board_entity_id]);
        assert_eq!(round2.cards_by_column, vec![column_id]);

        let mut model2 = model;
        let _ = model2.apply_resolved(Resolved {
            cards: Collection {
                by_id: [(
                    this_board_entity_id,
                    LoadState::Loaded(Card::new(board_id, column_id, "archived", 0)),
                )]
                .into(),
                by_parent: [(column_id, LoadState::Loaded(vec![]))].into(),
                ..Default::default()
            },
            ..Default::default()
        });
        let round3 = scope.next_round(&model2);
        assert!(round3.is_empty());
    }

    #[test]
    fn test_route_scope_for_board_cards_archived_only_plans_the_same_rounds_as_include() {
        let board_id = Uuid::new_v4();
        let include_round = RouteScope::BoardCards {
            board_id,
            column_id: None,
            archived: ArchivedFilter::Include,
            search: false,
        }
        .next_round(&Model::default());
        let archived_only_round = RouteScope::BoardCards {
            board_id,
            column_id: None,
            archived: ArchivedFilter::ArchivedOnly,
            search: false,
        }
        .next_round(&Model::default());

        assert_eq!(include_round, archived_only_round);
    }

    #[test]
    fn test_route_scope_for_board_cards_with_search_plans_the_board_sprint_tier() {
        let board_id = Uuid::new_v4();
        let round = RouteScope::BoardCards {
            board_id,
            column_id: None,
            archived: ArchivedFilter::LiveOnly,
            search: true,
        }
        .next_round(&Model::default());

        assert_eq!(round.sprints_by_board, vec![board_id]);
    }

    #[test]
    fn test_route_scope_for_board_cards_without_search_plans_no_sprint_tier() {
        let board_id = Uuid::new_v4();
        let round = RouteScope::BoardCards {
            board_id,
            column_id: None,
            archived: ArchivedFilter::LiveOnly,
            search: false,
        }
        .next_round(&Model::default());

        assert!(round.sprints_by_board.is_empty());
    }

    #[test]
    fn test_route_scope_for_board_archived_cards_requests_the_board_and_its_markers() {
        let board_id = Uuid::new_v4();
        let round = RouteScope::BoardArchivedCards(board_id).next_round(&Model::default());

        assert_eq!(round.boards, vec![board_id]);
        assert_eq!(round.archived_cards_by_board, vec![board_id]);
        assert!(!round.board_list);
        assert!(!round.archived_card_list);
    }

    #[test]
    fn test_route_scope_for_board_columns_and_board_sprints_request_the_board_and_its_scope() {
        let board_id = Uuid::new_v4();

        let columns_round = RouteScope::BoardColumns(board_id).next_round(&Model::default());
        assert_eq!(columns_round.boards, vec![board_id]);
        assert_eq!(columns_round.columns_by_board, vec![board_id]);
        assert_eq!(
            columns_round,
            FetchRound {
                boards: vec![board_id],
                columns_by_board: vec![board_id],
                ..Default::default()
            }
        );

        let sprints_round = RouteScope::BoardSprints(board_id).next_round(&Model::default());
        assert_eq!(
            sprints_round,
            FetchRound {
                boards: vec![board_id],
                sprints_by_board: vec![board_id],
                ..Default::default()
            }
        );
    }

    #[test]
    fn test_route_scope_for_a_nested_sprint_requests_its_board_but_the_flat_form_cannot() {
        let board_id = Uuid::new_v4();
        let sprint_id = Uuid::new_v4();

        let nested_round = RouteScope::Sprint {
            board_id: Some(board_id),
            sprint_id,
        }
        .next_round(&Model::default());
        assert_eq!(nested_round.sprints, vec![sprint_id]);
        assert_eq!(nested_round.boards, vec![board_id]);

        let flat_round = RouteScope::Sprint {
            board_id: None,
            sprint_id,
        }
        .next_round(&Model::default());
        assert_eq!(flat_round.sprints, vec![sprint_id]);
        assert!(flat_round.boards.is_empty());
        assert!(!flat_round.board_list);
    }

    #[test]
    fn test_route_scope_for_a_card_or_column_requests_only_that_entitys_per_id_tier() {
        let card_id = Uuid::new_v4();
        let card_round = RouteScope::Card(card_id).next_round(&Model::default());
        assert_eq!(card_round.cards, vec![card_id]);
        assert!(!card_round.is_empty());
        assert!(!card_round.board_list);
        assert!(card_round.boards.is_empty());
        assert!(card_round.columns_by_board.is_empty());

        let column_id = Uuid::new_v4();
        let column_round = RouteScope::Column(column_id).next_round(&Model::default());
        assert_eq!(column_round.columns, vec![column_id]);
    }

    #[test]
    fn test_route_scope_for_card_graph_requests_the_graph_and_the_card() {
        let card_id = Uuid::new_v4();
        let round = RouteScope::CardGraph(card_id).next_round(&Model::default());

        assert!(round.graph);
        assert_eq!(round.cards, vec![card_id]);
    }

    #[test]
    fn test_route_scope_for_graph_requests_only_the_graph_tier() {
        let round = RouteScope::Graph.next_round(&Model::default());

        assert_eq!(
            round,
            FetchRound {
                graph: true,
                ..Default::default()
            }
        );

        let mut model = Model::default();
        let _ = model.apply_resolved(kanban_domain::Resolved {
            graph: LoadState::Loaded(kanban_domain::DependencyGraph::default()),
            ..Default::default()
        });
        let second_round = RouteScope::Graph.next_round(&model);
        assert!(second_round.is_empty());
    }

    #[test]
    fn test_route_scope_retries_a_failed_tier_but_not_a_missing_one() {
        let id = Uuid::new_v4();

        let mut missing_model = Model::default();
        let _ = missing_model.apply_resolved(Resolved {
            boards: Collection {
                by_id: [(id, LoadState::Missing)].into(),
                ..Default::default()
            },
            ..Default::default()
        });
        let missing_round = RouteScope::Board(id).next_round(&missing_model);
        assert!(missing_round.boards.is_empty());

        let mut failed_model = Model::default();
        let _ = failed_model.apply_resolved(Resolved {
            boards: Collection {
                by_id: [(
                    id,
                    LoadState::Failed(Arc::new(KanbanError::Database("boom".into()))),
                )]
                .into(),
                ..Default::default()
            },
            ..Default::default()
        });
        let failed_round = RouteScope::Board(id).next_round(&failed_model);
        assert_eq!(failed_round.boards, vec![id]);
    }
}
