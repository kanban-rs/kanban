use crate::app::{App, DialogMode, Focus};
use crossterm::event::KeyCode;
use kanban_domain::CardFilters;
use kanban_view::filters::{FilterDialogSection, FilterDialogState};

impl App {
    pub fn handle_open_filter_dialog(&mut self) {
        if self.focus.active != Focus::Cards || self.selection.active_board_id.is_none() {
            return;
        }

        let filters = CardFilters {
            show_unassigned_sprints: self.filter.hide_assigned_cards,
            selected_sprint_ids: self.filter.active_sprint_filters.clone(),
            date_from: None,
            date_to: None,
            selected_tags: Default::default(),
        };

        self.filter.dialog_state = Some(FilterDialogState::new(filters));
        self.open_dialog(DialogMode::FilterOptions);
    }

    pub fn handle_filter_options_popup(&mut self, key_code: KeyCode) {
        use crossterm::event::KeyCode;

        if let Some(ref mut dialog_state) = self.filter.dialog_state {
            match key_code {
                KeyCode::Esc => {
                    self.filter.dialog_state = None;
                    self.pop_mode();
                }
                KeyCode::Char('j') | KeyCode::Down => match dialog_state.current_section {
                    FilterDialogSection::Sprints => {
                        if let Some(board_id) = self
                            .selection
                            .active_board_id
                            .and_then(|id| self.model.board_by_id(id).map(|b| b.id))
                        {
                            {
                                let sprint_count = self
                                    .model
                                    .sprints()
                                    .iter()
                                    .filter(|s| s.board_id == board_id)
                                    .count();
                                let total_items = 1 + sprint_count;
                                if dialog_state.item_selection < total_items.saturating_sub(1) {
                                    dialog_state.item_selection += 1;
                                } else {
                                    dialog_state.next_section();
                                }
                            }
                        }
                    }
                    _ => {
                        dialog_state.next_section();
                    }
                },
                KeyCode::Char('k') | KeyCode::Up => match dialog_state.current_section {
                    FilterDialogSection::Sprints if dialog_state.item_selection > 0 => {
                        dialog_state.item_selection -= 1;
                    }
                    _ => {
                        dialog_state.prev_section();
                    }
                },
                KeyCode::Char(' ') => {
                    if dialog_state.current_section == FilterDialogSection::Sprints {
                        if dialog_state.item_selection == 0 {
                            dialog_state.filters.show_unassigned_sprints =
                                !dialog_state.filters.show_unassigned_sprints;
                            tracing::info!(
                                "Toggled unassigned sprints filter: {}",
                                dialog_state.filters.show_unassigned_sprints
                            );
                            self.apply_filters();
                        } else if let Some(board) = self
                            .selection
                            .active_board_id
                            .and_then(|id| self.model.board_by_id(id))
                        {
                            {
                                let sprints = self.model.sprints();
                                let board_sprints: Vec<_> =
                                    sprints.iter().filter(|s| s.board_id == board.id).collect();

                                let sprint_idx = dialog_state.item_selection - 1;
                                if let Some(sprint) = board_sprints.get(sprint_idx) {
                                    if dialog_state
                                        .filters
                                        .selected_sprint_ids
                                        .contains(&sprint.id)
                                    {
                                        dialog_state.filters.selected_sprint_ids.remove(&sprint.id);
                                    } else {
                                        dialog_state.filters.selected_sprint_ids.insert(sprint.id);
                                    }
                                    tracing::info!(
                                        "Toggled sprint: {}",
                                        sprint.formatted_name(board, None)
                                    );
                                    self.apply_filters();
                                }
                            }
                        }
                    }
                }
                KeyCode::Enter => {
                    self.apply_filters();
                    self.filter.dialog_state = None;
                    self.pop_mode();
                }
                _ => {}
            }
        }
    }

    fn apply_filters(&mut self) {
        if let Some(dialog_state) = &self.filter.dialog_state {
            self.filter.hide_assigned_cards = dialog_state.filters.show_unassigned_sprints;
            self.filter.active_sprint_filters = dialog_state.filters.selected_sprint_ids.clone();
            tracing::info!(
                "Applied filters: unassigned={}, sprints={}",
                self.filter.hide_assigned_cards,
                self.filter.active_sprint_filters.len()
            );
        }
    }
}
