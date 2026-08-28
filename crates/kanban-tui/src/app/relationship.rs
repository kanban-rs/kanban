use kanban_core::SelectionState;
use kanban_view::ListComponent;
use std::collections::HashSet;
use uuid::Uuid;

pub struct RelationshipState {
    pub card_ids: Vec<Uuid>,
    pub selected: HashSet<Uuid>,
    pub selection: SelectionState,
    pub search: String,
    pub search_active: bool,
    pub parents_list: ListComponent,
    pub children_list: ListComponent,
}

impl Default for RelationshipState {
    fn default() -> Self {
        Self {
            card_ids: Vec::new(),
            selected: HashSet::new(),
            selection: SelectionState::new(),
            search: String::new(),
            search_active: false,
            parents_list: ListComponent::new(false),
            children_list: ListComponent::new(false),
        }
    }
}
