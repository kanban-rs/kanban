use super::KanbanContext;
use kanban_domain::KanbanResult;

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
    pub fn migrate_sprint_logs(&mut self) -> KanbanResult<usize> {
        // C3b FIDELITY: raw reads — sprint-log migration must touch ALL cards.
        let mut cards = self.backend.list_all_cards()?;
        let sprints = self.backend.list_all_sprints()?;
        let boards = self.backend.list_boards()?;
        let before_logs: Vec<_> = cards.iter().map(|c| c.sprint_logs.clone()).collect();
        let count =
            kanban_domain::card_lifecycle::migrate_sprint_logs(&mut cards, &sprints, &boards);
        if count > 0 {
            // Invalidate the entire undo history — a data migration
            // mutates state outside the command pipeline, so any
            // inverse captured before the migration would now reference
            // stale entity values.
            self.undo_stack.clear();
            tracing::info!("Migrated sprint logs for {} card(s)", count);
            for (card, before) in cards.into_iter().zip(before_logs) {
                if card.sprint_logs != before {
                    self.backend.upsert_card(card)?;
                }
            }
            self.dirty = true;
        }
        Ok(count)
    }

    /// Reload state from durable storage, discarding any uncommitted
    /// data cache. Drops the per-session `UndoStack` (entity ids from
    /// before the reload may no longer exist). The audit log is left
    /// untouched — it records what happened, and a reload does not
    /// unhappen it.
    pub async fn reload(&mut self) -> KanbanResult<()> {
        self.backend.reload().await?;
        self.undo_stack.clear();
        self.dirty = false;
        Ok(())
    }

    /// Persist any dirty state to durable storage.
    /// For SQLite this is a WAL checkpoint; for JSON this flushes the cache.
    pub async fn save(&self) -> KanbanResult<()> {
        self.backend.flush().await
    }
}
