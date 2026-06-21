use super::InMemoryStore;
use crate::{DependencyGraph, KanbanResult};

impl InMemoryStore {
    pub(super) fn get_graph_impl(&self) -> KanbanResult<DependencyGraph> {
        let state = self.read_state()?;
        Ok(state.graph.clone())
    }

    pub(super) fn set_graph_impl(&self, graph: DependencyGraph) -> KanbanResult<()> {
        let mut state = self.write_state()?;
        state.graph = graph;
        Ok(())
    }

    pub(super) fn modify_graph_impl(&self, f: crate::data_store::GraphMutFn) -> KanbanResult<()> {
        let mut state = self.write_state()?;
        let mut graph = state.graph.clone();
        f(&mut graph)?;
        state.graph = graph;
        Ok(())
    }
}
