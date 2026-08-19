use std::collections::HashMap;

use uuid::Uuid;

use crate::board::Board;
use crate::card::Card;
use crate::column::Column;
use crate::dependencies::DependencyGraph;
use crate::load_state::LoadState;
use crate::sprint::Sprint;

/// One entity kind's slice of a resolve pass, in two independent tiers.
///
/// `all` speaks about the whole collection; `by_id` speaks about individual
/// entities. `NotLoaded` in `all` and an empty `by_id` each mean the pass did
/// not touch that tier, so applying it leaves the target unchanged.
///
/// Appliers must, per entity kind: first, if `all` is `Loaded`, replace the
/// whole target collection with it; second, apply every `by_id` entry on top.
/// That order is what lets one pass say "here is the whole card list, and
/// additionally card X is `Missing`". Reversed, a whole-collection result
/// would silently overwrite a fresher per-id one.
///
/// `all` being `Missing` or `Failed` describes the collection read itself, not
/// any member of it.
#[derive(Debug, Clone)]
pub struct Collection<T> {
    pub all: LoadState<Vec<T>>,
    pub by_id: HashMap<Uuid, LoadState<T>>,
}

impl<T> Default for Collection<T> {
    fn default() -> Self {
        Self {
            all: LoadState::NotLoaded,
            by_id: HashMap::new(),
        }
    }
}

impl<T> Collection<T> {
    pub fn is_untouched(&self) -> bool {
        self.all.is_not_loaded() && self.by_id.is_empty()
    }
}

/// The outcome of one resolve pass. Each entity kind that has an id carries a
/// `Collection`; `graph` is a singleton and has no id to key a `by_id` map on,
/// so it stays a bare `LoadState`.
#[derive(Debug, Clone, Default)]
pub struct Resolved {
    pub boards: Collection<Board>,
    pub columns: Collection<Column>,
    pub cards: Collection<Card>,
    pub sprints: Collection<Sprint>,
    pub graph: LoadState<DependencyGraph>,
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
        assert!(resolved.boards.is_untouched());
        assert!(resolved.graph.is_not_loaded());
        assert!(resolved.columns.is_untouched());
        assert!(resolved.cards.is_untouched());
        assert!(resolved.sprints.is_untouched());
    }

    #[test]
    fn test_resolved_distinguishes_a_loaded_empty_board_list_from_an_unmentioned_one() {
        let loaded_empty = Resolved {
            boards: Collection {
                all: LoadState::Loaded(vec![]),
                ..Default::default()
            },
            ..Default::default()
        };
        assert!(loaded_empty.boards.all.is_loaded());
        assert!(loaded_empty.boards.all.loaded().unwrap().is_empty());

        let unmentioned = Resolved::default();
        assert!(unmentioned.boards.all.is_not_loaded());
    }

    #[test]
    fn test_resolved_carries_per_entity_missing_for_a_card_the_backend_did_not_have() {
        let id = Uuid::new_v4();
        let mut resolved = Resolved::default();
        resolved.cards.by_id.insert(id, LoadState::Missing);

        assert!(resolved.cards.by_id[&id].is_missing());
        assert!(resolved.cards.by_id[&id].is_terminal());
    }

    #[test]
    fn test_resolved_clone_shares_the_same_failed_error() {
        let id = Uuid::new_v4();
        let mut resolved = Resolved::default();
        resolved.cards.by_id.insert(
            id,
            LoadState::Failed(Arc::new(KanbanError::unsupported("x"))),
        );

        let cloned = resolved.clone();
        match (&resolved.cards.by_id[&id], &cloned.cards.by_id[&id]) {
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
