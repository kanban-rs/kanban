use super::super::{SortFieldDto, SortOrderDto, TaskListViewDto};
use chrono::{DateTime, Utc};
use kanban_domain::Board;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Response body for board reads. Omits internal allocation state
/// (`card_counter`, `next_sprint_number`, `sprint_counters`, `sprint_names`,
/// `sprint_name_used_count`); `active_sprint_id`/`position` are read-only.
/// Enums use the decoupled wire mirrors (snake_case); ids are plain `Uuid`.
/// `Deserialize` is derived intentionally (test round-trips / client use); the
/// server only serializes it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BoardResponse {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub sprint_prefix: Option<String>,
    pub card_prefix: Option<String>,
    pub task_sort_field: SortFieldDto,
    pub task_sort_order: SortOrderDto,
    pub sprint_duration_days: Option<u32>,
    pub task_list_view: TaskListViewDto,
    pub active_sprint_id: Option<Uuid>,
    pub completion_column_id: Option<Uuid>,
    pub position: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    /// `Some` iff this board is archived (the marker's `archived_at`); `None`
    /// for a live board. Skipped on the wire when `None` so live-board payloads
    /// are byte-identical to before this field existed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub archived_at: Option<DateTime<Utc>>,
}

impl BoardResponse {
    /// Project a live board and stamp it as archived at `archived_at`. Under the
    /// reference-marker model an archived board IS a live board plus a marker, so
    /// the archived wire shape is the live projection with `archived_at` set.
    pub fn archived(board: &Board, archived_at: DateTime<Utc>) -> Self {
        Self {
            archived_at: Some(archived_at),
            ..Self::from(board)
        }
    }
}

impl From<&Board> for BoardResponse {
    fn from(b: &Board) -> Self {
        Self {
            id: b.id,
            name: b.name.clone(),
            description: b.description.clone(),
            sprint_prefix: b.sprint_prefix.clone(),
            card_prefix: b.card_prefix.clone(),
            task_sort_field: b.task_sort_field.into(),
            task_sort_order: b.task_sort_order.into(),
            sprint_duration_days: b.sprint_duration_days,
            task_list_view: b.task_list_view.into(),
            active_sprint_id: b.active_sprint_id,
            completion_column_id: b.completion_column_id,
            position: b.position,
            created_at: b.created_at,
            updated_at: b.updated_at,
            archived_at: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_board_response_from_ref_omits_internal_state_and_uses_snake_case_enums() {
        let board = Board::new("Test", Some("KAN"));
        let resp = BoardResponse::from(&board);
        assert_eq!(resp.id, board.id);
        assert_eq!(resp.name, "Test");
        let json = serde_json::to_string(&resp).unwrap();
        for hidden in [
            "card_counter",
            "next_sprint_number",
            "sprint_counters",
            "sprint_names",
            "sprint_name_used_count",
        ] {
            assert!(
                !json.contains(hidden),
                "BoardResponse leaked {hidden}: {json}"
            );
        }
        // Decoupled wire enums serialize snake_case (default view is Flat):
        assert!(json.contains("\"task_list_view\":\"flat\""), "json: {json}");
    }

    // D2 (KAN-880): BoardResponse gains an optional `archived_at` so the live
    // response is the single wire type for both live and archived boards. Live
    // payloads stay byte-identical (the key is skipped when absent).
    #[test]
    fn test_board_response_from_board_has_null_archived_at() {
        let resp = BoardResponse::from(&Board::new("B", Some("KAN")));
        assert_eq!(resp.archived_at, None);
    }

    #[test]
    fn test_board_response_archived_stamps_archived_at() {
        let board = Board::new("B", Some("KAN"));
        let at = Utc::now();
        let archived = BoardResponse::archived(&board, at);
        assert_eq!(archived.archived_at, Some(at));
        assert_eq!(
            BoardResponse {
                archived_at: None,
                ..archived.clone()
            },
            BoardResponse::from(&board)
        );
    }

    #[test]
    fn test_board_response_archived_at_serde_round_trip() {
        let archived = BoardResponse::archived(&Board::new("B", Some("KAN")), Utc::now());
        let json = serde_json::to_string(&archived).unwrap();
        assert!(json.contains("archived_at"));
        let back: BoardResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(back, archived);
    }

    #[test]
    fn test_board_response_live_omits_archived_at_key() {
        let live = BoardResponse::from(&Board::new("B", Some("KAN")));
        let value = serde_json::to_value(&live).unwrap();
        assert!(
            value.get("archived_at").is_none(),
            "a live board payload must not carry an archived_at key"
        );
    }

    // B3 (KAN-919): `with_archived_at` is the single constructor a board is
    // built through — it carries the optional marker timestamp, so both live
    // (`None`) and archived (`Some`) boards share one shape and one code path.
    #[test]
    fn test_board_response_omits_archived_at_when_live() {
        let live = BoardResponse::with_archived_at(&Board::new("B", Some("KAN")), None);
        assert_eq!(live.archived_at, None);
        let value = serde_json::to_value(&live).unwrap();
        assert!(
            value.get("archived_at").is_none(),
            "a live board built via with_archived_at must not carry an archived_at key"
        );
    }

    #[test]
    fn test_board_response_includes_archived_at_when_archived() {
        use chrono::{TimeZone, Utc};
        let at = Utc.timestamp_opt(1_700_000_000, 0).unwrap();
        let resp = BoardResponse::with_archived_at(&Board::new("B", Some("KAN")), Some(at));
        assert_eq!(resp.archived_at, Some(at));
        let value = serde_json::to_value(&resp).unwrap();
        assert!(
            value.get("archived_at").is_some(),
            "an archived board must carry the archived_at key with its timestamp"
        );
    }

    #[test]
    fn test_board_response_archived_projects_board_and_archived_at() {
        use chrono::{TimeZone, Utc};
        let board = Board::new("Archived", Some("ARC"));
        let board_id = board.id;
        let at = Utc.timestamp_opt(1_700_000_000, 0).unwrap();

        // Reference-marker model: the archived wire shape is the live board
        // projection stamped with `archived_at` (no separate nested DTO).
        let resp = BoardResponse::archived(&board, at);

        assert_eq!(resp.id, board_id);
        assert_eq!(resp.name, "Archived");
        assert_eq!(resp.archived_at, Some(at));

        let json = serde_json::to_string(&resp).unwrap();
        let back: BoardResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(back, resp);
    }
}
