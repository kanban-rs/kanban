use kanban_domain::Invalidation;

use crate::fetch_plan::{FetchPlan, FetchRound, LoadedEntities, LoadedState};

/// A [`FetchPlan`] built from an [`Invalidation`] plus a pre-invalidation
/// [`LoadedState`] snapshot. Construct it *before* calling
/// [`kanban_domain::Model::invalidate`]: it re-requests exactly the tiers
/// `invalidate` is about to blank, restricted to tiers the snapshot shows
/// were already read, so it never asks for something the caller never
/// wanted in the first place.
#[derive(Debug, Clone)]
pub struct InvalidationPlan {
    round: FetchRound,
}

impl InvalidationPlan {
    pub fn for_invalidation(
        _invalidation: &Invalidation,
        _loaded: &dyn LoadedState,
    ) -> Option<Self> {
        todo!()
    }

    pub fn round(&self) -> &FetchRound {
        &self.round
    }
}

impl FetchPlan for InvalidationPlan {
    fn next_round(&self, _loaded: &dyn LoadedEntities) -> FetchRound {
        todo!()
    }
}

#[cfg(test)]
mod tests;
