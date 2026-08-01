use kanban_api::{BoardResponse, CardResponse, ColumnResponse};
use kanban_domain::{Board, BoardId, Card, Column};
use std::collections::HashMap;

/// Reconstructs a `Board` from the wire `BoardResponse`.
///
/// `BoardResponse` intentionally omits internal allocation state
/// (`card_counter`, `next_sprint_number`, `sprint_counters`, `sprint_names`,
/// `sprint_name_used_count`) and no endpoint exposes them, so they are
/// defaulted to zero/empty here. Safe today because every `HttpBackend` write
/// path is still `unsupported()` -- nothing can mint a card/sprint number off
/// these defaults. Revisit before implementing any write path: a real write
/// against a faked counter risks colliding with existing card/sprint numbers.
pub(crate) fn board_from_response(r: BoardResponse) -> Board {
    Board {
        id: r.id,
        name: r.name,
        description: r.description,
        sprint_prefix: r.sprint_prefix,
        card_prefix: r.card_prefix,
        task_sort_field: r.task_sort_field.into(),
        task_sort_order: r.task_sort_order.into(),
        sprint_duration_days: r.sprint_duration_days,
        sprint_names: Vec::new(),
        sprint_name_used_count: 0,
        next_sprint_number: 1,
        active_sprint_id: r.active_sprint_id,
        task_list_view: r.task_list_view.into(),
        card_counter: 0,
        sprint_counters: HashMap::new(),
        completion_column_id: r.completion_column_id,
        position: r.position,
        created_at: r.created_at,
        updated_at: r.updated_at,
    }
}

/// Reconstructs a `Column` from the wire `ColumnResponse`. Lossless --
/// `ColumnResponse` carries every `Column` field.
pub(crate) fn column_from_response(r: ColumnResponse) -> Column {
    Column {
        id: r.id,
        board_id: r.board_id,
        name: r.name,
        position: r.position,
        wip_limit: r.wip_limit,
        created_at: r.created_at,
        updated_at: r.updated_at,
    }
}

/// Reconstructs a `Card` from the wire `CardResponse`. `board_id` isn't on
/// the wire in any form -- `CardResponse` never carries it, on the flat or
/// board-scoped route alike -- so callers must supply it from context (the
/// board they queried to reach this card). `sprint_logs` has no read endpoint
/// at all and is always empty on a remotely-fetched `Card`.
pub(crate) fn card_from_response(r: CardResponse, board_id: BoardId) -> Card {
    Card {
        id: r.id,
        column_id: r.column_id,
        board_id,
        title: r.title,
        description: r.description,
        priority: r.priority.into(),
        status: r.status.into(),
        position: r.position,
        due_date: r.due_date,
        points: r.points,
        card_number: r.card_number,
        sprint_id: r.sprint_id,
        created_at: r.created_at,
        updated_at: r.updated_at,
        completed_at: r.completed_at,
        sprint_logs: Vec::new(),
    }
}
