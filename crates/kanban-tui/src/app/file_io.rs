use super::{App, BoardField, CardField};
use crate::editor::edit_in_external_editor;
use crate::events::EventHandler;
use kanban_core::Editable;
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;

impl App {
    pub fn edit_board_field(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
        event_handler: &EventHandler,
        field: BoardField,
    ) -> io::Result<()> {
        if let Some(board_idx) = self.selection.board.get() {
            let boards = self.model.boards();
            if let Some(board) = boards.get(board_idx) {
                let temp_dir = std::env::temp_dir();
                let (temp_file, current_content) = match field {
                    BoardField::Name => {
                        let temp_file = temp_dir.join(format!("kanban-board-{}-name.md", board.id));
                        (temp_file, board.name.clone())
                    }
                    BoardField::Description => {
                        let temp_file =
                            temp_dir.join(format!("kanban-board-{}-description.md", board.id));
                        let content = board.description.as_deref().unwrap_or("").to_string();
                        (temp_file, content)
                    }
                };

                let board_id = board.id;
                if let Some(new_content) =
                    edit_in_external_editor(terminal, event_handler, temp_file, &current_content)?
                {
                    let updates = match field {
                        BoardField::Name => {
                            if new_content.trim().is_empty() {
                                None
                            } else {
                                Some(kanban_domain::BoardUpdate {
                                    name: Some(new_content.trim().to_string()),
                                    ..Default::default()
                                })
                            }
                        }
                        BoardField::Description => {
                            let desc = if new_content.trim().is_empty() {
                                kanban_domain::FieldUpdate::Clear
                            } else {
                                kanban_domain::FieldUpdate::Set(new_content)
                            };
                            Some(kanban_domain::BoardUpdate {
                                description: desc,
                                ..Default::default()
                            })
                        }
                    };
                    if let Some(updates) = updates {
                        let cmd = kanban_domain::commands::Command::Board(
                            kanban_domain::commands::BoardCommand::Update(
                                kanban_domain::commands::UpdateBoard { board_id, updates },
                            ),
                        );
                        if let Err(e) = self.execute_command(cmd) {
                            tracing::error!("Failed to update board: {}", e);
                        }
                    }
                }
            }
        }
        Ok(())
    }

    pub fn edit_card_field(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
        event_handler: &EventHandler,
        field: CardField,
    ) -> io::Result<()> {
        if let Some(active_id) = self.selection.active_card_id {
            if let Some(card) = self.model.card(active_id) {
                let temp_dir = std::env::temp_dir();
                let (temp_file, current_content) = match field {
                    CardField::Title => {
                        let temp_file = temp_dir.join(format!("kanban-card-{}-title.md", card.id));
                        (temp_file, card.title.clone())
                    }
                    CardField::Description => {
                        let temp_file =
                            temp_dir.join(format!("kanban-card-{}-description.md", card.id));
                        let content = card.description.as_deref().unwrap_or("").to_string();
                        (temp_file, content)
                    }
                };

                let card_id = card.id;
                if let Some(new_content) =
                    edit_in_external_editor(terminal, event_handler, temp_file, &current_content)?
                {
                    let updates = match field {
                        CardField::Title => {
                            if new_content.trim().is_empty() {
                                None
                            } else {
                                Some(kanban_domain::CardUpdate {
                                    title: Some(new_content.trim().to_string()),
                                    ..Default::default()
                                })
                            }
                        }
                        CardField::Description => {
                            let desc = if new_content.trim().is_empty() {
                                kanban_domain::FieldUpdate::Clear
                            } else {
                                kanban_domain::FieldUpdate::Set(new_content)
                            };
                            Some(kanban_domain::CardUpdate {
                                description: desc,
                                ..Default::default()
                            })
                        }
                    };
                    if let Some(updates) = updates {
                        let cmd = kanban_domain::commands::Command::Card(
                            kanban_domain::commands::CardCommand::Update(
                                kanban_domain::commands::UpdateCard { card_id, updates },
                            ),
                        );
                        if let Err(e) = self.execute_command(cmd) {
                            tracing::error!("Failed to update card: {}", e);
                        }
                    }
                }
            }
        }
        Ok(())
    }

    pub fn edit_entity_json_impl<T: Editable<E>, E>(
        entity: &mut E,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
        event_handler: &EventHandler,
        temp_file: std::path::PathBuf,
    ) -> io::Result<()> {
        Self::edit_entity_impl::<T, E>(
            entity,
            terminal,
            event_handler,
            temp_file,
            crate::edit_format::EditFormat::Json,
        )
    }

    pub fn edit_entity_impl<T: Editable<E>, E>(
        entity: &mut E,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
        event_handler: &EventHandler,
        temp_file: std::path::PathBuf,
        format: crate::edit_format::EditFormat,
    ) -> io::Result<()> {
        let dto = T::from_entity(entity);
        let current_content = format.serialize(&dto).unwrap_or_else(|_| "{}".to_string());

        if let Some(new_content) =
            edit_in_external_editor(terminal, event_handler, temp_file, &current_content)?
        {
            match format.deserialize::<T>(&new_content) {
                Ok(updated_dto) => {
                    updated_dto.apply_to(entity);
                    tracing::info!("Updated entity via {} editor", format);
                }
                Err(e) => {
                    tracing::error!("Failed to parse {}: {}", format, e);
                }
            }
        }

        Ok(())
    }
}
