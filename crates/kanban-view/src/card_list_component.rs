use serde::Serialize;
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

/// A card-list action a renderer can advertise to the user.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum CardListHelpAction {
    Cancel,
    Navigate,
    Select,
    Edit,
    Complete,
    Priority,
    AssignSprint,
    Sort,
    Move,
    Create,
    ToggleCardSelection,
    MultiSelect,
}

/// One advertised card-list action. `key_hint` is the keyboard chord a
/// terminal renderer shows; renderers without a keyboard mapping use
/// `action`/`label` and ignore it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct CardListHelpEntry {
    pub action: CardListHelpAction,
    pub label: &'static str,
    pub key_hint: &'static str,
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

    pub fn help_entries(&self) -> Vec<CardListHelpEntry> {
        fn entry(
            action: CardListHelpAction,
            key_hint: &'static str,
            label: &'static str,
        ) -> CardListHelpEntry {
            CardListHelpEntry {
                action,
                label,
                key_hint,
            }
        }

        let mut entries = vec![entry(CardListHelpAction::Cancel, "ESC", "cancel")];

        if self.is_action_enabled(&CardListActionType::Navigation) {
            entries.push(entry(CardListHelpAction::Navigate, "j/k", "navigate"));
        }

        if self.is_action_enabled(&CardListActionType::Selection) {
            entries.push(entry(CardListHelpAction::Select, "Enter/Space", "select"));
        }

        if self.is_action_enabled(&CardListActionType::Editing) {
            entries.push(entry(CardListHelpAction::Edit, "e", "edit"));
        }

        if self.is_action_enabled(&CardListActionType::Completion) {
            entries.push(entry(CardListHelpAction::Complete, "c", "complete"));
        }

        if self.is_action_enabled(&CardListActionType::Priority) {
            entries.push(entry(CardListHelpAction::Priority, "p", "priority"));
        }

        if self.is_action_enabled(&CardListActionType::Sprint) {
            entries.push(entry(
                CardListHelpAction::AssignSprint,
                "a",
                "assign sprint",
            ));
        }

        if self.is_action_enabled(&CardListActionType::Sorting) {
            entries.push(entry(CardListHelpAction::Sort, "o", "sort"));
        }

        if self.is_action_enabled(&CardListActionType::Movement) {
            entries.push(entry(CardListHelpAction::Move, "H/L", "move"));
        }

        if self.is_action_enabled(&CardListActionType::Creation) {
            entries.push(entry(CardListHelpAction::Create, "n", "new"));
        }

        if self.allow_multi_select {
            entries.push(entry(
                CardListHelpAction::ToggleCardSelection,
                "v",
                "select card",
            ));
            entries.push(entry(CardListHelpAction::MultiSelect, "V", "multi-select"));
        }

        entries
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn actions(config: &CardListComponentConfig) -> Vec<CardListHelpAction> {
        config
            .help_entries()
            .into_iter()
            .map(|e| e.action)
            .collect()
    }

    #[test]
    fn test_help_entries_default_config_lists_every_action_in_order() {
        assert_eq!(
            actions(&CardListComponentConfig::default()),
            vec![
                CardListHelpAction::Cancel,
                CardListHelpAction::Navigate,
                CardListHelpAction::Select,
                CardListHelpAction::Edit,
                CardListHelpAction::Complete,
                CardListHelpAction::Priority,
                CardListHelpAction::AssignSprint,
                CardListHelpAction::Sort,
                CardListHelpAction::Move,
                CardListHelpAction::Create,
                CardListHelpAction::ToggleCardSelection,
                CardListHelpAction::MultiSelect,
            ]
        );
    }

    #[test]
    fn test_help_entries_limited_actions_omits_disabled_actions() {
        let config = CardListComponentConfig::new().with_actions(vec![
            CardListActionType::Navigation,
            CardListActionType::Selection,
        ]);
        let actions = actions(&config);
        assert!(actions.contains(&CardListHelpAction::Navigate));
        assert!(actions.contains(&CardListHelpAction::Select));
        assert!(!actions.contains(&CardListHelpAction::Edit));
        assert!(!actions.contains(&CardListHelpAction::Complete));
    }

    #[test]
    fn test_help_entries_multi_select_disabled_drops_both_selection_entries() {
        let config = CardListComponentConfig::new().with_multi_select(false);
        let actions = actions(&config);
        assert!(!actions.contains(&CardListHelpAction::ToggleCardSelection));
        assert!(!actions.contains(&CardListHelpAction::MultiSelect));
    }

    #[test]
    fn test_help_entries_carry_key_hint_and_label_separately() {
        let entries = CardListComponentConfig::default().help_entries();
        let sprint = entries
            .iter()
            .find(|e| e.action == CardListHelpAction::AssignSprint)
            .expect("default config enables sprint assignment");
        assert_eq!(sprint.key_hint, "a");
        assert_eq!(sprint.label, "assign sprint");
    }

    #[test]
    fn test_help_entries_contain_no_assembled_prose() {
        for entry in CardListComponentConfig::default().help_entries() {
            assert!(
                !entry.label.contains(" | ") && !entry.label.contains(':'),
                "label must be a bare verb, not a formatted hint: {}",
                entry.label
            );
        }
    }
}
