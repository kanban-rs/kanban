use kanban_api::{BoardResponse, CardResponse, ColumnResponse, PrefixResponse, SprintResponse};
use kanban_domain::{Board, Card, Column, Prefix, Sprint};

pub(crate) fn prefix_from_response(resp: &PrefixResponse) -> Prefix {
    let mut prefix = Prefix::new(&resp.name);
    prefix.card_counter = resp.last_card_number;
    prefix.sprint_counter = resp.last_sprint_number;
    prefix
}

/// `BoardResponse` omits `sprint_names`, `sprint_name_used_count` and
/// `next_sprint_number`; they are defaulted here to empty/zero/one. A board
/// converted through this path cannot allocate sprint numbers or resolve
/// sprint names -- that allocation state lives only on the server.
pub(crate) fn board_from_response(resp: &BoardResponse) -> Board {
    let BoardResponse {
        id,
        name,
        description,
        sprint_prefix,
        card_prefix,
        task_sort_field,
        task_sort_order,
        sprint_duration_days,
        task_list_view,
        active_sprint_id,
        position,
        created_at,
        updated_at,
        archived_at: _,
    } = resp.clone();
    Board {
        id,
        name,
        description,
        sprint_prefix,
        card_prefix,
        task_sort_field: task_sort_field.into(),
        task_sort_order: task_sort_order.into(),
        sprint_duration_days,
        sprint_names: Vec::new(),
        sprint_name_used_count: 0,
        next_sprint_number: 1,
        active_sprint_id,
        task_list_view: task_list_view.into(),
        position,
        created_at,
        updated_at,
    }
}

pub(crate) fn column_from_response(resp: &ColumnResponse) -> Column {
    let ColumnResponse {
        id,
        board_id,
        name,
        position,
        wip_limit,
        default_status,
        created_at,
        updated_at,
    } = resp.clone();
    Column {
        id,
        board_id,
        name,
        position,
        wip_limit,
        default_status: default_status.map(Into::into),
        created_at,
        updated_at,
    }
}

/// No read endpoint exposes card history, so `sprint_logs` is always empty
/// on a card converted through this path.
pub(crate) fn card_from_response(resp: &CardResponse) -> Card {
    let CardResponse {
        id,
        column_id,
        board_id,
        prefix,
        title,
        description,
        priority,
        status,
        position,
        due_date,
        points,
        card_number,
        sprint_id,
        created_at,
        updated_at,
        completed_at,
        archived_at: _,
    } = resp.clone();
    Card {
        id,
        column_id,
        board_id,
        title,
        description,
        priority: priority.into(),
        status: status.into(),
        position,
        due_date,
        points,
        card_number,
        prefix,
        sprint_id,
        created_at,
        updated_at,
        completed_at,
        sprint_logs: Vec::new(),
    }
}

/// `SprintResponse` exposes a resolved `name` in place of `name_index`, so
/// the index is unrecoverable here and a sprint converted through this path
/// always renders unnamed.
pub(crate) fn sprint_from_response(resp: &SprintResponse) -> Sprint {
    let SprintResponse {
        id,
        board_id,
        sprint_number,
        name: _,
        prefix,
        card_prefix,
        status,
        start_date,
        end_date,
        created_at,
        updated_at,
    } = resp.clone();
    Sprint {
        id,
        board_id,
        sprint_number,
        name_index: None,
        prefix,
        card_prefix,
        status: status.into(),
        start_date,
        end_date,
        created_at,
        updated_at,
    }
}
