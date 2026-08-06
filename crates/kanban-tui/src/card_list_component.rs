use crossterm::event::KeyCode;
use kanban_view::card_list::{CardList, CardListId};
use kanban_view::card_list_component::{
    CardListAction, CardListActionType, CardListComponentConfig, CardListHelpEntry,
};
use uuid::Uuid;

/// Render `kanban-view`'s structured card-list help entries as the TUI footer
/// string, e.g. `"ESC: cancel | j/k: navigate | ..."`.
pub fn help_entries_text(entries: &[CardListHelpEntry]) -> String {
    entries
        .iter()
        .map(|entry| format!("{}: {}", entry.key_hint, entry.label))
        .collect::<Vec<_>>()
        .join(" | ")
}

pub struct CardListComponent {
    pub card_list: CardList,
    pub config: CardListComponentConfig,
    pub multi_selected: std::collections::HashSet<Uuid>,
    pub viewport_height: usize,
}

impl CardListComponent {
    pub fn new(list_id: CardListId, config: CardListComponentConfig) -> Self {
        Self {
            card_list: CardList::new(list_id),
            config,
            multi_selected: std::collections::HashSet::new(),
            viewport_height: 20,
        }
    }

    pub fn with_config(list_id: CardListId, config: CardListComponentConfig) -> Self {
        Self::new(list_id, config)
    }

    pub fn update_cards(&mut self, cards: Vec<Uuid>) {
        self.card_list.update_cards(cards);
    }

    pub fn get_selected_card_id(&self) -> Option<Uuid> {
        self.card_list.get_selected_card_id()
    }

    pub fn get_multi_selected(&self) -> Vec<Uuid> {
        self.multi_selected.iter().copied().collect()
    }

    pub fn toggle_multi_select(&mut self, card_id: Uuid) {
        if self.config.allow_multi_select {
            if self.multi_selected.contains(&card_id) {
                self.multi_selected.remove(&card_id);
            } else {
                self.multi_selected.insert(card_id);
            }
        }
    }

    pub fn clear_multi_select(&mut self) {
        self.multi_selected.clear();
    }

    pub fn select_all(&mut self) {
        if self.config.allow_multi_select {
            for card_id in &self.card_list.cards {
                self.multi_selected.insert(*card_id);
            }
        }
    }

    pub fn navigate_up(&mut self) -> bool {
        self.card_list.navigate_up()
    }

    pub fn navigate_down(&mut self) -> bool {
        self.card_list.navigate_down()
    }

    pub fn is_empty(&self) -> bool {
        self.card_list.is_empty()
    }

    pub fn len(&self) -> usize {
        self.card_list.len()
    }

    pub fn get_selected_index(&self) -> Option<usize> {
        self.card_list.get_selected_index()
    }

    pub fn set_selected_index(&mut self, index: Option<usize>) {
        self.card_list.set_selected_index(index);
    }

    pub fn get_scroll_offset(&self) -> usize {
        self.card_list.get_scroll_offset()
    }

    pub fn set_scroll_offset(&mut self, offset: usize) {
        self.card_list.set_scroll_offset(offset);
    }

    pub fn ensure_selected_visible(&mut self, viewport_height: usize) {
        self.card_list.ensure_selected_visible(viewport_height);
    }

    pub fn help_text(&self) -> String {
        help_entries_text(&self.config.help_entries())
    }

    pub fn handle_key(&mut self, key: KeyCode) -> Option<CardListAction> {
        match key {
            KeyCode::Char('j') | KeyCode::Down => {
                if self
                    .config
                    .is_action_enabled(&CardListActionType::Navigation)
                {
                    self.navigate_down();
                }
                None
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if self
                    .config
                    .is_action_enabled(&CardListActionType::Navigation)
                {
                    self.navigate_up();
                }
                None
            }
            KeyCode::Enter | KeyCode::Char(' ') => {
                if self
                    .config
                    .is_action_enabled(&CardListActionType::Selection)
                {
                    self.get_selected_card_id().map(CardListAction::Select)
                } else {
                    None
                }
            }
            KeyCode::Char('e') => {
                if self.config.is_action_enabled(&CardListActionType::Editing) {
                    self.get_selected_card_id().map(CardListAction::Edit)
                } else {
                    None
                }
            }
            KeyCode::Char('c') => {
                if self
                    .config
                    .is_action_enabled(&CardListActionType::Completion)
                {
                    self.get_selected_card_id().map(CardListAction::Complete)
                } else {
                    None
                }
            }
            KeyCode::Char('p') => {
                if self.config.is_action_enabled(&CardListActionType::Priority) {
                    self.get_selected_card_id()
                        .map(CardListAction::TogglePriority)
                } else {
                    None
                }
            }
            KeyCode::Char('a') => {
                if self.config.is_action_enabled(&CardListActionType::Sprint) {
                    self.get_selected_card_id()
                        .map(CardListAction::AssignSprint)
                } else {
                    None
                }
            }
            KeyCode::Char('S') => {
                if self.config.is_action_enabled(&CardListActionType::Sprint) {
                    self.get_selected_card_id()
                        .map(CardListAction::ReassignSprint)
                } else {
                    None
                }
            }
            KeyCode::Char('o') => {
                if self.config.is_action_enabled(&CardListActionType::Sorting) {
                    Some(CardListAction::Sort)
                } else {
                    None
                }
            }
            KeyCode::Char('O') => {
                if self.config.is_action_enabled(&CardListActionType::Sorting) {
                    Some(CardListAction::OrderCards)
                } else {
                    None
                }
            }
            KeyCode::Char('H') => {
                if self.config.is_action_enabled(&CardListActionType::Movement)
                    && self.config.allow_movement
                {
                    self.get_selected_card_id()
                        .map(|id| CardListAction::MoveColumn(id, false))
                } else {
                    None
                }
            }
            KeyCode::Char('L') => {
                if self.config.is_action_enabled(&CardListActionType::Movement)
                    && self.config.allow_movement
                {
                    self.get_selected_card_id()
                        .map(|id| CardListAction::MoveColumn(id, true))
                } else {
                    None
                }
            }
            KeyCode::Char('n') => {
                if self.config.is_action_enabled(&CardListActionType::Creation) {
                    Some(CardListAction::Create)
                } else {
                    None
                }
            }
            KeyCode::Char('v') => {
                if self
                    .config
                    .is_action_enabled(&CardListActionType::MultiSelect)
                {
                    self.get_selected_card_id()
                        .map(CardListAction::ToggleMultiSelect)
                } else {
                    None
                }
            }
            KeyCode::Char('V') => {
                if self
                    .config
                    .is_action_enabled(&CardListActionType::MultiSelect)
                    && self.config.allow_multi_select
                {
                    Some(CardListAction::SelectAll)
                } else {
                    None
                }
            }
            _ => None,
        }
    }
}
