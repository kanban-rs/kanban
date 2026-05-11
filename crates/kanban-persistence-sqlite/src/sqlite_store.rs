use std::collections::HashMap;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use kanban_core::graph::{Edge, Graph};
use kanban_domain::data_store::DataStore;
use kanban_domain::{
    ArchivedCard, Board, Card, CardEdgeType, Column, DependencyGraph, KanbanError, KanbanResult,
    Snapshot, Sprint, SprintLog,
};
use kanban_persistence::{
    PersistenceError, PersistenceMetadata, PersistenceResult, PersistenceStore, StoreSnapshot,
};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions, SqliteRow};
use sqlx::{Pool, Row, Sqlite};
use uuid::Uuid;

const SCHEMA: &str = include_str!("schema.sql");

/// SQLite-backed persistence store using sqlx connection pool.
pub struct SqliteStore {
    pool: Pool<Sqlite>,
    path: PathBuf,
    instance_id: Uuid,
}

fn run<F: std::future::Future<Output = T>, T>(f: F) -> T {
    let handle = tokio::runtime::Handle::current();
    debug_assert!(
        handle.runtime_flavor() == tokio::runtime::RuntimeFlavor::MultiThread,
        "SqliteStore requires a multi-threaded Tokio runtime (e.g. #[tokio::main] or \
         tokio::runtime::Runtime::new()). The current_thread runtime is not supported \
         because synchronous DataStore methods need to block on async SQLite I/O."
    );
    tokio::task::block_in_place(|| handle.block_on(f))
}

fn db_err(e: sqlx::Error) -> KanbanError {
    KanbanError::Database(e.to_string())
}

fn ser_err(msg: impl std::fmt::Display) -> KanbanError {
    KanbanError::Serialization(msg.to_string())
}

fn p_uuid(s: &str) -> KanbanResult<Uuid> {
    Uuid::parse_str(s).map_err(ser_err)
}

fn p_dt(s: &str) -> KanbanResult<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(s)
        .map_err(ser_err)
        .map(|dt| dt.with_timezone(&Utc))
}

fn p_enum<T: serde::de::DeserializeOwned>(s: &str, label: &str) -> KanbanResult<T> {
    serde_json::from_value(serde_json::Value::String(s.to_owned()))
        .map_err(|_| ser_err(format!("unknown {label} variant: {s}")))
}

fn fmt_dt(dt: &DateTime<Utc>) -> String {
    dt.to_rfc3339_opts(chrono::SecondsFormat::AutoSi, true)
}

fn opt_dt(dt: &Option<DateTime<Utc>>) -> Option<String> {
    dt.as_ref().map(fmt_dt)
}

// --- Row parsers ---

fn row_to_board(
    row: &SqliteRow,
    sprint_names: Vec<String>,
    sprint_counters: HashMap<String, u32>,
) -> KanbanResult<Board> {
    let id_str: String = row.try_get("id").map_err(db_err)?;
    let active_sprint_id_str: Option<String> = row.try_get("active_sprint_id").map_err(db_err)?;
    let completion_column_id_str: Option<String> =
        row.try_get("completion_column_id").map_err(db_err)?;
    let task_sort_field_str: String = row.try_get("task_sort_field").map_err(db_err)?;
    let task_sort_order_str: String = row.try_get("task_sort_order").map_err(db_err)?;
    let task_list_view_str: String = row.try_get("task_list_view").map_err(db_err)?;
    let created_at_str: String = row.try_get("created_at").map_err(db_err)?;
    let updated_at_str: String = row.try_get("updated_at").map_err(db_err)?;
    let sprint_duration_days_raw: Option<i32> =
        row.try_get("sprint_duration_days").map_err(db_err)?;

    Ok(Board {
        id: p_uuid(&id_str)?,
        name: row.try_get("name").map_err(db_err)?,
        description: row.try_get("description").map_err(db_err)?,
        sprint_prefix: row.try_get("sprint_prefix").map_err(db_err)?,
        card_prefix: row.try_get("card_prefix").map_err(db_err)?,
        task_sort_field: p_enum(&task_sort_field_str, "task_sort_field")?,
        task_sort_order: p_enum(&task_sort_order_str, "task_sort_order")?,
        sprint_duration_days: sprint_duration_days_raw.map(|v| v as u32),
        sprint_names,
        sprint_name_used_count: row
            .try_get::<i32, _>("sprint_name_used_count")
            .map_err(db_err)? as usize,
        next_sprint_number: row
            .try_get::<i32, _>("next_sprint_number")
            .map_err(db_err)? as u32,
        active_sprint_id: active_sprint_id_str.as_deref().map(p_uuid).transpose()?,
        task_list_view: p_enum(&task_list_view_str, "task_list_view")?,
        card_counter: row.try_get::<i32, _>("card_counter").map_err(db_err)? as u32,
        sprint_counters,
        completion_column_id: completion_column_id_str
            .as_deref()
            .map(p_uuid)
            .transpose()?,
        position: row.try_get::<i32, _>("position").map_err(db_err)?,
        created_at: p_dt(&created_at_str)?,
        updated_at: p_dt(&updated_at_str)?,
    })
}

fn row_to_column(row: &SqliteRow) -> KanbanResult<Column> {
    let id_str: String = row.try_get("id").map_err(db_err)?;
    let board_id_str: String = row.try_get("board_id").map_err(db_err)?;
    let created_at_str: String = row.try_get("created_at").map_err(db_err)?;
    let updated_at_str: String = row.try_get("updated_at").map_err(db_err)?;

    Ok(Column {
        id: p_uuid(&id_str)?,
        board_id: p_uuid(&board_id_str)?,
        name: row.try_get("name").map_err(db_err)?,
        position: row.try_get("position").map_err(db_err)?,
        wip_limit: row.try_get("wip_limit").map_err(db_err)?,
        created_at: p_dt(&created_at_str)?,
        updated_at: p_dt(&updated_at_str)?,
    })
}

fn row_to_card(row: &SqliteRow, sprint_logs: Vec<SprintLog>) -> KanbanResult<Card> {
    let id_str: String = row.try_get("id").map_err(db_err)?;
    let column_id_str: String = row.try_get("column_id").map_err(db_err)?;
    let sprint_id_str: Option<String> = row.try_get("sprint_id").map_err(db_err)?;
    let created_at_str: String = row.try_get("created_at").map_err(db_err)?;
    let updated_at_str: String = row.try_get("updated_at").map_err(db_err)?;
    let completed_at_str: Option<String> = row.try_get("completed_at").map_err(db_err)?;
    let due_date_str: Option<String> = row.try_get("due_date").map_err(db_err)?;
    let priority_str: String = row.try_get("priority").map_err(db_err)?;
    let status_str: String = row.try_get("status").map_err(db_err)?;
    let points_raw: Option<i32> = row.try_get("points").map_err(db_err)?;

    Ok(Card {
        id: p_uuid(&id_str)?,
        column_id: p_uuid(&column_id_str)?,
        title: row.try_get("title").map_err(db_err)?,
        description: row.try_get("description").map_err(db_err)?,
        priority: p_enum(&priority_str, "priority")?,
        status: p_enum(&status_str, "status")?,
        position: row.try_get("position").map_err(db_err)?,
        due_date: due_date_str.as_deref().map(p_dt).transpose()?,
        points: points_raw
            .map(|v| u8::try_from(v).map_err(|_| ser_err(format!("points {v} out of range"))))
            .transpose()?,
        card_number: row.try_get::<i32, _>("card_number").map_err(db_err)? as u32,
        sprint_id: sprint_id_str.as_deref().map(p_uuid).transpose()?,
        created_at: p_dt(&created_at_str)?,
        updated_at: p_dt(&updated_at_str)?,
        completed_at: completed_at_str.as_deref().map(p_dt).transpose()?,
        sprint_logs,
    })
}

fn row_to_sprint(row: &SqliteRow) -> KanbanResult<Sprint> {
    let id_str: String = row.try_get("id").map_err(db_err)?;
    let board_id_str: String = row.try_get("board_id").map_err(db_err)?;
    let status_str: String = row.try_get("status").map_err(db_err)?;
    let created_at_str: String = row.try_get("created_at").map_err(db_err)?;
    let updated_at_str: String = row.try_get("updated_at").map_err(db_err)?;
    let start_date_str: Option<String> = row.try_get("start_date").map_err(db_err)?;
    let end_date_str: Option<String> = row.try_get("end_date").map_err(db_err)?;
    let name_index_raw: Option<i32> = row.try_get("name_index").map_err(db_err)?;

    Ok(Sprint {
        id: p_uuid(&id_str)?,
        board_id: p_uuid(&board_id_str)?,
        sprint_number: row.try_get::<i32, _>("sprint_number").map_err(db_err)? as u32,
        name_index: name_index_raw.map(|v| v as usize),
        prefix: row.try_get("prefix").map_err(db_err)?,
        card_prefix: row.try_get("card_prefix").map_err(db_err)?,
        status: p_enum(&status_str, "sprint status")?,
        start_date: start_date_str.as_deref().map(p_dt).transpose()?,
        end_date: end_date_str.as_deref().map(p_dt).transpose()?,
        created_at: p_dt(&created_at_str)?,
        updated_at: p_dt(&updated_at_str)?,
    })
}

fn rows_to_graph(rows: &[SqliteRow]) -> KanbanResult<DependencyGraph> {
    let mut graph: Graph<CardEdgeType> = Graph::new();
    for row in rows {
        let source_str: String = row.try_get("source_id").map_err(db_err)?;
        let target_str: String = row.try_get("target_id").map_err(db_err)?;
        let edge_type_str: String = row.try_get("edge_type").map_err(db_err)?;
        let direction_str: String = row.try_get("direction").map_err(db_err)?;
        let weight: Option<f64> = row.try_get("weight").map_err(db_err)?;
        let created_at_str: String = row.try_get("created_at").map_err(db_err)?;
        let archived_at_str: Option<String> = row.try_get("archived_at").map_err(db_err)?;

        graph.add_edge(Edge {
            source: p_uuid(&source_str)?,
            target: p_uuid(&target_str)?,
            edge_type: p_enum(&edge_type_str, "edge_type")?,
            direction: p_enum(&direction_str, "edge direction")?,
            weight: weight.map(|w| w as f32),
            created_at: p_dt(&created_at_str)?,
            archived_at: archived_at_str.as_deref().map(p_dt).transpose()?,
        });
    }
    Ok(DependencyGraph { cards: graph })
}

fn row_to_sprint_log(row: &SqliteRow) -> KanbanResult<SprintLog> {
    let sprint_id_str: String = row.try_get("sprint_id").map_err(db_err)?;
    let started_at_str: String = row.try_get("started_at").map_err(db_err)?;
    let ended_at_str: Option<String> = row.try_get("ended_at").map_err(db_err)?;

    Ok(SprintLog {
        sprint_id: p_uuid(&sprint_id_str)?,
        sprint_number: row.try_get::<i32, _>("sprint_number").map_err(db_err)? as u32,
        sprint_name: row.try_get("sprint_name").map_err(db_err)?,
        started_at: p_dt(&started_at_str)?,
        ended_at: ended_at_str.as_deref().map(p_dt).transpose()?,
        status: row.try_get("status").map_err(db_err)?,
    })
}

// --- SqliteStore ---

impl SqliteStore {
    pub async fn open(path: impl AsRef<Path>) -> KanbanResult<Self> {
        let handle = tokio::runtime::Handle::current();
        if handle.runtime_flavor() != tokio::runtime::RuntimeFlavor::MultiThread {
            return Err(KanbanError::Database(
                "SqliteStore requires a multi-threaded Tokio runtime (e.g. #[tokio::main] or \
                 tokio::runtime::Runtime::new()). The current_thread runtime is not supported \
                 because synchronous DataStore methods need to block on async SQLite I/O."
                    .to_string(),
            ));
        }

        let path_buf = path.as_ref().to_path_buf();

        let options = SqliteConnectOptions::new()
            .filename(&path_buf)
            .create_if_missing(true)
            .foreign_keys(true)
            .pragma("journal_mode", "wal");

        let pool = SqlitePoolOptions::new()
            .max_connections(2)
            .connect_with(options)
            .await
            .map_err(|e| KanbanError::Database(e.to_string()))?;

        sqlx::raw_sql(SCHEMA)
            .execute(&pool)
            .await
            .map_err(|e| KanbanError::Database(e.to_string()))?;

        Self::migrate(&pool).await?;

        let instance_id = Self::load_or_create_instance_id(&pool).await?;

        Ok(Self {
            pool,
            path: path_buf,
            instance_id,
        })
    }

    async fn load_or_create_instance_id(pool: &Pool<Sqlite>) -> KanbanResult<Uuid> {
        let row: Option<String> =
            sqlx::query_scalar("SELECT instance_id FROM metadata WHERE id = 1")
                .fetch_optional(pool)
                .await
                .map_err(db_err)?;
        match row {
            Some(s) => p_uuid(&s),
            None => {
                let id = Uuid::new_v4();
                let now = Utc::now().to_rfc3339();
                sqlx::query(
                    "INSERT INTO metadata (id, instance_id, saved_at, schema_version) VALUES (1, ?, ?, 1)",
                )
                .bind(id.to_string())
                .bind(&now)
                .execute(pool)
                .await
                .map_err(db_err)?;
                Ok(id)
            }
        }
    }

    async fn migrate(pool: &Pool<Sqlite>) -> KanbanResult<()> {
        // Schema version recorded in PRAGMA user_version. Every migration block
        // below is idempotent, but we skip the entire function on re-opens of
        // already-migrated databases to avoid the per-open sqlite_master / PRAGMA
        // table_info queries.
        const SCHEMA_VERSION: i64 = 1;

        let current_version: i64 = sqlx::query_scalar("PRAGMA user_version")
            .fetch_one(pool)
            .await
            .map_err(db_err)?;

        if current_version >= SCHEMA_VERSION {
            return Ok(());
        }

        // Drop command_log and undo_state tables if they exist (legacy persistence
        // of undo history — commands are now in-session only).
        let has_command_log: bool = sqlx::query_scalar(
            "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='command_log'",
        )
        .fetch_one(pool)
        .await
        .map_err(db_err)?;

        if has_command_log {
            tracing::info!(
                "dropping legacy command_log table: undo history is now in-session only and cannot be carried back to pre-KAN-405 builds"
            );
            sqlx::raw_sql("DROP TABLE IF EXISTS command_log")
                .execute(pool)
                .await
                .map_err(db_err)?;
        }

        let has_undo_state: bool = sqlx::query_scalar(
            "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='undo_state'",
        )
        .fetch_one(pool)
        .await
        .map_err(db_err)?;

        if has_undo_state {
            tracing::info!(
                "dropping legacy undo_state table: undo cursor is now in-session only and cannot be carried back to pre-KAN-405 builds"
            );
            sqlx::raw_sql("DROP TABLE IF EXISTS undo_state")
                .execute(pool)
                .await
                .map_err(db_err)?;
        }

        let has_position_col: bool = sqlx::query_scalar(
            "SELECT COUNT(*) > 0 FROM pragma_table_info('boards') WHERE name = 'position'",
        )
        .fetch_one(pool)
        .await
        .map_err(db_err)?;

        if !has_position_col {
            sqlx::raw_sql("ALTER TABLE boards ADD COLUMN position INTEGER NOT NULL DEFAULT 0")
                .execute(pool)
                .await
                .map_err(db_err)?;
        }

        let has_card_counter_col: bool = sqlx::query_scalar(
            "SELECT COUNT(*) > 0 FROM pragma_table_info('boards') WHERE name = 'card_counter'",
        )
        .fetch_one(pool)
        .await
        .map_err(db_err)?;

        if !has_card_counter_col {
            sqlx::raw_sql("ALTER TABLE boards ADD COLUMN card_counter INTEGER NOT NULL DEFAULT 1")
                .execute(pool)
                .await
                .map_err(db_err)?;
        }

        sqlx::raw_sql(&format!("PRAGMA user_version = {SCHEMA_VERSION}"))
            .execute(pool)
            .await
            .map_err(db_err)?;

        Ok(())
    }

    pub fn pool(&self) -> &Pool<Sqlite> {
        &self.pool
    }

    pub async fn checkpoint(&self) -> KanbanResult<()> {
        sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)")
            .execute(&self.pool)
            .await
            .map_err(|e| KanbanError::Database(e.to_string()))?;
        Ok(())
    }

    async fn fetch_board_aux(
        &self,
        board_id: &str,
    ) -> KanbanResult<(Vec<String>, HashMap<String, u32>)> {
        let name_rows =
            sqlx::query("SELECT name FROM board_sprint_names WHERE board_id = ? ORDER BY position")
                .bind(board_id)
                .fetch_all(&self.pool)
                .await
                .map_err(db_err)?;
        let sprint_names: Vec<String> = name_rows
            .iter()
            .map(|r| r.try_get("name").map_err(db_err))
            .collect::<KanbanResult<_>>()?;

        let counter_rows =
            sqlx::query("SELECT prefix, counter FROM board_sprint_counters WHERE board_id = ?")
                .bind(board_id)
                .fetch_all(&self.pool)
                .await
                .map_err(db_err)?;
        let mut sprint_counters = HashMap::new();
        for row in &counter_rows {
            let prefix: String = row.try_get("prefix").map_err(db_err)?;
            let counter: i32 = row.try_get("counter").map_err(db_err)?;
            sprint_counters.insert(prefix, counter as u32);
        }

        Ok((sprint_names, sprint_counters))
    }

    async fn write_board_with_conn(
        conn: &mut sqlx::SqliteConnection,
        board: &Board,
    ) -> KanbanResult<()> {
        let id = board.id.to_string();

        sqlx::query(
            "INSERT INTO boards (id, name, description, sprint_prefix, card_prefix,
                task_sort_field, task_sort_order, sprint_duration_days,
                sprint_name_used_count, next_sprint_number, active_sprint_id,
                task_list_view, card_counter, completion_column_id, position,
                created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(id) DO UPDATE SET
                name=excluded.name, description=excluded.description,
                sprint_prefix=excluded.sprint_prefix, card_prefix=excluded.card_prefix,
                task_sort_field=excluded.task_sort_field, task_sort_order=excluded.task_sort_order,
                sprint_duration_days=excluded.sprint_duration_days,
                sprint_name_used_count=excluded.sprint_name_used_count,
                next_sprint_number=excluded.next_sprint_number,
                active_sprint_id=excluded.active_sprint_id,
                task_list_view=excluded.task_list_view, card_counter=excluded.card_counter,
                completion_column_id=excluded.completion_column_id,
                position=excluded.position,
                updated_at=excluded.updated_at",
        )
        .bind(&id)
        .bind(&board.name)
        .bind(&board.description)
        .bind(&board.sprint_prefix)
        .bind(&board.card_prefix)
        .bind(format!("{:?}", board.task_sort_field))
        .bind(format!("{:?}", board.task_sort_order))
        .bind(board.sprint_duration_days.map(|v| v as i32))
        .bind(board.sprint_name_used_count as i32)
        .bind(board.next_sprint_number as i32)
        .bind(board.active_sprint_id.map(|id| id.to_string()))
        .bind(format!("{:?}", board.task_list_view))
        .bind(board.card_counter as i32)
        .bind(board.completion_column_id.map(|id| id.to_string()))
        .bind(board.position)
        .bind(fmt_dt(&board.created_at))
        .bind(fmt_dt(&board.updated_at))
        .execute(&mut *conn)
        .await
        .map_err(db_err)?;

        sqlx::query("DELETE FROM board_sprint_names WHERE board_id = ?")
            .bind(&id)
            .execute(&mut *conn)
            .await
            .map_err(db_err)?;
        for (i, name) in board.sprint_names.iter().enumerate() {
            sqlx::query(
                "INSERT INTO board_sprint_names (board_id, position, name) VALUES (?, ?, ?)",
            )
            .bind(&id)
            .bind(i as i32)
            .bind(name)
            .execute(&mut *conn)
            .await
            .map_err(db_err)?;
        }

        sqlx::query("DELETE FROM board_sprint_counters WHERE board_id = ?")
            .bind(&id)
            .execute(&mut *conn)
            .await
            .map_err(db_err)?;
        for (prefix, counter) in &board.sprint_counters {
            sqlx::query(
                "INSERT INTO board_sprint_counters (board_id, prefix, counter) VALUES (?, ?, ?)",
            )
            .bind(&id)
            .bind(prefix)
            .bind(*counter as i32)
            .execute(&mut *conn)
            .await
            .map_err(db_err)?;
        }

        Ok(())
    }

    async fn write_board_async(&self, board: &Board) -> KanbanResult<()> {
        let mut tx = self.pool.begin().await.map_err(db_err)?;
        Self::write_board_with_conn(&mut tx, board).await?;
        tx.commit().await.map_err(db_err)?;
        Ok(())
    }

    async fn fetch_sprint_logs_for_card(&self, card_id: &str) -> KanbanResult<Vec<SprintLog>> {
        let rows = sqlx::query(
            "SELECT sprint_id, sprint_number, sprint_name, started_at, ended_at, status
             FROM sprint_logs WHERE card_id = ? ORDER BY id",
        )
        .bind(card_id)
        .fetch_all(&self.pool)
        .await
        .map_err(db_err)?;
        rows.iter().map(row_to_sprint_log).collect()
    }

    async fn write_card_with_conn(
        conn: &mut sqlx::SqliteConnection,
        card: &Card,
    ) -> KanbanResult<()> {
        let id = card.id.to_string();

        sqlx::query(
            "INSERT INTO cards (id, column_id, title, description, priority, status, position,
                due_date, points, card_number, sprint_id, created_at, updated_at, completed_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(id) DO UPDATE SET
                column_id=excluded.column_id, title=excluded.title,
                description=excluded.description, priority=excluded.priority,
                status=excluded.status, position=excluded.position,
                due_date=excluded.due_date, points=excluded.points,
                card_number=excluded.card_number, sprint_id=excluded.sprint_id,
                updated_at=excluded.updated_at, completed_at=excluded.completed_at",
        )
        .bind(&id)
        .bind(card.column_id.to_string())
        .bind(&card.title)
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
            .bind(&log.status)
            .execute(&mut *conn)
            .await
            .map_err(db_err)?;
        }

        Ok(())
    }

    async fn write_card_async(&self, card: &Card) -> KanbanResult<()> {
        let mut tx = self.pool.begin().await.map_err(db_err)?;
        Self::write_card_with_conn(&mut tx, card).await?;
        tx.commit().await.map_err(db_err)?;
        Ok(())
    }

    async fn fetch_sprint_logs_batch(
        &self,
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
        let rows = query.fetch_all(&self.pool).await.map_err(db_err)?;
        let mut map: HashMap<String, Vec<SprintLog>> = HashMap::new();
        for row in &rows {
            let card_id: String = row.try_get("card_id").map_err(db_err)?;
            let log = row_to_sprint_log(row)?;
            map.entry(card_id).or_default().push(log);
        }
        Ok(map)
    }

    async fn fetch_cards_with_filter(
        &self,
        where_clause: &str,
        binds: &[String],
    ) -> KanbanResult<Vec<Card>> {
        let sql = format!(
            "SELECT id, column_id, title, description, priority, status, position,
                    due_date, points, card_number, sprint_id, created_at, updated_at, completed_at
             FROM cards WHERE id NOT IN (SELECT card_id FROM archived_cards) {}
             ORDER BY position ASC, created_at ASC",
            where_clause
        );
        let mut query = sqlx::query(&sql);
        for b in binds {
            query = query.bind(b);
        }
        let rows = query.fetch_all(&self.pool).await.map_err(db_err)?;

        let card_ids: Vec<String> = rows
            .iter()
            .map(|r| r.try_get("id").map_err(db_err))
            .collect::<KanbanResult<_>>()?;
        let mut logs_map = self.fetch_sprint_logs_batch(&card_ids).await?;

        let mut cards = Vec::with_capacity(rows.len());
        for row in &rows {
            let id_str: String = row.try_get("id").map_err(db_err)?;
            let logs = logs_map.remove(&id_str).unwrap_or_default();
            cards.push(row_to_card(row, logs)?);
        }
        Ok(cards)
    }

    async fn get_graph_with_conn(
        conn: &mut sqlx::SqliteConnection,
    ) -> KanbanResult<DependencyGraph> {
        let rows = sqlx::query(
            "SELECT source_id, target_id, edge_type, direction, weight, created_at, archived_at
             FROM card_edges",
        )
        .fetch_all(&mut *conn)
        .await
        .map_err(db_err)?;
        rows_to_graph(&rows)
    }

    async fn modify_graph_async(&self, f: kanban_domain::GraphMutFn) -> KanbanResult<()> {
        let mut tx = self.pool.begin().await.map_err(db_err)?;
        let mut graph = Self::get_graph_with_conn(&mut tx).await?;
        f(&mut graph)?;
        Self::write_graph_with_conn(&mut tx, &graph).await?;
        tx.commit().await.map_err(db_err)?;
        Ok(())
    }

    async fn write_graph_with_conn(
        conn: &mut sqlx::SqliteConnection,
        graph: &DependencyGraph,
    ) -> KanbanResult<()> {
        sqlx::query("DELETE FROM card_edges")
            .execute(&mut *conn)
            .await
            .map_err(db_err)?;

        for edge in graph.cards.edges() {
            sqlx::query(
                "INSERT INTO card_edges
                    (source_id, target_id, edge_type, direction, weight, created_at, archived_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(edge.source.to_string())
            .bind(edge.target.to_string())
            .bind(format!("{:?}", edge.edge_type))
            .bind(format!("{:?}", edge.direction))
            .bind(edge.weight.map(|w| w as f64))
            .bind(fmt_dt(&edge.created_at))
            .bind(opt_dt(&edge.archived_at))
            .execute(&mut *conn)
            .await
            .map_err(db_err)?;
        }

        Ok(())
    }

    async fn write_graph_async(&self, graph: &DependencyGraph) -> KanbanResult<()> {
        let mut tx = self.pool.begin().await.map_err(db_err)?;
        Self::write_graph_with_conn(&mut tx, graph).await?;
        tx.commit().await.map_err(db_err)?;
        Ok(())
    }

    async fn snapshot_async(&self) -> KanbanResult<Snapshot> {
        let boards = self.list_boards_async().await?;
        let columns = self.list_all_columns_async().await?;
        let cards = self.fetch_cards_with_filter("", &[]).await?;
        let archived_cards = self.list_archived_cards_async().await?;
        let sprints = self.list_all_sprints_async().await?;
        let graph = self.get_graph_async().await?;
        Ok(Snapshot::from_data(
            boards,
            columns,
            cards,
            archived_cards,
            sprints,
            graph,
        ))
    }

    async fn apply_snapshot_async(&self, snapshot: Snapshot) -> KanbanResult<()> {
        let mut tx = self.pool.begin().await.map_err(db_err)?;

        sqlx::query("PRAGMA defer_foreign_keys = ON")
            .execute(&mut *tx)
            .await
            .map_err(db_err)?;

        sqlx::query("DELETE FROM card_edges")
            .execute(&mut *tx)
            .await
            .map_err(db_err)?;
        sqlx::query("DELETE FROM archived_cards")
            .execute(&mut *tx)
            .await
            .map_err(db_err)?;
        sqlx::query("DELETE FROM sprint_logs")
            .execute(&mut *tx)
            .await
            .map_err(db_err)?;
        sqlx::query("DELETE FROM cards")
            .execute(&mut *tx)
            .await
            .map_err(db_err)?;
        sqlx::query("DELETE FROM sprints")
            .execute(&mut *tx)
            .await
            .map_err(db_err)?;
        sqlx::query("DELETE FROM board_sprint_names")
            .execute(&mut *tx)
            .await
            .map_err(db_err)?;
        sqlx::query("DELETE FROM board_sprint_counters")
            .execute(&mut *tx)
            .await
            .map_err(db_err)?;
        sqlx::query("DELETE FROM columns")
            .execute(&mut *tx)
            .await
            .map_err(db_err)?;
        sqlx::query("DELETE FROM boards")
            .execute(&mut *tx)
            .await
            .map_err(db_err)?;

        for board in &snapshot.boards {
            Self::write_board_with_conn(&mut tx, board).await?;
        }
        for column in &snapshot.columns {
            Self::write_column_with_conn(&mut tx, column).await?;
        }
        for sprint in &snapshot.sprints {
            Self::write_sprint_with_conn(&mut tx, sprint).await?;
        }
        for card in &snapshot.cards {
            Self::write_card_with_conn(&mut tx, card).await?;
        }
        for ac in &snapshot.archived_cards {
            Self::write_archived_card_with_conn(&mut tx, ac).await?;
        }
        Self::write_graph_with_conn(&mut tx, &snapshot.graph).await?;

        tx.commit().await.map_err(db_err)?;
        Ok(())
    }

    async fn fetch_all_board_aux(
        &self,
    ) -> KanbanResult<(
        HashMap<String, Vec<String>>,
        HashMap<String, HashMap<String, u32>>,
    )> {
        let name_rows = sqlx::query(
            "SELECT board_id, name FROM board_sprint_names ORDER BY board_id, position",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(db_err)?;
        let mut names_map: HashMap<String, Vec<String>> = HashMap::new();
        for row in &name_rows {
            let board_id: String = row.try_get("board_id").map_err(db_err)?;
            let name: String = row.try_get("name").map_err(db_err)?;
            names_map.entry(board_id).or_default().push(name);
        }

        let counter_rows =
            sqlx::query("SELECT board_id, prefix, counter FROM board_sprint_counters")
                .fetch_all(&self.pool)
                .await
                .map_err(db_err)?;
        let mut counters_map: HashMap<String, HashMap<String, u32>> = HashMap::new();
        for row in &counter_rows {
            let board_id: String = row.try_get("board_id").map_err(db_err)?;
            let prefix: String = row.try_get("prefix").map_err(db_err)?;
            let counter: i32 = row.try_get("counter").map_err(db_err)?;
            counters_map
                .entry(board_id)
                .or_default()
                .insert(prefix, counter as u32);
        }

        Ok((names_map, counters_map))
    }

    async fn list_boards_async(&self) -> KanbanResult<Vec<Board>> {
        let rows = sqlx::query(
            "SELECT id, name, description, sprint_prefix, card_prefix, task_sort_field,
                    task_sort_order, sprint_duration_days, sprint_name_used_count,
                    next_sprint_number, active_sprint_id, task_list_view,
                    COALESCE(card_counter, 1) as card_counter,
                    completion_column_id, position, created_at, updated_at
             FROM boards ORDER BY position ASC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(db_err)?;

        let (mut names_map, mut counters_map) = self.fetch_all_board_aux().await?;

        let mut boards = Vec::with_capacity(rows.len());
        for row in &rows {
            let id_str: String = row.try_get("id").map_err(db_err)?;
            let names = names_map.remove(&id_str).unwrap_or_default();
            let counters = counters_map.remove(&id_str).unwrap_or_default();
            boards.push(row_to_board(row, names, counters)?);
        }
        Ok(boards)
    }

    async fn list_all_columns_async(&self) -> KanbanResult<Vec<Column>> {
        let rows = sqlx::query(
            "SELECT id, board_id, name, position, wip_limit, created_at, updated_at
             FROM columns ORDER BY position",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(db_err)?;
        rows.iter().map(row_to_column).collect()
    }

    async fn list_all_sprints_async(&self) -> KanbanResult<Vec<Sprint>> {
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

    async fn list_archived_cards_async(&self) -> KanbanResult<Vec<ArchivedCard>> {
        let rows = sqlx::query(
            "SELECT c.id, c.column_id, c.title, c.description, c.priority, c.status,
                    c.position, c.due_date, c.points, c.card_number, c.sprint_id,
                    c.created_at, c.updated_at, c.completed_at,
                    ac.archived_at, ac.original_column_id, ac.original_position
             FROM archived_cards ac
             JOIN cards c ON ac.card_id = c.id
             ORDER BY ac.archived_at",
        )
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
            let card = row_to_card(row, logs)?;
            let archived_at_str: String = row.try_get("archived_at").map_err(db_err)?;
            let orig_col_str: String = row.try_get("original_column_id").map_err(db_err)?;
            result.push(ArchivedCard {
                card,
                archived_at: p_dt(&archived_at_str)?,
                original_column_id: p_uuid(&orig_col_str)?,
                original_position: row.try_get("original_position").map_err(db_err)?,
            });
        }
        Ok(result)
    }

    async fn get_graph_async(&self) -> KanbanResult<DependencyGraph> {
        let rows = sqlx::query(
            "SELECT source_id, target_id, edge_type, direction, weight, created_at, archived_at
             FROM card_edges",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(db_err)?;
        rows_to_graph(&rows)
    }

    async fn write_column_with_conn(
        conn: &mut sqlx::SqliteConnection,
        column: &Column,
    ) -> KanbanResult<()> {
        sqlx::query(
            "INSERT INTO columns (id, board_id, name, position, wip_limit, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(id) DO UPDATE SET
                board_id=excluded.board_id, name=excluded.name,
                position=excluded.position, wip_limit=excluded.wip_limit,
                updated_at=excluded.updated_at",
        )
        .bind(column.id.to_string())
        .bind(column.board_id.to_string())
        .bind(&column.name)
        .bind(column.position)
        .bind(column.wip_limit)
        .bind(fmt_dt(&column.created_at))
        .bind(fmt_dt(&column.updated_at))
        .execute(&mut *conn)
        .await
        .map_err(db_err)?;
        Ok(())
    }

    async fn write_column_async(&self, column: &Column) -> KanbanResult<()> {
        Self::write_column_with_conn(&mut *self.pool.acquire().await.map_err(db_err)?, column).await
    }

    async fn write_sprint_with_conn(
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

    async fn write_sprint_async(&self, sprint: &Sprint) -> KanbanResult<()> {
        Self::write_sprint_with_conn(&mut *self.pool.acquire().await.map_err(db_err)?, sprint).await
    }

    async fn write_archived_card_with_conn(
        conn: &mut sqlx::SqliteConnection,
        ac: &ArchivedCard,
    ) -> KanbanResult<()> {
        Self::write_card_with_conn(conn, &ac.card).await?;
        sqlx::query(
            "INSERT INTO archived_cards (card_id, archived_at, original_column_id, original_position)
             VALUES (?, ?, ?, ?)
             ON CONFLICT(card_id) DO UPDATE SET
                archived_at=excluded.archived_at,
                original_column_id=excluded.original_column_id,
                original_position=excluded.original_position",
        )
        .bind(ac.card.id.to_string())
        .bind(fmt_dt(&ac.archived_at))
        .bind(ac.original_column_id.to_string())
        .bind(ac.original_position)
        .execute(&mut *conn)
        .await
        .map_err(db_err)?;
        Ok(())
    }

    async fn write_archived_card_async(&self, ac: &ArchivedCard) -> KanbanResult<()> {
        let mut tx = self.pool.begin().await.map_err(db_err)?;
        Self::write_archived_card_with_conn(&mut tx, ac).await?;
        tx.commit().await.map_err(db_err)?;
        Ok(())
    }
}

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
                 FROM boards WHERE id = ?",
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
            sqlx::query("DELETE FROM boards WHERE id = ?")
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
            let row = sqlx::query(
                "SELECT id, column_id, title, description, priority, status, position,
                        due_date, points, card_number, sprint_id, created_at, updated_at,
                        completed_at
                 FROM cards
                 WHERE id = ? AND id NOT IN (SELECT card_id FROM archived_cards)",
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

    fn list_cards_by_sprint(&self, sprint_id: Uuid) -> KanbanResult<Vec<Card>> {
        run(self.fetch_cards_with_filter("AND sprint_id = ?", &[sprint_id.to_string()]))
    }

    fn count_cards_in_column(&self, column_id: Uuid) -> KanbanResult<usize> {
        run(async {
            let row = sqlx::query(
                "SELECT COUNT(*) as cnt FROM cards
                 WHERE column_id = ? AND id NOT IN (SELECT card_id FROM archived_cards)",
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
                   AND id NOT IN (SELECT card_id FROM archived_cards)
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
                 WHERE id = ? AND id NOT IN (SELECT card_id FROM archived_cards)",
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
                   AND id NOT IN (SELECT card_id FROM archived_cards)"
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
                   AND id NOT IN (SELECT card_id FROM archived_cards)",
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
                        ac.archived_at, ac.original_column_id, ac.original_position
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
                    let card = row_to_card(&row, logs)?;
                    let archived_at_str: String = row.try_get("archived_at").map_err(db_err)?;
                    let orig_col_str: String = row.try_get("original_column_id").map_err(db_err)?;
                    Ok(Some(ArchivedCard {
                        card,
                        archived_at: p_dt(&archived_at_str)?,
                        original_column_id: p_uuid(&orig_col_str)?,
                        original_position: row.try_get("original_position").map_err(db_err)?,
                    }))
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

    fn list_archived_cards_by_columns(
        &self,
        column_ids: &[Uuid],
    ) -> KanbanResult<Vec<ArchivedCard>> {
        if column_ids.is_empty() {
            return Ok(Vec::new());
        }
        run(async {
            let placeholders: Vec<&str> = column_ids.iter().map(|_| "?").collect();
            let sql = format!(
                "SELECT c.id, c.column_id, c.title, c.description, c.priority, c.status,
                        c.position, c.due_date, c.points, c.card_number, c.sprint_id,
                        c.created_at, c.updated_at, c.completed_at,
                        ac.archived_at, ac.original_column_id, ac.original_position
                 FROM archived_cards ac
                 JOIN cards c ON ac.card_id = c.id
                 WHERE ac.original_column_id IN ({})
                 ORDER BY ac.archived_at",
                placeholders.join(", ")
            );
            let mut query = sqlx::query(&sql);
            for id in column_ids {
                query = query.bind(id.to_string());
            }
            let rows = query.fetch_all(&self.pool).await.map_err(db_err)?;

            let card_ids: Vec<String> = rows
                .iter()
                .map(|r| r.try_get("id").map_err(db_err))
                .collect::<KanbanResult<_>>()?;
            let mut logs_map = self.fetch_sprint_logs_batch(&card_ids).await?;

            let mut result = Vec::with_capacity(rows.len());
            for row in &rows {
                let id_str: String = row.try_get("id").map_err(db_err)?;
                let logs = logs_map.remove(&id_str).unwrap_or_default();
                let card = row_to_card(row, logs)?;
                let archived_at_str: String = row.try_get("archived_at").map_err(db_err)?;
                let orig_col_str: String = row.try_get("original_column_id").map_err(db_err)?;
                result.push(ArchivedCard {
                    card,
                    archived_at: p_dt(&archived_at_str)?,
                    original_column_id: p_uuid(&orig_col_str)?,
                    original_position: row.try_get("original_position").map_err(db_err)?,
                });
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
                   AND id IN (SELECT card_id FROM archived_cards)",
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

#[async_trait::async_trait]
impl PersistenceStore for SqliteStore {
    async fn save(&self, snapshot: StoreSnapshot) -> PersistenceResult<PersistenceMetadata> {
        let domain_snapshot: Snapshot = serde_json::from_slice(&snapshot.data)
            .map_err(|e| PersistenceError::Serialization(e.to_string()))?;
        self.apply_snapshot_async(domain_snapshot)
            .await
            .map_err(|e| PersistenceError::Database(e.to_string()))?;
        self.checkpoint()
            .await
            .map_err(|e| PersistenceError::Database(e.to_string()))?;
        Ok(PersistenceMetadata::new(self.instance_id))
    }

    async fn load(&self) -> PersistenceResult<(StoreSnapshot, PersistenceMetadata)> {
        let domain_snapshot = self
            .snapshot_async()
            .await
            .map_err(|e| PersistenceError::Database(e.to_string()))?;
        let data = serde_json::to_vec(&domain_snapshot)
            .map_err(|e| PersistenceError::Serialization(e.to_string()))?;
        let meta = PersistenceMetadata::new(self.instance_id);
        Ok((
            StoreSnapshot {
                data,
                metadata: meta.clone(),
            },
            meta,
        ))
    }

    async fn exists(&self) -> bool {
        self.path.exists()
    }

    fn path(&self) -> &std::path::Path {
        &self.path
    }

    fn instance_id(&self) -> Uuid {
        self.instance_id
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_rt() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap()
    }

    #[test]
    fn test_sqlitestore_path_is_preserved() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("test.db");
        let rt = make_rt();
        let store = rt.block_on(SqliteStore::open(&path)).unwrap();
        assert_eq!(store.path(), path.as_path());
    }

    #[test]
    fn test_sqlitestore_instance_id_persists_across_reopen() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("test.db");
        let rt = make_rt();
        let id1 = rt.block_on(SqliteStore::open(&path)).unwrap().instance_id();
        let id2 = rt.block_on(SqliteStore::open(&path)).unwrap().instance_id();
        assert_eq!(id1, id2, "instance_id must be stable across reopens");
    }

    #[test]
    fn test_sqlitestore_persistence_save_load_roundtrip() {
        use kanban_domain::{Board, DependencyGraph};
        use kanban_persistence::snapshot_to_json_bytes;

        let dir = TempDir::new().unwrap();
        let path = dir.path().join("test.db");
        let rt = make_rt();

        rt.block_on(async {
            let store = SqliteStore::open(&path).await.unwrap();
            let board = Board::new("Test Board".to_string(), None);
            let snapshot = Snapshot::from_data(
                vec![board],
                vec![],
                vec![],
                vec![],
                vec![],
                DependencyGraph::new(),
            );
            let data = snapshot_to_json_bytes(&snapshot).unwrap();
            let meta = PersistenceMetadata::new(store.instance_id());
            let store_snap = StoreSnapshot {
                data,
                metadata: meta,
            };

            PersistenceStore::save(&store, store_snap).await.unwrap();

            let (loaded_snap, _meta) = PersistenceStore::load(&store).await.unwrap();
            let loaded: Snapshot = serde_json::from_slice(&loaded_snap.data).unwrap();
            assert_eq!(loaded.boards.len(), 1);
            assert_eq!(loaded.boards[0].name, "Test Board");
        });
    }

    #[test]
    fn test_sqlitestore_exists_returns_true_after_open() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("test.db");
        let rt = make_rt();
        rt.block_on(async {
            let store = SqliteStore::open(&path).await.unwrap();
            assert!(PersistenceStore::exists(&store).await);
        });
    }

    #[test]
    fn test_checkpoint_executes_without_error() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("test.sqlite3");
        let rt = make_rt();
        rt.block_on(async {
            let store = SqliteStore::open(&path).await.unwrap();
            store.checkpoint().await.unwrap();
        });
    }

    #[test]
    fn test_save_checkpoints_wal_file_stays_minimal() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("test.sqlite3");
        let rt = make_rt();
        rt.block_on(async {
            let store = SqliteStore::open(&path).await.unwrap();
            let (snapshot, _) = PersistenceStore::load(&store).await.unwrap();
            PersistenceStore::save(&store, snapshot).await.unwrap();
            let wal_path = path.with_extension("sqlite3-wal");
            if wal_path.exists() {
                assert!(
                    wal_path.metadata().unwrap().len() < 32 * 1024,
                    "WAL file should be minimal after save+checkpoint"
                );
            }
        });
    }

    #[test]
    fn test_delete_archived_card_orphaned_cards_row_is_still_cleaned_up() {
        use kanban_domain::data_store::DataStore;
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("test.sqlite3");
        let rt = make_rt();
        rt.block_on(async {
            let store = SqliteStore::open(&path).await.unwrap();

            let mut board = kanban_domain::Board::new("B".to_string(), None);
            let column = kanban_domain::Column::new(board.id, "Col".to_string(), 0);
            let card = kanban_domain::Card::new(&mut board, column.id, "Task".to_string(), 0);
            let card_id = card.id;
            let column_id = column.id;
            store.upsert_board(board).unwrap();
            store.upsert_column(column).unwrap();
            store.upsert_card(card.clone()).unwrap();

            // Insert into archived_cards WITHOUT calling delete_card first,
            // leaving an orphaned row in the cards table.
            let archived = kanban_domain::ArchivedCard::new(card, column_id, 0);
            store.insert_archived_card(archived).unwrap();

            store.delete_archived_card(card_id).unwrap();

            assert!(
                store.list_archived_cards().unwrap().is_empty(),
                "card should be gone from archived_cards"
            );
            assert!(
                store.list_all_cards().unwrap().is_empty(),
                "orphaned cards row should also be removed by delete_archived_card"
            );
        });
    }

    #[test]
    fn test_delete_archived_card_removes_from_cards_table() {
        use kanban_domain::data_store::DataStore;
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("test.sqlite3");
        let rt = make_rt();
        rt.block_on(async {
            let store = SqliteStore::open(&path).await.unwrap();

            let mut board = kanban_domain::Board::new("B".to_string(), None);
            let column = kanban_domain::Column::new(board.id, "Col".to_string(), 0);
            let card = kanban_domain::Card::new(&mut board, column.id, "Task".to_string(), 0);
            let card_id = card.id;
            let column_id = column.id;
            store.upsert_board(board).unwrap();
            store.upsert_column(column).unwrap();
            store.upsert_card(card.clone()).unwrap();

            let archived = kanban_domain::ArchivedCard::new(card, column_id, 0);
            store.insert_archived_card(archived).unwrap();
            store.delete_card(card_id).unwrap();

            assert_eq!(store.list_archived_cards().unwrap().len(), 1);

            store.delete_archived_card(card_id).unwrap();

            assert!(
                store.list_archived_cards().unwrap().is_empty(),
                "card should be gone from archived_cards"
            );
            assert!(
                store.list_all_cards().unwrap().is_empty(),
                "card should also be gone from cards table, not restored as active"
            );
            assert!(
                store.get_card(card_id).unwrap().is_none(),
                "get_card should return None for permanently deleted card"
            );
        });
    }
}
