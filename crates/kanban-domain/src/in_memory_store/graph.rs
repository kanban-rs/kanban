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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_store::DataStore;
    use uuid::Uuid;

    #[test]
    fn test_modify_graph_atomic_on_error_leaves_graph_unchanged() {
        let store = InMemoryStore::new();
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();

        let mut graph = store.get_graph().unwrap();
        graph.set_block(a, b).unwrap();
        store.set_graph(graph).unwrap();

        let result = store.modify_graph(Box::new(move |graph| {
            graph.remove_node(a);
            Err(crate::KanbanError::validation("rollback"))
        }));
        assert!(result.is_err());

        let graph = store.get_graph().unwrap();
        assert_eq!(
            graph.len(),
            1,
            "modify_graph should not apply partial changes on error"
        );
    }

    #[test]
    fn test_set_and_get_graph() {
        let store = InMemoryStore::new();
        let graph = DependencyGraph::new();
        store.set_graph(graph.clone()).unwrap();
        let fetched = store.get_graph().unwrap();
        assert_eq!(fetched, graph);
    }
}
