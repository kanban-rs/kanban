use kanban_domain::{KanbanError, LoadState, Model, ModelLoadStates};
use kanban_tui::app::App;
use std::sync::Arc;

#[test]
fn test_with_load_states_is_reachable_from_an_out_of_crate_integration_test() {
    let mut app = App::test_default();
    app.model = Model::with_load_states(ModelLoadStates {
        cards: LoadState::Failed(Arc::new(KanbanError::unsupported("boom"))),
        ..Default::default()
    });
    assert!(app.model.cards_state().is_failed());
}
