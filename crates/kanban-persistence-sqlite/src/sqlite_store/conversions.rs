use std::collections::HashMap;

use kanban_domain::{
    Board, BoardRecord, Card, CardRecord, Column, ColumnRecord, KanbanResult, Sprint, SprintLog,
    SprintRecord,
};
use sqlx::sqlite::SqliteRow;
use sqlx::Row;

use super::helpers::{db_err, p_dt, p_enum, p_uuid, ser_err};

pub(crate) fn row_to_board(
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

    let record = BoardRecord {
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
    };
    Board::reconstitute(record)
}

pub(crate) fn row_to_column(row: &SqliteRow) -> KanbanResult<Column> {
    let id_str: String = row.try_get("id").map_err(db_err)?;
    let board_id_str: String = row.try_get("board_id").map_err(db_err)?;
    let created_at_str: String = row.try_get("created_at").map_err(db_err)?;
    let updated_at_str: String = row.try_get("updated_at").map_err(db_err)?;

    let record = ColumnRecord {
        id: p_uuid(&id_str)?,
        board_id: p_uuid(&board_id_str)?,
        name: row.try_get("name").map_err(db_err)?,
        position: row.try_get("position").map_err(db_err)?,
        wip_limit: row.try_get("wip_limit").map_err(db_err)?,
        created_at: p_dt(&created_at_str)?,
        updated_at: p_dt(&updated_at_str)?,
    };
    Column::reconstitute(record)
}

pub(crate) fn row_to_card(row: &SqliteRow, sprint_logs: Vec<SprintLog>) -> KanbanResult<Card> {
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

    let record = CardRecord {
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
    };

    Card::reconstitute(record)
}

pub(crate) fn row_to_sprint(row: &SqliteRow) -> KanbanResult<Sprint> {
    let id_str: String = row.try_get("id").map_err(db_err)?;
    let board_id_str: String = row.try_get("board_id").map_err(db_err)?;
    let status_str: String = row.try_get("status").map_err(db_err)?;
    let created_at_str: String = row.try_get("created_at").map_err(db_err)?;
    let updated_at_str: String = row.try_get("updated_at").map_err(db_err)?;
    let start_date_str: Option<String> = row.try_get("start_date").map_err(db_err)?;
    let end_date_str: Option<String> = row.try_get("end_date").map_err(db_err)?;
    let name_index_raw: Option<i32> = row.try_get("name_index").map_err(db_err)?;

    let record = SprintRecord {
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
    };

    Sprint::reconstitute(record)
}

/// Parse the four common edge columns (source / target / timestamps)
/// shared by `spawns_edges`, `blocks_edges`, and `relates_edges`.
pub(crate) fn row_to_edge_base(row: &SqliteRow) -> KanbanResult<kanban_core::EdgeBase> {
    let source_str: String = row.try_get("source_id").map_err(db_err)?;
    let target_str: String = row.try_get("target_id").map_err(db_err)?;
    let created_at_str: String = row.try_get("created_at").map_err(db_err)?;
    let archived_at_str: Option<String> = row.try_get("archived_at").map_err(db_err)?;
    Ok(kanban_core::EdgeBase {
        source: p_uuid(&source_str)?,
        target: p_uuid(&target_str)?,
        created_at: p_dt(&created_at_str)?,
        archived_at: archived_at_str.as_deref().map(p_dt).transpose()?,
    })
}

pub(crate) fn row_to_sprint_log(row: &SqliteRow) -> KanbanResult<SprintLog> {
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
