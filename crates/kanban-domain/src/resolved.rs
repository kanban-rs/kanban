use std::collections::HashMap;

use uuid::Uuid;

use crate::board::Board;
use crate::card::Card;
use crate::column::Column;
use crate::dependencies::DependencyGraph;
use crate::load_state::LoadState;
use crate::sprint::Sprint;

/// The outcome of one resolve pass. A `NotLoaded` field or an absent map key
/// means the pass did not touch that entity, and applying the result must
/// leave it as it was.
#[derive(Debug, Clone, Default)]
pub struct Resolved {
    pub boards: LoadState<Vec<Board>>,
    pub graph: LoadState<DependencyGraph>,
    pub columns: HashMap<Uuid, LoadState<Column>>,
    pub cards: HashMap<Uuid, LoadState<Card>>,
    pub sprints: HashMap<Uuid, LoadState<Sprint>>,
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use uuid::Uuid;

    use super::*;
    use crate::error::KanbanError;
    use crate::load_state::LoadState;

    #[test]
    fn test_resolved_default_mentions_no_entity_kind() {
        let resolved = Resolved::default();
        assert!(resolved.boards.is_not_loaded());
        assert!(resolved.graph.is_not_loaded());
        assert!(resolved.columns.is_empty());
        assert!(resolved.cards.is_empty());
        assert!(resolved.sprints.is_empty());
    }

    #[test]
    fn test_resolved_distinguishes_a_loaded_empty_board_list_from_an_unmentioned_one() {
        let loaded_empty = Resolved {
            boards: LoadState::Loaded(vec![]),
            ..Default::default()
        };
        assert!(loaded_empty.boards.is_loaded());
        assert!(loaded_empty.boards.loaded().unwrap().is_empty());

        let unmentioned = Resolved::default();
        assert!(unmentioned.boards.is_not_loaded());
    }

    #[test]
    fn test_resolved_carries_per_entity_missing_for_a_card_the_backend_did_not_have() {
        let id = Uuid::new_v4();
        let mut resolved = Resolved::default();
        resolved.cards.insert(id, LoadState::Missing);

        assert!(resolved.cards[&id].is_missing());
        assert!(resolved.cards[&id].is_terminal());
    }

    #[test]
    fn test_resolved_clone_shares_the_same_failed_error() {
        let id = Uuid::new_v4();
        let mut resolved = Resolved::default();
        resolved
            .cards
            .insert(id, LoadState::Failed(Arc::new(KanbanError::unsupported("x"))));

        let cloned = resolved.clone();
        match (&resolved.cards[&id], &cloned.cards[&id]) {
            (LoadState::Failed(a), LoadState::Failed(b)) => assert!(Arc::ptr_eq(a, b)),
            _ => panic!("expected Failed variant"),
        }
    }
}
