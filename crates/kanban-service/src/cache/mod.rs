use kanban_domain::resolved::Collection;
use kanban_domain::{
    Board, Card, Column, DataStore, DependencyGraph, FetchPlan, Invalidation, KanbanResult,
    LoadState, Resolved, Sprint,
};
use uuid::Uuid;

#[derive(Debug, Default)]
pub struct EntityCache {
    boards: Collection<Board>,
    columns: Collection<Column>,
    cards: Collection<Card>,
    sprints: Collection<Sprint>,
    graph: LoadState<DependencyGraph>,
}

impl EntityCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn invalidate(&mut self, _invalidation: Invalidation) {
        todo!()
    }

    pub fn resolve(
        &mut self,
        _plan: &dyn FetchPlan,
        _store: &dyn DataStore,
    ) -> KanbanResult<Resolved> {
        todo!()
    }

    pub fn board_list(&self) -> LoadState<&Vec<Board>> {
        todo!()
    }

    pub fn column_list(&self) -> LoadState<&Vec<Column>> {
        todo!()
    }

    pub fn card_list(&self) -> LoadState<&Vec<Card>> {
        todo!()
    }

    pub fn sprint_list(&self) -> LoadState<&Vec<Sprint>> {
        todo!()
    }

    pub fn graph(&self) -> LoadState<&DependencyGraph> {
        todo!()
    }

    pub fn column(&self, _id: Uuid) -> LoadState<&Column> {
        todo!()
    }

    pub fn card(&self, _id: Uuid) -> LoadState<&Card> {
        todo!()
    }

    pub fn sprint(&self, _id: Uuid) -> LoadState<&Sprint> {
        todo!()
    }
}

#[cfg(test)]
mod tests;
