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
    // Board

    fn get_board(&self, id: Uuid) -> KanbanResult<Option<Board>> {
        run(async {
            let id_str = id.to_string();
            let row = sqlx::query(
                "SELECT id, name, description, sprint_prefix, card_prefix, task_sort_field,
                        task_sort_order, sprint_duration_days, sprint_name_used_count,
                        next_sprint_number, active_sprint_id, task_list_view,
                        COALESCE(card_counter, 1) as card_counter,
                        completion_column_id, position, created_at, updated_at
                 FROM boards
                 WHERE id = ?
                   AND NOT EXISTS (SELECT 1 FROM board_archival ba WHERE ba.board_id = boards.id)",
            )
            .bind(&id_str)
            .fetch_optional(&self.pool)
            .await
            .map_err(db_err)?;

            match row {
                Some(row) => {
                    let (names, counters) = self.fetch_board_aux(&id_str).await?;
                    Ok(Some(row_to_board(&row, names, counters)?))
                }
                None => Ok(None),
            }
        })
    }

    fn list_boards(&self) -> KanbanResult<Vec<Board>> {
        run(self.list_boards_async())
    }

    fn upsert_board(&self, board: Board) -> KanbanResult<()> {
        run(self.write_board_async(&board))
    }

    fn delete_board(&self, id: Uuid) -> KanbanResult<()> {
        run(async {
            sqlx::query(
                "DELETE FROM boards
                 WHERE id = ?
                   AND NOT EXISTS (SELECT 1 FROM board_archival ba WHERE ba.board_id = boards.id)",
            )
            .bind(id.to_string())
            .execute(&self.pool)
            .await
            .map_err(db_err)?;
            Ok(())
        })
    }

    // Column

    fn get_column(&self, id: Uuid) -> KanbanResult<Option<Column>> {
        run(async {
            let row = sqlx::query(
                "SELECT id, board_id, name, position, wip_limit, created_at, updated_at
                 FROM columns WHERE id = ?",
            )
            .bind(id.to_string())
            .fetch_optional(&self.pool)
            .await
            .map_err(db_err)?;
            row.as_ref().map(row_to_column).transpose()
        })
    }

    fn list_columns_by_board(&self, board_id: Uuid) -> KanbanResult<Vec<Column>> {
        run(async {
            let rows = sqlx::query(
                "SELECT id, board_id, name, position, wip_limit, created_at, updated_at
                 FROM columns WHERE board_id = ? ORDER BY position",
            )
            .bind(board_id.to_string())
            .fetch_all(&self.pool)
            .await
            .map_err(db_err)?;
            rows.iter().map(row_to_column).collect()
        })
    }

    fn list_all_columns(&self) -> KanbanResult<Vec<Column>> {
        run(self.list_all_columns_async())
    }

    fn upsert_column(&self, column: Column) -> KanbanResult<()> {
        run(self.write_column_async(&column))
    }

    fn delete_column(&self, id: Uuid) -> KanbanResult<()> {
        run(async {
            sqlx::query("DELETE FROM columns WHERE id = ?")
                .bind(id.to_string())
                .execute(&self.pool)
                .await
                .map_err(db_err)?;
            Ok(())
        })
    }

    fn delete_columns_by_board(&self, board_id: Uuid) -> KanbanResult<()> {
        run(async {
            sqlx::query("DELETE FROM columns WHERE board_id = ?")
                .bind(board_id.to_string())
                .execute(&self.pool)
                .await
                .map_err(db_err)?;
            Ok(())
        })
    }

    // Card

    fn get_card(&self, id: Uuid) -> KanbanResult<Option<Card>> {
        run(async {
            let id_str = id.to_string();
            // F1 (KAN-870): get_card is UNFILTERED — an archived card stays in
            // `cards` behind a marker and is reachable by id (it is an ordinary,
            // editable card). The archived/live distinction is a LIST-level filter.
            let row = sqlx::query(
                "SELECT id, column_id, title, description, priority, status, position,
                        due_date, points, card_number, sprint_id, created_at, updated_at,
                        completed_at
                 FROM cards
                 WHERE id = ?",
            )
            .bind(&id_str)
            .fetch_optional(&self.pool)
            .await
            .map_err(db_err)?;

            match row {
                Some(row) => {
                    let logs = self.fetch_sprint_logs_for_card(&id_str).await?;
                    Ok(Some(row_to_card(&row, logs)?))
                }
                None => Ok(None),
            }
        })
    }

    fn list_all_cards(&self) -> KanbanResult<Vec<Card>> {
        run(self.fetch_cards_with_filter("", &[]))
    }

    fn list_cards_by_column(&self, column_id: Uuid) -> KanbanResult<Vec<Card>> {
        run(self.fetch_cards_with_filter("AND column_id = ?", &[column_id.to_string()]))
    }

    fn list_cards_by_columns(&self, column_ids: &[Uuid]) -> KanbanResult<Vec<Card>> {
        if column_ids.is_empty() {
            return Ok(Vec::new());
        }
        let placeholders = column_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let where_clause = format!("AND column_id IN ({placeholders})");
        let binds: Vec<String> = column_ids.iter().map(|id| id.to_string()).collect();
        run(self.fetch_cards_with_filter(&where_clause, &binds))
    }

    fn list_cards_by_sprint(&self, sprint_id: Uuid) -> KanbanResult<Vec<Card>> {
        run(self.fetch_cards_with_filter("AND sprint_id = ?", &[sprint_id.to_string()]))
    }

    fn count_cards_in_column(&self, column_id: Uuid) -> KanbanResult<usize> {
        run(async {
            let row = sqlx::query(
                "SELECT COUNT(*) as cnt FROM cards
                 WHERE column_id = ? AND NOT EXISTS (SELECT 1 FROM archived_cards a WHERE a.card_id = cards.id)",
            )
            .bind(column_id.to_string())
            .fetch_one(&self.pool)
            .await
            .map_err(db_err)?;
            Ok(row.try_get::<i32, _>("cnt").map_err(db_err)? as usize)
        })
    }

    fn count_cards_in_column_excluding(
        &self,
        column_id: Uuid,
        exclude: &[Uuid],
    ) -> KanbanResult<usize> {
        run(async {
            if exclude.is_empty() {
                return self.count_cards_in_column(column_id);
            }
            let placeholders = exclude.iter().map(|_| "?").collect::<Vec<_>>().join(",");
            let sql = format!(
                "SELECT COUNT(*) as cnt FROM cards
                 WHERE column_id = ?
                   AND NOT EXISTS (SELECT 1 FROM archived_cards a WHERE a.card_id = cards.id)
                   AND id NOT IN ({placeholders})"
            );
            let mut query = sqlx::query(&sql).bind(column_id.to_string());
            for id in exclude {
                query = query.bind(id.to_string());
            }
            let row = query.fetch_one(&self.pool).await.map_err(db_err)?;
            Ok(row.try_get::<i32, _>("cnt").map_err(db_err)? as usize)
        })
    }

    fn upsert_card(&self, card: Card) -> KanbanResult<()> {
        run(self.write_card_async(&card))
    }

    fn delete_card(&self, id: Uuid) -> KanbanResult<()> {
        run(async {
            sqlx::query(
                "DELETE FROM cards
                 WHERE id = ? AND NOT EXISTS (SELECT 1 FROM archived_cards a WHERE a.card_id = cards.id)",
            )
            .bind(id.to_string())
            .execute(&self.pool)
            .await
            .map_err(db_err)?;
            Ok(())
        })
    }

    fn delete_cards_by_columns(&self, column_ids: &[Uuid]) -> KanbanResult<()> {
        run(async {
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
            for id in column_ids {
                query = query.bind(id.to_string());
            }
            query.execute(&self.pool).await.map_err(db_err)?;
            Ok(())
        })
    }

    fn clear_sprint_from_cards(
        &self,
        sprint_id: Uuid,
        timestamp: DateTime<Utc>,
    ) -> KanbanResult<()> {
        run(async {
            let now = fmt_dt(&timestamp);
            sqlx::query(
                "UPDATE cards SET sprint_id = NULL, updated_at = ?
                 WHERE sprint_id = ?
                   AND NOT EXISTS (SELECT 1 FROM archived_cards a WHERE a.card_id = cards.id)",
            )
            .bind(&now)
            .bind(sprint_id.to_string())
            .execute(&self.pool)
            .await
            .map_err(db_err)?;
            Ok(())
        })
    }

    // Archived card

    fn get_archived_card(&self, card_id: Uuid) -> KanbanResult<Option<ArchivedCard>> {
        run(async {
            let id_str = card_id.to_string();
            let row = sqlx::query(
                "SELECT c.id, c.column_id, c.title, c.description, c.priority, c.status,
                        c.position, c.due_date, c.points, c.card_number, c.sprint_id,
                        c.created_at, c.updated_at, c.completed_at,
                        ac.board_id, ac.archived_at, ac.original_column_id, ac.original_position
                 FROM archived_cards ac
                 JOIN cards c ON ac.card_id = c.id
                 WHERE ac.card_id = ?",
            )
            .bind(&id_str)
            .fetch_optional(&self.pool)
            .await
            .map_err(db_err)?;

            match row {
                Some(row) => {
                    let logs = self.fetch_sprint_logs_for_card(&id_str).await?;
                    Ok(Some(row_to_archived_card(&row, logs)?))
                }
                None => Ok(None),
            }
        })
    }

    fn list_archived_cards(&self) -> KanbanResult<Vec<ArchivedCard>> {
        run(self.list_archived_cards_async())
    }

    fn insert_archived_card(&self, ac: ArchivedCard) -> KanbanResult<()> {
        run(self.write_archived_card_async(&ac))
    }

    fn delete_archived_card(&self, card_id: Uuid) -> KanbanResult<()> {
        run(async {
            let mut tx = self.pool.begin().await.map_err(db_err)?;
            sqlx::query("DELETE FROM archived_cards WHERE card_id = ?")
                .bind(card_id.to_string())
                .execute(&mut *tx)
                .await
                .map_err(db_err)?;
            sqlx::query("DELETE FROM cards WHERE id = ?")
                .bind(card_id.to_string())
                .execute(&mut *tx)
                .await
                .map_err(db_err)?;
            tx.commit().await.map_err(db_err)
        })
    }

    // Archived board (C5): board row stays in `boards`; a `board_archival` marker
    // row marks it out of the live set. Reconstitute Archived<Board> by join.

    fn get_archived_board(&self, board_id: Uuid) -> KanbanResult<Option<ArchivedBoard>> {
        run(async {
            let id_str = board_id.to_string();
            let row = sqlx::query(
                "SELECT b.id, b.name, b.description, b.sprint_prefix, b.card_prefix, b.task_sort_field,
                        b.task_sort_order, b.sprint_duration_days, b.sprint_name_used_count,
                        b.next_sprint_number, b.active_sprint_id, b.task_list_view,
                        COALESCE(b.card_counter, 1) as card_counter,
                        b.completion_column_id, b.position, b.created_at, b.updated_at, ba.archived_at
                 FROM board_archival ba JOIN boards b ON ba.board_id = b.id
                 WHERE ba.board_id = ?",
            )
            .bind(&id_str)
            .fetch_optional(&self.pool)
            .await
            .map_err(db_err)?;
            match row {
                Some(row) => {
                    let (names, counters) = self.fetch_board_aux(&id_str).await?;
                    let board = row_to_board(&row, names, counters)?;
                    let at: String = row.try_get("archived_at").map_err(db_err)?;
                    Ok(Some(Archived::at(board, p_dt(&at)?)))
                }
                None => Ok(None),
            }
        })
    }

    fn list_archived_boards(&self) -> KanbanResult<Vec<ArchivedBoard>> {
        run(self.list_archived_boards_async())
    }

    fn insert_archived_board(&self, ab: ArchivedBoard) -> KanbanResult<()> {
        run(async {
            let mut tx = self.pool.begin().await.map_err(db_err)?;
            Self::write_board_with_conn(&mut tx, &ab.entity).await?;
            sqlx::query(
                "INSERT INTO board_archival (board_id, archived_at) VALUES (?, ?)
                 ON CONFLICT(board_id) DO UPDATE SET archived_at = excluded.archived_at",
            )
            .bind(ab.entity.id.to_string())
            .bind(fmt_dt(&ab.metadata.archived_at))
            .execute(&mut *tx)
            .await
            .map_err(db_err)?;
            tx.commit().await.map_err(db_err)
        })
    }

    fn delete_archived_board(&self, board_id: Uuid) -> KanbanResult<()> {
        run(async {
            let id_str = board_id.to_string();
            let mut tx = self.pool.begin().await.map_err(db_err)?;
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
            .execute(&mut *tx)
            .await
            .map_err(db_err)?;
            sqlx::query("DELETE FROM board_archival WHERE board_id = ?")
                .bind(&id_str)
                .execute(&mut *tx)
                .await
                .map_err(db_err)?;
            tx.commit().await.map_err(db_err)
        })
    }

    /// RESTORE path: drop only the archived MARKER, leaving the shared board row
    /// and its subtree intact. `delete_archived_board` above deletes the row
    /// (cascading the subtree) which is right for permanent delete but would
    /// destroy the subtree on restore (KAN-863). No-op on a live board.
    fn unarchive_board(&self, board_id: Uuid) -> KanbanResult<()> {
        run(async {
            sqlx::query("DELETE FROM board_archival WHERE board_id = ?")
                .bind(board_id.to_string())
                .execute(&self.pool)
                .await
                .map_err(db_err)?;
            Ok(())
        })
    }

    /// Board-scoped archived cards. Overrides the trait default (which filters
    /// the full list) with a direct `WHERE board_id = ?` on the extension table,
    /// so board scoping is a single indexed query.
    fn list_archived_cards_by_board(&self, board_id: Uuid) -> KanbanResult<Vec<ArchivedCard>> {
        run(async {
            let rows = sqlx::query(
                "SELECT c.id, c.column_id, c.title, c.description, c.priority, c.status,
                        c.position, c.due_date, c.points, c.card_number, c.sprint_id,
                        c.created_at, c.updated_at, c.completed_at,
                        ac.board_id, ac.archived_at, ac.original_column_id, ac.original_position
                 FROM archived_cards ac
                 JOIN cards c ON ac.card_id = c.id
                 WHERE ac.board_id = ?
                 ORDER BY ac.archived_at",
            )
            .bind(board_id.to_string())
            .fetch_all(&self.pool)
            .await
            .map_err(db_err)?;

            let card_ids: Vec<String> = rows
                .iter()
                .map(|r| r.try_get("id").map_err(db_err))
                .collect::<KanbanResult<_>>()?;
            let mut logs_map = self.fetch_sprint_logs_batch(&card_ids).await?;

            let mut result = Vec::with_capacity(rows.len());
            for row in &rows {
                let id_str: String = row.try_get("id").map_err(db_err)?;
                let logs = logs_map.remove(&id_str).unwrap_or_default();
                result.push(row_to_archived_card(row, logs)?);
            }
            Ok(result)
        })
    }

    fn clear_sprint_from_archived_cards(
        &self,
        sprint_id: Uuid,
        timestamp: DateTime<Utc>,
    ) -> KanbanResult<()> {
        run(async {
            let now = fmt_dt(&timestamp);
            sqlx::query(
                "UPDATE cards SET sprint_id = NULL, updated_at = ?
                 WHERE sprint_id = ?
                   AND EXISTS (SELECT 1 FROM archived_cards a WHERE a.card_id = cards.id)",
            )
            .bind(&now)
            .bind(sprint_id.to_string())
            .execute(&self.pool)
            .await
            .map_err(db_err)?;
            Ok(())
        })
    }

    // Sprint

    fn get_sprint(&self, id: Uuid) -> KanbanResult<Option<Sprint>> {
        run(async {
            let row = sqlx::query(
                "SELECT id, board_id, sprint_number, name_index, prefix, card_prefix,
                        status, start_date, end_date, created_at, updated_at
                 FROM sprints WHERE id = ?",
            )
            .bind(id.to_string())
            .fetch_optional(&self.pool)
            .await
            .map_err(db_err)?;
            row.as_ref().map(row_to_sprint).transpose()
        })
    }

    fn list_sprints_by_board(&self, board_id: Uuid) -> KanbanResult<Vec<Sprint>> {
        run(async {
            let rows = sqlx::query(
                "SELECT id, board_id, sprint_number, name_index, prefix, card_prefix,
                        status, start_date, end_date, created_at, updated_at
                 FROM sprints WHERE board_id = ? ORDER BY sprint_number",
            )
            .bind(board_id.to_string())
            .fetch_all(&self.pool)
            .await
            .map_err(db_err)?;
            rows.iter().map(row_to_sprint).collect()
        })
    }

    fn list_all_sprints(&self) -> KanbanResult<Vec<Sprint>> {
        run(self.list_all_sprints_async())
    }

    fn upsert_sprint(&self, sprint: Sprint) -> KanbanResult<()> {
        run(self.write_sprint_async(&sprint))
    }

    fn delete_sprint(&self, id: Uuid) -> KanbanResult<()> {
        run(async {
            sqlx::query("DELETE FROM sprints WHERE id = ?")
                .bind(id.to_string())
                .execute(&self.pool)
                .await
                .map_err(db_err)?;
            Ok(())
        })
    }

    fn delete_sprints_by_board(&self, board_id: Uuid) -> KanbanResult<()> {
        run(async {
            sqlx::query("DELETE FROM sprints WHERE board_id = ?")
                .bind(board_id.to_string())
                .execute(&self.pool)
                .await
                .map_err(db_err)?;
            Ok(())
        })
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
