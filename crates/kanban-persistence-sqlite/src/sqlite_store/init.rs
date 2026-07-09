use std::path::PathBuf;

use chrono::Utc;
use kanban_domain::{KanbanError, KanbanResult};
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
        //
        // KAN-845: this function's steps are idempotent presence-checks, not
        // gated by SUPPORTED_SCHEMA_VERSION themselves. Any step added here
        // that performs an irreversible/structural change must be paired
        // with bumping SUPPORTED_SCHEMA_VERSION (mod.rs) so
        // write_pre_migration_backup fires for it - see that constant's doc
        // comment.

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

    /// The durable pre-migration backup path for a given DB file and source
    /// schema version: `<db_path>.v<from_version>.backup`. Mirrors the JSON
    /// backend's `.v{N}.backup` convention
    /// (`kanban-persistence-json::migration::backup::pre_latest_backup_path_for`)
    /// so both backends' rollback artifacts are found the same way. Exposed
    /// `pub(crate)` so the test module computes the same path instead of
    /// duplicating the formula.
    pub(crate) fn backup_path_for(db_path: &std::path::Path, from_version: u32) -> PathBuf {
        let mut backup = db_path.as_os_str().to_owned();
        backup.push(format!(".v{from_version}.backup"));
        PathBuf::from(backup)
    }

    /// Process-unique scratch path `write_pre_migration_backup` writes to
    /// before renaming into place. `pub(crate)` for the same reason as
    /// [`Self::backup_path_for`].
    pub(crate) fn tmp_backup_path_for(backup: &std::path::Path) -> PathBuf {
        let mut tmp = backup.as_os_str().to_owned();
        tmp.push(format!(".tmp.{}", std::process::id()));
        PathBuf::from(tmp)
    }

    /// Durable, transactionally-consistent copy of the DB written to
    /// [`Self::backup_path_for`] before an IRREVERSIBLE schema upgrade, so a
    /// user can roll back after downgrading the binary. Kept on success
    /// (unlike the migration's own transaction, which only guards a
    /// mid-migration crash) — a deliberate divergence from the JSON
    /// backend's `.v{N}.backup`, which is removed once its migration step
    /// succeeds: JSON's backup exists only to survive a crash mid-*step*,
    /// while this one is the rollback artifact for the whole
    /// binary-downgrade window, so it must outlive a successful process
    /// exit. No-op if a backup already exists (a prior run's snapshot is
    /// still a valid pre-upgrade copy - never clobber it).
    ///
    /// Written atomically: `VACUUM INTO` targets a process-unique
    /// [`Self::tmp_backup_path_for`] scratch file first, then `rename`s it
    /// into place. A crash or ENOSPC mid-copy therefore never leaves a
    /// truncated file at the final `backup` path for the `backup.exists()`
    /// check above to mistake for a good backup — worst case it leaves an
    /// orphaned `.tmp.<pid>` file, which the next attempt clears before
    /// retrying.
    ///
    /// Uses `VACUUM INTO`: a single consistent file (no `-wal`/`-shm`
    /// sidecars, no manual checkpoint). NOTE: `VACUUM INTO` takes a string
    /// LITERAL, not a bind parameter, so the path is interpolated with single
    /// quotes escaped. It also cannot run inside a transaction and fails if
    /// the target file already exists, both satisfied here: `open()` has no
    /// transaction open at this point, and the scratch path is freshly
    /// cleared (if stale) before every attempt.
    ///
    /// Does not implement `kanban_persistence::MigrationStrategy` — that
    /// trait models a file-to-file transform (`detect_version` + `migrate`
    /// returning a new path) built for the JSON backend's
    /// rewrite-the-whole-file chain. SQLite's migrations are in-place
    /// ALTER/CREATE statements against a live connection pool, not file
    /// transforms, so this backend intentionally uses its own mechanism
    /// rather than force-fitting that trait's shape; unifying the two is a
    /// larger cross-crate design change, deferred rather than attempted here.
    ///
    /// No rotation/cleanup policy today: only one migration boundary (2->3)
    /// exists, so at most one backup file accumulates per database. Add a
    /// retention policy when a second irreversible migration is introduced.
    pub(crate) async fn write_pre_migration_backup(
        pool: &Pool<Sqlite>,
        db_path: &std::path::Path,
        from_version: u32,
    ) -> KanbanResult<()> {
        let backup = Self::backup_path_for(db_path, from_version);

        if backup.exists() {
            tracing::info!(
                path = %backup.display(),
                "pre-migration SQLite backup already present; keeping it"
            );
            return Ok(());
        }

        let tmp = Self::tmp_backup_path_for(&backup);
        // Clear a scratch file orphaned by a crashed prior attempt - VACUUM
        // INTO refuses to write over an existing file.
        let _ = std::fs::remove_file(&tmp);

        tracing::info!(
            path = %backup.display(),
            from_version,
            "writing durable pre-migration SQLite backup before irreversible schema upgrade \
             (full copy of the database; may take a while for large databases)"
        );

        let target = tmp.to_string_lossy().replace('\'', "''");
        if let Err(e) = sqlx::query(&format!("VACUUM INTO '{target}'"))
            .execute(pool)
            .await
        {
            let _ = std::fs::remove_file(&tmp);
            return Err(db_err(e));
        }

        match std::fs::rename(&tmp, &backup) {
            Ok(()) => {}
            Err(_) if backup.exists() => {
                // Lost a race with a concurrent writer (or the target
                // platform refuses to replace an existing destination, e.g.
                // Windows). Their copy is equally valid; drop ours.
                let _ = std::fs::remove_file(&tmp);
            }
            Err(e) => return Err(KanbanError::Database(e.to_string())),
        }

        tracing::warn!(
            path = %backup.display(),
            from_version,
            to_version = SUPPORTED_SCHEMA_VERSION,
            "wrote durable pre-migration SQLite backup before irreversible schema upgrade \
             (full copy of the database file)"
        );
        Ok(())
    }

    /// schema 2 -> 3 (KAN-832). Idempotent, additive-in-place (no `.backup`
    /// of its own - see [`Self::write_pre_migration_backup`], which the
    /// `open()` call site invokes before this runs). Two structural changes:
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

        // Count the rows whose `original_column_id` no longer resolves to a
        // column (the backfill subquery would yield NULL) so we can warn. This
        // is computed up front against the pre-migration table; the count is
        // identical to the post-backfill `board_id IS NULL` count.
        let unresolved: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM archived_cards
              WHERE (SELECT c.board_id FROM columns c
                       WHERE c.id = archived_cards.original_column_id) IS NULL",
        )
        .fetch_one(pool)
        .await
        .map_err(db_err)?;
        if unresolved > 0 {
            tracing::warn!(
                count = unresolved,
                "archived_cards board_id backfill: unresolvable original_column_id; \
                 setting board_id = nil"
            );
        }

        // Run the whole migration as one `raw_sql` batch on the pool executor.
        // `&Pool` acquires a SINGLE connection for the entire multi-statement
        // string and runs it in one await, so:
        //   * `PRAGMA foreign_keys = OFF` applies to the same connection that
        //     performs the `cards` table swap (a table-swap under FK ON fires
        //     ON DELETE CASCADE on sprint_logs/archived_cards and wipes them);
        //     the pragma is issued OUTSIDE the transaction (before BEGIN), where
        //     it is not a no-op, and restored afterwards.
        //   * the future stays `Send`-provable for the `tokio::spawn`ed
        //     store-open path. Holding an acquired `&mut SqliteConnection`
        //     executor across multiple awaits instead leaves the higher-ranked
        //     `for<'c> &'c mut SqliteConnection: Executor<'c>` / `Send` bound
        //     unresolved, and `tokio::spawn` fails with "implementation of
        //     `Executor`/`Send` is not general enough".
        // The unresolvable rows are set to `Uuid::nil()` via a SQL literal so
        // the batch needs no dynamic binds.
        sqlx::raw_sql(
            "PRAGMA foreign_keys = OFF;
            BEGIN;
            ALTER TABLE archived_cards ADD COLUMN board_id TEXT;
            UPDATE archived_cards
               SET board_id = (SELECT c.board_id FROM columns c
                                 WHERE c.id = archived_cards.original_column_id);
            UPDATE archived_cards
               SET board_id = '00000000-0000-0000-0000-000000000000'
             WHERE board_id IS NULL;
            CREATE TABLE cards_new (
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
            ALTER TABLE cards_new RENAME TO cards;
            COMMIT;
            PRAGMA foreign_keys = ON;",
        )
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
