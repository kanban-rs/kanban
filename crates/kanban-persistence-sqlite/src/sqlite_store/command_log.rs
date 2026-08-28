use chrono::Utc;
use kanban_domain::KanbanResult;
use sqlx::Row;

use super::helpers::{db_err, fmt_dt};
use super::SqliteStore;

// ── Command log (audit foundation; not yet wired through SqliteBackend) ──

impl SqliteStore {
    async fn append_command_batch_with_conn(
        conn: &mut sqlx::SqliteConnection,
        batch_index: u64,
        commands_json: &str,
    ) -> KanbanResult<()> {
        sqlx::query(
            "INSERT INTO command_log (batch_index, commands_json, created_at) VALUES (?, ?, ?)",
        )
        .bind(batch_index as i64)
        .bind(commands_json)
        .bind(fmt_dt(&Utc::now()))
        .execute(&mut *conn)
        .await
        .map_err(db_err)?;
        Ok(())
    }

    /// Append a single command batch at logical index `batch_index`.
    /// `commands_json` is the serde-JSON encoding of the `Vec<Command>` batch.
    pub async fn append_command_batch(
        &self,
        batch_index: u64,
        commands_json: &str,
    ) -> KanbanResult<()> {
        let commands_json = commands_json.to_string();
        self.db_conn(|conn| {
            Box::pin(async move {
                Self::append_command_batch_with_conn(conn, batch_index, &commands_json).await
            })
        })
        .await
    }

    async fn load_all_command_batches_with_conn(
        conn: &mut sqlx::SqliteConnection,
    ) -> KanbanResult<Vec<String>> {
        let rows = sqlx::query("SELECT commands_json FROM command_log ORDER BY batch_index ASC")
            .fetch_all(&mut *conn)
            .await
            .map_err(db_err)?;
        let mut out = Vec::with_capacity(rows.len());
        for row in &rows {
            out.push(row.try_get::<String, _>("commands_json").map_err(db_err)?);
        }
        Ok(out)
    }

    /// Load all persisted command batches in order. Returns the JSON strings
    /// so callers can deserialise inside the domain layer.
    pub async fn load_all_command_batches(&self) -> KanbanResult<Vec<String>> {
        self.db_conn(|conn| {
            Box::pin(async move { Self::load_all_command_batches_with_conn(conn).await })
        })
        .await
    }

    async fn truncate_command_log_after_with_conn(
        conn: &mut sqlx::SqliteConnection,
        after: u64,
    ) -> KanbanResult<()> {
        sqlx::query("DELETE FROM command_log WHERE batch_index >= ?")
            .bind(after as i64)
            .execute(&mut *conn)
            .await
            .map_err(db_err)?;
        Ok(())
    }

    /// Remove batches with logical index >= `after`. Retains [0, after).
    pub async fn truncate_command_log_after(&self, after: u64) -> KanbanResult<()> {
        self.db_conn(|conn| {
            Box::pin(async move { Self::truncate_command_log_after_with_conn(conn, after).await })
        })
        .await
    }

    async fn shift_command_log_with_conn(
        conn: &mut sqlx::SqliteConnection,
        drop_count: u64,
    ) -> KanbanResult<()> {
        if drop_count == 0 {
            return Ok(());
        }
        sqlx::query("DELETE FROM command_log WHERE batch_index < ?")
            .bind(drop_count as i64)
            .execute(&mut *conn)
            .await
            .map_err(db_err)?;
        sqlx::query("UPDATE command_log SET batch_index = batch_index - ?")
            .bind(drop_count as i64)
            .execute(&mut *conn)
            .await
            .map_err(db_err)?;
        Ok(())
    }

    /// Remove the oldest `drop_count` batches and renumber the rest so the
    /// surviving log starts at index 0. Runs both statements against
    /// whichever connection `db_conn` hands back — either the caller's
    /// ambient transaction or a fresh one it opens and commits itself — so
    /// this never opens a second, nested transaction.
    pub async fn shift_command_log(&self, drop_count: u64) -> KanbanResult<()> {
        self.db_conn(|conn| {
            Box::pin(async move { Self::shift_command_log_with_conn(conn, drop_count).await })
        })
        .await
    }
}
