use kanban_api::{
    ArchivedBoardResponse, ArchivedCardResponse, BoardResponse, CardResponse, ColumnResponse,
    PrefixResponse, SprintResponse,
};
use kanban_domain::{
    ArchiveMetadata, Archived, ArchivedBoard, ArchivedCard, Board, Card, CardRestoreContext,
    Column, Prefix, Sprint,
};

pub(crate) fn archived_card_from_response(resp: &ArchivedCardResponse) -> ArchivedCard {
    Archived::with_context(
        resp.entity_id,
        CardRestoreContext {
            board_id: resp.board_id,
        },
        ArchiveMetadata::at(resp.archived_at),
    )
}

/// `None` for a live card: `CardResponse::archived_at` is `Some` iff archived.
pub(crate) fn archived_card_from_card_response(resp: &CardResponse) -> Option<ArchivedCard> {
    resp.archived_at.map(|archived_at| {
        Archived::with_context(
            resp.id,
            CardRestoreContext {
                board_id: resp.board_id,
            },
            ArchiveMetadata::at(archived_at),
        )
    })
}

pub(crate) fn archived_board_from_response(resp: &ArchivedBoardResponse) -> ArchivedBoard {
    ArchivedBoard::at(resp.entity_id, resp.archived_at)
}

pub(crate) fn prefix_from_response(resp: &PrefixResponse) -> Prefix {
    let mut prefix = Prefix::new(&resp.name);
    prefix.card_counter = resp.last_card_number;
    prefix.sprint_counter = resp.last_sprint_number;
    prefix
}

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
        sprint_names,
        sprint_name_used_count,
        next_sprint_number,
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
        sprint_names,
        sprint_name_used_count,
        next_sprint_number,
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

/// `SprintResponse` exposes a resolved `name` in place of `name_index`; this
/// re-derives the index by position within the owning board's `sprint_names`
/// pool, so the caller must pass that same pool (from `BoardResponse`).
pub(crate) fn sprint_from_response(resp: &SprintResponse, sprint_names: &[String]) -> Sprint {
    let SprintResponse {
        id,
        board_id,
        sprint_number,
        name,
        prefix,
        card_prefix,
        status,
        start_date,
        end_date,
        created_at,
        updated_at,
    } = resp.clone();
    let name_index = name.and_then(|n| sprint_names.iter().position(|candidate| *candidate == n));
    Sprint {
        id,
        board_id,
        sprint_number,
        name_index,
        prefix,
        card_prefix,
        status: status.into(),
        start_date,
        end_date,
        created_at,
        updated_at,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use kanban_api::{SortFieldDto, SortOrderDto, SprintStatusDto, TaskListViewDto};
    use uuid::Uuid;

    fn board_response(sprint_names: Vec<String>, used: usize, next: u32) -> BoardResponse {
        BoardResponse {
            id: Uuid::new_v4(),
            name: "Board".to_string(),
            description: None,
            sprint_prefix: None,
            card_prefix: None,
            task_sort_field: SortFieldDto::Default,
            task_sort_order: SortOrderDto::Ascending,
            sprint_duration_days: None,
            task_list_view: TaskListViewDto::Flat,
            active_sprint_id: None,
            position: 0,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            sprint_names,
            sprint_name_used_count: used,
            next_sprint_number: next,
            archived_at: None,
        }
    }

    fn sprint_response(board_id: Uuid, name: Option<String>) -> SprintResponse {
        SprintResponse {
            id: Uuid::new_v4(),
            board_id,
            sprint_number: 1,
            name,
            prefix: None,
            card_prefix: None,
            status: SprintStatusDto::Planning,
            start_date: None,
            end_date: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn test_board_from_response_carries_the_sprint_name_pool() {
        let resp = board_response(vec!["an-eventful-october".to_string()], 1, 2);

        let board = board_from_response(&resp);

        assert_eq!(board.sprint_names, vec!["an-eventful-october".to_string()]);
        assert_eq!(board.sprint_name_used_count, 1);
        assert_eq!(board.next_sprint_number, 2);
    }

    #[test]
    fn test_sprint_converted_from_response_resolves_its_name() {
        let board_id = Uuid::new_v4();
        let board_resp = board_response(
            vec!["alpha".to_string(), "an-eventful-october".to_string()],
            2,
            3,
        );
        let sprint_resp = sprint_response(board_id, Some("an-eventful-october".to_string()));

        let board = board_from_response(&board_resp);
        let sprint = sprint_from_response(&sprint_resp, &board_resp.sprint_names);

        assert_eq!(sprint.get_name(&board), Some("an-eventful-october"));
    }

    #[test]
    fn test_sprint_converted_from_response_with_no_name_leaves_name_index_none() {
        let board_id = Uuid::new_v4();
        let sprint_resp = sprint_response(board_id, None);

        let sprint = sprint_from_response(&sprint_resp, &[]);

        assert_eq!(sprint.name_index, None);
    }
}
