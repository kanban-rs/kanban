use kanban_domain::{Column, ColumnRecord, KanbanResult};

use crate::sqlite_store::helpers::{db_err, fmt_dt, required_str};
use crate::sqlite_store::SqliteStore;

impl SqliteStore {
    pub(crate) async fn write_column_with_conn(
        conn: &mut sqlx::SqliteConnection,
        column: &Column,
    ) -> KanbanResult<()> {
        let rec = ColumnRecord::from(column);
        sqlx::query(
            "INSERT INTO columns (id, board_id, name, position, wip_limit, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(id) DO UPDATE SET
                board_id=excluded.board_id, name=excluded.name,
                position=excluded.position, wip_limit=excluded.wip_limit,
                updated_at=excluded.updated_at",
        )
        .bind(rec.id.to_string())
        .bind(rec.board_id.to_string())
        .bind(required_str(&rec.name, "column.name")?)
        .bind(rec.position)
        .bind(rec.wip_limit)
        .bind(fmt_dt(&rec.created_at))
        .bind(fmt_dt(&rec.updated_at))
        .execute(&mut *conn)
        .await
        .map_err(db_err)?;
        Ok(())
    }

    pub(crate) async fn write_column_async(&self, column: &Column) -> KanbanResult<()> {
        let column = column.clone();
        self.db_conn(|conn| {
            Box::pin(async move { Self::write_column_with_conn(conn, &column).await })
        })
        .await
    }
}
