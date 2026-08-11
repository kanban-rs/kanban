use std::collections::HashMap;

use kanban_domain::{Card, KanbanResult, SprintLog};
use sqlx::Row;

use crate::sqlite_store::conversions::{row_to_card, row_to_sprint_log};
use crate::sqlite_store::helpers::{db_err, fmt_dt, opt_dt, required_str};
use crate::sqlite_store::SqliteStore;

impl SqliteStore {
    pub(crate) async fn fetch_sprint_logs_for_card_with_conn(
        conn: &mut sqlx::SqliteConnection,
        card_id: &str,
    ) -> KanbanResult<Vec<SprintLog>> {
        let rows = sqlx::query(
            "SELECT sprint_id, sprint_number, sprint_name, started_at, ended_at, status
             FROM sprint_logs WHERE card_id = ? ORDER BY id",
        )
        .bind(card_id)
        .fetch_all(&mut *conn)
        .await
        .map_err(db_err)?;
        rows.iter().map(row_to_sprint_log).collect()
    }

    pub(crate) async fn write_card_with_conn(
        conn: &mut sqlx::SqliteConnection,
        card: &Card,
    ) -> KanbanResult<()> {
        let id = card.id.to_string();

        sqlx::query(
            "INSERT INTO cards (id, column_id, board_id, title, description, priority, status,
                position, due_date, points, card_number, sprint_id, created_at, updated_at,
                completed_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(id) DO UPDATE SET
                column_id=excluded.column_id, board_id=excluded.board_id, title=excluded.title,
                description=excluded.description, priority=excluded.priority,
                status=excluded.status, position=excluded.position,
                due_date=excluded.due_date, points=excluded.points,
                card_number=excluded.card_number, sprint_id=excluded.sprint_id,
                updated_at=excluded.updated_at, completed_at=excluded.completed_at",
        )
        .bind(&id)
        .bind(card.column_id.to_string())
        .bind(card.board_id.to_string())
        .bind(required_str(&card.title, "card.title")?)
        .bind(&card.description)
        .bind(format!("{:?}", card.priority))
        .bind(format!("{:?}", card.status))
        .bind(card.position)
        .bind(opt_dt(&card.due_date))
        .bind(card.points.map(|v| v as i32))
        .bind(card.card_number as i32)
        .bind(card.sprint_id.map(|id| id.to_string()))
        .bind(fmt_dt(&card.created_at))
        .bind(fmt_dt(&card.updated_at))
        .bind(opt_dt(&card.completed_at))
        .execute(&mut *conn)
        .await
        .map_err(db_err)?;

        sqlx::query("DELETE FROM sprint_logs WHERE card_id = ?")
            .bind(&id)
            .execute(&mut *conn)
            .await
            .map_err(db_err)?;
        for log in &card.sprint_logs {
            sqlx::query(
                "INSERT INTO sprint_logs (card_id, sprint_id, sprint_number, sprint_name,
                    started_at, ended_at, status)
                 VALUES (?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(&id)
            .bind(log.sprint_id.to_string())
            .bind(log.sprint_number as i32)
            .bind(&log.sprint_name)
            .bind(fmt_dt(&log.started_at))
            .bind(opt_dt(&log.ended_at))
            .bind(required_str(&log.status, "sprint_log.status")?)
            .execute(&mut *conn)
            .await
            .map_err(db_err)?;
        }

        Ok(())
    }

    pub(crate) async fn write_card_async(&self, card: &Card) -> KanbanResult<()> {
        let card = card.clone();
        self.db_conn(|conn| Box::pin(async move { Self::write_card_with_conn(conn, &card).await }))
            .await
    }

    pub(crate) async fn fetch_sprint_logs_batch_with_conn(
        conn: &mut sqlx::SqliteConnection,
        card_ids: &[String],
    ) -> KanbanResult<HashMap<String, Vec<SprintLog>>> {
        if card_ids.is_empty() {
            return Ok(HashMap::new());
        }
        let placeholders = card_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let sql = format!(
            "SELECT card_id, sprint_id, sprint_number, sprint_name, started_at, ended_at, status
             FROM sprint_logs WHERE card_id IN ({placeholders}) ORDER BY id"
        );
        let mut query = sqlx::query(&sql);
        for id in card_ids {
            query = query.bind(id);
        }
        let rows = query.fetch_all(&mut *conn).await.map_err(db_err)?;
        let mut map: HashMap<String, Vec<SprintLog>> = HashMap::new();
        for row in &rows {
            let card_id: String = row.try_get("card_id").map_err(db_err)?;
            let log = row_to_sprint_log(row)?;
            map.entry(card_id).or_default().push(log);
        }
        Ok(map)
    }

    pub(crate) async fn fetch_cards_with_filter(
        &self,
        where_clause: impl Into<String>,
        binds: Vec<String>,
    ) -> KanbanResult<Vec<Card>> {
        // LIVE-scoped reads exclude archived cards (they stay live behind a marker
        // but are hidden from the live list). Snapshot/export fidelity uses
        // `fetch_all_cards_unfiltered` instead.
        let filter = "WHERE NOT EXISTS (SELECT 1 FROM archived_cards a WHERE a.card_id = cards.id)"
            .to_string();
        let where_clause = where_clause.into();
        self.db_conn(move |conn| {
            Box::pin(async move {
                Self::fetch_cards_query_with_conn(conn, &filter, &where_clause, &binds).await
            })
        })
        .await
    }

    /// The `WHERE`-prefixed archived predicate for an [`ArchivedFilter`]. Empty
    /// for [`Include`](kanban_domain::ArchivedFilter::Include) (no archived
    /// restriction). Callers that append an AND-prefixed column predicate must
    /// use [`connector_for`](Self::connector_for) to pick `WHERE`/`AND` when the
    /// base is empty.
    pub(crate) fn archived_base_clause(archived: kanban_domain::ArchivedFilter) -> &'static str {
        match archived {
            kanban_domain::ArchivedFilter::LiveOnly => {
                "WHERE NOT EXISTS (SELECT 1 FROM archived_cards a WHERE a.card_id = cards.id)"
            }
            kanban_domain::ArchivedFilter::ArchivedOnly => {
                "WHERE EXISTS (SELECT 1 FROM archived_cards a WHERE a.card_id = cards.id)"
            }
            kanban_domain::ArchivedFilter::Include => "",
        }
    }

    /// The connector (`WHERE` or `AND`) that must prefix an extra column/sprint
    /// predicate given whether the archived base clause is empty. When the base
    /// is empty (Include), the predicate opens the WHERE itself; otherwise it
    /// extends the existing WHERE with AND.
    pub(crate) fn connector_for(base_clause: &str) -> &'static str {
        if base_clause.is_empty() {
            "WHERE"
        } else {
            "AND"
        }
    }

    /// Filter-aware column read backing `list_cards_by_column_filtered`. Builds
    /// the archived base clause for `archived` and appends the column predicate
    /// with the correct `WHERE`/`AND` connector so the empty-base (Include) case
    /// stays valid SQL.
    pub(crate) async fn fetch_cards_in_column_filtered(
        &self,
        column_id: &str,
        archived: kanban_domain::ArchivedFilter,
    ) -> KanbanResult<Vec<Card>> {
        let base = Self::archived_base_clause(archived).to_string();
        let column_clause = format!("{} column_id = ?", Self::connector_for(&base));
        let column_id = column_id.to_string();
        self.db_conn(move |conn| {
            Box::pin(async move {
                Self::fetch_cards_query_with_conn(conn, &base, &column_clause, &[column_id]).await
            })
        })
        .await
    }

    /// Filter-aware column count backing `count_cards_in_column_filtered`. Same
    /// base-clause / connector logic as [`fetch_cards_in_column_filtered`].
    pub(crate) async fn count_cards_in_column_filtered_impl(
        &self,
        column_id: &str,
        archived: kanban_domain::ArchivedFilter,
    ) -> KanbanResult<usize> {
        let column_id = column_id.to_string();
        self.db_conn(move |conn| {
            Box::pin(async move {
                Self::count_cards_in_column_filtered_impl_with_conn(conn, &column_id, archived)
                    .await
            })
        })
        .await
    }

    pub(crate) async fn count_cards_in_column_filtered_impl_with_conn(
        conn: &mut sqlx::SqliteConnection,
        column_id: &str,
        archived: kanban_domain::ArchivedFilter,
    ) -> KanbanResult<usize> {
        let base = Self::archived_base_clause(archived);
        let sql = format!(
            "SELECT COUNT(*) as cnt FROM cards {} {} column_id = ?",
            base,
            Self::connector_for(base),
        );
        let row = sqlx::query(&sql)
            .bind(column_id)
            .fetch_one(&mut *conn)
            .await
            .map_err(db_err)?;
        Ok(row.try_get::<i32, _>("cnt").map_err(db_err)? as usize)
    }

    /// Associated fn taking an already-held connection, so
    /// `count_cards_in_column_excluding`'s empty-`exclude` fast path can call
    /// it without re-entering `db_conn` through a `self` capture (would
    /// violate the CAPTURE RULE — see `data_store.rs`).
    pub(crate) async fn count_cards_in_column_with_conn(
        conn: &mut sqlx::SqliteConnection,
        column_id: &str,
    ) -> KanbanResult<usize> {
        let row = sqlx::query(
            "SELECT COUNT(*) as cnt FROM cards
             WHERE column_id = ? AND NOT EXISTS (SELECT 1 FROM archived_cards a WHERE a.card_id = cards.id)",
        )
        .bind(column_id)
        .fetch_one(&mut *conn)
        .await
        .map_err(db_err)?;
        Ok(row.try_get::<i32, _>("cnt").map_err(db_err)? as usize)
    }

    /// ALL card rows, live AND archived (unfiltered). Reference-marker model
    /// (F3b): `snapshot.cards` is the single source of truth for every card, so a
    /// snapshot must carry the archived cards' live rows too (their archival is
    /// recorded separately by the `archived_cards` markers).
    pub(crate) async fn fetch_all_cards_unfiltered(&self) -> KanbanResult<Vec<Card>> {
        self.db_conn(|conn| {
            Box::pin(async move { Self::fetch_cards_query_with_conn(conn, "", "", &[]).await })
        })
        .await
    }

    pub(crate) async fn fetch_cards_query_with_conn(
        conn: &mut sqlx::SqliteConnection,
        base_filter: &str,
        where_clause: &str,
        binds: &[String],
    ) -> KanbanResult<Vec<Card>> {
        let sql = format!(
            "SELECT id, column_id, board_id, title, description, priority, status, position,
                    due_date, points, card_number, sprint_id, created_at, updated_at, completed_at
             FROM cards {} {}
             ORDER BY position ASC, created_at ASC, id ASC",
            base_filter, where_clause
        );
        let mut query = sqlx::query(&sql);
        for b in binds {
            query = query.bind(b);
        }
        let rows = query.fetch_all(&mut *conn).await.map_err(db_err)?;

        let card_ids: Vec<String> = rows
            .iter()
            .map(|r| r.try_get("id").map_err(db_err))
            .collect::<KanbanResult<_>>()?;
        let mut logs_map = Self::fetch_sprint_logs_batch_with_conn(conn, &card_ids).await?;

        let mut cards = Vec::with_capacity(rows.len());
        for row in &rows {
            let id_str: String = row.try_get("id").map_err(db_err)?;
            let logs = logs_map.remove(&id_str).unwrap_or_default();
            cards.push(row_to_card(row, logs)?);
        }
        Ok(cards)
    }
}
