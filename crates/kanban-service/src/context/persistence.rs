use super::KanbanContext;
use kanban_domain::{EntityIds, Invalidation, KanbanResult};

impl KanbanContext {
    /// Backfill `sprint_logs` for cards that have a `sprint_id` but empty logs.
    ///
    /// This is a one-time data-migration utility, not a regular operation —
    /// it bypasses the undo stack on purpose. The actual rule for what
    /// constitutes a correctly migrated log lives in
    /// [`kanban_domain::card_lifecycle::migrate_sprint_logs`]; this method
    /// just orchestrates the read → transform → persist-changed loop.
    ///
    /// `sprints` and `boards` are passed by shared reference to the pure
    /// function — they are reference data only, never mutated, so the
    /// persist loop correctly iterates `cards` alone.
    ///
    /// Returns the number of cards that received a backfilled log.
    pub fn migrate_sprint_logs(&mut self) -> KanbanResult<(usize, Option<Invalidation>)> {
        // C3b FIDELITY: raw reads — sprint-log migration must touch ALL cards.
        let mut cards = self.backend.list_all_cards()?;
        let sprints = self.backend.list_all_sprints()?;
        let boards = self.backend.list_boards()?;
        let before_logs: Vec<_> = cards.iter().map(|c| c.sprint_logs.clone()).collect();
        let count =
            kanban_domain::card_lifecycle::migrate_sprint_logs(&mut cards, &sprints, &boards);
        if count == 0 {
            return Ok((0, None));
        }
        // Invalidate the entire undo history — a data migration
        // mutates state outside the command pipeline, so any
        // inverse captured before the migration would now reference
        // stale entity values.
        self.undo_stack.clear();
        tracing::info!("Migrated sprint logs for {} card(s)", count);
        let mut written = Vec::new();
        for (card, before) in cards.into_iter().zip(before_logs) {
            if card.sprint_logs != before {
                written.push(card.id);
                self.backend.upsert_card(card)?;
            }
        }
        self.dirty = true;
        Ok((
            count,
            Some(Invalidation::Entities(EntityIds::cards(written))),
        ))
    }

    /// Reload state from durable storage, discarding any uncommitted
    /// data cache. Drops the per-session `UndoStack` (entity ids from
    /// before the reload may no longer exist). The audit log is left
    /// untouched — it records what happened, and a reload does not
    /// unhappen it.
    pub async fn reload(&mut self) -> KanbanResult<Invalidation> {
        self.backend.reload().await?;
        self.undo_stack.clear();
        self.dirty = false;
        Ok(Invalidation::All)
    }

    /// Persist any dirty state to durable storage.
    /// For SQLite this is a WAL checkpoint; for JSON this flushes the cache.
    pub async fn save(&self) -> KanbanResult<()> {
        self.backend.flush().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend_test_support::{MockBackend, MockRemoteWritesImpl};
    use kanban_core::AppConfig;
    use kanban_domain::{Board, Card, DataStore, KanbanError, Sprint};
    use std::sync::Arc;

    #[tokio::test]
    async fn test_migrate_sprint_logs_on_a_remote_backend_declines_before_any_read() {
        let backend = Arc::new(MockBackend::with_remote_writes(Arc::new(
            MockRemoteWritesImpl,
        )));

        let board = Board::new("Roadmap", None::<String>);
        backend.upsert_board(board.clone()).unwrap();
        let sprint = Sprint::new(board.id, 1, None, None::<String>);
        backend.upsert_sprint(sprint.clone()).unwrap();
        let mut card = Card::new(board.id, uuid::Uuid::new_v4(), "Do the thing", 0);
        card.sprint_id = Some(sprint.id);
        backend.upsert_card(card.clone()).unwrap();

        let mut ctx = KanbanContext::open(backend.clone(), AppConfig::default())
            .await
            .unwrap();

        let err = ctx.migrate_sprint_logs().unwrap_err();

        assert!(err.is_unsupported(), "got: {err:?}");
        assert_eq!(
            err.to_string(),
            KanbanError::unsupported("migrate_sprint_logs").to_string()
        );

        let readback = backend.get_card(card.id).unwrap().unwrap();
        assert_eq!(readback, card);
    }
}
