use std::collections::HashMap;

use kanban_domain::{Board, BoardRecord, KanbanResult};
use sqlx::Row;

use crate::sqlite_store::helpers::{db_err, fmt_dt, required_str};
use crate::sqlite_store::SqliteStore;

impl SqliteStore {
    pub(crate) async fn fetch_board_aux_with_conn(
        conn: &mut sqlx::SqliteConnection,
        board_id: &str,
    ) -> KanbanResult<Vec<String>> {
        let name_rows =
            sqlx::query("SELECT name FROM board_sprint_names WHERE board_id = ? ORDER BY position")
                .bind(board_id)
                .fetch_all(&mut *conn)
                .await
                .map_err(db_err)?;
        let sprint_names: Vec<String> = name_rows
            .iter()
            .map(|r| r.try_get("name").map_err(db_err))
            .collect::<KanbanResult<_>>()?;

        Ok(sprint_names)
    }

    pub(crate) async fn write_board_with_conn(
        conn: &mut sqlx::SqliteConnection,
        board: &Board,
    ) -> KanbanResult<()> {
        let rec = BoardRecord::from(board);
        let id = rec.id.to_string();

        sqlx::query(
            "INSERT INTO boards (id, name, description, sprint_prefix, card_prefix,
                task_sort_field, task_sort_order, sprint_duration_days,
                sprint_name_used_count, next_sprint_number, active_sprint_id,
                task_list_view, position,
                created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(id) DO UPDATE SET
                name=excluded.name, description=excluded.description,
                sprint_prefix=excluded.sprint_prefix, card_prefix=excluded.card_prefix,
                task_sort_field=excluded.task_sort_field, task_sort_order=excluded.task_sort_order,
                sprint_duration_days=excluded.sprint_duration_days,
                sprint_name_used_count=excluded.sprint_name_used_count,
                next_sprint_number=excluded.next_sprint_number,
                active_sprint_id=excluded.active_sprint_id,
                task_list_view=excluded.task_list_view,
                position=excluded.position,
                updated_at=excluded.updated_at",
        )
        .bind(&id)
        .bind(required_str(&rec.name, "board.name")?)
        .bind(&rec.description)
        .bind(&rec.sprint_prefix)
        .bind(&rec.card_prefix)
        .bind(format!("{:?}", rec.task_sort_field))
        .bind(format!("{:?}", rec.task_sort_order))
        .bind(rec.sprint_duration_days.map(|v| v as i32))
        .bind(rec.sprint_name_used_count as i32)
        .bind(rec.next_sprint_number as i32)
        .bind(rec.active_sprint_id.map(|id| id.to_string()))
        .bind(format!("{:?}", rec.task_list_view))
        .bind(rec.position)
        .bind(fmt_dt(&rec.created_at))
        .bind(fmt_dt(&rec.updated_at))
        .execute(&mut *conn)
        .await
        .map_err(db_err)?;

        sqlx::query("DELETE FROM board_sprint_names WHERE board_id = ?")
            .bind(&id)
            .execute(&mut *conn)
            .await
            .map_err(db_err)?;
        for (i, name) in rec.sprint_names.iter().enumerate() {
            sqlx::query(
                "INSERT INTO board_sprint_names (board_id, position, name) VALUES (?, ?, ?)",
            )
            .bind(&id)
            .bind(i as i32)
            .bind(required_str(name, "board.sprint_names[*]")?)
            .execute(&mut *conn)
            .await
            .map_err(db_err)?;
        }

        Ok(())
    }

    pub(crate) async fn write_board_async(&self, board: &Board) -> KanbanResult<()> {
        let board = board.clone();
        self.db_conn(|conn| {
            Box::pin(async move { Self::write_board_with_conn(conn, &board).await })
        })
        .await
    }

    pub(crate) async fn fetch_all_board_aux_with_conn(
        conn: &mut sqlx::SqliteConnection,
    ) -> KanbanResult<HashMap<String, Vec<String>>> {
        let name_rows = sqlx::query(
            "SELECT board_id, name FROM board_sprint_names ORDER BY board_id, position",
        )
        .fetch_all(&mut *conn)
        .await
        .map_err(db_err)?;
        let mut names_map: HashMap<String, Vec<String>> = HashMap::new();
        for row in &name_rows {
            let board_id: String = row.try_get("board_id").map_err(db_err)?;
            let name: String = row.try_get("name").map_err(db_err)?;
            names_map.entry(board_id).or_default().push(name);
        }

        Ok(names_map)
    }
}
