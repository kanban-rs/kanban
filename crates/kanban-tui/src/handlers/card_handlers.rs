use crate::app::{App, AppMode, CardField, Focus};
use crate::events::EventHandler;
use kanban_domain::{Card, CardStatus, Column, SortOrder};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;

impl App {
    pub fn handle_create_card_key(&mut self) {
        if self.focus == Focus::Cards && self.active_board_index.is_some() {
            self.mode = AppMode::CreateCard;
            self.input.clear();
        }
    }

    pub fn handle_toggle_card_completion(&mut self) {
        if self.focus != Focus::Cards {
            return;
        }

        if !self.selected_cards.is_empty() {
            self.toggle_selected_cards_completion();
        } else if self.card_selection.get().is_some() {
            self.toggle_card_completion();
        }
    }

    pub fn handle_card_selection_toggle(&mut self) {
        if self.focus == Focus::Cards && self.card_selection.get().is_some() {
            if let Some(board_idx) = self.active_board_index {
                if let Some(board) = self.boards.get(board_idx) {
                    let card_id = {
                        let sorted_cards = self.get_sorted_board_cards(board.id);
                        if let Some(sorted_idx) = self.card_selection.get() {
                            sorted_cards.get(sorted_idx).map(|c| c.id)
                        } else {
                            None
                        }
                    };

                    if let Some(id) = card_id {
                        if self.selected_cards.contains(&id) {
                            self.selected_cards.remove(&id);
                        } else {
                            self.selected_cards.insert(id);
                        }
                    }
                }
            }
        }
    }

    pub fn handle_assign_to_sprint_key(&mut self) {
        if self.focus != Focus::Cards {
            return;
        }

        if !self.selected_cards.is_empty() {
            self.sprint_assign_selection.clear();
            self.mode = AppMode::AssignMultipleCardsToSprint;
        } else if let Some(sorted_idx) = self.card_selection.get() {
            if let Some(board_idx) = self.active_board_index {
                if let Some(board) = self.boards.get(board_idx) {
                    let sprint_count = self
                        .sprints
                        .iter()
                        .filter(|s| s.board_id == board.id)
                        .count();
                    if sprint_count > 0 {
                        let sorted_cards = self.get_sorted_board_cards(board.id);
                        if let Some(selected_card) = sorted_cards.get(sorted_idx) {
                            let card_id = selected_card.id;
                            let actual_idx = self.cards.iter().position(|c| c.id == card_id);
                            self.active_card_index = actual_idx;
                        }
                        self.sprint_assign_selection.set(Some(0));
                        self.mode = AppMode::AssignCardToSprint;
                    }
                }
            }
        }
    }

    pub fn handle_order_cards_key(&mut self) {
        if self.focus == Focus::Cards && self.active_board_index.is_some() {
            self.sort_field_selection.set(Some(0));
            self.mode = AppMode::OrderCards;
        }
    }

    pub fn handle_toggle_sort_order_key(&mut self) {
        if self.focus == Focus::Cards && self.active_board_index.is_some() {
            if let Some(current_order) = self.current_sort_order {
                let new_order = match current_order {
                    SortOrder::Ascending => SortOrder::Descending,
                    SortOrder::Descending => SortOrder::Ascending,
                };
                self.current_sort_order = Some(new_order);

                if let Some(board_idx) = self.active_board_index {
                    if let Some(board) = self.boards.get_mut(board_idx) {
                        if let Some(field) = self.current_sort_field {
                            board.update_task_sort(field, new_order);
                        }
                    }
                }

                tracing::info!("Toggled sort order to: {:?}", new_order);
            }
        }
    }

    pub fn handle_toggle_hide_assigned(&mut self) {
        if self.focus == Focus::Cards && self.active_board_index.is_some() {
            self.hide_assigned_cards = !self.hide_assigned_cards;
            let status = if self.hide_assigned_cards {
                "enabled"
            } else {
                "disabled"
            };
            tracing::info!("Hide assigned cards: {}", status);

            if let Some(board_idx) = self.active_board_index {
                if let Some(board) = self.boards.get(board_idx) {
                    let card_count = self.get_board_card_count(board.id);
                    if card_count > 0 {
                        self.card_selection.set(Some(0));
                    } else {
                        self.card_selection.clear();
                    }
                }
            }
        }
    }

    pub fn handle_toggle_sprint_filter(&mut self) {
        if self.focus == Focus::Cards && self.active_board_index.is_some() {
            if let Some(board_idx) = self.active_board_index {
                if let Some(board) = self.boards.get(board_idx) {
                    if let Some(active_sprint_id) = board.active_sprint_id {
                        if self.active_sprint_filter == Some(active_sprint_id) {
                            self.active_sprint_filter = None;
                            tracing::info!("Disabled sprint filter - showing all cards");
                        } else {
                            self.active_sprint_filter = Some(active_sprint_id);
                            tracing::info!("Enabled sprint filter - showing active sprint only");
                        }

                        let card_count = self.get_board_card_count(board.id);
                        if card_count > 0 {
                            self.card_selection.set(Some(0));
                        } else {
                            self.card_selection.clear();
                        }
                    } else {
                        tracing::warn!("No active sprint set for filtering");
                    }
                }
            }
        }
    }

    pub fn handle_edit_card_key(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
        event_handler: &EventHandler,
    ) -> bool {
        let mut should_restart = false;
        if self.focus == Focus::Cards && self.card_selection.get().is_some() {
            if let Some(sorted_idx) = self.card_selection.get() {
                if let Some(board_idx) = self.active_board_index {
                    if let Some(board) = self.boards.get(board_idx) {
                        let sorted_cards = self.get_sorted_board_cards(board.id);
                        if let Some(selected_card) = sorted_cards.get(sorted_idx) {
                            let card_id = selected_card.id;
                            let actual_idx = self.cards.iter().position(|c| c.id == card_id);
                            self.active_card_index = actual_idx;

                            if let Err(e) = self.edit_card_field(terminal, event_handler, CardField::Description) {
                                tracing::error!("Failed to edit card description: {}", e);
                            }
                            should_restart = true;
                        }
                    }
                }
            }
        }
        should_restart
    }

    fn toggle_card_completion(&mut self) {
        if let Some(sorted_idx) = self.card_selection.get() {
            if let Some(board_idx) = self.active_board_index {
                if let Some(board) = self.boards.get(board_idx) {
                    let sorted_cards = self.get_sorted_board_cards(board.id);

                    if let Some(card) = sorted_cards.get(sorted_idx) {
                        let card_id = card.id;
                        let new_status = if card.status == CardStatus::Done {
                            CardStatus::Todo
                        } else {
                            CardStatus::Done
                        };

                        if let Some(card) = self.cards.iter_mut().find(|c| c.id == card_id) {
                            card.update_status(new_status);
                            tracing::info!(
                                "Toggled card '{}' to status: {:?}",
                                card.title,
                                new_status
                            );
                        }
                    }
                }
            }
        }
    }

    fn toggle_selected_cards_completion(&mut self) {
        let card_ids: Vec<uuid::Uuid> = self.selected_cards.iter().copied().collect();
        let mut toggled_count = 0;

        for card_id in card_ids {
            if let Some(card) = self.cards.iter_mut().find(|c| c.id == card_id) {
                let new_status = if card.status == CardStatus::Done {
                    CardStatus::Todo
                } else {
                    CardStatus::Done
                };
                card.update_status(new_status);
                toggled_count += 1;
            }
        }

        tracing::info!("Toggled {} cards completion status", toggled_count);
        self.selected_cards.clear();
    }

    pub fn create_card(&mut self) {
        if let Some(idx) = self.active_board_index {
            if let Some(board) = self.boards.get_mut(idx) {
                let column = self
                    .columns
                    .iter()
                    .find(|col| col.board_id == board.id)
                    .cloned();

                let column = match column {
                    Some(col) => col,
                    None => {
                        let new_column = Column::new(board.id, "Todo".to_string(), 0);
                        self.columns.push(new_column.clone());
                        new_column
                    }
                };

                let position = self
                    .cards
                    .iter()
                    .filter(|c| c.column_id == column.id)
                    .count() as i32;
                let card = Card::new(board, column.id, self.input.as_str().to_string(), position);
                let new_card_id = card.id;
                let board_id = board.id;
                tracing::info!("Creating card: {} (id: {})", card.title, card.id);
                self.cards.push(card);

                let sorted_cards = self.get_sorted_board_cards(board_id);
                if let Some(pos) = sorted_cards.iter().position(|c| c.id == new_card_id) {
                    self.card_selection.set(Some(pos));
                }
            }
        }
    }
}
