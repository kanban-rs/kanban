use super::KanbanContext;
use crate::fetch_plan::FetchPlan;
use crate::invalidation_plan::InvalidationPlan;
use kanban_domain::{DerivedProjections, Invalidation, Model, ModelChanged};

impl KanbanContext {
    fn resolve_into(&self, plan: &dyn FetchPlan, model: &mut Model) -> ModelChanged {
        let resolved = self.resolve(plan, &*model);
        model.apply_resolved(resolved)
    }

    /// Runs `plan` against `model`, folds the result in, and resyncs `proj`.
    /// A failed read is recorded as `LoadState::Failed` on the affected tier
    /// rather than returned, so a partial failure is visible per tier
    /// instead of collapsing the sync.
    pub fn sync(
        &self,
        plan: &dyn FetchPlan,
        model: &mut Model,
        proj: &mut impl DerivedProjections,
    ) {
        let changed = self.resolve_into(plan, model);
        proj.resync(model, changed);
    }

    /// Applies `inv` to `model` before the plan is consulted, so the
    /// mutated entity is refetched instead of being left `Loaded` and
    /// skipped by the plan's `requestable` gate.
    pub fn sync_invalidated(
        &self,
        inv: Invalidation,
        plan: &dyn FetchPlan,
        model: &mut Model,
        proj: &mut impl DerivedProjections,
    ) {
        let invalidated = model.invalidate(inv);
        let changed = invalidated.merge(self.resolve_into(plan, model));
        proj.resync(model, changed);
    }

    /// Mutate-then-read sibling of [`sync_invalidated`](Self::sync_invalidated).
    /// Builds the repair plan from `model`'s pre-invalidation state before
    /// `inv` is moved into [`Model::invalidate`]; the move is what stops the
    /// repair capture from being reordered after the invalidation.
    pub fn resync_invalidated(
        &self,
        inv: Invalidation,
        plan: &dyn FetchPlan,
        model: &mut Model,
        proj: &mut impl DerivedProjections,
    ) {
        let repair = InvalidationPlan::for_invalidation(&inv, &*model);
        let mut changed = model.invalidate(inv);
        if let Some(repair) = repair {
            changed = changed.merge(self.resolve_into(&repair, model));
        }
        changed = changed.merge(self.resolve_into(plan, model));
        proj.resync(model, changed);
    }
}

#[cfg(test)]
mod tests;
