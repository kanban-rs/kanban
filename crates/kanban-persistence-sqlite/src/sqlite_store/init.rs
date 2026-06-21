use chrono::Utc;
use kanban_domain::KanbanResult;
use sqlx::{Pool, Sqlite};
use uuid::Uuid;

use super::helpers::{db_err, p_uuid};
use super::{SqliteStore, SUPPORTED_SCHEMA_VERSION};

impl SqliteStore {
    pub(crate) async fn load_or_create_instance_id(pool: &Pool<Sqlite>) -> KanbanResult<Uuid> {
        let row: Option<String> =
            sqlx::query_scalar("SELECT instance_id FROM metadata WHERE id = 1")
                .fetch_optional(pool)
                .await
                .map_err(db_err)?;
        match row {
            Some(s) => p_uuid(&s),
            None => {
                let id = Uuid::new_v4();
                let now = Utc::now().to_rfc3339();
                sqlx::query(
                    "INSERT INTO metadata (id, instance_id, saved_at, schema_version) VALUES (1, ?, ?, ?)",
                )
                .bind(id.to_string())
                .bind(&now)
                .bind(SUPPORTED_SCHEMA_VERSION)
                .execute(pool)
                .await
                .map_err(db_err)?;
                Ok(id)
            }
        }
    }

    /// Drops the legacy command_log table if it exists and lacks the new
    /// `batch_index` column. Called before SCHEMA so the new table can be
    /// created cleanly via `CREATE TABLE IF NOT EXISTS`.
    pub(crate) async fn drop_legacy_command_log_if_present(
        pool: &Pool<Sqlite>,
    ) -> KanbanResult<()> {
        let has_command_log: bool = sqlx::query_scalar(
            "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='command_log'",
        )
        .fetch_one(pool)
        .await
        .map_err(db_err)?;
        if !has_command_log {
            return Ok(());
        }
        let has_batch_index_col: bool = sqlx::query_scalar(
            "SELECT COUNT(*) > 0 FROM pragma_table_info('command_log') WHERE name = 'batch_index'",
        )
        .fetch_one(pool)
        .await
        .map_err(db_err)?;
        if !has_batch_index_col {
            tracing::info!(
                "dropping legacy command_log table (pre-KAN-405 schema) so the KAN-191 schema can be applied"
            );
            sqlx::raw_sql("DROP TABLE IF EXISTS command_log")
                .execute(pool)
                .await
                .map_err(db_err)?;
        }
        Ok(())
    }

    pub(crate) async fn migrate(pool: &Pool<Sqlite>) -> KanbanResult<()> {
        // KAN-191 reintroduces command_log persistence (KAN-405 had dropped it).
        // The dense batch_index → JSON mapping is created by SCHEMA at open
        // time; no migration of the legacy column-set is needed because the
        // schema is owned by this crate.

        let has_undo_state: bool = sqlx::query_scalar(
            "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='undo_state'",
        )
        .fetch_one(pool)
        .await
        .map_err(db_err)?;

        if has_undo_state {
            tracing::info!(
                "dropping legacy undo_state table: undo cursor stays in-session, only command_log persists"
            );
            sqlx::raw_sql("DROP TABLE IF EXISTS undo_state")
                .execute(pool)
                .await
                .map_err(db_err)?;
        }

        let has_position_col: bool = sqlx::query_scalar(
            "SELECT COUNT(*) > 0 FROM pragma_table_info('boards') WHERE name = 'position'",
        )
        .fetch_one(pool)
        .await
        .map_err(db_err)?;

        if !has_position_col {
            sqlx::raw_sql("ALTER TABLE boards ADD COLUMN position INTEGER NOT NULL DEFAULT 0")
                .execute(pool)
                .await
                .map_err(db_err)?;
        }

        let has_card_counter_col: bool = sqlx::query_scalar(
            "SELECT COUNT(*) > 0 FROM pragma_table_info('boards') WHERE name = 'card_counter'",
        )
        .fetch_one(pool)
        .await
        .map_err(db_err)?;

        if !has_card_counter_col {
            sqlx::raw_sql("ALTER TABLE boards ADD COLUMN card_counter INTEGER NOT NULL DEFAULT 1")
                .execute(pool)
                .await
                .map_err(db_err)?;
        }

        Self::drop_legacy_card_edges_if_present(pool).await?;

        // KAN-522: ALTER in writer-stamp columns on pre-v2 metadata tables.
        for col in ["writer_version", "writer_commit"] {
            let has_col: bool = sqlx::query_scalar(&format!(
                "SELECT COUNT(*) > 0 FROM pragma_table_info('metadata') WHERE name = '{col}'"
            ))
            .fetch_one(pool)
            .await
            .map_err(db_err)?;
            if !has_col {
                sqlx::raw_sql(&format!("ALTER TABLE metadata ADD COLUMN {col} TEXT"))
                    .execute(pool)
                    .await
                    .map_err(db_err)?;
            }
        }
        // Once the ALTERs above have caught the schema up, normalise
        // schema_version. Doing it unconditionally is idempotent and
        // also self-heals any DBs where the field drifted.
        sqlx::query("UPDATE metadata SET schema_version = ? WHERE id = 1 AND schema_version < ?")
            .bind(SUPPORTED_SCHEMA_VERSION)
            .bind(SUPPORTED_SCHEMA_VERSION)
            .execute(pool)
            .await
            .map_err(db_err)?;

        Ok(())
    }

    /// Drop the pre-KAN-504 `card_edges` table (single table with an
    /// `edge_type` column) if present. The per-kind `spawns_edges` /
    /// `blocks_edges` / `relates_edges` tables created by SCHEMA
    /// replace it; nothing of KAN-504's graph work is live so we
    /// don't need to copy data forward — any rows in the legacy
    /// table belong to a development-only database.
    pub(crate) async fn drop_legacy_card_edges_if_present(pool: &Pool<Sqlite>) -> KanbanResult<()> {
        let has_card_edges: bool = sqlx::query_scalar(
            "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='card_edges'",
        )
        .fetch_one(pool)
        .await
        .map_err(db_err)?;
        if !has_card_edges {
            return Ok(());
        }
        tracing::info!(
            "dropping legacy card_edges table (pre per-kind schema); per-kind tables take over"
        );
        sqlx::raw_sql(
            "DROP INDEX IF EXISTS idx_card_edges_source;
             DROP INDEX IF EXISTS idx_card_edges_target;
             DROP TABLE IF EXISTS card_edges;",
        )
        .execute(pool)
        .await
        .map_err(db_err)?;
        Ok(())
    }
}
