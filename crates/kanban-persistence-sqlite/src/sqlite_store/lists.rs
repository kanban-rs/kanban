use kanban_domain::{ArchivedCard, Board, Column, DependencyGraph, KanbanResult, Sprint};
use sqlx::Row;

use super::conversions::{row_to_archived_card, row_to_board, row_to_column, row_to_sprint};
use super::helpers::db_err;
use super::SqliteStore;

impl SqliteStore {
    pub(crate) async fn list_boards_async(&self) -> KanbanResult<Vec<Board>> {
        let rows = sqlx::query(
            "SELECT id, name, description, sprint_prefix, card_prefix, task_sort_field,
                    task_sort_order, sprint_duration_days, sprint_name_used_count,
                    next_sprint_number, active_sprint_id, task_list_view,
                    COALESCE(card_counter, 1) as card_counter,
                    position, created_at, updated_at
             FROM boards
             WHERE NOT EXISTS (SELECT 1 FROM board_archival ba WHERE ba.board_id = boards.id)
             ORDER BY position ASC, created_at ASC, id ASC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(db_err)?;

        let (mut names_map, mut counters_map, mut completion_map) =
            self.fetch_all_board_aux().await?;

        let mut boards = Vec::with_capacity(rows.len());
        for row in &rows {
            let id_str: String = row.try_get("id").map_err(db_err)?;
            let names = names_map.remove(&id_str).unwrap_or_default();
            let counters = counters_map.remove(&id_str).unwrap_or_default();
            let completion = completion_map.remove(&id_str).unwrap_or_default();
            boards.push(row_to_board(row, names, counters, completion)?);
        }
        Ok(boards)
    }

    /// ALL board heads, live AND archived (unfiltered). Snapshot/export fidelity:
    /// under the reference-marker model an archived board's head stays in `boards`
    /// and must be carried in `snapshot.boards`, with archived-ness recorded
    /// separately via the `board_archival` markers.
    pub(crate) async fn all_boards_async(&self) -> KanbanResult<Vec<Board>> {
        let rows = sqlx::query(
            "SELECT id, name, description, sprint_prefix, card_prefix, task_sort_field,
                    task_sort_order, sprint_duration_days, sprint_name_used_count,
                    next_sprint_number, active_sprint_id, task_list_view,
                    COALESCE(card_counter, 1) as card_counter,
                    position, created_at, updated_at
             FROM boards
             ORDER BY position ASC, created_at ASC, id ASC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(db_err)?;

        let (mut names_map, mut counters_map, mut completion_map) =
            self.fetch_all_board_aux().await?;

        let mut boards = Vec::with_capacity(rows.len());
        for row in &rows {
            let id_str: String = row.try_get("id").map_err(db_err)?;
            let names = names_map.remove(&id_str).unwrap_or_default();
            let counters = counters_map.remove(&id_str).unwrap_or_default();
            let completion = completion_map.remove(&id_str).unwrap_or_default();
            boards.push(row_to_board(row, names, counters, completion)?);
        }
        Ok(boards)
    }

    pub(crate) async fn list_all_columns_async(&self) -> KanbanResult<Vec<Column>> {
        let rows = sqlx::query(
            "SELECT id, board_id, name, position, wip_limit, created_at, updated_at
             FROM columns ORDER BY position ASC, created_at ASC, id ASC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(db_err)?;
        rows.iter().map(row_to_column).collect()
    }

    pub(crate) async fn list_all_sprints_async(&self) -> KanbanResult<Vec<Sprint>> {
        let rows = sqlx::query(
            "SELECT id, board_id, sprint_number, name_index, prefix, card_prefix,
                    status, start_date, end_date, created_at, updated_at
             FROM sprints ORDER BY sprint_number",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(db_err)?;
        rows.iter().map(row_to_sprint).collect()
    }

    pub(crate) async fn list_archived_cards_async(&self) -> KanbanResult<Vec<ArchivedCard>> {
        // Reference-marker model: markers only. No card JOIN — the live cards stay
        // in `cards` and are read there when their fields are needed.
        let rows = sqlx::query(
            "SELECT card_id AS id, board_id, archived_at
             FROM archived_cards
             ORDER BY archived_at",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(db_err)?;

        rows.iter().map(row_to_archived_card).collect()
    }

    pub(crate) async fn get_graph_async(&self) -> KanbanResult<DependencyGraph> {
        // Wrap the three per-kind edge reads in a single transaction so
        // a concurrent writer between query 1 (spawns) and query 3
        // (relates) cannot yield an inconsistent in-memory snapshot.
        // SQLite under WAL gives the transaction a stable read view; the
        // tx is read-only and is committed without writes.
        let mut tx = self.pool.begin().await.map_err(db_err)?;
        let graph = Self::get_graph_with_conn(&mut tx).await?;
        tx.commit().await.map_err(db_err)?;
        Ok(graph)
    }
}
