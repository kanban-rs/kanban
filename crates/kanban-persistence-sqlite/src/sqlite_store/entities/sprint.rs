use kanban_domain::{KanbanResult, Sprint};

use crate::sqlite_store::helpers::{db_err, fmt_dt, opt_dt};
use crate::sqlite_store::SqliteStore;

impl SqliteStore {
    pub(crate) async fn write_sprint_with_conn(
        conn: &mut sqlx::SqliteConnection,
        sprint: &Sprint,
    ) -> KanbanResult<()> {
        sqlx::query(
            "INSERT INTO sprints (id, board_id, sprint_number, name_index, prefix, card_prefix,
                status, start_date, end_date, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(id) DO UPDATE SET
                board_id=excluded.board_id, sprint_number=excluded.sprint_number,
                name_index=excluded.name_index, prefix=excluded.prefix,
                card_prefix=excluded.card_prefix, status=excluded.status,
                start_date=excluded.start_date, end_date=excluded.end_date,
                updated_at=excluded.updated_at",
        )
        .bind(sprint.id.to_string())
        .bind(sprint.board_id.to_string())
        .bind(sprint.sprint_number as i32)
        .bind(sprint.name_index.map(|v| v as i32))
        .bind(&sprint.prefix)
        .bind(&sprint.card_prefix)
        .bind(format!("{:?}", sprint.status))
        .bind(opt_dt(&sprint.start_date))
        .bind(opt_dt(&sprint.end_date))
        .bind(fmt_dt(&sprint.created_at))
        .bind(fmt_dt(&sprint.updated_at))
        .execute(&mut *conn)
        .await
        .map_err(db_err)?;
        Ok(())
    }

    pub(crate) async fn write_sprint_async(&self, sprint: &Sprint) -> KanbanResult<()> {
        Self::write_sprint_with_conn(&mut *self.pool.acquire().await.map_err(db_err)?, sprint).await
    }
}
