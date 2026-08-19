use std::sync::Arc;

use crate::error::KanbanError;

/// Per-entity load status. `NotLoaded` is the only non-terminal state.
#[derive(Debug, Clone, Default)]
pub enum LoadState<T> {
    #[default]
    NotLoaded,
    Loaded(T),
    Missing,
    Failed(Arc<KanbanError>),
}

impl<T> LoadState<T> {
    pub fn as_ref(&self) -> LoadState<&T> {
        match self {
            LoadState::NotLoaded => LoadState::NotLoaded,
            LoadState::Loaded(v) => LoadState::Loaded(v),
            LoadState::Missing => LoadState::Missing,
            LoadState::Failed(e) => LoadState::Failed(Arc::clone(e)),
        }
    }

    pub fn loaded(&self) -> Option<&T> {
        match self {
            LoadState::Loaded(v) => Some(v),
            LoadState::NotLoaded | LoadState::Missing | LoadState::Failed(_) => None,
        }
    }

    pub fn is_loaded(&self) -> bool {
        matches!(self, LoadState::Loaded(_))
    }

    pub fn is_not_loaded(&self) -> bool {
        matches!(self, LoadState::NotLoaded)
    }

    pub fn is_missing(&self) -> bool {
        matches!(self, LoadState::Missing)
    }

    pub fn is_failed(&self) -> bool {
        matches!(self, LoadState::Failed(_))
    }

    /// True once a fetch has produced a result. Only a non-terminal state is
    /// requestable by a fetch round.
    pub fn is_terminal(&self) -> bool {
        match self {
            LoadState::NotLoaded => false,
            LoadState::Loaded(_) | LoadState::Missing | LoadState::Failed(_) => true,
        }
    }

    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> LoadState<U> {
        match self {
            LoadState::NotLoaded => LoadState::NotLoaded,
            LoadState::Loaded(v) => LoadState::Loaded(f(v)),
            LoadState::Missing => LoadState::Missing,
            LoadState::Failed(e) => LoadState::Failed(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_loadstate_default_is_not_loaded() {
        let state = LoadState::<u32>::default();
        assert!(state.is_not_loaded());
    }

    #[test]
    fn test_loadstate_loaded_returns_some_via_loaded_accessor() {
        let state = LoadState::Loaded(5);
        assert_eq!(state.loaded(), Some(&5));
    }

    #[test]
    fn test_loadstate_not_loaded_and_failed_return_none_via_loaded_accessor() {
        let not_loaded: LoadState<u32> = LoadState::NotLoaded;
        let failed: LoadState<u32> = LoadState::Failed(Arc::new(KanbanError::unsupported("x")));
        assert_eq!(not_loaded.loaded(), None);
        assert_eq!(failed.loaded(), None);
    }

    #[test]
    fn test_loadstate_missing_returns_none_via_loaded_accessor() {
        let missing: LoadState<u32> = LoadState::Missing;
        assert_eq!(missing.loaded(), None);
    }

    #[test]
    fn test_loadstate_is_loaded_is_failed_is_not_loaded_are_mutually_exclusive() {
        let not_loaded: LoadState<u32> = LoadState::NotLoaded;
        let loaded: LoadState<u32> = LoadState::Loaded(1);
        let failed: LoadState<u32> = LoadState::Failed(Arc::new(KanbanError::unsupported("x")));

        assert!(not_loaded.is_not_loaded() && !not_loaded.is_loaded() && !not_loaded.is_failed());
        assert!(loaded.is_loaded() && !loaded.is_not_loaded() && !loaded.is_failed());
        assert!(failed.is_failed() && !failed.is_loaded() && !failed.is_not_loaded());
    }

    #[test]
    fn test_loadstate_missing_is_missing_and_not_the_other_three_variants() {
        let missing: LoadState<u32> = LoadState::Missing;
        assert!(missing.is_missing());
        assert!(!missing.is_loaded() && !missing.is_not_loaded() && !missing.is_failed());
    }

    #[test]
    fn test_loadstate_map_transforms_loaded_value_only() {
        let loaded: LoadState<i32> = LoadState::Loaded(2);
        assert!(matches!(loaded.map(|x| x * 2), LoadState::Loaded(4)));

        let not_loaded: LoadState<i32> = LoadState::NotLoaded;
        assert!(not_loaded.map(|x| x * 2).is_not_loaded());

        let failed: LoadState<i32> = LoadState::Failed(Arc::new(KanbanError::unsupported("x")));
        assert!(failed.map(|x| x * 2).is_failed());
    }

    #[test]
    fn test_loadstate_map_preserves_missing_without_invoking_closure() {
        let missing: LoadState<i32> = LoadState::Missing;
        let mapped = missing.map(|x| -> i32 {
            panic!("map must not invoke its closure on Missing, got {x}");
        });
        assert!(mapped.is_missing());
    }

    #[test]
    fn test_loadstate_failed_arc_clone_is_cheap_and_shares_the_same_error() {
        let state: LoadState<u32> = LoadState::Failed(Arc::new(KanbanError::unsupported("x")));
        let cloned = state.clone();

        match (&state, &cloned) {
            (LoadState::Failed(a), LoadState::Failed(b)) => assert!(Arc::ptr_eq(a, b)),
            _ => panic!("expected Failed variant"),
        }
    }

    #[test]
    fn test_loadstate_as_ref_preserves_variant() {
        let loaded: LoadState<u32> = LoadState::Loaded(7);
        assert!(matches!(loaded.as_ref(), LoadState::Loaded(v) if *v == 7));

        let not_loaded: LoadState<u32> = LoadState::NotLoaded;
        assert!(not_loaded.as_ref().is_not_loaded());

        let failed: LoadState<u32> = LoadState::Failed(Arc::new(KanbanError::unsupported("x")));
        match (&failed, failed.as_ref()) {
            (LoadState::Failed(a), LoadState::Failed(b)) => assert!(Arc::ptr_eq(a, &b)),
            _ => panic!("expected Failed variant"),
        }
    }

    #[test]
    fn test_loadstate_as_ref_preserves_missing_variant() {
        let missing: LoadState<u32> = LoadState::Missing;
        assert!(missing.as_ref().is_missing());
    }

    #[test]
    fn test_loaded_or_empty_returns_the_contents_when_loaded() {
        let state: LoadState<Vec<u32>> = LoadState::Loaded(vec![1, 2, 3]);
        assert_eq!(state.loaded_or_empty(), &[1, 2, 3]);
    }

    #[test]
    fn test_loaded_or_empty_returns_an_empty_slice_for_every_non_loaded_state() {
        let not_loaded: LoadState<Vec<u32>> = LoadState::NotLoaded;
        let missing: LoadState<Vec<u32>> = LoadState::Missing;
        let failed: LoadState<Vec<u32>> = LoadState::Failed(Arc::new(KanbanError::unsupported("x")));

        assert!(not_loaded.loaded_or_empty().is_empty());
        assert!(missing.loaded_or_empty().is_empty());
        assert!(failed.loaded_or_empty().is_empty());
    }

    #[test]
    fn test_loadstate_only_not_loaded_is_non_terminal() {
        let not_loaded: LoadState<u32> = LoadState::NotLoaded;
        let loaded: LoadState<u32> = LoadState::Loaded(1);
        let missing: LoadState<u32> = LoadState::Missing;
        let failed: LoadState<u32> = LoadState::Failed(Arc::new(KanbanError::unsupported("x")));

        assert!(!not_loaded.is_terminal());
        assert!(loaded.is_terminal());
        assert!(missing.is_terminal());
        assert!(failed.is_terminal());
    }
}

#[cfg(test)]
mod design_validation {
    use super::*;
    use crate::card::Card;

    #[derive(Clone)]
    struct Holder {
        #[allow(dead_code)]
        cards: LoadState<Vec<Card>>,
    }

    #[test]
    fn test_struct_holding_loadstate_can_derive_clone() {
        let holder = Holder {
            cards: LoadState::NotLoaded,
        };
        let _cloned = holder.clone();
    }
}
