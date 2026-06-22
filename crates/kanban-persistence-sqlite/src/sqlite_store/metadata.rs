use chrono::{DateTime, Utc};
use kanban_domain::{KanbanError, KanbanResult};
use kanban_persistence::PersistenceMetadata;

use super::helpers::{db_err, p_dt, p_uuid, run};
use super::{MetadataRow, SqliteStore};

impl SqliteStore {
    /// Read the metadata singleton row from the DB. Cheap (single row, indexed by primary key).
    /// Returns `Ok(None)` if the row is absent — only possible on a brand-new DB
    /// before `load_or_create_instance_id` has run, which the public API doesn't expose.
    pub fn read_metadata_sync(&self) -> KanbanResult<Option<PersistenceMetadata>> {
        run(async {
            let row: Option<MetadataRow> = sqlx::query_as(
                "SELECT instance_id, saved_at, writer_version, writer_commit, schema_version \
                 FROM metadata WHERE id = 1",
            )
            .fetch_optional(&self.pool)
            .await
            .map_err(db_err)?;
            let Some(row) = row else {
                return Ok(None);
            };
            let instance_id = p_uuid(&row.0)?;
            let saved_at = p_dt(&row.1)?;
            Ok(Some(PersistenceMetadata {
                instance_id,
                saved_at,
                writer_version: row.2,
                writer_commit: row.3,
                format_version: Some(row.4),
            }))
        })
    }

    /// Record the current binary as the most-recent writer of this DB by
    /// stamping `saved_at`, `writer_version`, and `writer_commit` into the
    /// metadata singleton row. Returns the timestamp it wrote so callers can
    /// echo it back into a `PersistenceMetadata` without re-reading the row.
    ///
    /// Separated from [`checkpoint`] so each function does one thing — see
    /// the post-PR-288 review for the SRP rationale.
    pub async fn stamp_writer(&self) -> KanbanResult<DateTime<Utc>> {
        let now = Utc::now();
        sqlx::query(
            "UPDATE metadata SET saved_at = ?, writer_version = ?, writer_commit = ? WHERE id = 1",
        )
        .bind(now.to_rfc3339())
        .bind(kanban_core::KANBAN_VERSION)
        .bind(kanban_core::KANBAN_COMMIT)
        .execute(&self.pool)
        .await
        .map_err(|e| KanbanError::Database(e.to_string()))?;
        Ok(now)
    }

    /// Truncate the WAL. Pure I/O step; does not touch the writer-stamp
    /// columns. Callers that want a durable save with attribution should
    /// invoke [`stamp_writer`] alongside this.
    pub async fn checkpoint(&self) -> KanbanResult<()> {
        sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)")
            .execute(&self.pool)
            .await
            .map_err(|e| KanbanError::Database(e.to_string()))?;
        Ok(())
    }
}
