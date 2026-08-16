use chrono::{DateTime, Utc};
use kanban_domain::data_store::DataStore;
use kanban_domain::{
    Archived, ArchivedBoard, ArchivedCard, Board, Card, Column, DependencyGraph, KanbanResult,
    Snapshot, Sprint,
};
use sqlx::Row;
use uuid::Uuid;

use super::conversions::{
    row_to_archived_card, row_to_board, row_to_card, row_to_column, row_to_sprint,
};
use super::helpers::{db_err, fmt_dt, p_dt, run};
use super::SqliteStore;

impl DataStore for SqliteStore {
    // Prefix

    fn get_prefix(&self, name: &str) -> KanbanResult<Option<kanban_domain::Prefix>> {
        // `prefixes.name` is `COLLATE NOCASE`, so `=` already matches
        // case-insensitively at the schema level. Normalising the probe as
        // well keeps this agreeing with the in-memory store, whose comparison
        // is the only one on the JSON path.
        let wanted = kanban_domain::Prefix::normalize(name);
        run(self.db_conn(|conn| {
            Box::pin(async move {
                let row: Option<(String, i64, i64)> = sqlx::query_as(
                    "SELECT name, card_counter, sprint_counter FROM prefixes WHERE name = ?",
                )
                .bind(wanted)
                .fetch_optional(&mut *conn)
                .await
                .map_err(db_err)?;
                Ok(row.map(
                    |(name, card_counter, sprint_counter)| kanban_domain::Prefix {
                        name,
                        card_counter: card_counter as u32,
                        sprint_counter: sprint_counter as u32,
                    },
                ))
            })
        }))
    }

    fn list_prefixes(&self) -> KanbanResult<Vec<kanban_domain::Prefix>> {
        run(self.db_conn(|conn| {
            Box::pin(async move {
                let rows: Vec<(String, i64, i64)> = sqlx::query_as(
                    "SELECT name, card_counter, sprint_counter FROM prefixes ORDER BY name ASC",
                )
                .fetch_all(&mut *conn)
                .await
                .map_err(db_err)?;
                Ok(rows
                    .into_iter()
                    .map(
                        |(name, card_counter, sprint_counter)| kanban_domain::Prefix {
                            name,
                            card_counter: card_counter as u32,
                            sprint_counter: sprint_counter as u32,
                        },
                    )
                    .collect())
            })
        }))
    }

    fn upsert_prefix(&self, prefix: kanban_domain::Prefix) -> KanbanResult<()> {
        let name = kanban_domain::Prefix::normalize(&prefix.name);
        let card_counter = prefix.card_counter as i64;
        let sprint_counter = prefix.sprint_counter as i64;
        run(self.db_conn(move |conn| {
            Box::pin(async move {
                sqlx::query(
                    "INSERT INTO prefixes (name, card_counter, sprint_counter)
                     VALUES (?, ?, ?)
                     ON CONFLICT(name) DO UPDATE SET
                         card_counter = excluded.card_counter,
                         sprint_counter = excluded.sprint_counter",
                )
                .bind(name)
                .bind(card_counter)
                .bind(sprint_counter)
                .execute(&mut *conn)
                .await
                .map_err(db_err)?;
                Ok(())
            })
        }))
    }

    // Board

    fn get_board(&self, id: Uuid) -> KanbanResult<Option<Board>> {
        run(self.db_conn(|conn| {
            Box::pin(async move {
                let id_str = id.to_string();
                // Reference-marker model: `get_board` is UNFILTERED — it returns the
                // head whether the board is live OR archived (an `archived_board`
                // marker hides it only from the LIVE `list_boards`). Callers
                // discriminate archived-ness via `get_archived_board`.
                let row = sqlx::query(
                    "SELECT id, name, description, sprint_prefix, card_prefix, task_sort_field,
                            task_sort_order, sprint_duration_days, sprint_name_used_count,
                            next_sprint_number, active_sprint_id, task_list_view,
                            COALESCE(card_counter, 1) as card_counter,
                            position, created_at, updated_at
                     FROM boards
                     WHERE id = ?",
                )
                .bind(&id_str)
                .fetch_optional(&mut *conn)
                .await
                .map_err(db_err)?;

                match row {
                    Some(row) => {
                        let (names, counters) =
                            SqliteStore::fetch_board_aux_with_conn(conn, &id_str).await?;
                        Ok(Some(row_to_board(&row, names, counters)?))
                    }
                    None => Ok(None),
                }
            })
        }))
    }

    fn list_boards(&self) -> KanbanResult<Vec<Board>> {
        run(self.list_boards_async())
    }

    fn upsert_board(&self, board: Board) -> KanbanResult<()> {
        run(self.write_board_async(&board))
    }

    fn delete_board(&self, id: Uuid) -> KanbanResult<()> {
        run(self.db_conn(|conn| {
            Box::pin(async move {
                sqlx::query(
                    "DELETE FROM boards
                     WHERE id = ?
                       AND NOT EXISTS (SELECT 1 FROM board_archival ba WHERE ba.board_id = boards.id)",
                )
                .bind(id.to_string())
                .execute(&mut *conn)
                .await
                .map_err(db_err)?;
                Ok(())
            })
        }))
    }

    // Column

    fn get_column(&self, id: Uuid) -> KanbanResult<Option<Column>> {
        run(self.db_conn(|conn| {
            Box::pin(async move {
                let row = sqlx::query(
                    "SELECT id, board_id, name, position, wip_limit, default_status, created_at, updated_at
                     FROM columns WHERE id = ?",
                )
                .bind(id.to_string())
                .fetch_optional(&mut *conn)
                .await
                .map_err(db_err)?;
                row.as_ref().map(row_to_column).transpose()
            })
        }))
    }

    fn list_columns_by_board(&self, board_id: Uuid) -> KanbanResult<Vec<Column>> {
        run(self.db_conn(|conn| {
            Box::pin(async move {
                let rows = sqlx::query(
                    "SELECT id, board_id, name, position, wip_limit, default_status, created_at, updated_at
                     FROM columns WHERE board_id = ?
                     ORDER BY position ASC, created_at ASC, id ASC",
                )
                .bind(board_id.to_string())
                .fetch_all(&mut *conn)
                .await
                .map_err(db_err)?;
                rows.iter().map(row_to_column).collect()
            })
        }))
    }

    fn list_all_columns(&self) -> KanbanResult<Vec<Column>> {
        run(self.list_all_columns_async())
    }

    fn upsert_column(&self, column: Column) -> KanbanResult<()> {
        run(self.write_column_async(&column))
    }

    fn delete_column(&self, id: Uuid) -> KanbanResult<()> {
        run(self.db_conn(|conn| {
            Box::pin(async move {
                sqlx::query("DELETE FROM columns WHERE id = ?")
                    .bind(id.to_string())
                    .execute(&mut *conn)
                    .await
                    .map_err(db_err)?;
                Ok(())
            })
        }))
    }

    fn delete_columns_by_board(&self, board_id: Uuid) -> KanbanResult<()> {
        run(self.db_conn(|conn| {
            Box::pin(async move {
                sqlx::query("DELETE FROM columns WHERE board_id = ?")
                    .bind(board_id.to_string())
                    .execute(&mut *conn)
                    .await
                    .map_err(db_err)?;
                Ok(())
            })
        }))
    }

    // Card

    fn get_card(&self, id: Uuid) -> KanbanResult<Option<Card>> {
        run(self.db_conn(|conn| {
            Box::pin(async move {
                let id_str = id.to_string();
                // F1 (KAN-870): get_card is UNFILTERED — an archived card stays in
                // `cards` behind a marker and is reachable by id (it is an ordinary,
                // editable card). The archived/live distinction is a LIST-level filter.
                let row = sqlx::query(
                    "SELECT id, column_id, board_id, title, description, priority, status, position,
                            due_date, points, card_number, prefix, sprint_id, created_at,
                            updated_at, completed_at
                     FROM cards
                     WHERE id = ?",
                )
                .bind(&id_str)
                .fetch_optional(&mut *conn)
                .await
                .map_err(db_err)?;

                match row {
                    Some(row) => {
                        let logs = SqliteStore::fetch_sprint_logs_for_card_with_conn(conn, &id_str)
                            .await?;
                        Ok(Some(row_to_card(&row, logs)?))
                    }
                    None => Ok(None),
                }
            })
        }))
    }

    fn list_all_cards(&self) -> KanbanResult<Vec<Card>> {
        run(self.fetch_cards_with_filter("", vec![]))
    }

    fn list_cards_by_column(&self, column_id: Uuid) -> KanbanResult<Vec<Card>> {
        run(self.fetch_cards_with_filter("AND column_id = ?", vec![column_id.to_string()]))
    }

    fn list_cards_by_columns(&self, column_ids: &[Uuid]) -> KanbanResult<Vec<Card>> {
        if column_ids.is_empty() {
            return Ok(Vec::new());
        }
        let placeholders = column_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let where_clause = format!("AND column_id IN ({placeholders})");
        let binds: Vec<String> = column_ids.iter().map(|id| id.to_string()).collect();
        run(self.fetch_cards_with_filter(where_clause, binds))
    }

    fn list_cards_by_sprint(&self, sprint_id: Uuid) -> KanbanResult<Vec<Card>> {
        run(self.fetch_cards_with_filter("AND sprint_id = ?", vec![sprint_id.to_string()]))
    }

    fn list_cards_by_prefix_and_number(
        &self,
        prefix: &str,
        card_number: u32,
    ) -> KanbanResult<Vec<Card>> {
        // Indexed by idx_cards_prefix_nocase_number. The COLLATE must match the
        // index's, or SQLite scans instead.
        run(self.fetch_cards_with_filter(
            "AND prefix = ? COLLATE NOCASE AND card_number = ?",
            vec![
                kanban_domain::Prefix::normalize(prefix),
                card_number.to_string(),
            ],
        ))
    }

    fn list_cards_by_number(&self, card_number: u32) -> KanbanResult<Vec<Card>> {
        // Indexed by idx_cards_number.
        run(self.fetch_cards_with_filter("AND card_number = ?", vec![card_number.to_string()]))
    }

    fn get_card_by_board_and_number(
        &self,
        board_id: Uuid,
        card_number: u32,
    ) -> KanbanResult<Option<Card>> {
        let cards = run(self.fetch_cards_with_filter(
            "AND board_id = ? AND card_number = ?",
            vec![board_id.to_string(), card_number.to_string()],
        ))?;
        Ok(cards.into_iter().next())
    }

    fn get_card_by_sprint_and_number(
        &self,
        sprint_id: Uuid,
        card_number: u32,
    ) -> KanbanResult<Option<Card>> {
        let cards = run(self.fetch_cards_with_filter(
            "AND sprint_id = ? AND card_number = ?",
            vec![sprint_id.to_string(), card_number.to_string()],
        ))?;
        Ok(cards.into_iter().next())
    }

    /// 3-state archived-aware column read. Overrides the loud-floor default so
    /// SQLite honours `ArchivedOnly`/`Include`; `LiveOnly` stays byte-identical
    /// to `list_cards_by_column` (same `NOT EXISTS` base clause).
    fn list_cards_by_column_filtered(
        &self,
        column_id: Uuid,
        archived: kanban_domain::ArchivedFilter,
    ) -> KanbanResult<Vec<Card>> {
        run(self.fetch_cards_in_column_filtered(&column_id.to_string(), archived))
    }

    fn count_cards_in_column(&self, column_id: Uuid) -> KanbanResult<usize> {
        run(self.db_conn(|conn| {
            Box::pin(async move {
                let column_id = column_id.to_string();
                SqliteStore::count_cards_in_column_with_conn(conn, &column_id).await
            })
        }))
    }

    /// 3-state archived-aware column count. Mirrors
    /// `list_cards_by_column_filtered`: `LiveOnly` matches `count_cards_in_column`
    /// exactly, `ArchivedOnly`/`Include` are served via the archived base clause.
    fn count_cards_in_column_filtered(
        &self,
        column_id: Uuid,
        archived: kanban_domain::ArchivedFilter,
    ) -> KanbanResult<usize> {
        run(self.count_cards_in_column_filtered_impl(&column_id.to_string(), archived))
    }

    fn count_cards_in_column_excluding(
        &self,
        column_id: Uuid,
        exclude: &[Uuid],
    ) -> KanbanResult<usize> {
        let exclude: Vec<Uuid> = exclude.to_vec();
        run(self.db_conn(move |conn| {
            Box::pin(async move {
                let column_id_str = column_id.to_string();
                if exclude.is_empty() {
                    // Associated fn on the already-held conn — calling
                    // `self.count_cards_in_column` here would capture `self`
                    // and violate db_conn's CAPTURE RULE.
                    return SqliteStore::count_cards_in_column_with_conn(conn, &column_id_str)
                        .await;
                }
                let placeholders = exclude.iter().map(|_| "?").collect::<Vec<_>>().join(",");
                let sql = format!(
                    "SELECT COUNT(*) as cnt FROM cards
                     WHERE column_id = ?
                       AND NOT EXISTS (SELECT 1 FROM archived_cards a WHERE a.card_id = cards.id)
                       AND id NOT IN ({placeholders})"
                );
                let mut query = sqlx::query(&sql).bind(&column_id_str);
                for id in &exclude {
                    query = query.bind(id.to_string());
                }
                let row = query.fetch_one(&mut *conn).await.map_err(db_err)?;
                Ok(row.try_get::<i32, _>("cnt").map_err(db_err)? as usize)
            })
        }))
    }

    fn upsert_card(&self, card: Card) -> KanbanResult<()> {
        run(self.write_card_async(&card))
    }

    fn delete_card(&self, id: Uuid) -> KanbanResult<()> {
        run(self.db_conn(|conn| {
            Box::pin(async move {
                sqlx::query(
                    "DELETE FROM cards
                     WHERE id = ? AND NOT EXISTS (SELECT 1 FROM archived_cards a WHERE a.card_id = cards.id)",
                )
                .bind(id.to_string())
                .execute(&mut *conn)
                .await
                .map_err(db_err)?;
                Ok(())
            })
        }))
    }

    fn delete_cards_by_columns(&self, column_ids: &[Uuid]) -> KanbanResult<()> {
        let column_ids: Vec<Uuid> = column_ids.to_vec();
        run(self.db_conn(move |conn| {
            Box::pin(async move {
                if column_ids.is_empty() {
                    return Ok(());
                }
                let placeholders = column_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
                let sql = format!(
                    "DELETE FROM cards
                     WHERE column_id IN ({placeholders})
                       AND NOT EXISTS (SELECT 1 FROM archived_cards a WHERE a.card_id = cards.id)"
                );
                let mut query = sqlx::query(&sql);
                for id in &column_ids {
                    query = query.bind(id.to_string());
                }
                query.execute(&mut *conn).await.map_err(db_err)?;
                Ok(())
            })
        }))
    }

    fn clear_sprint_from_cards(
        &self,
        sprint_id: Uuid,
        timestamp: DateTime<Utc>,
    ) -> KanbanResult<()> {
        run(self.db_conn(move |conn| {
            Box::pin(async move {
                let now = fmt_dt(&timestamp);
                sqlx::query(
                    "UPDATE cards SET sprint_id = NULL, updated_at = ?
                     WHERE sprint_id = ?
                       AND NOT EXISTS (SELECT 1 FROM archived_cards a WHERE a.card_id = cards.id)",
                )
                .bind(&now)
                .bind(sprint_id.to_string())
                .execute(&mut *conn)
                .await
                .map_err(db_err)?;
                Ok(())
            })
        }))
    }

    // Archived card

    fn get_archived_card(&self, card_id: Uuid) -> KanbanResult<Option<ArchivedCard>> {
        run(self.db_conn(|conn| {
            Box::pin(async move {
                let id_str = card_id.to_string();
                // Reference-marker model: a marker needs only card id, board scope, and
                // archive time. No card JOIN required.
                let row = sqlx::query(
                    "SELECT card_id AS id, board_id, archived_at
                     FROM archived_cards
                     WHERE card_id = ?",
                )
                .bind(&id_str)
                .fetch_optional(&mut *conn)
                .await
                .map_err(db_err)?;

                match row {
                    Some(row) => Ok(Some(row_to_archived_card(&row)?)),
                    None => Ok(None),
                }
            })
        }))
    }

    fn list_archived_cards(&self) -> KanbanResult<Vec<ArchivedCard>> {
        run(self.list_archived_cards_async())
    }

    fn insert_archived_card(&self, ac: ArchivedCard) -> KanbanResult<()> {
        run(self.write_archived_card_async(&ac))
    }

    fn delete_archived_card(&self, card_id: Uuid) -> KanbanResult<()> {
        run(self.db_conn(|conn| {
            Box::pin(async move {
                sqlx::query("DELETE FROM archived_cards WHERE card_id = ?")
                    .bind(card_id.to_string())
                    .execute(&mut *conn)
                    .await
                    .map_err(db_err)?;
                sqlx::query("DELETE FROM cards WHERE id = ?")
                    .bind(card_id.to_string())
                    .execute(&mut *conn)
                    .await
                    .map_err(db_err)?;
                Ok(())
            })
        }))
    }

    // Archived board (C5): board row stays in `boards`; a `board_archival` marker
    // row marks it out of the live set. Reconstitute Archived<Board> by join.

    fn get_archived_board(&self, board_id: Uuid) -> KanbanResult<Option<ArchivedBoard>> {
        run(self.db_conn(move |conn| {
            Box::pin(async move {
                let id_str = board_id.to_string();
                // Reference-marker model: the marker carries only the board id + archive
                // time. The board row stays live in `boards`.
                let row = sqlx::query(
                    "SELECT board_id, archived_at FROM board_archival WHERE board_id = ?",
                )
                .bind(&id_str)
                .fetch_optional(&mut *conn)
                .await
                .map_err(db_err)?;
                match row {
                    Some(row) => {
                        let at: String = row.try_get("archived_at").map_err(db_err)?;
                        Ok(Some(Archived::at(board_id, p_dt(&at)?)))
                    }
                    None => Ok(None),
                }
            })
        }))
    }

    fn list_archived_boards(&self) -> KanbanResult<Vec<ArchivedBoard>> {
        run(self.db_conn(|conn| {
            Box::pin(async move { SqliteStore::list_archived_boards_with_conn(conn).await })
        }))
    }

    fn insert_archived_board(&self, ab: ArchivedBoard) -> KanbanResult<()> {
        run(self.db_conn(move |conn| {
            Box::pin(async move {
                // Reference-marker model: the board row must already be live in
                // `boards` (archive keeps the head in place). Only record the marker.
                sqlx::query(
                    "INSERT INTO board_archival (board_id, archived_at) VALUES (?, ?)
                     ON CONFLICT(board_id) DO UPDATE SET archived_at = excluded.archived_at",
                )
                .bind(ab.entity_id.to_string())
                .bind(fmt_dt(&ab.metadata.archived_at))
                .execute(&mut *conn)
                .await
                .map_err(db_err)?;
                Ok(())
            })
        }))
    }

    fn delete_archived_board(&self, board_id: Uuid) -> KanbanResult<()> {
        run(self.db_conn(move |conn| {
            Box::pin(async move {
                let id_str = board_id.to_string();
                // Only remove the board row if it is actually archived — parity with
                // the in-memory store, whose delete_archived_board touches only the
                // archived collection and no-ops on a live board. Guarded delete runs
                // FIRST (the marker still exists to satisfy the EXISTS check); the
                // board_archival FK (ON DELETE CASCADE) drops the marker with the row,
                // and the explicit marker delete below is belt-and-suspenders.
                sqlx::query(
                    "DELETE FROM boards
                     WHERE id = ?
                       AND EXISTS (SELECT 1 FROM board_archival ba WHERE ba.board_id = boards.id)",
                )
                .bind(&id_str)
                .execute(&mut *conn)
                .await
                .map_err(db_err)?;
                sqlx::query("DELETE FROM board_archival WHERE board_id = ?")
                    .bind(&id_str)
                    .execute(&mut *conn)
                    .await
                    .map_err(db_err)?;
                Ok(())
            })
        }))
    }

    /// RESTORE path: drop only the archived MARKER, leaving the shared board row
    /// and its subtree intact. `delete_archived_board` above deletes the row
    /// (cascading the subtree) which is right for permanent delete but would
    /// destroy the subtree on restore (KAN-863). No-op on a live board.
    fn unarchive_board(&self, board_id: Uuid) -> KanbanResult<()> {
        run(self.db_conn(|conn| {
            Box::pin(async move {
                sqlx::query("DELETE FROM board_archival WHERE board_id = ?")
                    .bind(board_id.to_string())
                    .execute(&mut *conn)
                    .await
                    .map_err(db_err)?;
                Ok(())
            })
        }))
    }

    /// Board-scoped archived cards. Overrides the trait default (which filters
    /// the full list) with a direct `WHERE board_id = ?` on the extension table,
    /// so board scoping is a single indexed query.
    fn list_archived_cards_by_board(&self, board_id: Uuid) -> KanbanResult<Vec<ArchivedCard>> {
        run(self.db_conn(|conn| {
            Box::pin(async move {
                // Reference-marker model: markers only, scoped by the first-class
                // `board_id`. Single indexed query, no card JOIN.
                let rows = sqlx::query(
                    "SELECT card_id AS id, board_id, archived_at
                     FROM archived_cards
                     WHERE board_id = ?
                     ORDER BY archived_at",
                )
                .bind(board_id.to_string())
                .fetch_all(&mut *conn)
                .await
                .map_err(db_err)?;

                rows.iter().map(row_to_archived_card).collect()
            })
        }))
    }

    fn clear_sprint_from_archived_cards(
        &self,
        sprint_id: Uuid,
        timestamp: DateTime<Utc>,
    ) -> KanbanResult<()> {
        run(self.db_conn(move |conn| {
            Box::pin(async move {
                let now = fmt_dt(&timestamp);
                sqlx::query(
                    "UPDATE cards SET sprint_id = NULL, updated_at = ?
                     WHERE sprint_id = ?
                       AND EXISTS (SELECT 1 FROM archived_cards a WHERE a.card_id = cards.id)",
                )
                .bind(&now)
                .bind(sprint_id.to_string())
                .execute(&mut *conn)
                .await
                .map_err(db_err)?;
                Ok(())
            })
        }))
    }

    // Sprint

    fn get_sprint(&self, id: Uuid) -> KanbanResult<Option<Sprint>> {
        run(self.db_conn(|conn| {
            Box::pin(async move {
                let row = sqlx::query(
                    "SELECT id, board_id, sprint_number, name_index, prefix, card_prefix,
                            status, start_date, end_date, created_at, updated_at
                     FROM sprints WHERE id = ?",
                )
                .bind(id.to_string())
                .fetch_optional(&mut *conn)
                .await
                .map_err(db_err)?;
                row.as_ref().map(row_to_sprint).transpose()
            })
        }))
    }

    fn list_sprints_by_board(&self, board_id: Uuid) -> KanbanResult<Vec<Sprint>> {
        run(self.db_conn(|conn| {
            Box::pin(async move {
                let rows = sqlx::query(
                    "SELECT id, board_id, sprint_number, name_index, prefix, card_prefix,
                            status, start_date, end_date, created_at, updated_at
                     FROM sprints WHERE board_id = ? ORDER BY sprint_number",
                )
                .bind(board_id.to_string())
                .fetch_all(&mut *conn)
                .await
                .map_err(db_err)?;
                rows.iter().map(row_to_sprint).collect()
            })
        }))
    }

    fn list_all_sprints(&self) -> KanbanResult<Vec<Sprint>> {
        run(self.list_all_sprints_async())
    }

    fn upsert_sprint(&self, sprint: Sprint) -> KanbanResult<()> {
        run(self.write_sprint_async(&sprint))
    }

    fn delete_sprint(&self, id: Uuid) -> KanbanResult<()> {
        run(self.db_conn(|conn| {
            Box::pin(async move {
                sqlx::query("DELETE FROM sprints WHERE id = ?")
                    .bind(id.to_string())
                    .execute(&mut *conn)
                    .await
                    .map_err(db_err)?;
                Ok(())
            })
        }))
    }

    fn delete_sprints_by_board(&self, board_id: Uuid) -> KanbanResult<()> {
        run(self.db_conn(|conn| {
            Box::pin(async move {
                sqlx::query("DELETE FROM sprints WHERE board_id = ?")
                    .bind(board_id.to_string())
                    .execute(&mut *conn)
                    .await
                    .map_err(db_err)?;
                Ok(())
            })
        }))
    }

    // Graph

    fn get_graph(&self) -> KanbanResult<DependencyGraph> {
        run(self.get_graph_async())
    }

    fn set_graph(&self, graph: DependencyGraph) -> KanbanResult<()> {
        run(self.write_graph_async(&graph))
    }

    fn modify_graph(&self, f: kanban_domain::GraphMutFn) -> KanbanResult<()> {
        run(self.modify_graph_async(f))
    }

    // Snapshot

    fn snapshot(&self) -> KanbanResult<Snapshot> {
        run(self.snapshot_async())
    }

    fn apply_snapshot(&self, snapshot: Snapshot) -> KanbanResult<()> {
        run(self.apply_snapshot_async(snapshot))
    }
}
