use sqlx::Row;

use kanban_domain::{Archived, ArchivedBoard, KanbanResult};

use super::helpers::{db_err, p_dt, p_uuid};
use super::SqliteStore;

impl SqliteStore {
    pub(crate) async fn list_archived_boards_with_conn(
        conn: &mut sqlx::SqliteConnection,
    ) -> KanbanResult<Vec<ArchivedBoard>> {
        let rows = sqlx::query(
            "SELECT board_id, archived_at FROM board_archival ORDER BY archived_at ASC",
        )
        .fetch_all(&mut *conn)
        .await
        .map_err(db_err)?;
        let mut out = Vec::with_capacity(rows.len());
        for row in &rows {
            let id_str: String = row.try_get("board_id").map_err(db_err)?;
            let at: String = row.try_get("archived_at").map_err(db_err)?;
            out.push(Archived::at(p_uuid(&id_str)?, p_dt(&at)?));
        }
        Ok(out)
    }
}
