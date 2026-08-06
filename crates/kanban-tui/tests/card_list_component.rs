use crossterm::event::KeyCode;
use kanban_tui::card_list_component::{help_entries_text, CardListComponent};
use kanban_view::card_list::CardListId;
use kanban_view::card_list_component::{
    CardListAction, CardListActionType, CardListComponentConfig,
};
use uuid::Uuid;

fn create_test_component() -> CardListComponent {
    CardListComponent::new(CardListId::All, CardListComponentConfig::new())
}

fn create_test_component_with_config(config: CardListComponentConfig) -> CardListComponent {
    CardListComponent::new(CardListId::All, config)
}

// Configuration Tests
#[test]
fn test_default_config_has_all_actions_enabled() {
    let config = CardListComponentConfig::default();
    assert_eq!(config.enabled_actions.len(), 10);
    assert!(config.is_action_enabled(&CardListActionType::Navigation));
    assert!(config.is_action_enabled(&CardListActionType::Selection));
    assert!(config.is_action_enabled(&CardListActionType::Editing));
    assert!(config.is_action_enabled(&CardListActionType::Completion));
    assert!(config.is_action_enabled(&CardListActionType::Priority));
    assert!(config.is_action_enabled(&CardListActionType::Sprint));
    assert!(config.is_action_enabled(&CardListActionType::Sorting));
    assert!(config.is_action_enabled(&CardListActionType::Movement));
    assert!(config.is_action_enabled(&CardListActionType::Creation));
    assert!(config.is_action_enabled(&CardListActionType::MultiSelect));
}

#[test]
fn test_config_builder_with_actions() {
    let config = CardListComponentConfig::new().with_actions(vec![
        CardListActionType::Navigation,
        CardListActionType::Selection,
    ]);
    assert_eq!(config.enabled_actions.len(), 2);
    assert!(config.is_action_enabled(&CardListActionType::Navigation));
    assert!(config.is_action_enabled(&CardListActionType::Selection));
    assert!(!config.is_action_enabled(&CardListActionType::Editing));
}

#[test]
fn test_config_builder_with_multi_select() {
    let config = CardListComponentConfig::new().with_multi_select(false);
    assert!(!config.allow_multi_select);
}

#[test]
fn test_config_builder_with_movement() {
    let config = CardListComponentConfig::new().with_movement(false);
    assert!(!config.allow_movement);
}

#[test]
fn test_help_entries_text_renders_every_default_action() {
    let config = CardListComponentConfig::default();
    let help = help_entries_text(&config.help_entries());
    assert!(help.contains("j/k: navigate"));
    assert!(help.contains("Enter/Space: select"));
    assert!(help.contains("e: edit"));
    assert!(help.contains("c: complete"));
    assert!(help.contains("p: priority"));
    assert!(help.contains("a: assign sprint"));
    assert!(help.contains("o: sort"));
    assert!(help.contains("H/L: move"));
    assert!(help.contains("n: new"));
    assert!(help.contains("v: select card"));
}

#[test]
fn test_help_entries_text_omits_disabled_actions() {
    let config = CardListComponentConfig::new().with_actions(vec![
        CardListActionType::Navigation,
        CardListActionType::Selection,
    ]);
    let help = help_entries_text(&config.help_entries());
    assert!(help.contains("j/k: navigate"));
    assert!(help.contains("Enter/Space: select"));
    assert!(!help.contains("e: edit"));
    assert!(!help.contains("c: complete"));
}

// Navigation Tests
#[test]
fn test_navigation_with_cards() {
    let mut component = create_test_component();
    let card1 = Uuid::new_v4();
    let card2 = Uuid::new_v4();
    let card3 = Uuid::new_v4();

    component.update_cards(vec![card1, card2, card3]);

    assert_eq!(component.get_selected_index(), Some(0));

    component.handle_key(KeyCode::Char('j'));
    assert_eq!(component.get_selected_index(), Some(1));

    component.handle_key(KeyCode::Char('j'));
    assert_eq!(component.get_selected_index(), Some(2));

    component.handle_key(KeyCode::Char('k'));
    assert_eq!(component.get_selected_index(), Some(1));
}

#[test]
fn test_navigation_down_key() {
    let mut component = create_test_component();
    let card1 = Uuid::new_v4();
    let card2 = Uuid::new_v4();

    component.update_cards(vec![card1, card2]);
    assert_eq!(component.get_selected_index(), Some(0));

    component.handle_key(KeyCode::Down);
    assert_eq!(component.get_selected_index(), Some(1));
}

#[test]
fn test_navigation_up_key() {
    let mut component = create_test_component();
    let card1 = Uuid::new_v4();
    let card2 = Uuid::new_v4();

    component.update_cards(vec![card1, card2]);
    component.handle_key(KeyCode::Down);
    assert_eq!(component.get_selected_index(), Some(1));

    component.handle_key(KeyCode::Up);
    assert_eq!(component.get_selected_index(), Some(0));
}

#[test]
fn test_navigation_up_on_empty_list() {
    let mut component = CardListComponent::new(CardListId::All, CardListComponentConfig::new());
    // Don't set any cards - list is empty
    assert_eq!(component.get_selected_index(), None);

    // Pressing k (up) on empty list should signal to navigate to adjacent column
    // but not modify the selection state
    component.handle_key(KeyCode::Char('k'));
    assert_eq!(component.get_selected_index(), None);

    component.handle_key(KeyCode::Up);
    assert_eq!(component.get_selected_index(), None);
}

#[test]
fn test_navigation_down_on_empty_list() {
    let mut component = CardListComponent::new(CardListId::All, CardListComponentConfig::new());
    // Don't set any cards - list is empty
    assert_eq!(component.get_selected_index(), None);

    // Pressing j (down) on empty list should signal to navigate to adjacent column
    // but not modify the selection state
    component.handle_key(KeyCode::Char('j'));
    assert_eq!(component.get_selected_index(), None);

    component.handle_key(KeyCode::Down);
    assert_eq!(component.get_selected_index(), None);
}

#[test]
fn test_navigation_disabled() {
    let config = CardListComponentConfig::new().with_actions(vec![CardListActionType::Selection]);
    let mut component = create_test_component_with_config(config);
    let card1 = Uuid::new_v4();
    let card2 = Uuid::new_v4();

    component.update_cards(vec![card1, card2]);
    assert_eq!(component.get_selected_index(), Some(0));

    component.handle_key(KeyCode::Char('j'));
    assert_eq!(component.get_selected_index(), Some(0)); // Should not move
}

// Selection Action Tests
#[test]
fn test_select_action_with_enter() {
    let mut component = create_test_component();
    let card_id = Uuid::new_v4();
    component.update_cards(vec![card_id]);

    let action = component.handle_key(KeyCode::Enter);
    assert_eq!(action, Some(CardListAction::Select(card_id)));
}

#[test]
fn test_select_action_with_space() {
    let mut component = create_test_component();
    let card_id = Uuid::new_v4();
    component.update_cards(vec![card_id]);

    let action = component.handle_key(KeyCode::Char(' '));
    assert_eq!(action, Some(CardListAction::Select(card_id)));
}

#[test]
fn test_select_action_disabled() {
    let config = CardListComponentConfig::new().with_actions(vec![CardListActionType::Navigation]);
    let mut component = create_test_component_with_config(config);
    let card_id = Uuid::new_v4();
    component.update_cards(vec![card_id]);

    let action = component.handle_key(KeyCode::Enter);
    assert_eq!(action, None);
}

// Action Tests
#[test]
fn test_edit_action() {
    let mut component = create_test_component();
    let card_id = Uuid::new_v4();
    component.update_cards(vec![card_id]);

    let action = component.handle_key(KeyCode::Char('e'));
    assert_eq!(action, Some(CardListAction::Edit(card_id)));
}

#[test]
fn test_edit_action_disabled() {
    let config = CardListComponentConfig::new().with_actions(vec![CardListActionType::Navigation]);
    let mut component = create_test_component_with_config(config);
    let card_id = Uuid::new_v4();
    component.update_cards(vec![card_id]);

    let action = component.handle_key(KeyCode::Char('e'));
    assert_eq!(action, None);
}

#[test]
fn test_complete_action() {
    let mut component = create_test_component();
    let card_id = Uuid::new_v4();
    component.update_cards(vec![card_id]);

    let action = component.handle_key(KeyCode::Char('c'));
    assert_eq!(action, Some(CardListAction::Complete(card_id)));
}

#[test]
fn test_priority_action() {
    let mut component = create_test_component();
    let card_id = Uuid::new_v4();
    component.update_cards(vec![card_id]);

    let action = component.handle_key(KeyCode::Char('p'));
    assert_eq!(action, Some(CardListAction::TogglePriority(card_id)));
}

#[test]
fn test_assign_sprint_action() {
    let mut component = create_test_component();
    let card_id = Uuid::new_v4();
    component.update_cards(vec![card_id]);

    let action = component.handle_key(KeyCode::Char('a'));
    assert_eq!(action, Some(CardListAction::AssignSprint(card_id)));
}

#[test]
fn test_reassign_sprint_action() {
    let mut component = create_test_component();
    let card_id = Uuid::new_v4();
    component.update_cards(vec![card_id]);

    let action = component.handle_key(KeyCode::Char('S'));
    assert_eq!(action, Some(CardListAction::ReassignSprint(card_id)));
}

#[test]
fn test_sort_action() {
    let mut component = create_test_component();

    let action = component.handle_key(KeyCode::Char('o'));
    assert_eq!(action, Some(CardListAction::Sort));
}

#[test]
fn test_order_cards_action() {
    let mut component = create_test_component();

    let action = component.handle_key(KeyCode::Char('O'));
    assert_eq!(action, Some(CardListAction::OrderCards));
}

#[test]
fn test_sort_action_disabled() {
    let config = CardListComponentConfig::new().with_actions(vec![CardListActionType::Navigation]);
    let mut component = create_test_component_with_config(config);

    let action = component.handle_key(KeyCode::Char('o'));
    assert_eq!(action, None);
}

#[test]
fn test_move_column_left() {
    let mut component = create_test_component();
    let card_id = Uuid::new_v4();
    component.update_cards(vec![card_id]);

    let action = component.handle_key(KeyCode::Char('H'));
    assert_eq!(action, Some(CardListAction::MoveColumn(card_id, false)));
}

#[test]
fn test_move_column_right() {
    let mut component = create_test_component();
    let card_id = Uuid::new_v4();
    component.update_cards(vec![card_id]);

    let action = component.handle_key(KeyCode::Char('L'));
    assert_eq!(action, Some(CardListAction::MoveColumn(card_id, true)));
}

#[test]
fn test_move_column_disabled() {
    let config = CardListComponentConfig::new().with_movement(false);
    let mut component = create_test_component_with_config(config);
    let card_id = Uuid::new_v4();
    component.update_cards(vec![card_id]);

    let action = component.handle_key(KeyCode::Char('H'));
    assert_eq!(action, None);
}

#[test]
fn test_create_action() {
    let mut component = create_test_component();

    let action = component.handle_key(KeyCode::Char('n'));
    assert_eq!(action, Some(CardListAction::Create));
}

#[test]
fn test_create_action_disabled() {
    let config = CardListComponentConfig::new().with_actions(vec![CardListActionType::Navigation]);
    let mut component = create_test_component_with_config(config);

    let action = component.handle_key(KeyCode::Char('n'));
    assert_eq!(action, None);
}

// Multi-Select Tests
#[test]
fn test_toggle_multi_select() {
    let mut component = create_test_component();
    let card_id = Uuid::new_v4();
    component.update_cards(vec![card_id]);

    component.toggle_multi_select(card_id);
    assert!(component.multi_selected.contains(&card_id));

    component.toggle_multi_select(card_id);
    assert!(!component.multi_selected.contains(&card_id));
}

#[test]
fn test_toggle_multi_select_disabled() {
    let config = CardListComponentConfig::new().with_multi_select(false);
    let mut component = create_test_component_with_config(config);
    let card_id = Uuid::new_v4();
    component.update_cards(vec![card_id]);

    component.toggle_multi_select(card_id);
    assert!(!component.multi_selected.contains(&card_id));
}

#[test]
fn test_toggle_multi_select_action() {
    let mut component = create_test_component();
    let card_id = Uuid::new_v4();
    component.update_cards(vec![card_id]);

    let action = component.handle_key(KeyCode::Char('v'));
    assert_eq!(action, Some(CardListAction::ToggleMultiSelect(card_id)));
}

#[test]
fn test_select_all_action() {
    let mut component = create_test_component();
    let card_id = Uuid::new_v4();
    component.update_cards(vec![card_id]);

    let action = component.handle_key(KeyCode::Char('V'));
    assert_eq!(action, Some(CardListAction::SelectAll));
}

#[test]
fn test_select_all() {
    let mut component = create_test_component();
    let card1 = Uuid::new_v4();
    let card2 = Uuid::new_v4();
    let card3 = Uuid::new_v4();

    component.update_cards(vec![card1, card2, card3]);
    component.select_all();

    assert_eq!(component.multi_selected.len(), 3);
    assert!(component.multi_selected.contains(&card1));
    assert!(component.multi_selected.contains(&card2));
    assert!(component.multi_selected.contains(&card3));
}

#[test]
fn test_select_all_disabled() {
    let config = CardListComponentConfig::new().with_multi_select(false);
    let mut component = create_test_component_with_config(config);
    let card1 = Uuid::new_v4();
    let card2 = Uuid::new_v4();

    component.update_cards(vec![card1, card2]);
    component.select_all();

    assert_eq!(component.multi_selected.len(), 0);
}

#[test]
fn test_clear_multi_select() {
    let mut component = create_test_component();
    let card1 = Uuid::new_v4();
    let card2 = Uuid::new_v4();

    component.update_cards(vec![card1, card2]);
    component.select_all();
    assert_eq!(component.multi_selected.len(), 2);

    component.clear_multi_select();
    assert_eq!(component.multi_selected.len(), 0);
}

#[test]
fn test_get_multi_selected() {
    let mut component = create_test_component();
    let card1 = Uuid::new_v4();
    let card2 = Uuid::new_v4();
    let card3 = Uuid::new_v4();

    component.update_cards(vec![card1, card2, card3]);
    component.toggle_multi_select(card1);
    component.toggle_multi_select(card3);

    let selected = component.get_multi_selected();
    assert_eq!(selected.len(), 2);
    assert!(selected.contains(&card1));
    assert!(selected.contains(&card3));
    assert!(!selected.contains(&card2));
}

// Misc Tests
#[test]
fn test_unknown_key() {
    let mut component = create_test_component();

    let action = component.handle_key(KeyCode::F(1));
    assert_eq!(action, None);
}

#[test]
fn test_component_is_empty() {
    let component = create_test_component();
    assert!(component.is_empty());
}

#[test]
fn test_component_length() {
    let mut component = create_test_component();
    let cards = vec![Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4()];
    component.update_cards(cards.clone());

    assert_eq!(component.len(), 3);
    assert!(!component.is_empty());
}

#[test]
fn test_set_selected_index() {
    let mut component = create_test_component();
    let cards = vec![Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4()];
    component.update_cards(cards);

    component.set_selected_index(Some(2));
    assert_eq!(component.get_selected_index(), Some(2));

    component.set_selected_index(None);
    assert_eq!(component.get_selected_index(), None);
}

#[test]
fn test_get_selected_card_id() {
    let mut component = create_test_component();
    let card1 = Uuid::new_v4();
    let card2 = Uuid::new_v4();

    component.update_cards(vec![card1, card2]);
    assert_eq!(component.get_selected_card_id(), Some(card1));

    component.handle_key(KeyCode::Char('j'));
    assert_eq!(component.get_selected_card_id(), Some(card2));
}

#[test]
fn test_help_text() {
    let component = create_test_component();
    let help = component.help_text();
    assert!(help.contains("ESC: cancel"));
    assert!(help.contains("j/k: navigate"));
}

#[test]
fn test_help_entries_text_renders_both_multi_select_hints_separately() {
    let config = CardListComponentConfig::default();
    let help = help_entries_text(&config.help_entries());
    assert!(help.ends_with("v: select card | V: multi-select"));
}
