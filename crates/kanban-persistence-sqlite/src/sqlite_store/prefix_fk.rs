use kanban_domain::{DomainError, KanbanResult};
use sqlx::{Pool, Row, Sqlite};

use super::helpers::db_err;
use super::init::{column_present, table_present};
use super::SqliteStore;

const CARDS_COLUMNS: &str = "id, column_id, board_id, title, description, priority, status, \
    position, due_date, points, card_number, prefix, sprint_id, created_at, updated_at, \
    completed_at";

impl SqliteStore {
    /// Schema 12 -> 13: gives `cards.prefix` a real foreign key to
    /// `prefixes(name)`, `ON DELETE RESTRICT ON UPDATE RESTRICT`, carried on
    /// the generated column `prefix_ref` so the empty prefix -- exempt from
    /// the domain rule this backstops -- stays writable.
    ///
    /// Rebuilds `cards` with foreign keys disabled: `DROP TABLE cards` with
    /// enforcement on would fire `ON DELETE CASCADE` on `sprint_logs` and
    /// `archived_cards` and empty them. The copy into the new table is
    /// verified against the constraint before the swap, so a database that
    /// cannot satisfy it is refused with the original table untouched.
    ///
    /// Idempotence gate: presence of the foreign key itself, not the
    /// generated column -- `pragma_table_info` does not list virtual
    /// generated columns, so gating on that would rebuild on every open.
    pub(crate) async fn migrate_v12_to_v13_prefix_fk(pool: &Pool<Sqlite>) -> KanbanResult<()> {
        if !table_present(pool, "cards").await? || !table_present(pool, "prefixes").await? {
            return Ok(());
        }
        if !column_present(pool, "cards", "prefix").await? {
            return Ok(());
        }

        let already_present: bool = sqlx::query_scalar(
            "SELECT COUNT(*) > 0 FROM pragma_foreign_key_list('cards') WHERE \"table\" = 'prefixes'",
        )
        .fetch_one(pool)
        .await
        .map_err(db_err)?;
        if already_present {
            return Ok(());
        }

        tracing::info!(
            "migrating SQLite schema 12 -> 13: adding a restricting foreign key from cards.prefix to prefixes.name"
        );

        if let Some((prefix, card_number)) = first_unbacked_namespace(pool).await? {
            tracing::error!(
                prefix = %prefix,
                card_number,
                "cannot upgrade to schema 13: card names a prefix with no matching row"
            );
            return Err(DomainError::PrefixNotBacked {
                card_number: card_number as u32,
                prefix,
            }
            .into());
        }

        let index_ddl: Vec<String> = sqlx::query(
            "SELECT sql FROM sqlite_master WHERE type = 'index' AND tbl_name = 'cards' AND sql IS NOT NULL",
        )
        .fetch_all(pool)
        .await
        .map_err(db_err)?
        .iter()
        .map(|row| row.try_get::<String, _>("sql"))
        .collect::<Result<_, _>>()
        .map_err(db_err)?;

        sqlx::raw_sql(&format!(
            "PRAGMA foreign_keys = OFF;
            BEGIN;
            DROP TABLE IF EXISTS cards_v13;
            CREATE TABLE cards_v13 (
                id TEXT PRIMARY KEY,
                column_id TEXT NOT NULL,
                board_id TEXT NOT NULL,
                title TEXT NOT NULL,
                description TEXT,
                priority TEXT NOT NULL DEFAULT 'Medium',
                status TEXT NOT NULL DEFAULT 'Todo',
                position INTEGER NOT NULL,
                due_date TEXT,
                points INTEGER CHECK (points >= 0 AND points <= 255),
                card_number INTEGER NOT NULL DEFAULT 0,
                prefix TEXT NOT NULL DEFAULT '',
                prefix_ref TEXT GENERATED ALWAYS AS (NULLIF(prefix, '')) VIRTUAL
                    REFERENCES prefixes(name) ON DELETE RESTRICT ON UPDATE RESTRICT,
                sprint_id TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                completed_at TEXT,
                FOREIGN KEY (sprint_id) REFERENCES sprints(id) ON DELETE SET NULL
            );
            INSERT INTO cards_v13 ({CARDS_COLUMNS})
                SELECT {CARDS_COLUMNS} FROM cards;
            COMMIT;
            PRAGMA foreign_keys = ON;"
        ))
        .execute(pool)
        .await
        .map_err(db_err)?;

        let copy_violations: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM pragma_foreign_key_check('cards_v13') WHERE \"parent\" = 'prefixes'",
        )
        .fetch_one(pool)
        .await
        .map_err(db_err)?;

        if copy_violations > 0 {
            sqlx::raw_sql("DROP TABLE IF EXISTS cards_v13;")
                .execute(pool)
                .await
                .map_err(db_err)?;
            let Some((prefix, card_number)) = first_unbacked_namespace(pool).await? else {
                return Err(kanban_domain::KanbanError::Database(
                    "schema 12 -> 13 upgrade: copy failed pragma_foreign_key_check but no unbacked namespace was found"
                        .to_string(),
                ));
            };
            tracing::error!(
                prefix = %prefix,
                card_number,
                "cannot upgrade to schema 13: card names a prefix with no matching row"
            );
            return Err(DomainError::PrefixNotBacked {
                card_number: card_number as u32,
                prefix,
            }
            .into());
        }

        let index_batch = index_ddl.join(";\n");
        sqlx::raw_sql(&format!(
            "PRAGMA foreign_keys = OFF;
            BEGIN;
            DROP TABLE cards;
            ALTER TABLE cards_v13 RENAME TO cards;
            {index_batch};
            COMMIT;
            PRAGMA foreign_keys = ON;"
        ))
        .execute(pool)
        .await
        .map_err(db_err)?;

        Ok(())
    }
}

/// First card (by lowest `card_number`, ties broken by prefix) whose
/// non-empty `prefix` has no matching row in `prefixes`. The parent column
/// sits on the LEFT of the comparison so it takes `prefixes.name`'s
/// `COLLATE NOCASE`, matching the foreign key's own semantics; the reversed
/// spelling would compare BINARY and report false violations.
async fn first_unbacked_namespace(pool: &Pool<Sqlite>) -> KanbanResult<Option<(String, i64)>> {
    let row: Option<(String, i64)> = sqlx::query_as(
        "SELECT c.prefix, MIN(c.card_number) FROM cards c
         WHERE c.prefix <> '' AND NOT EXISTS (SELECT 1 FROM prefixes p WHERE p.name = c.prefix)
         GROUP BY c.prefix
         ORDER BY 2
         LIMIT 1",
    )
    .fetch_optional(pool)
    .await
    .map_err(db_err)?;
    Ok(row)
}
