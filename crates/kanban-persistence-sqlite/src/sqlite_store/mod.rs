use std::path::{Path, PathBuf};

use kanban_domain::{KanbanError, KanbanResult};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Pool, Sqlite};
use uuid::Uuid;

mod command_log;
mod conversions;
mod data_store;
mod entities;
mod graph;
mod helpers;
mod init;
mod lists;
mod metadata;
mod persistence_store;
mod snapshot;

#[cfg(test)]
mod tests;

const SCHEMA: &str = include_str!("../schema.sql");

/// The highest schema_version this binary understands. Used both to
/// stamp fresh databases and to refuse files written by a future binary.
///
/// Also gates [`SqliteStore::write_pre_migration_backup`] (see `open()`
/// below): the backup only fires when the on-disk `schema_version` is
/// LOWER than this constant. Any future irreversible/structural change
/// added to `init::migrate` or a sibling `migrate_*` function MUST be
/// paired with bumping this constant, or it will run unbacked-up — the two
/// are intentionally coupled but not enforced by the type system.
pub const SUPPORTED_SCHEMA_VERSION: u32 = 6;

/// (instance_id, saved_at, writer_version, writer_commit, schema_version).
/// Tuple shape returned by the metadata-singleton SELECT — extracted to a
/// type alias to keep clippy's type-complexity lint happy.
type MetadataRow = (String, String, Option<String>, Option<String>, u32);

/// SQLite-backed persistence store using sqlx connection pool.
pub struct SqliteStore {
    pub(crate) pool: Pool<Sqlite>,
    pub(crate) path: PathBuf,
    pub(crate) instance_id: Uuid,
}

impl SqliteStore {
    pub async fn open(path: impl AsRef<Path>) -> KanbanResult<Self> {
        let handle = tokio::runtime::Handle::current();
        if handle.runtime_flavor() != tokio::runtime::RuntimeFlavor::MultiThread {
            return Err(KanbanError::Database(
                "SqliteStore requires a multi-threaded Tokio runtime (e.g. #[tokio::main] or \
                 tokio::runtime::Runtime::new()). The current_thread runtime is not supported \
                 because synchronous DataStore methods need to block on async SQLite I/O."
                    .to_string(),
            ));
        }

        let path_buf = path.as_ref().to_path_buf();

        let options = SqliteConnectOptions::new()
            .filename(&path_buf)
            .create_if_missing(true)
            .foreign_keys(true)
            .pragma("journal_mode", "wal");

        let pool = SqlitePoolOptions::new()
            .max_connections(2)
            .connect_with(options)
            .await
            .map_err(|e| KanbanError::Database(e.to_string()))?;

        // KAN-522: refuse a future-version DB BEFORE any schema-modifying
        // step runs. Otherwise the legacy-table drops, the SCHEMA's
        // CREATE TABLE IF NOT EXISTS for tables the file lacks, and
        // migrate()'s ALTERs would all mutate a file we're about to refuse.
        //
        // Why two round-trips rather than one correlated subquery: SQLite
        // parses the inner SELECT eagerly at prepare time and errors with
        // "no such table: metadata" on a fresh DB, even when an outer
        // `WHERE EXISTS` would short-circuit it at runtime. So we split:
        // first probe `sqlite_master`, then read `schema_version` only if
        // the table is there. ~µs overhead, executes once per `open()`.
        let metadata_table_exists: bool = sqlx::query_scalar(
            "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='metadata'",
        )
        .fetch_one(&pool)
        .await
        .map_err(|e| KanbanError::Database(e.to_string()))?;
        if metadata_table_exists {
            let pre_migrate_version: Option<u32> =
                sqlx::query_scalar("SELECT schema_version FROM metadata WHERE id = 1")
                    .fetch_optional(&pool)
                    .await
                    .map_err(|e| KanbanError::Database(e.to_string()))?;
            if let Some(v) = pre_migrate_version {
                if v > SUPPORTED_SCHEMA_VERSION {
                    return Err(KanbanError::UnsupportedFutureVersion {
                        file_version: v,
                        binary_max: SUPPORTED_SCHEMA_VERSION,
                    });
                }
                // KAN-845: snapshot the DB before any irreversible schema
                // upgrade so a downgraded binary can restore it. Aborts
                // open() on failure rather than risk an unbacked-up upgrade.
                if v < SUPPORTED_SCHEMA_VERSION {
                    Self::write_pre_migration_backup(&pool, &path_buf, v).await?;
                }
            }
        }

        // Drop the legacy command_log table (pre-KAN-405 schema with
        // columns `idx` / `cmd_json`) before SCHEMA runs, so the new
        // command_log schema (`batch_index` / `commands_json` / `created_at`)
        // can be created cleanly. Detected by absence of the new
        // `batch_index` column on an existing command_log table.
        Self::drop_legacy_command_log_if_present(&pool).await?;

        // schema 2 -> 3: add archived_cards.board_id (+ backfill) and break the
        // cards -> columns delete cascade so archived cards survive column
        // deletion. Must run BEFORE SCHEMA: SCHEMA declares
        // idx_archived_cards_board_id, which fails against the old-shape table.
        Self::migrate_v2_to_v3_archived_cards(&pool).await?;

        // schema 4 -> 5: add cards.board_id (+ backfill). Must also run BEFORE
        // SCHEMA: SCHEMA declares idx_cards_board_id, which fails against the
        // old-shape table.
        Self::migrate_v4_to_v5_cards_board_id(&pool).await?;

        sqlx::raw_sql(SCHEMA)
            .execute(&pool)
            .await
            .map_err(|e| KanbanError::Database(e.to_string()))?;

        Self::migrate(&pool).await?;

        let instance_id = Self::load_or_create_instance_id(&pool).await?;

        Ok(Self {
            pool,
            path: path_buf,
            instance_id,
        })
    }

    pub fn pool(&self) -> &Pool<Sqlite> {
        &self.pool
    }
}
