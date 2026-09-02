use std::collections::HashMap;

use uuid::Uuid;

use crate::archived_board::ArchivedBoard;
use crate::archived_card::ArchivedCard;
use crate::board::Board;
use crate::card::Card;
use crate::column::Column;
use crate::dependencies::DependencyGraph;
use crate::load_state::LoadState;
use crate::sprint::Sprint;

/// One entity kind's slice of a resolve pass, in three independent tiers.
///
/// `all` speaks about the whole collection; `by_id` speaks about individual
/// entities; `by_parent` speaks about the whole child set of one parent id.
/// `NotLoaded` in `all`, an empty `by_id` and an empty `by_parent` each mean
/// the pass did not touch that tier, so applying it leaves the target
/// unchanged.
///
/// The three tiers are mutually independent. No tier may ever be inferred from
/// another, in either direction. Concretely for cards: `list_all_cards` and
/// `list_cards_by_column` both exclude archived cards while `get_card` does
/// not, so inferring `by_id` from either list tier would report an archived
/// card as `Missing`; and because a pass reads each tier at its own moment,
/// even two tiers that agree on archival semantics can disagree about content.
///
/// The parent key per entity kind is fixed:
/// - `columns.by_parent` is keyed by board id (`list_columns_by_board`)
/// - `cards.by_parent` is keyed by column id (`list_cards_by_column`)
/// - `sprints.by_parent` is keyed by board id (`list_sprints_by_board`)
/// - `archived_cards.by_parent` is keyed by board id
///   (`list_archived_cards_by_board`)
/// - `boards.by_parent` is unused and stays permanently empty: a board has no
///   parent.
///
/// `LoadState::Missing` is unrepresentable in `by_parent` by contract: the
/// scoped `DataStore` reads return `Ok(Vec::new())` for an unknown parent,
/// never `None`, so an empty scope is `Loaded(vec![])`. Only `NotLoaded`,
/// `Loaded` and `Failed` may ever appear there.
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
    pub by_parent: HashMap<Uuid, LoadState<Vec<T>>>,
}

impl<T> Default for Collection<T> {
    fn default() -> Self {
        Self {
            all: LoadState::NotLoaded,
            by_id: HashMap::new(),
            by_parent: HashMap::new(),
        }
    }
}

impl<T> Collection<T> {
    pub fn is_untouched(&self) -> bool {
        self.all.is_not_loaded() && self.by_id.is_empty() && self.by_parent.is_empty()
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
    /// Archival markers, not cards. `by_id` stays permanently empty: no
    /// `FetchRound` tier requests a single marker.
    pub archived_cards: Collection<ArchivedCard>,
    /// Archival markers, not boards. `by_id` and `by_parent` stay
    /// permanently empty: no `FetchRound` tier requests a single marker or a
    /// board-scoped one, only the whole list.
    pub archived_boards: Collection<ArchivedBoard>,
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
        assert!(resolved.archived_boards.is_untouched());
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
    fn test_collection_default_has_an_empty_by_parent() {
        let c = Collection::<Card>::default();
        assert!(c.by_parent.is_empty());
        assert!(c.is_untouched());
    }

    #[test]
    fn test_a_collection_with_a_populated_by_parent_is_not_untouched() {
        let mut c = Collection::<Card>::default();
        c.by_parent
            .insert(Uuid::new_v4(), LoadState::Loaded(Vec::new()));

        assert!(c.all.is_not_loaded());
        assert!(c.by_id.is_empty());
        assert!(!c.is_untouched());
    }

    #[test]
    fn test_resolved_carries_a_scoped_tier_for_every_scopable_kind() {
        let board_id = Uuid::new_v4();
        let column = Column::new(board_id, "todo", 0);
        let column_id = column.id;
        let card = Card::new(board_id, column_id, "a", 0);
        let card_id = card.id;
        let sprint = Sprint::new(board_id, 1, None, None::<String>);
        let sprint_id = sprint.id;

        let mut r = Resolved::default();
        r.columns
            .by_parent
            .insert(board_id, LoadState::Loaded(vec![column]));
        r.cards
            .by_parent
            .insert(column_id, LoadState::Loaded(vec![card]));
        r.sprints
            .by_parent
            .insert(board_id, LoadState::Loaded(vec![sprint]));

        assert_eq!(
            r.columns.by_parent[&board_id].loaded().unwrap()[0].id,
            column_id
        );
        assert_eq!(
            r.cards.by_parent[&column_id].loaded().unwrap()[0].id,
            card_id
        );
        assert_eq!(
            r.sprints.by_parent[&board_id].loaded().unwrap()[0].id,
            sprint_id
        );
        assert!(r.boards.by_parent.is_empty());
        assert!(r.graph.is_not_loaded());
    }

    #[test]
    fn test_the_three_tiers_of_a_collection_are_independent() {
        let missing_id = Uuid::new_v4();
        let column_id = Uuid::new_v4();
        let card = Card::new(Uuid::new_v4(), column_id, "a", 0);
        let card_id = card.id;

        let mut c = Collection::<Card>::default();
        c.by_id.insert(missing_id, LoadState::Missing);
        c.by_parent.insert(column_id, LoadState::Loaded(vec![card]));

        assert!(c.all.is_not_loaded());
        assert!(c.by_id[&missing_id].is_missing());
        assert!(!c.by_parent.contains_key(&card_id));
        assert_eq!(c.by_parent[&column_id].loaded().unwrap()[0].id, card_id);
    }

    #[test]
    fn test_a_loaded_empty_scope_is_distinguishable_from_an_unmentioned_one() {
        let known = Uuid::new_v4();
        let unmentioned = Uuid::new_v4();

        let mut c = Collection::<Column>::default();
        c.by_parent.insert(known, LoadState::Loaded(Vec::new()));

        assert!(c.by_parent[&known].is_loaded());
        assert!(c.by_parent[&known].loaded().unwrap().is_empty());
        let unmentioned_scope = c.by_parent.get(&unmentioned);
        assert!(unmentioned_scope.is_none());
    }

    #[test]
    fn test_collection_default_works_for_a_non_default_type() {
        let c = Collection::<NoDefault>::default();
        assert!(c.is_untouched());

        let loaded = Collection {
            all: LoadState::Loaded(vec![NoDefault]),
            by_id: HashMap::new(),
            ..Default::default()
        };
        assert!(!loaded.is_untouched());
        let _ = loaded.clone();
        let _ = format!("{loaded:?}");
    }
}
