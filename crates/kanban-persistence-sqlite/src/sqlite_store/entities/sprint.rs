use kanban_domain::{KanbanResult, Sprint, SprintRecord};

use crate::sqlite_store::helpers::{db_err, fmt_dt, opt_dt};
use crate::sqlite_store::SqliteStore;

impl SqliteStore {
    pub(crate) async fn write_sprint_with_conn(
        conn: &mut sqlx::SqliteConnection,
        sprint: &Sprint,
    ) -> KanbanResult<()> {
        let rec = SprintRecord::from(sprint);
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
        .bind(rec.id.to_string())
        .bind(rec.board_id.to_string())
        .bind(rec.sprint_number as i32)
        .bind(rec.name_index.map(|v| v as i32))
        .bind(&rec.prefix)
        .bind(&rec.card_prefix)
        .bind(format!("{:?}", rec.status))
        .bind(opt_dt(&rec.start_date))
        .bind(opt_dt(&rec.end_date))
        .bind(fmt_dt(&rec.created_at))
        .bind(fmt_dt(&rec.updated_at))
        .execute(&mut *conn)
        .await
        .map_err(db_err)?;
        Ok(())
    }

    pub(crate) async fn write_sprint_async(&self, sprint: &Sprint) -> KanbanResult<()> {
        Self::write_sprint_with_conn(&mut *self.pool.acquire().await.map_err(db_err)?, sprint).await
    }
}
