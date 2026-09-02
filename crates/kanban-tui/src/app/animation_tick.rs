use super::{animation, App};
use kanban_domain::{AnimationType, LoadState};
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
                    let cards = self.model.cards_state().loaded_or_empty();
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
        // one user-perceived "delete" maps to one `u` press to undo. Kept as
        // its own `execute_commands_batch` call, separate from the delete
        // batch below, so archiving and permanently deleting stay two
        // distinct undo entries even when both complete in the same tick.
        let archived_ok = if had_archives {
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

            match self.execute_commands_batch(commands) {
                Err(e) => {
                    tracing::error!("Failed to archive cards: {}", e);
                    self.set_error(format!("Failed to archive cards: {}", e));
                    false
                }
                Ok(_) => true,
            }
        } else {
            false
        };

        // Execute batch delete commands
        let deleted_ok = if had_deletes {
            let mut delete_commands: Vec<kanban_domain::commands::Command> = Vec::new();
            for card_id in delete_cards {
                let cmd = kanban_domain::commands::Command::Card(
                    kanban_domain::commands::CardCommand::Delete(
                        kanban_domain::commands::DeleteCard { card_id },
                    ),
                );
                delete_commands.push(cmd);
            }
            match self.execute_commands_batch(delete_commands) {
                Err(e) => {
                    tracing::error!("Failed to delete cards: {}", e);
                    self.set_error(format!("Failed to delete cards: {}", e));
                    false
                }
                Ok(_) => true,
            }
        } else {
            false
        };

        // Restores have no intra-loop dependency: `RestoreCard` only ever
        // touches the card being restored (its own column/position/marker),
        // never another card's, so each iteration is safe to run against a
        // model that predates the previous iteration's restore. Each stays
        // its own `execute_command` call (its own undo entry, one user
        // action each), but none of them reloads on its own.
        let mut restored_ids = Vec::new();
        for card_id in restore_cards {
            if self.complete_restore_animation(card_id) {
                restored_ids.push(card_id);
            }
        }
        let restored_any = !restored_ids.is_empty();

        // One refresh for the tick, regardless of how many animation kinds
        // completed in it.
        if archived_ok || deleted_ok || restored_any {
            self.reload_model();
        }

        // Selection reconciliation runs against the refreshed model.
        // `select_card_by_id` resolves through the task lists, which are
        // still stale until the next `prepare_frame`, so a card restored in
        // this same tick would silently no-op the selection if it were
        // picked as the candidate. Exclude it, so the fix-up only lands on
        // cards the task lists already know about, exactly as it would have
        // before restores shared this tick's reload.
        if archived_ok {
            if let Some((column_id, position)) = self.animation.archive_anchor.take() {
                self.select_card_after_deletion_excluding(column_id, position, &restored_ids);
            }
        }
    }

    fn complete_restore_animation(&mut self, card_id: uuid::Uuid) -> bool {
        if let Some(archived_card) = self
            .model
            .archived_card_markers()
            .iter()
            .find(|dc| dc.entity_id == card_id)
            .cloned()
        {
            self.restore_card_without_reload(archived_card)
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{animation, App};
    use crate::app::CardAnimation;
    use kanban_domain::{CreateCardOptions, EntityIds, Invalidation, KanbanOperations, Snapshot};
    use std::time::{Duration, Instant};

    fn refresh(app: &mut App) {
        let snap = Snapshot {
            archived_boards: Vec::new(),
            boards: app.ctx.data_store().list_boards().unwrap(),
            columns: app.ctx.data_store().list_all_columns().unwrap(),
            cards: app.ctx.data_store().list_all_cards().unwrap(),
            archived_cards: app.ctx.data_store().list_archived_cards().unwrap(),
            sprints: app.ctx.data_store().list_all_sprints().unwrap(),
            graph: app.ctx.data_store().get_graph().unwrap(),
            prefixes: Vec::new(),
        };
        app.load_snapshot(snap);
    }

    #[test]
    fn test_handle_animation_tick_with_a_not_loaded_cards_tier_skips_the_archive_without_a_banner()
    {
        let mut app = App::test_default();
        let board = app.ctx.create_board("Board".into(), None).unwrap();
        let column = app
            .ctx
            .create_column(board.id, "Todo".into(), None)
            .unwrap();
        let card = app
            .ctx
            .create_card(
                board.id,
                column.id,
                "Card".into(),
                CreateCardOptions::default(),
            )
            .unwrap();
        refresh(&mut app);

        app.animation.animating.insert(
            card.id,
            CardAnimation {
                animation_type: kanban_domain::AnimationType::Archiving,
                start_time: Instant::now()
                    - Duration::from_millis(animation::ANIMATION_DURATION_MS as u64 + 50),
            },
        );

        let _ = app
            .model
            .invalidate(Invalidation::Entities(EntityIds::cards([
                uuid::Uuid::new_v4(),
            ])));

        app.handle_animation_tick();

        assert!(
            app.ui_state.banner.is_none(),
            "a NotLoaded cards tier on the per-frame archive path must degrade silently, not bannering every tick"
        );
        assert!(
            !app.animation.animating.contains_key(&card.id),
            "the completed animation entry must still be removed"
        );
        let stored = app.ctx.data_store().get_card(card.id).unwrap();
        assert!(
            stored.is_some(),
            "the card must not be archived while the cards tier is not loaded, since it was silently skipped"
        );
    }
}
