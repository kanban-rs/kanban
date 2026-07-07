use chrono::Utc;
use kanban_domain::KanbanResult;
use sqlx::{Acquire, Pool, Sqlite};
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

    /// schema 2 -> 3 (KAN-832). Idempotent, additive-in-place (no `.backup`,
    /// SQLite has no backup mechanism). Two structural changes:
    ///   1. `archived_cards` gains `board_id`, backfilled from the archived
    ///      card's `original_column_id -> columns.board_id`. A row whose
    ///      original column no longer exists gets `Uuid::nil()` + a warning.
    ///   2. `cards` is table-swapped to DROP the `column_id -> columns` FK, so
    ///      deleting a column no longer cascade-deletes an archived card's row.
    ///
    /// Must run BEFORE SCHEMA (SCHEMA's `idx_archived_cards_board_id` fails
    /// against the pre-3 `board_id`-less table).
    ///
    /// FK note (spike-validated): `DROP TABLE cards` with `foreign_keys` ON
    /// fires `ON DELETE CASCADE` on `sprint_logs`/`archived_cards` and WIPES
    /// them. `PRAGMA defer_foreign_keys` defers CHECKING, not cascade ACTIONS,
    /// so it does NOT help. We disable enforcement with `PRAGMA foreign_keys =
    /// OFF` on a dedicated connection OUTSIDE any transaction (the pragma is a
    /// no-op inside a tx), then restore it.
    pub(crate) async fn migrate_v2_to_v3_archived_cards(pool: &Pool<Sqlite>) -> KanbanResult<()> {
        // Fresh DB: archived_cards does not exist yet (SCHEMA creates it in the
        // correct v3 shape). Nothing to migrate.
        let has_archived_cards: bool = sqlx::query_scalar(
            "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='archived_cards'",
        )
        .fetch_one(pool)
        .await
        .map_err(db_err)?;
        if !has_archived_cards {
            return Ok(());
        }
        // Idempotent: already migrated if board_id is present.
        let has_board_id: bool = sqlx::query_scalar(
            "SELECT COUNT(*) > 0 FROM pragma_table_info('archived_cards') WHERE name = 'board_id'",
        )
        .fetch_one(pool)
        .await
        .map_err(db_err)?;
        if has_board_id {
            return Ok(());
        }

        tracing::info!(
            "migrating SQLite schema 2 -> 3: archived_cards.board_id + cards FK decouple"
        );

        let mut conn = pool.acquire().await.map_err(db_err)?;
        sqlx::query("PRAGMA foreign_keys = OFF")
            .execute(&mut *conn)
            .await
            .map_err(db_err)?;

        let mut tx = conn.begin().await.map_err(db_err)?;

        // 1a. Add board_id (nullable ADD; backfilled next).
        sqlx::query("ALTER TABLE archived_cards ADD COLUMN board_id TEXT")
            .execute(&mut *tx)
            .await
            .map_err(db_err)?;
        // 1b. Backfill from original_column_id -> columns.board_id.
        sqlx::query(
            "UPDATE archived_cards
                SET board_id = (SELECT c.board_id FROM columns c
                                WHERE c.id = archived_cards.original_column_id)",
        )
        .execute(&mut *tx)
        .await
        .map_err(db_err)?;
        // 1c. Rows whose original column is gone: nil + warn (matches D2's "may
        // dangle").
        let unresolved: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM archived_cards WHERE board_id IS NULL")
                .fetch_one(&mut *tx)
                .await
                .map_err(db_err)?;
        if unresolved > 0 {
            tracing::warn!(
                count = unresolved,
                "archived_cards board_id backfill: unresolvable original_column_id; \
                 setting board_id = nil"
            );
            sqlx::query("UPDATE archived_cards SET board_id = ? WHERE board_id IS NULL")
                .bind(uuid::Uuid::nil().to_string())
                .execute(&mut *tx)
                .await
                .map_err(db_err)?;
        }

        // 2. Table-swap `cards` to drop the column_id -> columns FK. Indexes are
        //    recreated by SCHEMA (CREATE INDEX IF NOT EXISTS) after this runs.
        sqlx::raw_sql(
            "CREATE TABLE cards_new (
                id TEXT PRIMARY KEY,
                column_id TEXT NOT NULL,
                title TEXT NOT NULL,
                description TEXT,
                priority TEXT NOT NULL DEFAULT 'Medium',
                status TEXT NOT NULL DEFAULT 'Todo',
                position INTEGER NOT NULL,
                due_date TEXT,
                points INTEGER CHECK (points >= 0 AND points <= 255),
                card_number INTEGER NOT NULL DEFAULT 0,
                sprint_id TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                completed_at TEXT,
                FOREIGN KEY (sprint_id) REFERENCES sprints(id) ON DELETE SET NULL
            );
            INSERT INTO cards_new SELECT
                id, column_id, title, description, priority, status, position,
                due_date, points, card_number, sprint_id, created_at, updated_at,
                completed_at
              FROM cards;
            DROP TABLE cards;
            ALTER TABLE cards_new RENAME TO cards;",
        )
        .execute(&mut *tx)
        .await
        .map_err(db_err)?;

        tx.commit().await.map_err(db_err)?;

        sqlx::query("PRAGMA foreign_keys = ON")
            .execute(&mut *conn)
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
