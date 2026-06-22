use chrono::Utc;
use kanban_domain::KanbanResult;
use sqlx::Row;

use super::helpers::{db_err, fmt_dt};
use super::SqliteStore;

// ── Command log (audit foundation; not yet wired through SqliteBackend) ──

impl SqliteStore {
    /// Append a single command batch at logical index `batch_index`.
    /// `commands_json` is the serde-JSON encoding of the `Vec<Command>` batch.
    pub async fn append_command_batch(
        &self,
        batch_index: u64,
        commands_json: &str,
    ) -> KanbanResult<()> {
        sqlx::query(
            "INSERT INTO command_log (batch_index, commands_json, created_at) VALUES (?, ?, ?)",
        )
        .bind(batch_index as i64)
        .bind(commands_json)
        .bind(fmt_dt(&Utc::now()))
        .execute(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(())
    }

    /// Load all persisted command batches in order. Returns the JSON strings
    /// so callers can deserialise inside the domain layer.
    pub async fn load_all_command_batches(&self) -> KanbanResult<Vec<String>> {
        let rows = sqlx::query("SELECT commands_json FROM command_log ORDER BY batch_index ASC")
            .fetch_all(&self.pool)
            .await
            .map_err(db_err)?;
        let mut out = Vec::with_capacity(rows.len());
        for row in &rows {
            out.push(row.try_get::<String, _>("commands_json").map_err(db_err)?);
        }
        Ok(out)
    }

    /// Remove batches with logical index >= `after`. Retains [0, after).
    pub async fn truncate_command_log_after(&self, after: u64) -> KanbanResult<()> {
        sqlx::query("DELETE FROM command_log WHERE batch_index >= ?")
            .bind(after as i64)
            .execute(&self.pool)
            .await
            .map_err(db_err)?;
        Ok(())
    }

    /// Remove the oldest `drop_count` batches and renumber the rest so the
    /// surviving log starts at index 0.
    pub async fn shift_command_log(&self, drop_count: u64) -> KanbanResult<()> {
        if drop_count == 0 {
            return Ok(());
        }
        let mut tx = self.pool.begin().await.map_err(db_err)?;
        sqlx::query("DELETE FROM command_log WHERE batch_index < ?")
            .bind(drop_count as i64)
            .execute(&mut *tx)
            .await
            .map_err(db_err)?;
        sqlx::query("UPDATE command_log SET batch_index = batch_index - ?")
            .bind(drop_count as i64)
            .execute(&mut *tx)
            .await
            .map_err(db_err)?;
        tx.commit().await.map_err(db_err)?;
        Ok(())
    }
}
