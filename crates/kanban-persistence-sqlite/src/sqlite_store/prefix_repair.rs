use std::collections::HashSet;

use chrono::{DateTime, Utc};
use kanban_domain::{
    resolve_card_prefix_by_ids, Card, CardPriority, CardRecord, CardStatus, KanbanResult, Prefix,
    DEFAULT_CARD_PREFIX,
};
use sqlx::{Pool, Sqlite};
use uuid::Uuid;

use super::helpers::{db_err, p_uuid};
use super::init::{column_present, table_present};
use super::SqliteStore;

/// Only `prefix` and `card_number` are meaningful on these cards; every other
/// field is a placeholder the integrity rules do not read.
async fn stamped_cards(pool: &Pool<Sqlite>) -> KanbanResult<Vec<Card>> {
    let rows: Vec<(String, i64)> = sqlx::query_as("SELECT prefix, card_number FROM cards")
        .fetch_all(pool)
        .await
        .map_err(db_err)?;

    rows.into_iter()
        .map(|(prefix, card_number)| {
            Card::reconstitute(CardRecord {
                id: Uuid::nil(),
                column_id: Uuid::nil(),
                board_id: Uuid::nil(),
                title: String::new(),
                description: None,
                priority: CardPriority::Medium,
                status: CardStatus::Todo,
                position: 0,
                due_date: None,
                points: None,
                card_number: card_number as u32,
                prefix,
                sprint_id: None,
                created_at: DateTime::<Utc>::UNIX_EPOCH,
                updated_at: DateTime::<Utc>::UNIX_EPOCH,
                completed_at: None,
                sprint_logs: Vec::new(),
            })
        })
        .collect()
}

impl SqliteStore {
    /// Stamps every card that carries no prefix with the namespace it is
    /// addressed by, so no card names the empty namespace. The value comes
    /// from `resolve_card_prefix_by_ids` -- the function the identifier
    /// reader and both backends' backfills route through -- and falls back
    /// to `DEFAULT_CARD_PREFIX` when the card's column id does not parse, or
    /// when resolution itself yields the empty string. Returns how many
    /// cards were stamped. Idempotent; never rewrites a prefix a card
    /// already carries.
    pub(crate) async fn stamp_empty_card_prefixes(pool: &Pool<Sqlite>) -> KanbanResult<usize> {
        let empty_cards: Vec<(String, String, Option<String>)> =
            sqlx::query_as("SELECT id, column_id, sprint_id FROM cards WHERE prefix = ''")
                .fetch_all(pool)
                .await
                .map_err(db_err)?;
        if empty_cards.is_empty() {
            return Ok(0);
        }

        let columns: Vec<(Uuid, Uuid)> = {
            let rows: Vec<(String, String)> = sqlx::query_as("SELECT id, board_id FROM columns")
                .fetch_all(pool)
                .await
                .map_err(db_err)?;
            rows.into_iter()
                .filter_map(|(c, b)| Some((p_uuid(&c).ok()?, p_uuid(&b).ok()?)))
                .collect()
        };
        let boards: Vec<(Uuid, Option<String>)> =
            if column_present(pool, "boards", "card_prefix").await? {
                let rows: Vec<(String, Option<String>)> =
                    sqlx::query_as("SELECT id, card_prefix FROM boards")
                        .fetch_all(pool)
                        .await
                        .map_err(db_err)?;
                rows.into_iter()
                    .filter_map(|(id, p)| Some((p_uuid(&id).ok()?, p)))
                    .collect()
            } else {
                Vec::new()
            };
        let sprints: Vec<(Uuid, Option<String>)> = if table_present(pool, "sprints").await?
            && column_present(pool, "sprints", "card_prefix").await?
        {
            let rows: Vec<(String, Option<String>)> =
                sqlx::query_as("SELECT id, card_prefix FROM sprints")
                    .fetch_all(pool)
                    .await
                    .map_err(db_err)?;
            rows.into_iter()
                .filter_map(|(id, p)| Some((p_uuid(&id).ok()?, p)))
                .collect()
        } else {
            Vec::new()
        };

        let mut stamped = 0usize;
        let mut tx = pool.begin().await.map_err(db_err)?;
        for (card_id, column_id, sprint_id) in empty_cards {
            let resolved = match p_uuid(&column_id) {
                Ok(column_uuid) => resolve_card_prefix_by_ids(
                    column_uuid,
                    sprint_id.as_deref().and_then(|s| p_uuid(s).ok()),
                    &columns,
                    &boards,
                    &sprints,
                    None,
                ),
                Err(_) => DEFAULT_CARD_PREFIX.to_string(),
            };
            let resolved = if resolved.is_empty() {
                DEFAULT_CARD_PREFIX.to_string()
            } else {
                resolved
            };
            sqlx::query("UPDATE cards SET prefix = ? WHERE id = ?")
                .bind(&resolved)
                .bind(&card_id)
                .execute(&mut *tx)
                .await
                .map_err(db_err)?;
            stamped += 1;
        }
        tx.commit().await.map_err(db_err)?;

        if stamped > 0 {
            tracing::info!(stamped, "stamped cards that carried no prefix");
        }

        Ok(stamped)
    }

    /// Inserts a row for every namespace a card names that has none. The
    /// counter is set to the highest `card_number` among the cards naming
    /// it. Returns how many rows were inserted. Idempotent; never lowers an
    /// existing counter.
    pub(crate) async fn repair_unbacked_card_namespaces(
        pool: &Pool<Sqlite>,
    ) -> KanbanResult<usize> {
        let rows: Vec<(String, i64, i64)> =
            sqlx::query_as("SELECT name, card_counter, sprint_counter FROM prefixes")
                .fetch_all(pool)
                .await
                .map_err(db_err)?;
        let rows: Vec<Prefix> = rows
            .into_iter()
            .map(|(name, card_counter, sprint_counter)| Prefix {
                name,
                card_counter: card_counter as u32,
                sprint_counter: sprint_counter as u32,
            })
            .collect();

        let cards = stamped_cards(pool).await?;

        let unbacked: HashSet<String> = kanban_domain::unbacked_namespaces(&cards, &rows)
            .into_iter()
            .collect();
        if unbacked.is_empty() {
            return Ok(0);
        }

        let implied = kanban_domain::counters_implied_by(&cards, &[], &[], &[], None);

        let mut inserted = 0usize;
        let mut tx = pool.begin().await.map_err(db_err)?;
        for prefix in implied.iter().filter(|p| unbacked.contains(&p.name)) {
            let result = sqlx::query(
                "INSERT INTO prefixes (name, card_counter, sprint_counter) VALUES (?, ?, ?) \
                 ON CONFLICT(name) DO NOTHING",
            )
            .bind(&prefix.name)
            .bind(i64::from(prefix.card_counter))
            .bind(i64::from(prefix.sprint_counter))
            .execute(&mut *tx)
            .await
            .map_err(db_err)?;
            if result.rows_affected() > 0 {
                inserted += 1;
            }
        }
        tx.commit().await.map_err(db_err)?;

        if inserted > 0 {
            tracing::info!(
                inserted,
                "inserted prefix rows for namespaces named by cards but backed by none"
            );
        }

        Ok(inserted)
    }
}
