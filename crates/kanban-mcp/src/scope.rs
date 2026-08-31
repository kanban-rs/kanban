#![allow(dead_code)]

use kanban_service::{requestable, FetchPlan, FetchRound, LoadedEntities};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Ref {
    Id,
    Name,
}

impl Ref {
    pub(crate) fn of(raw: &str) -> Self {
        if Uuid::parse_str(raw).is_ok() {
            Ref::Id
        } else {
            Ref::Name
        }
    }
}

#[derive(Debug, Default)]
pub(crate) struct ToolScope {
    pub(crate) board: Option<Ref>,
    pub(crate) column: Option<Ref>,
    pub(crate) sprint: Option<Ref>,
    pub(crate) cards: Vec<Ref>,
    pub(crate) wants_graph: bool,
    /// Some tools render the board entity itself (not just resolve it by
    /// name), so even a uuid reference still needs the board list loaded.
    pub(crate) renders_board_entity: bool,
    /// Set once a board reference has been resolved to an id; the
    /// parent-scoped tiers cannot be requested before it is known.
    pub(crate) resolved_board: Option<Uuid>,
    /// Requests the board's columns independently of how, or whether, a
    /// column reference is written.
    pub(crate) wants_board_columns: bool,
    pub(crate) wants_board_sprints: bool,
}

impl ToolScope {
    pub(crate) fn for_board(mut self, board_id: Uuid) -> Self {
        self.resolved_board = Some(board_id);
        self
    }
}

pub(crate) trait ToolScoped {
    fn scope(&self) -> ToolScope;
}

impl FetchPlan for ToolScope {
    fn next_round(&self, loaded: &dyn LoadedEntities) -> FetchRound {
        let wants_board_list = matches!(self.board, Some(Ref::Name))
            || (self.renders_board_entity && self.board.is_some())
            || (self.resolved_board.is_some() && self.wants_board_sprints);
        FetchRound {
            board_list: wants_board_list && requestable(loaded.board_list()),
            column_list: matches!(self.column, Some(Ref::Name))
                && !self.wants_board_columns
                && requestable(loaded.column_list()),
            card_list: self.cards.iter().any(|r| matches!(r, Ref::Name))
                && requestable(loaded.card_list()),
            sprint_list: matches!(self.sprint, Some(Ref::Name))
                && !self.wants_board_sprints
                && requestable(loaded.sprint_list()),
            graph: self.wants_graph && requestable(loaded.graph()),
            columns_by_board: self
                .resolved_board
                .filter(|_| self.wants_board_columns)
                .filter(|id| requestable(loaded.columns_of_board(*id)))
                .into_iter()
                .collect(),
            sprints_by_board: self
                .resolved_board
                .filter(|_| self.wants_board_sprints)
                .filter(|id| requestable(loaded.sprints_of_board(*id)))
                .into_iter()
                .collect(),
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kanban_domain::{EntityIds, KanbanError, LoadState, Model, Resolved};
    use std::sync::Arc;

    #[test]
    fn test_tool_scope_from_named_board_requests_the_board_list() {
        let scope = ToolScope {
            board: Some(Ref::Name),
            ..Default::default()
        };

        assert_eq!(
            scope.next_round(&Model::default()),
            FetchRound {
                board_list: true,
                ..Default::default()
            }
        );
    }

    #[test]
    fn test_tool_scope_from_uuid_board_requests_no_board_tier() {
        let scope = ToolScope {
            board: Some(Ref::Id),
            ..Default::default()
        };

        assert!(scope.next_round(&Model::default()).is_empty());
        assert_eq!(Ref::of(&Uuid::new_v4().to_string()), Ref::Id);
        assert_eq!(Ref::of("Kanban"), Ref::Name);
    }

    #[test]
    fn test_tool_scope_from_uuid_card_requests_no_card_tier() {
        let scope = ToolScope {
            cards: vec![Ref::Id],
            ..Default::default()
        };
        let round = scope.next_round(&Model::default());

        assert!(!round.card_list);
        assert!(round.cards.is_empty());
        assert!(round.is_empty());

        let named_scope = ToolScope {
            cards: vec![Ref::Name],
            ..Default::default()
        };
        assert!(named_scope.next_round(&Model::default()).card_list);
    }

    #[test]
    fn test_tool_scope_for_get_board_requests_the_board_list_even_for_a_uuid() {
        let scope = ToolScope {
            board: Some(Ref::Id),
            renders_board_entity: true,
            ..Default::default()
        };
        assert!(scope.next_round(&Model::default()).board_list);

        let non_rendering_scope = ToolScope {
            board: Some(Ref::Id),
            renders_board_entity: false,
            ..Default::default()
        };
        assert!(non_rendering_scope.next_round(&Model::default()).is_empty());
    }

    #[test]
    fn test_a_named_column_in_board_resolves_after_a_second_round() {
        let scope = ToolScope {
            board: Some(Ref::Name),
            column: Some(Ref::Name),
            wants_board_columns: true,
            ..Default::default()
        };

        let round1 = scope.next_round(&Model::default());
        assert!(round1.board_list);
        assert!(round1.columns_by_board.is_empty());

        let mut model = Model::default();
        let _ = model.apply_resolved(Resolved {
            boards: kanban_domain::resolved::Collection {
                all: LoadState::Loaded(vec![]),
                ..Default::default()
            },
            ..Default::default()
        });

        let board_id = Uuid::new_v4();
        let round = scope.for_board(board_id).next_round(&model);
        assert_eq!(round.columns_by_board, vec![board_id]);
        assert!(!round.column_list);
        assert!(!round.board_list);
    }

    #[test]
    fn test_a_named_sprint_in_board_resolves_after_a_second_round() {
        let scope = ToolScope {
            board: Some(Ref::Id),
            sprint: Some(Ref::Name),
            wants_board_sprints: true,
            ..Default::default()
        };

        let board_id = Uuid::new_v4();
        let round = scope.for_board(board_id).next_round(&Model::default());
        assert_eq!(round.sprints_by_board, vec![board_id]);
        assert!(!round.sprint_list);
        assert!(round.board_list);
    }

    #[test]
    fn test_tool_scope_stops_requesting_once_the_by_parent_tiers_are_loaded() {
        let board_id = Uuid::new_v4();
        let scope = ToolScope {
            board: Some(Ref::Name),
            column: Some(Ref::Name),
            sprint: Some(Ref::Name),
            wants_board_columns: true,
            wants_board_sprints: true,
            ..Default::default()
        }
        .for_board(board_id);

        let mut model = Model::default();
        let _ = model.apply_resolved(Resolved {
            boards: kanban_domain::resolved::Collection {
                all: LoadState::Loaded(vec![]),
                ..Default::default()
            },
            columns: kanban_domain::resolved::Collection {
                by_parent: [(board_id, LoadState::Loaded(vec![]))].into(),
                ..Default::default()
            },
            sprints: kanban_domain::resolved::Collection {
                by_parent: [(board_id, LoadState::Loaded(vec![]))].into(),
                ..Default::default()
            },
            ..Default::default()
        });

        assert!(scope.next_round(&model).is_empty());
    }

    #[test]
    fn test_a_by_parent_tier_is_not_requested_when_the_tool_does_not_want_it() {
        let board_id = Uuid::new_v4();
        let scope = ToolScope {
            board: Some(Ref::Id),
            column: Some(Ref::Name),
            ..Default::default()
        }
        .for_board(board_id);

        let round = scope.next_round(&Model::default());
        assert!(round.columns_by_board.is_empty());
        assert!(round.column_list);
    }

    #[test]
    fn test_by_parent_tiers_are_not_requested_before_the_board_is_resolved() {
        let scope = ToolScope {
            board: Some(Ref::Name),
            column: Some(Ref::Name),
            wants_board_columns: true,
            wants_board_sprints: true,
            ..Default::default()
        };

        let round = scope.next_round(&Model::default());
        assert!(round.columns_by_board.is_empty());
        assert!(round.sprints_by_board.is_empty());
        assert!(round.board_list);
    }

    #[test]
    fn test_a_wanted_by_parent_tier_suppresses_its_flat_tier() {
        let scope = ToolScope {
            column: Some(Ref::Name),
            sprint: Some(Ref::Name),
            wants_board_columns: true,
            wants_board_sprints: true,
            ..Default::default()
        };
        let round = scope.next_round(&Model::default());
        assert!(!round.column_list);
        assert!(!round.sprint_list);

        let control = ToolScope {
            column: Some(Ref::Name),
            sprint: Some(Ref::Name),
            ..Default::default()
        };
        let control_round = control.next_round(&Model::default());
        assert!(control_round.column_list);
        assert!(control_round.sprint_list);
    }

    #[test]
    fn test_tool_scope_stops_requesting_once_everything_is_loaded() {
        let scope = ToolScope {
            board: Some(Ref::Name),
            column: Some(Ref::Name),
            sprint: Some(Ref::Name),
            cards: vec![Ref::Name],
            wants_graph: true,
            renders_board_entity: false,
            resolved_board: None,
            wants_board_columns: false,
            wants_board_sprints: false,
        };

        let mut model = Model::default();
        let _ = model.apply_resolved(Resolved {
            boards: kanban_domain::resolved::Collection {
                all: LoadState::Loaded(vec![]),
                ..Default::default()
            },
            columns: kanban_domain::resolved::Collection {
                all: LoadState::Loaded(vec![]),
                ..Default::default()
            },
            cards: kanban_domain::resolved::Collection {
                all: LoadState::Loaded(vec![]),
                ..Default::default()
            },
            sprints: kanban_domain::resolved::Collection {
                all: LoadState::Loaded(vec![]),
                ..Default::default()
            },
            graph: LoadState::Loaded(Default::default()),
            ..Default::default()
        });

        assert!(scope.next_round(&model).is_empty());
    }

    #[test]
    fn test_tool_scope_retries_a_failed_tier_but_not_a_missing_one() {
        let scope = ToolScope {
            board: Some(Ref::Name),
            ..Default::default()
        };

        let mut failed_model = Model::default();
        let _ = failed_model.mark_failed(
            EntityIds::boards([Uuid::new_v4()]),
            Arc::new(KanbanError::Database("boom".into())),
        );
        assert!(scope.next_round(&failed_model).board_list);

        let mut missing_model = Model::default();
        let _ = missing_model.apply_resolved(Resolved {
            boards: kanban_domain::resolved::Collection {
                all: LoadState::Missing,
                ..Default::default()
            },
            ..Default::default()
        });
        assert!(scope.next_round(&missing_model).is_empty());
    }

    /// Mirrors crates/kanban-cli/src/scope.rs:86-95: a `Some(Ref::Name)`
    /// board reference requests exactly `board_list`, and a `Some(Ref::Id)`
    /// one requests nothing.
    #[test]
    fn test_tool_scope_matches_command_scope_for_the_same_reference_shape() {
        let named = ToolScope {
            board: Some(Ref::Name),
            ..Default::default()
        };
        assert_eq!(
            named.next_round(&Model::default()),
            FetchRound {
                board_list: true,
                ..Default::default()
            }
        );

        let by_id = ToolScope {
            board: Some(Ref::Id),
            ..Default::default()
        };
        assert!(by_id.next_round(&Model::default()).is_empty());
    }
}
