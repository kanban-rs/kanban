use std::path::PathBuf;

use chrono::Utc;
use kanban_domain::{
    plan_prefix_backfill, resolve_card_prefix_by_ids, BackfillBoard, BackfillSprint, KanbanError,
    KanbanResult, DEFAULT_CARD_PREFIX, DEFAULT_SPRINT_PREFIX,
};
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
        Self::migrate_v5_to_v6_completion_columns(pool).await?;
        Self::migrate_v6_to_v7_column_default_status(pool).await?;
        Self::migrate_v7_to_v8_default_status_derivation(pool).await?;
        Self::migrate_v8_to_v9_drop_completion_columns(pool).await?;
        Self::migrate_v9_to_v10_prefixes(pool).await?;

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
    /// schema version: `<db_path>.v<from_version>.backup`. The JSON
    /// backend's analogous `.v{N}.backup` convention
    /// (`kanban-persistence-json::migration::backup::pre_latest_backup_path_for`)
    /// uses `Path::with_extension`, which REPLACES an existing extension
    /// (`board.json` -> `board.v2.backup`); this uses `OsString::push`,
    /// which APPENDS after it (`kanban.db` -> `kanban.db.v2.backup`), so the
    /// two backends' artifacts share a suffix format but not identical
    /// naming mechanics. Exposed `pub(crate)` so the test module computes
    /// the same path instead of duplicating the formula.
    pub(crate) fn backup_path_for(db_path: &std::path::Path, from_version: u32) -> PathBuf {
        let mut backup = db_path.as_os_str().to_owned();
        backup.push(format!(".v{from_version}.backup"));
        PathBuf::from(backup)
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
    /// Written atomically and genuinely no-clobber: `VACUUM INTO` targets a
    /// fresh [`tempfile::NamedTempFile`] scratch path (cryptographically
    /// unique per call, not derived from the PID - two concurrent callers,
    /// even within the same process, can never collide on it), then
    /// [`std::fs::hard_link`] installs it at `backup`. Unlike `rename(2)`,
    /// which on POSIX silently REPLACES an existing destination and returns
    /// `Ok`, `hard_link` atomically fails with `AlreadyExists` if `backup`
    /// already exists - so a genuine concurrent-writer race is resolved by
    /// dropping our copy, never by silently overwriting a valid backup. A
    /// crash or ENOSPC mid-copy never leaves a truncated file at the final
    /// `backup` path for the `backup.exists()` check above to mistake for a
    /// good backup - worst case it leaves an orphaned scratch file in the
    /// same directory. Because its name is single-use-random rather than
    /// derived from `backup`'s path, nothing in this codebase can recognize
    /// and clean it up after a hard crash (no `Drop` runs); this is a
    /// bounded, rare disk-space leak, not a correctness issue - the design
    /// deliberately trades "guessable stale-tmp cleanup" (which the
    /// PID-based scheme this replaces only pretended to provide - it could
    /// only ever clean up its own process's leftovers) for "no
    /// same-path collision risk", which matters more here.
    ///
    /// Uses `VACUUM INTO`: a single consistent file (no `-wal`/`-shm`
    /// sidecars, no manual checkpoint). NOTE: `VACUUM INTO` takes a string
    /// LITERAL, not a bind parameter, so the path is interpolated with
    /// single quotes escaped; a scratch path containing non-UTF8 bytes is
    /// rejected outright with a clear error rather than silently mangled
    /// via a lossy conversion, since a mangled literal would make `VACUUM
    /// INTO` write somewhere other than where this function expects to find
    /// its own output. It also cannot run inside a transaction and fails if
    /// the target file already exists, both satisfied here: `open()` has no
    /// transaction open at this point, and the scratch file is a brand-new
    /// [`tempfile::NamedTempFile`] whose placeholder is removed immediately
    /// before `VACUUM INTO` claims the now-free name.
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
    ///
    /// Runs synchronously on the `open()` call path: for a large database
    /// this blocks startup for the duration of the full-file copy and
    /// requires roughly 2x the database's disk space with no upfront
    /// space check. Acceptable for now (this only fires on the rare
    /// irreversible-migration boundary, not on every startup) but a known,
    /// deliberately deferred limitation - a progress indicator and
    /// disk-space precheck belong at the CLI/TUI/MCP call sites that have a
    /// UI to show one, not in this persistence-layer function.
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

        let parent = db_path
            .parent()
            .map(std::path::Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));
        let tmp = tokio::task::spawn_blocking(move || -> KanbanResult<PathBuf> {
            let named = tempfile::NamedTempFile::new_in(&parent).map_err(KanbanError::Io)?;
            let tmp_path = named.path().to_path_buf();
            // VACUUM INTO refuses to write into a file that already exists,
            // so free the just-reserved unique name for it to claim. The
            // window between this and VACUUM INTO writing to `tmp_path` is
            // not exploitable in practice: the name carries enough entropy
            // that another process guessing it is astronomically unlikely,
            // and even a collision here only fails this attempt's `VACUUM
            // INTO`, never corrupts `backup` itself.
            drop(named);
            Ok(tmp_path)
        })
        .await
        .map_err(|e| KanbanError::Database(e.to_string()))??;

        tracing::info!(
            path = %backup.display(),
            from_version,
            "writing durable pre-migration SQLite backup before irreversible schema upgrade \
             (full copy of the database; may take a while for large databases)"
        );

        let target = match tmp.to_str() {
            Some(s) => s.replace('\'', "''"),
            None => {
                return Err(KanbanError::Database(format!(
                    "pre-migration SQLite backup scratch path is not valid UTF-8: {}",
                    tmp.display()
                )));
            }
        };
        if let Err(e) = sqlx::query(&format!("VACUUM INTO '{target}'"))
            .execute(pool)
            .await
        {
            let _ = tokio::fs::remove_file(&tmp).await;
            return Err(db_err(e));
        }

        match tokio::fs::hard_link(&tmp, &backup).await {
            Ok(()) => {
                let _ = tokio::fs::remove_file(&tmp).await;
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                // Lost a race with a concurrent writer: `backup` was
                // created between our check above and this hard_link.
                // Their copy is equally valid; drop ours.
                let _ = tokio::fs::remove_file(&tmp).await;
            }
            Err(e) => {
                let _ = tokio::fs::remove_file(&tmp).await;
                return Err(KanbanError::Io(e));
            }
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

    /// Schema 4 -> 5 (KAN-963): add `cards.board_id`, a durable board reference
    /// independent of `column_id` (same rationale as `archived_cards.board_id`
    /// above — a card's column can be legitimately deleted once the card is
    /// archived, and board resolution must survive that). Unlike the 2->3
    /// migration, `cards` needs no table rebuild here: `column_id` already
    /// carries no FK, so a plain `ALTER TABLE ADD COLUMN` + backfill suffices.
    pub(crate) async fn migrate_v4_to_v5_cards_board_id(pool: &Pool<Sqlite>) -> KanbanResult<()> {
        // Fresh DB: SCHEMA creates `cards` in the correct v5 shape. Nothing to
        // migrate.
        let has_cards: bool = sqlx::query_scalar(
            "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='cards'",
        )
        .fetch_one(pool)
        .await
        .map_err(db_err)?;
        if !has_cards {
            return Ok(());
        }
        // Idempotent: already migrated if board_id is present.
        let has_board_id: bool = sqlx::query_scalar(
            "SELECT COUNT(*) > 0 FROM pragma_table_info('cards') WHERE name = 'board_id'",
        )
        .fetch_one(pool)
        .await
        .map_err(db_err)?;
        if has_board_id {
            return Ok(());
        }

        tracing::info!("migrating SQLite schema 4 -> 5: cards.board_id + backfill");

        let unresolved: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM cards
              WHERE (SELECT c.board_id FROM columns c
                       WHERE c.id = cards.column_id) IS NULL",
        )
        .fetch_one(pool)
        .await
        .map_err(db_err)?;
        if unresolved > 0 {
            tracing::warn!(
                count = unresolved,
                "cards board_id backfill: unresolvable column_id; setting board_id = nil"
            );
        }

        sqlx::raw_sql(
            "BEGIN;
            ALTER TABLE cards ADD COLUMN board_id TEXT;
            UPDATE cards
               SET board_id = (SELECT c.board_id FROM columns c
                                 WHERE c.id = cards.column_id);
            UPDATE cards
               SET board_id = '00000000-0000-0000-0000-000000000000'
             WHERE board_id IS NULL;
            COMMIT;",
        )
        .execute(pool)
        .await
        .map_err(db_err)?;
        Ok(())
    }

    /// Schema 5 -> 6: replace `boards.completion_column_id` with the ordered
    /// `board_completion_columns` join table. Runs inside `migrate()`, i.e.
    /// AFTER SCHEMA (which already created the empty join table) and after the
    /// `boards` ALTER catch-ups above. Idempotence gate: presence of the
    /// legacy column, which only a pre-6 database still has.
    ///
    /// Backfill, per board with at least one column: the legacy id when it
    /// names a live column of that board, otherwise the last column by
    /// (position, created_at, id) — the same deterministic ordering
    /// `sorted_board_columns` uses, replacing the storage-order tie-break of
    /// the old runtime fallback. The ORDER BY compares TEXT where the domain
    /// compares `DateTime`/`Uuid`; the orders coincide because this backend
    /// writes uniform RFC 3339 timestamps and canonical lowercase-hex UUIDs,
    /// whose lexicographic order equals the chronological/byte order.
    ///
    /// The legacy column is named in the boards table's own FOREIGN KEY
    /// clause, so `ALTER TABLE DROP COLUMN` refuses it; the table is rebuilt
    /// instead (same swap pattern as the 2->3 cards rebuild). `PRAGMA
    /// foreign_keys = OFF` outside the transaction is load-bearing: dropping
    /// `boards` with enforcement on would fire ON DELETE CASCADE on every
    /// child table and wipe columns/cards/sprints.
    pub(crate) async fn migrate_v5_to_v6_completion_columns(
        pool: &Pool<Sqlite>,
    ) -> KanbanResult<()> {
        let has_legacy_col: bool = sqlx::query_scalar(
            "SELECT COUNT(*) > 0 FROM pragma_table_info('boards') WHERE name = 'completion_column_id'",
        )
        .fetch_one(pool)
        .await
        .map_err(db_err)?;
        if !has_legacy_col {
            return Ok(());
        }

        tracing::info!(
            "migrating SQLite schema 5 -> 6: board_completion_columns join table + backfill"
        );

        sqlx::raw_sql(
            "PRAGMA foreign_keys = OFF;
            BEGIN;
            CREATE TABLE IF NOT EXISTS board_completion_columns (
                board_id  TEXT NOT NULL REFERENCES boards(id)  ON DELETE CASCADE DEFERRABLE INITIALLY DEFERRED,
                column_id TEXT NOT NULL REFERENCES columns(id) ON DELETE CASCADE DEFERRABLE INITIALLY DEFERRED,
                position  INTEGER NOT NULL,
                PRIMARY KEY (board_id, column_id)
            );
            INSERT INTO board_completion_columns (board_id, column_id, position)
            SELECT board_id, target, 0 FROM (
                SELECT b.id AS board_id,
                       COALESCE(
                           (SELECT c.id FROM columns c
                             WHERE c.id = b.completion_column_id AND c.board_id = b.id),
                           (SELECT c.id FROM columns c
                             WHERE c.board_id = b.id
                             ORDER BY c.position DESC, c.created_at DESC, c.id DESC
                             LIMIT 1)
                       ) AS target
                  FROM boards b
            ) WHERE target IS NOT NULL;
            CREATE TABLE boards_new (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                description TEXT,
                sprint_prefix TEXT,
                card_prefix TEXT,
                task_sort_field TEXT NOT NULL DEFAULT 'Default',
                task_sort_order TEXT NOT NULL DEFAULT 'Ascending',
                sprint_duration_days INTEGER,
                sprint_name_used_count INTEGER NOT NULL DEFAULT 0,
                next_sprint_number INTEGER NOT NULL DEFAULT 1,
                active_sprint_id TEXT,
                task_list_view TEXT NOT NULL DEFAULT 'Flat',
                card_counter INTEGER NOT NULL DEFAULT 1,
                position INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                FOREIGN KEY (active_sprint_id) REFERENCES sprints(id) ON DELETE SET NULL DEFERRABLE INITIALLY DEFERRED
            );
            INSERT INTO boards_new SELECT
                id, name, description, sprint_prefix, card_prefix,
                task_sort_field, task_sort_order, sprint_duration_days,
                sprint_name_used_count, next_sprint_number, active_sprint_id,
                task_list_view, card_counter, position, created_at, updated_at
              FROM boards;
            DROP TABLE boards;
            ALTER TABLE boards_new RENAME TO boards;
            COMMIT;
            PRAGMA foreign_keys = ON;",
        )
        .execute(pool)
        .await
        .map_err(db_err)?;
        Ok(())
    }

    /// Schema 6 -> 7: add `columns.default_status`, nullable, backfilled `NULL`
    /// for every existing column regardless of name. No table rebuild is
    /// needed here (unlike the 5->6 migration): the column carries no FK and
    /// `ALTER TABLE ADD COLUMN` is sufficient. Idempotence gate: presence of
    /// the column, which only a pre-7 database lacks.
    pub(crate) async fn migrate_v6_to_v7_column_default_status(
        pool: &Pool<Sqlite>,
    ) -> KanbanResult<()> {
        let has_default_status: bool = sqlx::query_scalar(
            "SELECT COUNT(*) > 0 FROM pragma_table_info('columns') WHERE name = 'default_status'",
        )
        .fetch_one(pool)
        .await
        .map_err(db_err)?;
        if has_default_status {
            return Ok(());
        }

        tracing::info!("migrating SQLite schema 6 -> 7: columns.default_status (backfilled NULL)");

        sqlx::raw_sql("ALTER TABLE columns ADD COLUMN default_status TEXT")
            .execute(pool)
            .await
            .map_err(db_err)?;
        Ok(())
    }

    /// Schema 7 -> 8: derive `columns.default_status` for every column still
    /// carrying `NULL` at the time a pre-8 database is opened, using
    /// `board_completion_columns` as the source of completion membership.
    /// `board_completion_columns` is left in place. A column whose
    /// `default_status` is already set (by an earlier write or a prior
    /// partial run of this migration) keeps that value; only `NULL` rows
    /// are touched.
    ///
    /// Idempotence gate: `metadata.schema_version` read directly (not the
    /// `NULL`-presence check the shape-changing migrations above use) — a
    /// column created after this migration has already run is allowed to
    /// carry `default_status = NULL` deliberately (`NewColumn.default_status:
    /// None`), and `migrate()` runs on every `open()`, so gating on "any NULL
    /// row exists" would re-backfill those columns on the next open instead
    /// of leaving the one-time migration's job done. A missing metadata row
    /// means a brand-new database, which has no columns to backfill either
    /// way.
    pub(crate) async fn migrate_v7_to_v8_default_status_derivation(
        pool: &Pool<Sqlite>,
    ) -> KanbanResult<()> {
        let schema_version: Option<u32> =
            sqlx::query_scalar("SELECT schema_version FROM metadata WHERE id = 1")
                .fetch_optional(pool)
                .await
                .map_err(db_err)?;
        if !matches!(schema_version, Some(v) if v < 8) {
            return Ok(());
        }

        tracing::info!(
            "migrating SQLite schema 7 -> 8: columns.default_status derived from \
             board_completion_columns"
        );

        // A DB old enough to have skipped straight past schema 6 (no boards
        // table, so `migrate_v5_to_v6_completion_columns`'s `has_legacy_col`
        // check no-ops) never gets `board_completion_columns` created at
        // all — SCHEMA no longer creates it unconditionally, since current
        // databases have no use for it. Every default_status-null column on
        // such a DB simply has no completion membership to derive.
        let has_table: bool = sqlx::query_scalar(
            "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='board_completion_columns'",
        )
        .fetch_one(pool)
        .await
        .map_err(db_err)?;

        if has_table {
            sqlx::raw_sql(
                "BEGIN;
                UPDATE columns
                   SET default_status = 'Done'
                 WHERE default_status IS NULL
                   AND id IN (SELECT column_id FROM board_completion_columns);
                UPDATE columns
                   SET default_status = 'Todo'
                 WHERE default_status IS NULL;
                COMMIT;",
            )
            .execute(pool)
            .await
            .map_err(db_err)?;
        } else {
            sqlx::query("UPDATE columns SET default_status = 'Todo' WHERE default_status IS NULL")
                .execute(pool)
                .await
                .map_err(db_err)?;
        }
        Ok(())
    }

    /// Schema 8 -> 9: drop `board_completion_columns`. By the time this runs,
    /// `migrate_v7_to_v8_default_status_derivation` has already copied every
    /// membership it recorded onto `columns.default_status`, which is now the
    /// only source of completion membership. Idempotence gate: table
    /// presence.
    pub(crate) async fn migrate_v8_to_v9_drop_completion_columns(
        pool: &Pool<Sqlite>,
    ) -> KanbanResult<()> {
        let has_table: bool = sqlx::query_scalar(
            "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='board_completion_columns'",
        )
        .fetch_one(pool)
        .await
        .map_err(db_err)?;
        if !has_table {
            return Ok(());
        }

        tracing::info!("migrating SQLite schema 8 -> 9: dropping board_completion_columns");

        sqlx::raw_sql("DROP TABLE board_completion_columns")
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

async fn column_present(pool: &Pool<Sqlite>, table: &str, column: &str) -> KanbanResult<bool> {
    sqlx::query_scalar(&format!(
        "SELECT COUNT(*) > 0 FROM pragma_table_info('{table}') WHERE name = '{column}'"
    ))
    .fetch_one(pool)
    .await
    .map_err(db_err)
}

async fn table_present(pool: &Pool<Sqlite>, table: &str) -> KanbanResult<bool> {
    sqlx::query_scalar("SELECT COUNT(*) > 0 FROM sqlite_master WHERE type = 'table' AND name = ?")
        .bind(table)
        .fetch_one(pool)
        .await
        .map_err(db_err)
}

impl SqliteStore {
    /// schema 10 -> 11: give every existing card the prefix it is addressed by
    /// TODAY, freezing its identifier.
    ///
    /// The column is added by `SCHEMA` for fresh databases; this ALTERs it onto
    /// older ones and backfills. The backfill calls
    /// `kanban_domain::resolve_card_prefix`, the SAME function the identifier
    /// reader uses, so the frozen value cannot drift from the value it exists
    /// to preserve. Reimplementing the rule here is how the two prefix
    /// backfills came to disagree earlier in this epic.
    ///
    /// Guarded on the column being absent, so it is idempotent and skips
    /// databases that already have it.
    ///
    /// Runs BEFORE `SCHEMA` rather than inside `migrate()`, because `SCHEMA`
    /// declares `idx_cards_prefix_number` and `CREATE INDEX IF NOT EXISTS`
    /// against a missing column is a hard error, not a skip.
    /// `migrate_v4_to_v5_cards_board_id` runs early for the same reason.
    pub(crate) async fn migrate_v10_to_v11_card_prefix(pool: &Pool<Sqlite>) -> KanbanResult<()> {
        if !table_present(pool, "cards").await? {
            return Ok(());
        }
        if column_present(pool, "cards", "prefix").await? {
            return Ok(());
        }

        sqlx::query("ALTER TABLE cards ADD COLUMN prefix TEXT NOT NULL DEFAULT ''")
            .execute(pool)
            .await
            .map_err(db_err)?;

        // Project only what the resolution rule reads. Deserializing whole
        // domain structs here would fail against fixtures that predate fields
        // those structs now require.
        let has_sprint_prefix = table_present(pool, "sprints").await?
            && column_present(pool, "sprints", "card_prefix").await?;
        let sprint_prefixes: Vec<(String, Option<String>)> = if has_sprint_prefix {
            sqlx::query_as("SELECT id, card_prefix FROM sprints")
                .fetch_all(pool)
                .await
                .map_err(db_err)?
        } else {
            Vec::new()
        };
        let board_prefixes: Vec<(String, Option<String>)> =
            if column_present(pool, "boards", "card_prefix").await? {
                sqlx::query_as("SELECT id, card_prefix FROM boards")
                    .fetch_all(pool)
                    .await
                    .map_err(db_err)?
            } else {
                Vec::new()
            };
        let columns: Vec<(String, String)> = sqlx::query_as("SELECT id, board_id FROM columns")
            .fetch_all(pool)
            .await
            .map_err(db_err)?;
        let cards: Vec<(String, String, Option<String>)> =
            sqlx::query_as("SELECT id, column_id, sprint_id FROM cards")
                .fetch_all(pool)
                .await
                .map_err(db_err)?;

        // Same rule, same function as the identifier reader and the JSON
        // backfill. Ids rather than domain structs, because these files predate
        // fields those structs now require.
        let columns: Vec<(Uuid, Uuid)> = columns
            .into_iter()
            .filter_map(|(c, b)| Some((p_uuid(&c).ok()?, p_uuid(&b).ok()?)))
            .collect();
        let board_prefixes: Vec<(Uuid, Option<String>)> = board_prefixes
            .into_iter()
            .filter_map(|(id, p)| Some((p_uuid(&id).ok()?, p)))
            .collect();
        let sprint_prefixes: Vec<(Uuid, Option<String>)> = sprint_prefixes
            .into_iter()
            .filter_map(|(id, p)| Some((p_uuid(&id).ok()?, p)))
            .collect();

        let mut tx = pool.begin().await.map_err(db_err)?;
        for (card_id, column_id, sprint_id) in cards {
            let Ok(column_uuid) = p_uuid(&column_id) else {
                continue;
            };
            let resolved = resolve_card_prefix_by_ids(
                column_uuid,
                sprint_id.as_deref().and_then(|s| p_uuid(s).ok()),
                &columns,
                &board_prefixes,
                &sprint_prefixes,
                DEFAULT_CARD_PREFIX,
            );
            sqlx::query("UPDATE cards SET prefix = ? WHERE id = ?")
                .bind(resolved)
                .bind(card_id)
                .execute(&mut *tx)
                .await
                .map_err(db_err)?;
        }
        tx.commit().await.map_err(db_err)?;

        Ok(())
    }

    /// schema 9 -> 10: additive backfill of the `prefixes` table (created by
    /// `SCHEMA` before this runs). Populates one row per distinct effective
    /// prefix a workspace would currently hand out, seeded from the CURRENT
    /// `boards.card_counter` / `board_sprint_counters.counter` values so no
    /// counter resets. `boards.card_counter` and `board_sprint_counters` are
    /// left untouched — they remain the live source of truth until a later
    /// card switches reads over.
    ///
    /// Guards every raw column/table read with a presence check: `migrate()`
    /// runs this step unconditionally against every earlier migration
    /// boundary's hand-seeded test fixtures, several of which predate
    /// `boards.card_prefix`/`sprints.card_prefix`/`board_sprint_counters` by
    /// construction. A fixture missing `boards.card_prefix` predates prefixes
    /// entirely and has nothing to backfill.
    pub(crate) async fn migrate_v9_to_v10_prefixes(pool: &Pool<Sqlite>) -> KanbanResult<()> {
        let already_populated: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM prefixes")
            .fetch_one(pool)
            .await
            .map_err(db_err)?;
        if already_populated > 0 {
            return Ok(());
        }

        if !column_present(pool, "boards", "card_prefix").await?
            || !column_present(pool, "boards", "sprint_prefix").await?
            || !column_present(pool, "boards", "card_counter").await?
        {
            return Ok(());
        }

        let board_rows: Vec<(String, Option<String>, Option<String>, i64)> =
            sqlx::query_as("SELECT id, card_prefix, sprint_prefix, card_counter FROM boards")
                .fetch_all(pool)
                .await
                .map_err(db_err)?;

        let sprints_have_card_prefix = table_present(pool, "sprints").await?
            && column_present(pool, "sprints", "card_prefix").await?;
        let sprint_override_rows: Vec<(String,)> = if sprints_have_card_prefix {
            sqlx::query_as("SELECT card_prefix FROM sprints WHERE card_prefix IS NOT NULL")
                .fetch_all(pool)
                .await
                .map_err(db_err)?
        } else {
            Vec::new()
        };

        let sprint_counter_rows: Vec<(String, String, i64)> =
            if table_present(pool, "board_sprint_counters").await? {
                sqlx::query_as("SELECT board_id, prefix, counter FROM board_sprint_counters")
                    .fetch_all(pool)
                    .await
                    .map_err(db_err)?
            } else {
                Vec::new()
            };

        struct BoardRow {
            id: Uuid,
            card_prefix: Option<String>,
            sprint_prefix: Option<String>,
            card_counter: i64,
        }
        let boards: Vec<BoardRow> = board_rows
            .into_iter()
            .map(|(id, card_prefix, sprint_prefix, card_counter)| {
                Ok(BoardRow {
                    id: p_uuid(&id)?,
                    card_prefix,
                    sprint_prefix,
                    card_counter,
                })
            })
            .collect::<KanbanResult<_>>()?;

        let sprint_overrides: Vec<String> = sprint_override_rows
            .into_iter()
            .map(|(card_prefix,)| card_prefix)
            .collect();

        let backfill_boards: Vec<BackfillBoard> = boards
            .iter()
            .map(|b| BackfillBoard {
                id: b.id,
                card_prefix: b.card_prefix.clone(),
                sprint_prefix: b.sprint_prefix.clone(),
                card_counter: b.card_counter,
                sprint_counters: sprint_counter_rows
                    .iter()
                    .filter(|(board_id, _, _)| board_id == &b.id.to_string())
                    .map(|(_, prefix, counter)| (prefix.clone(), *counter))
                    .collect(),
            })
            .collect();
        let backfill_sprints: Vec<BackfillSprint> = sprint_overrides
            .iter()
            .map(|card_prefix| BackfillSprint {
                card_prefix: card_prefix.clone(),
            })
            .collect();

        let rows = plan_prefix_backfill(
            &backfill_boards,
            &backfill_sprints,
            DEFAULT_CARD_PREFIX,
            DEFAULT_SPRINT_PREFIX,
        );

        // A single transaction: one row per effective prefix inserted
        // individually is otherwise one implicit fsync-bound transaction
        // per row, which dominates migration time on a large workspace.
        let mut tx = pool.begin().await.map_err(db_err)?;
        for row in rows {
            sqlx::query(
                "INSERT INTO prefixes (name, card_counter, sprint_counter)
                 VALUES (?, ?, ?)",
            )
            .bind(row.name)
            .bind(row.card_counter)
            .bind(row.sprint_counter)
            .execute(&mut *tx)
            .await
            .map_err(db_err)?;
        }
        tx.commit().await.map_err(db_err)?;

        Ok(())
    }
}
