use kanban_domain::{ArchivedCard, Board, Column, DependencyGraph, KanbanResult, Sprint};
use sqlx::Row;

use super::conversions::{row_to_archived_card, row_to_board, row_to_column, row_to_sprint};
use super::helpers::db_err;
use super::SqliteStore;

impl SqliteStore {
    pub(crate) async fn list_boards_async(&self) -> KanbanResult<Vec<Board>> {
        self.db_conn(|conn| {
            Box::pin(async move {
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
                .fetch_all(&mut *conn)
                .await
                .map_err(db_err)?;

                let (mut names_map, mut counters_map) = Self::fetch_all_board_aux_with_conn(conn).await?;

                let mut boards = Vec::with_capacity(rows.len());
                for row in &rows {
                    let id_str: String = row.try_get("id").map_err(db_err)?;
                    let names = names_map.remove(&id_str).unwrap_or_default();
                    let counters = counters_map.remove(&id_str).unwrap_or_default();
                    boards.push(row_to_board(row, names, counters)?);
                }
                Ok(boards)
            })
        })
        .await
    }

    /// ALL board heads, live AND archived (unfiltered). Snapshot/export fidelity:
    /// under the reference-marker model an archived board's head stays in `boards`
    /// and must be carried in `snapshot.boards`, with archived-ness recorded
    /// separately via the `board_archival` markers.
    pub(crate) async fn all_boards_async(&self) -> KanbanResult<Vec<Board>> {
        self.db_conn(|conn| {
            Box::pin(async move {
                let rows = sqlx::query(
                    "SELECT id, name, description, sprint_prefix, card_prefix, task_sort_field,
                            task_sort_order, sprint_duration_days, sprint_name_used_count,
                            next_sprint_number, active_sprint_id, task_list_view,
                            COALESCE(card_counter, 1) as card_counter,
                            position, created_at, updated_at
                     FROM boards
                     ORDER BY position ASC, created_at ASC, id ASC",
                )
                .fetch_all(&mut *conn)
                .await
                .map_err(db_err)?;

                let (mut names_map, mut counters_map) =
                    Self::fetch_all_board_aux_with_conn(conn).await?;

                let mut boards = Vec::with_capacity(rows.len());
                for row in &rows {
                    let id_str: String = row.try_get("id").map_err(db_err)?;
                    let names = names_map.remove(&id_str).unwrap_or_default();
                    let counters = counters_map.remove(&id_str).unwrap_or_default();
                    boards.push(row_to_board(row, names, counters)?);
                }
                Ok(boards)
            })
        })
        .await
    }

    pub(crate) async fn list_all_columns_async(&self) -> KanbanResult<Vec<Column>> {
        self.db_conn(|conn| {
            Box::pin(async move {
                let rows = sqlx::query(
                    "SELECT id, board_id, name, position, wip_limit, default_status, created_at, updated_at
                     FROM columns ORDER BY position ASC, created_at ASC, id ASC",
                )
                .fetch_all(&mut *conn)
                .await
                .map_err(db_err)?;
                rows.iter().map(row_to_column).collect()
            })
        })
        .await
    }

    pub(crate) async fn list_all_sprints_async(&self) -> KanbanResult<Vec<Sprint>> {
        self.db_conn(|conn| {
            Box::pin(async move {
                let rows = sqlx::query(
                    "SELECT id, board_id, sprint_number, name_index, prefix, card_prefix,
                            status, start_date, end_date, created_at, updated_at
                     FROM sprints ORDER BY sprint_number",
                )
                .fetch_all(&mut *conn)
                .await
                .map_err(db_err)?;
                rows.iter().map(row_to_sprint).collect()
            })
        })
        .await
    }

    pub(crate) async fn list_archived_cards_async(&self) -> KanbanResult<Vec<ArchivedCard>> {
        // Reference-marker model: markers only. No card JOIN — the live cards stay
        // in `cards` and are read there when their fields are needed.
        self.db_conn(|conn| {
            Box::pin(async move {
                let rows = sqlx::query(
                    "SELECT card_id AS id, board_id, archived_at
                     FROM archived_cards
                     ORDER BY archived_at",
                )
                .fetch_all(&mut *conn)
                .await
                .map_err(db_err)?;

                rows.iter().map(row_to_archived_card).collect()
            })
        })
        .await
    }

    pub(crate) async fn get_graph_async(&self) -> KanbanResult<DependencyGraph> {
        self.db_conn(|conn| Box::pin(async move { Self::get_graph_with_conn(conn).await }))
            .await
    }
}
