use super::{animation, App};
use kanban_domain::AnimationType;
use std::time::Instant;

impl App {
    pub fn handle_animation_tick(&mut self) {
        let now = Instant::now();
        let mut completed_animations = Vec::new();

        for (&card_id, animation) in &self.animation.animating {
            let elapsed = now.duration_since(animation.start_time).as_millis();
            if elapsed >= animation::ANIMATION_DURATION_MS {
                completed_animations.push((card_id, animation.animation_type));
            }
        }

        // Group animations by type for batch processing
        let mut archive_cards = Vec::new();
        let mut affected_columns: Vec<uuid::Uuid> = Vec::new();
        let mut restore_cards = Vec::new();
        let mut delete_cards = Vec::new();

        for (card_id, animation_type) in completed_animations {
            self.animation.animating.remove(&card_id);
            match animation_type {
                AnimationType::Archiving => {
                    let cards = self.model.all_cards();
                    if let Some(card_pos) = cards.iter().position(|c| c.id == card_id) {
                        let card = &cards[card_pos];
                        if !affected_columns.contains(&card.column_id) {
                            affected_columns.push(card.column_id);
                        }
                        archive_cards.push(card_id);
                    }
                }
                AnimationType::Restoring => {
                    restore_cards.push(card_id);
                }
                AnimationType::Deleting => {
                    delete_cards.push(card_id);
                }
            }
        }

        let had_archives = !archive_cards.is_empty();
        let had_deletes = !delete_cards.is_empty();

        // Execute archive + per-column compact as a single undo batch so that
        // one user-perceived "delete" maps to one `u` press to undo.
        if had_archives {
            let mut commands = vec![kanban_domain::commands::Command::Card(
                kanban_domain::commands::CardCommand::Archive(
                    kanban_domain::commands::ArchiveCards { ids: archive_cards },
                ),
            )];
            for column_id in &affected_columns {
                commands.push(kanban_domain::commands::Command::Card(
                    kanban_domain::commands::CardCommand::CompactPositions(
                        kanban_domain::commands::CompactColumnPositions {
                            column_id: *column_id,
                        },
                    ),
                ));
            }

            if let Err(e) = self.execute_commands_batch(commands) {
                tracing::error!("Failed to archive cards: {}", e);
            } else {
                self.reload_model();
                if let Some((column_id, position)) = self.animation.archive_anchor.take() {
                    self.select_card_after_deletion(column_id, position);
                }
            }
        }

        // Execute batch delete commands
        if had_deletes {
            let mut delete_commands: Vec<kanban_domain::commands::Command> = Vec::new();
            for card_id in delete_cards {
                let cmd = kanban_domain::commands::Command::Card(
                    kanban_domain::commands::CardCommand::Delete(
                        kanban_domain::commands::DeleteCard { card_id },
                    ),
                );
                delete_commands.push(cmd);
            }
            if let Err(e) = self.execute_commands_batch(delete_commands) {
                tracing::error!("Failed to delete cards: {}", e);
            } else {
                self.reload_model();
            }
        }

        // Handle restore animations individually (less common)
        for card_id in restore_cards {
            self.complete_restore_animation(card_id);
        }
    }

    fn complete_restore_animation(&mut self, card_id: uuid::Uuid) {
        if let Some(archived_card) = self
            .model
            .archived_card_markers()
            .iter()
            .find(|dc| dc.entity_id == card_id)
            .cloned()
        {
            self.restore_card(archived_card);
        }
    }
}
