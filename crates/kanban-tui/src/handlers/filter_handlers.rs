use crate::app::{App, DialogMode, Focus};
use crossterm::event::KeyCode;
use kanban_domain::{CardFilters, LoadState};
use kanban_view::filters::{FilterDialogSection, FilterDialogState};

impl App {
    pub fn handle_open_filter_dialog(&mut self) {
        if self.focus.active != Focus::Cards || self.selection.active_board_id.is_none() {
            return;
        }

        // Pre-populate the label filter with the currently active labels so the
        // popup opens on the user's existing filter rather than blank.
        let initial_tag_input = self
            .filter
            .active_labels
            .as_deref()
            .map(|v| v.join(","))
            .unwrap_or_default();

        let mut filters = CardFilters {
            show_unassigned_sprints: self.filter.hide_assigned_cards,
            selected_sprint_ids: self.filter.active_sprint_filters.clone(),
            date_from: None,
            date_to: None,
            selected_labels: Default::default(),
        };
        if let Some(ref labels) = self.filter.active_labels {
            for t in labels.iter() {
                filters.selected_labels.insert(t.clone());
            }
        }

        let mut dialog = FilterDialogState::new(filters);
        dialog.labels_input = initial_tag_input;

        self.filter.dialog_state = Some(dialog);
        self.open_dialog(DialogMode::FilterOptions);
    }

    pub fn handle_filter_options_popup(&mut self, key_code: KeyCode) {
        use crossterm::event::KeyCode;

        if self.filter.dialog_state.is_none() {
            return;
        }

        let board_id = self
            .selection
            .active_board_id
            .and_then(|id| self.model.board_by_id_state(id).loaded().copied())
            .map(|board| board.id);
        let board_sprints = board_id.map(|id| self.board_sprints_view(id));

        if let Some(ref mut dialog_state) = self.filter.dialog_state {
            // ---- Tags section: free-form text input ----
            let in_tags_section = dialog_state.current_section == FilterDialogSection::Labels;

            match key_code {
                KeyCode::Esc => {
                    self.filter.dialog_state = None;
                    self.pop_mode();
                }
                // Printable characters appended to the labels buffer only when the
                // Tags section is focused; ignored otherwise. `j` and `k` are
                // reserved for section navigation (see below) and are excluded.
                KeyCode::Char(c)
                    if in_tags_section && c.is_alphanumeric() && c != 'j' && c != 'k' =>
                {
                    dialog_state.labels_input.push(c);
                }
                KeyCode::Char(c) if in_tags_section && (c == ',' || c == ' ') => {
                    dialog_state.labels_input.push(c);
                }
                KeyCode::Backspace if in_tags_section => {
                    dialog_state.labels_input.pop();
                }
                // Enter applies the filter and closes the popup.
                KeyCode::Enter => {
                    // Parse the raw labels input into the dialog's CardFilters so
                    // `apply_filters` can pick it up; an empty string clears the
                    // label filter.
                    let parsed =
                        kanban_domain::parse_csv_labels(Some(dialog_state.labels_input.as_str()));
                    dialog_state.filters.selected_labels.clear();
                    if let Some(ref labels) = parsed {
                        for t in labels.iter() {
                            dialog_state.filters.selected_labels.insert(t.clone());
                        }
                    }
                    self.apply_filters();
                    self.filter.dialog_state = None;
                    self.pop_mode();
                }
                KeyCode::Char('j') | KeyCode::Down => match dialog_state.current_section {
                    FilterDialogSection::Sprints => match &board_sprints {
                        Some(LoadState::Loaded(sprints)) => {
                            let total_items = 1 + sprints.len();
                            if dialog_state.item_selection < total_items.saturating_sub(1) {
                                dialog_state.item_selection += 1;
                            } else {
                                dialog_state.next_section();
                            }
                        }
                        Some(_) => {
                            self.set_error("Sprints are not loaded yet".to_string());
                        }
                        None => {}
                    },
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
                        // Entering Sprints from below (Date Range) —
                        // select the last item so the cursor is closest to
                        // where the user came from.
                        if dialog_state.current_section == FilterDialogSection::Sprints {
                            if let Some(LoadState::Loaded(sprints)) = &board_sprints {
                                let total = 1 + sprints.len();
                                if total > 1 {
                                    dialog_state.item_selection = total - 1;
                                }
                            }
                        }
                    }
                },
                KeyCode::Char(' ')
                    if dialog_state.current_section == FilterDialogSection::Sprints =>
                {
                    if dialog_state.item_selection == 0 {
                        dialog_state.filters.show_unassigned_sprints =
                            !dialog_state.filters.show_unassigned_sprints;
                        tracing::info!(
                            "Toggled unassigned sprints filter: {}",
                            dialog_state.filters.show_unassigned_sprints
                        );
                        self.apply_filters();
                    } else {
                        match &board_sprints {
                            Some(LoadState::Loaded(sprints)) => {
                                let sprint_idx = dialog_state.item_selection - 1;
                                if let Some(sprint) = sprints.get(sprint_idx) {
                                    let sprint_id = sprint.id;
                                    if dialog_state
                                        .filters
                                        .selected_sprint_ids
                                        .contains(&sprint_id)
                                    {
                                        dialog_state.filters.selected_sprint_ids.remove(&sprint_id);
                                    } else {
                                        dialog_state.filters.selected_sprint_ids.insert(sprint_id);
                                    }
                                    tracing::info!("Toggled sprint: {sprint_id}");
                                    self.apply_filters();
                                }
                            }
                            Some(_) => {
                                self.set_error("Sprints are not loaded yet".to_string());
                            }
                            None => {}
                        }
                    }
                }
                _ => {}
            }
        }
    }

    fn apply_filters(&mut self) {
        if let Some(dialog_state) = &self.filter.dialog_state {
            self.filter.hide_assigned_cards = dialog_state.filters.show_unassigned_sprints;
            self.filter.active_sprint_filters = dialog_state.filters.selected_sprint_ids.clone();

            // Convert the selected_labels set into the active_labels vec (or None
            // when empty, meaning "no label filter").
            if dialog_state.filters.selected_labels.is_empty() {
                self.filter.active_labels = None;
            } else {
                let mut labels: Vec<String> =
                    dialog_state.filters.selected_labels.iter().cloned().collect();
                labels.sort();
                self.filter.active_labels = Some(labels);
            }

            tracing::info!(
                "Applied filters: unassigned={}, sprints={}, labels={:?}",
                self.filter.hide_assigned_cards,
                self.filter.active_sprint_filters.len(),
                self.filter.active_labels
            );
        }
    }
}
