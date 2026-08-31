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
}

pub(crate) trait ToolScoped {
    fn scope(&self) -> ToolScope;
}

impl FetchPlan for ToolScope {
    fn next_round(&self, _loaded: &dyn LoadedEntities) -> FetchRound {
        FetchRound::default()
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
    fn test_tool_scope_stops_requesting_once_everything_is_loaded() {
        let scope = ToolScope {
            board: Some(Ref::Name),
            column: Some(Ref::Name),
            sprint: Some(Ref::Name),
            cards: vec![Ref::Name],
            wants_graph: true,
            renders_board_entity: false,
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
