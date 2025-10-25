use crate::app::{App, AppMode, CardField, Focus};
use crate::card_list::CardListId;
use crate::events::EventHandler;
use crate::view_strategy::{GroupedViewStrategy, KanbanViewStrategy, ViewStrategy};
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

    fn get_focused_column_id(&mut self) -> Option<uuid::Uuid> {
        let grouped_strategy = self
            .view_strategy
            .as_any_mut()
            .downcast_mut::<GroupedViewStrategy>();

        if let Some(grouped) = grouped_strategy {
            if let Some(task_list) = grouped.get_active_task_list() {
                if let CardListId::Column(column_id) = task_list.id {
                    return Some(column_id);
                }
            }
        }

        let kanban_strategy = self
            .view_strategy
            .as_any_mut()
            .downcast_mut::<KanbanViewStrategy>();

        if let Some(kanban) = kanban_strategy {
            if let Some(task_list) = kanban.get_active_task_list() {
                if let CardListId::Column(column_id) = task_list.id {
                    return Some(column_id);
                }
            }
        }

        None
    }

    pub fn handle_toggle_card_completion(&mut self) {
        if self.focus != Focus::Cards {
            return;
        }

        if !self.selected_cards.is_empty() {
            self.toggle_selected_cards_completion();
        } else {
            self.toggle_card_completion();
        }
    }

    pub fn handle_card_selection_toggle(&mut self) {
        if self.focus == Focus::Cards {
            if let Some(card) = self.get_selected_card_in_context() {
                let card_id = card.id;
                if self.selected_cards.contains(&card_id) {
                    self.selected_cards.remove(&card_id);
                } else {
                    self.selected_cards.insert(card_id);
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
        } else if self.get_selected_card_id().is_some() {
            if let Some(board_idx) = self.active_board_index {
                if let Some(board) = self.boards.get(board_idx) {
                    let sprint_count = self
                        .sprints
                        .iter()
                        .filter(|s| s.board_id == board.id)
                        .count();
                    if sprint_count > 0 {
                        if let Some(selected_card) = self.get_selected_card_in_context() {
                            let card_id = selected_card.id;
                            let actual_idx = self.cards.iter().position(|c| c.id == card_id);
                            self.active_card_index = actual_idx;
                        }
                        let selection_idx = self.get_current_sprint_selection_index();
                        self.sprint_assign_selection.set(Some(selection_idx));
                        self.mode = AppMode::AssignCardToSprint;
                    }
                }
            }
        }
    }

    pub fn handle_order_cards_key(&mut self) {
        if self.focus == Focus::Cards && self.active_board_index.is_some() {
            let sort_idx = self.get_current_sort_field_selection_index();
            self.sort_field_selection.set(Some(sort_idx));
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
                self.refresh_view();
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

            self.refresh_view();
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

                        self.refresh_view();
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
        if self.focus == Focus::Cards {
            if let Some(selected_card) = self.get_selected_card_in_context() {
                let card_id = selected_card.id;
                let actual_idx = self.cards.iter().position(|c| c.id == card_id);
                self.active_card_index = actual_idx;

                if let Err(e) =
                    self.edit_card_field(terminal, event_handler, CardField::Description)
                {
                    tracing::error!("Failed to edit card description: {}", e);
                }
                should_restart = true;
            }
        }
        should_restart
    }

    fn toggle_card_completion(&mut self) {
        if let Some(card) = self.get_selected_card_in_context() {
            let card_id = card.id;
            let new_status = if card.status == CardStatus::Done {
                CardStatus::Todo
            } else {
                CardStatus::Done
            };

            let target_column_and_position = if let Some(board_idx) = self.active_board_index {
                if let Some(board) = self.boards.get(board_idx) {
                    let mut board_columns: Vec<_> = self
                        .columns
                        .iter()
                        .filter(|col| col.board_id == board.id)
                        .collect();
                    board_columns.sort_by_key(|col| col.position);

                    let old_column_id = card.column_id;
                    let current_column_pos =
                        board_columns.iter().position(|col| col.id == old_column_id);

                    if new_status == CardStatus::Done {
                        // Moving to Done: move to last column
                        if let Some(last_col) = board_columns.last() {
                            if last_col.id != old_column_id {
                                let position = self
                                    .cards
                                    .iter()
                                    .filter(|c| c.column_id == last_col.id)
                                    .count() as i32;
                                Some((last_col.id, position, "last"))
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    } else {
                        // Moving from Done to Todo: move to second-to-last column if in last column
                        if let Some(pos) = current_column_pos {
                            if pos == board_columns.len() - 1 && board_columns.len() > 1 {
                                // Currently in last column, move to second-to-last
                                let target_col = &board_columns[board_columns.len() - 2];
                                let position = self
                                    .cards
                                    .iter()
                                    .filter(|c| c.column_id == target_col.id)
                                    .count() as i32;
                                Some((target_col.id, position, "second-to-last"))
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    }
                } else {
                    None
                }
            } else {
                None
            };

            if let Some(card) = self.cards.iter_mut().find(|c| c.id == card_id) {
                card.update_status(new_status);

                if let Some((target_column_id, position, column_desc)) = target_column_and_position
                {
                    card.move_to_column(target_column_id, position);
                    tracing::info!(
                        "Moved card '{}' to {} column (status: {:?})",
                        card.title,
                        column_desc,
                        new_status
                    );
                } else {
                    tracing::info!("Toggled card '{}' to status: {:?}", card.title, new_status);
                }
            }
            self.refresh_view();
            self.select_card_by_id(card_id);
        }
    }

    fn toggle_selected_cards_completion(&mut self) {
        let card_ids: Vec<uuid::Uuid> = self.selected_cards.iter().copied().collect();
        let mut toggled_count = 0;
        let first_card_id = card_ids.first().copied();

        let board_columns: Vec<_> = if let Some(board_idx) = self.active_board_index {
            if let Some(board) = self.boards.get(board_idx) {
                let mut cols: Vec<_> = self
                    .columns
                    .iter()
                    .filter(|col| col.board_id == board.id)
                    .collect();
                cols.sort_by_key(|col| col.position);
                cols
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };

        for card_id in card_ids {
            // First, get card info without mutable borrow
            let (old_column_id, old_status) =
                if let Some(card) = self.cards.iter().find(|c| c.id == card_id) {
                    (card.column_id, card.status)
                } else {
                    continue;
                };

            let new_status = if old_status == CardStatus::Done {
                CardStatus::Todo
            } else {
                CardStatus::Done
            };

            let current_column_pos = board_columns.iter().position(|col| col.id == old_column_id);

            // Determine target column
            let target_column_id = if new_status == CardStatus::Done {
                // Moving to Done: move to last column
                board_columns.last().and_then(|last_col| {
                    if last_col.id != old_column_id {
                        Some(last_col.id)
                    } else {
                        None
                    }
                })
            } else {
                // Moving from Done to Todo: move to second-to-last column if in last column
                if let Some(pos) = current_column_pos {
                    if pos == board_columns.len() - 1 && board_columns.len() > 1 {
                        Some(board_columns[board_columns.len() - 2].id)
                    } else {
                        None
                    }
                } else {
                    None
                }
            };

            // Calculate position in target column before mutable borrow
            let target_position = target_column_id
                .map(|col_id| self.cards.iter().filter(|c| c.column_id == col_id).count() as i32);

            // Now mutate the card with calculated values
            if let Some(card) = self.cards.iter_mut().find(|c| c.id == card_id) {
                card.update_status(new_status);

                if let (Some(target_col_id), Some(position)) = (target_column_id, target_position) {
                    card.move_to_column(target_col_id, position);
                }

                toggled_count += 1;
            }
        }

        tracing::info!("Toggled {} cards completion status", toggled_count);
        self.selected_cards.clear();
        self.refresh_view();
        if let Some(card_id) = first_card_id {
            self.select_card_by_id(card_id);
        }
    }

    pub fn create_card(&mut self) {
        if let Some(idx) = self.active_board_index {
            let focused_col_id = self.get_focused_column_id();
            let board_id = self.boards.get(idx).map(|b| b.id);

            if let Some(bid) = board_id {
                if let Some(board) = self.boards.get_mut(idx) {
                    let target_column_id = if let Some(focused_col_id) = focused_col_id {
                        Some(focused_col_id)
                    } else {
                        self.columns
                            .iter()
                            .find(|col| col.board_id == bid)
                            .map(|col| col.id)
                    };

                    let column = if let Some(col_id) = target_column_id {
                        self.columns.iter().find(|col| col.id == col_id).cloned()
                    } else {
                        None
                    };

                    let column = match column {
                        Some(col) => col,
                        None => {
                            let new_column = Column::new(bid, "Todo".to_string(), 0);
                            self.columns.push(new_column.clone());
                            new_column
                        }
                    };

                    let position = self
                        .cards
                        .iter()
                        .filter(|c| c.column_id == column.id)
                        .count() as i32;
                    let mut card =
                        Card::new(board, column.id, self.input.as_str().to_string(), position);
                    let new_card_id = card.id;
                    let column_name = column.name.clone();

                    let board_columns: Vec<_> = self
                        .columns
                        .iter()
                        .filter(|col| col.board_id == bid)
                        .collect();

                    if board_columns.len() > 2 {
                        let sorted_cols: Vec<_> = {
                            let mut cols = board_columns.clone();
                            cols.sort_by_key(|col| col.position);
                            cols
                        };
                        if let Some(last_col) = sorted_cols.last() {
                            if last_col.id == column.id {
                                card.update_status(CardStatus::Done);
                                tracing::info!(
                                    "Creating card: {} (id: {}) in column: {} [marked as complete]",
                                    card.title,
                                    card.id,
                                    column_name
                                );
                            } else {
                                tracing::info!(
                                    "Creating card: {} (id: {}) in column: {}",
                                    card.title,
                                    card.id,
                                    column_name
                                );
                            }
                        } else {
                            tracing::info!(
                                "Creating card: {} (id: {}) in column: {}",
                                card.title,
                                card.id,
                                column_name
                            );
                        }
                    } else {
                        tracing::info!(
                            "Creating card: {} (id: {}) in column: {}",
                            card.title,
                            card.id,
                            column_name
                        );
                    }

                    self.cards.push(card);

                    self.refresh_view();
                    self.select_card_by_id(new_card_id);
                }
            }
        }
    }

    pub fn handle_move_card_left(&mut self) {
        if self.focus != Focus::Cards {
            return;
        }

        if let Some(card) = self.get_selected_card_in_context() {
            if let Some(board_idx) = self.active_board_index {
                if let Some(board) = self.boards.get(board_idx) {
                    let card_id = card.id;
                    let current_column_id = card.column_id;

                    let mut board_columns: Vec<_> = self
                        .columns
                        .iter()
                        .filter(|col| col.board_id == board.id)
                        .collect();
                    board_columns.sort_by_key(|col| col.position);

                    let current_position = board_columns
                        .iter()
                        .position(|col| col.id == current_column_id);

                    if let Some(pos) = current_position {
                        if pos > 0 {
                            let target_column_id = board_columns[pos - 1].id;
                            let is_moving_from_last = pos == board_columns.len() - 1;

                            let new_position = self
                                .cards
                                .iter()
                                .filter(|c| c.column_id == target_column_id)
                                .count() as i32;

                            if let Some(card) = self.cards.iter_mut().find(|c| c.id == card_id) {
                                card.move_to_column(target_column_id, new_position);

                                // If moving from last column and board has more than 1 column, unmark as complete
                                if is_moving_from_last
                                    && board_columns.len() > 1
                                    && card.status == CardStatus::Done
                                {
                                    card.update_status(CardStatus::Todo);
                                    tracing::info!(
                                        "Moved card '{}' from last column (unmarked as complete)",
                                        card.title
                                    );
                                } else {
                                    tracing::info!("Moved card '{}' to previous column", card.title);
                                }
                            }

                            if self.is_kanban_view() {
                                if let Some(current_col_idx) = self.column_selection.get() {
                                    if current_col_idx > 0 {
                                        self.column_selection.set(Some(current_col_idx - 1));
                                    }
                                }
                                self.refresh_view();
                                self.select_card_by_id(card_id);
                            } else {
                                self.refresh_view();
                                self.select_card_by_id(card_id);
                            }
                        } else {
                            tracing::warn!("Card is already in the first column");
                        }
                    }
                }
            }
        }
    }

    pub fn handle_move_card_right(&mut self) {
        if self.focus != Focus::Cards {
            return;
        }

        if let Some(card) = self.get_selected_card_in_context() {
            if let Some(board_idx) = self.active_board_index {
                if let Some(board) = self.boards.get(board_idx) {
                    let card_id = card.id;
                    let current_column_id = card.column_id;

                    let mut board_columns: Vec<_> = self
                        .columns
                        .iter()
                        .filter(|col| col.board_id == board.id)
                        .collect();
                    board_columns.sort_by_key(|col| col.position);

                    let current_position = board_columns
                        .iter()
                        .position(|col| col.id == current_column_id);

                    if let Some(pos) = current_position {
                        if pos < board_columns.len() - 1 {
                            let target_column_id = board_columns[pos + 1].id;
                            let is_moving_to_last = pos + 1 == board_columns.len() - 1;

                            let new_position = self
                                .cards
                                .iter()
                                .filter(|c| c.column_id == target_column_id)
                                .count() as i32;

                            if let Some(card) = self.cards.iter_mut().find(|c| c.id == card_id) {
                                card.move_to_column(target_column_id, new_position);

                                // If moving to last column and board has more than 1 column, mark as complete
                                if is_moving_to_last
                                    && board_columns.len() > 1
                                    && card.status != CardStatus::Done
                                {
                                    card.update_status(CardStatus::Done);
                                    tracing::info!(
                                        "Moved card '{}' to last column (marked as complete)",
                                        card.title
                                    );
                                } else {
                                    tracing::info!("Moved card '{}' to next column", card.title);
                                }
                            }

                            if self.is_kanban_view() {
                                let column_count = board_columns.len();
                                if let Some(current_col_idx) = self.column_selection.get() {
                                    if current_col_idx < column_count - 1 {
                                        self.column_selection.set(Some(current_col_idx + 1));
                                    }
                                }
                                self.refresh_view();
                                self.select_card_by_id(card_id);
                            } else {
                                self.refresh_view();
                                self.select_card_by_id(card_id);
                            }
                        } else {
                            tracing::warn!("Card is already in the last column");
                        }
                    }
                }
            }
        }
    }
}
