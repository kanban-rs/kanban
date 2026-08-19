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
        resolved.cards.insert(
            id,
            LoadState::Failed(Arc::new(KanbanError::unsupported("x"))),
        );

        let cloned = resolved.clone();
        match (&resolved.cards[&id], &cloned.cards[&id]) {
            (LoadState::Failed(a), LoadState::Failed(b)) => assert!(Arc::ptr_eq(a, b)),
            _ => panic!("expected Failed variant"),
        }
    }

    #[derive(Debug, Clone)]
    struct NoDefault;

    #[test]
    fn test_collection_default_is_untouched() {
        let c = Collection::<Card>::default();
        assert!(c.all.is_not_loaded());
        assert!(c.by_id.is_empty());
        assert!(c.is_untouched());
    }

    #[test]
    fn test_a_collection_with_a_loaded_empty_all_is_not_untouched() {
        let c = Collection::<Column> {
            all: LoadState::Loaded(vec![]),
            ..Default::default()
        };
        assert!(!c.is_untouched());

        let mut c = Collection::<Column>::default();
        c.by_id.insert(Uuid::new_v4(), LoadState::Missing);
        assert!(!c.is_untouched());
    }

    #[test]
    fn test_resolved_columns_can_be_loaded_and_empty() {
        let r = Resolved {
            columns: Collection {
                all: LoadState::Loaded(vec![]),
                ..Default::default()
            },
            ..Default::default()
        };
        assert!(r.columns.all.is_loaded());
        assert!(r.columns.all.loaded().unwrap().is_empty());
        assert!(!r.columns.is_untouched());

        let untouched = Resolved::default();
        assert!(untouched.columns.all.is_not_loaded());
        assert!(untouched.columns.is_untouched());
    }

    #[test]
    fn test_resolved_can_name_the_whole_card_list_and_one_missing_card() {
        let card = Card::new(Uuid::new_v4(), Uuid::new_v4(), "a", 0);
        let card_id = card.id;
        let other = Uuid::new_v4();

        let mut r = Resolved::default();
        r.cards.all = LoadState::Loaded(vec![card]);
        r.cards.by_id.insert(other, LoadState::Missing);

        assert_eq!(r.cards.all.loaded().unwrap().len(), 1);
        assert_eq!(r.cards.all.loaded().unwrap()[0].id, card_id);
        assert_eq!(r.cards.by_id.len(), 1);
        assert!(r.cards.by_id[&other].is_missing());
    }

    #[test]
    fn test_collection_default_works_for_a_non_default_type() {
        let c = Collection::<NoDefault>::default();
        assert!(c.is_untouched());

        let loaded = Collection {
            all: LoadState::Loaded(vec![NoDefault]),
            by_id: HashMap::new(),
        };
        assert!(!loaded.is_untouched());
        let _ = loaded.clone();
        let _ = format!("{loaded:?}");
    }
}
