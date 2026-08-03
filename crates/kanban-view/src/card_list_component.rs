use uuid::Uuid;

#[derive(Debug, Clone, PartialEq)]
pub enum CardListAction {
    Select(Uuid),
    Edit(Uuid),
    Complete(Uuid),
    TogglePriority(Uuid),
    AssignSprint(Uuid),
    ReassignSprint(Uuid),
    Sort,
    OrderCards,
    MoveColumn(Uuid, bool),
    Create,
    ToggleMultiSelect(Uuid),
    ClearMultiSelect,
    SelectAll,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum CardListActionType {
    Navigation,
    Selection,
    Editing,
    Completion,
    Priority,
    Sprint,
    Sorting,
    Movement,
    Creation,
    MultiSelect,
}

pub struct CardListComponentConfig {
    pub enabled_actions: Vec<CardListActionType>,
    pub allow_multi_select: bool,
    pub allow_reordering: bool,
    pub allow_movement: bool,
    pub show_sprint_names: bool,
}

impl Default for CardListComponentConfig {
    fn default() -> Self {
        Self {
            enabled_actions: vec![
                CardListActionType::Navigation,
                CardListActionType::Selection,
                CardListActionType::Editing,
                CardListActionType::Completion,
                CardListActionType::Priority,
                CardListActionType::Sprint,
                CardListActionType::Sorting,
                CardListActionType::Movement,
                CardListActionType::Creation,
                CardListActionType::MultiSelect,
            ],
            allow_multi_select: true,
            allow_reordering: true,
            allow_movement: true,
            show_sprint_names: true,
        }
    }
}

impl CardListComponentConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_actions(mut self, actions: Vec<CardListActionType>) -> Self {
        self.enabled_actions = actions;
        self
    }

    pub fn with_multi_select(mut self, allow: bool) -> Self {
        self.allow_multi_select = allow;
        self
    }

    pub fn with_reordering(mut self, allow: bool) -> Self {
        self.allow_reordering = allow;
        self
    }

    pub fn with_movement(mut self, allow: bool) -> Self {
        self.allow_movement = allow;
        self
    }

    pub fn with_sprint_names(mut self, show: bool) -> Self {
        self.show_sprint_names = show;
        self
    }

    pub fn is_action_enabled(&self, action_type: &CardListActionType) -> bool {
        self.enabled_actions.contains(action_type)
    }

    pub fn help_text(&self) -> String {
        let mut parts = vec!["ESC: cancel"];

        if self.is_action_enabled(&CardListActionType::Navigation) {
            parts.push("j/k: navigate");
        }

        if self.is_action_enabled(&CardListActionType::Selection) {
            parts.push("Enter/Space: select");
        }

        if self.is_action_enabled(&CardListActionType::Editing) {
            parts.push("e: edit");
        }

        if self.is_action_enabled(&CardListActionType::Completion) {
            parts.push("c: complete");
        }

        if self.is_action_enabled(&CardListActionType::Priority) {
            parts.push("p: priority");
        }

        if self.is_action_enabled(&CardListActionType::Sprint) {
            parts.push("a: assign sprint");
        }

        if self.is_action_enabled(&CardListActionType::Sorting) {
            parts.push("o: sort");
        }

        if self.is_action_enabled(&CardListActionType::Movement) {
            parts.push("H/L: move");
        }

        if self.is_action_enabled(&CardListActionType::Creation) {
            parts.push("n: new");
        }

        if self.allow_multi_select {
            parts.push("v: select card | V: multi-select");
        }

        parts.join(" | ")
    }
}
