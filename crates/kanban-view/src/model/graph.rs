use super::*;

impl Model {
    pub fn empty_graph() -> &'static DependencyGraph {
        static EMPTY: std::sync::OnceLock<DependencyGraph> = std::sync::OnceLock::new();
        EMPTY.get_or_init(DependencyGraph::default)
    }

    pub fn graph_state(&self) -> &LoadState<DependencyGraph> {
        &self.graph
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kanban_domain::Snapshot;

    #[test]
    fn test_graph_state_is_not_loaded_on_a_default_model() {
        let m = Model::default();
        assert!(m.graph_state().is_not_loaded());
    }

    #[test]
    fn test_graph_state_is_loaded_after_load_from_snapshot() {
        let mut m = Model::default();
        m.load_from_snapshot(Snapshot::default());
        assert!(m.graph_state().is_loaded());
    }

    #[test]
    fn test_graph_accessor_returns_one_shared_empty_graph_when_not_loaded() {
        let a = Model::default();
        let b = Model::default();
        assert!(std::ptr::eq(
            a.graph_state()
                .loaded()
                .unwrap_or_else(|| Model::empty_graph()),
            b.graph_state()
                .loaded()
                .unwrap_or_else(|| Model::empty_graph())
        ));
        assert_eq!(
            a.graph_state()
                .loaded()
                .unwrap_or_else(|| Model::empty_graph()),
            &DependencyGraph::default()
        );
        assert_eq!(
            b.graph_state()
                .loaded()
                .unwrap_or_else(|| Model::empty_graph()),
            &DependencyGraph::default()
        );
    }
}
