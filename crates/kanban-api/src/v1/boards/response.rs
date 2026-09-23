use super::super::{SortFieldDto, SortOrderDto, TaskListViewDto};
use chrono::{DateTime, Utc};
use kanban_domain::Board;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

fn default_next_sprint_number() -> u32 {
    1
}

/// Response body for board reads. Carries the board's sprint-name pool
/// (`sprint_names`, `sprint_name_used_count`, `next_sprint_number`) so a client
/// converting this into a domain `Board` can resolve a sprint's name, which
/// `Sprint` stores as an index into that pool rather than as a string. Sprint
/// numbers are NOT allocated from `next_sprint_number`; allocation reads the
/// `prefixes` row's `sprint_counter`, and every sprint write over HTTP is
/// declined, so the field is carried for fidelity only.
/// `active_sprint_id`/`position` are read-only.
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
    pub position: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    /// Defaulted on deserialize so a newer client keeps working against a
    /// server that predates these fields; it degrades to unnamed sprints
    /// rather than failing every board read.
    #[serde(default)]
    pub sprint_names: Vec<String>,
    #[serde(default)]
    pub sprint_name_used_count: usize,
    #[serde(default = "default_next_sprint_number")]
    pub next_sprint_number: u32,
    /// `Some` iff this board is archived (the marker's `archived_at`); `None`
    /// for a live board. Skipped on the wire when `None` so live-board payloads
    /// are byte-identical to before this field existed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub archived_at: Option<DateTime<Utc>>,
}

impl BoardResponse {
    /// Project a board and stamp its optional `archived_at`. A board is one
    /// shape: `None` yields the live projection (the wire key is skipped), `Some`
    /// stamps the marker's timestamp. This is the single constructor callers
    /// build a board through; `from(&Board)` is `with_archived_at(board, None)`.
    pub fn with_archived_at(board: &Board, archived_at: Option<DateTime<Utc>>) -> Self {
        Self {
            archived_at,
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
            position: b.position,
            created_at: b.created_at,
            updated_at: b.updated_at,
            sprint_names: b.sprint_names.clone(),
            sprint_name_used_count: b.sprint_name_used_count,
            next_sprint_number: b.next_sprint_number,
            archived_at: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_board_response_serializes_no_completion_column_key() {
        let board = Board::new("Test", Some("KAN"));
        let resp = BoardResponse::from(&board);
        let json = serde_json::to_string(&resp).unwrap();
        assert!(
            !json.contains("completion_column_id"),
            "BoardResponse must serialize neither completion_column_id nor \
             completion_column_ids (the singular is a substring of the plural, so \
             this one check covers both): {json}"
        );
    }

    #[test]
    fn test_board_response_from_ref_uses_snake_case_enums() {
        let board = Board::new("Test", Some("KAN"));
        let resp = BoardResponse::from(&board);
        assert_eq!(resp.id, board.id);
        assert_eq!(resp.name, "Test");
        let json = serde_json::to_string(&resp).unwrap();
        // Decoupled wire enums serialize snake_case (default view is Flat):
        assert!(json.contains("\"task_list_view\":\"flat\""), "json: {json}");
    }

    #[test]
    fn test_board_response_round_trips_sprint_naming_state() {
        let mut board = Board::new("Test", Some("KAN"));
        board.sprint_names = vec!["alpha".to_string(), "an-eventful-october".to_string()];
        board.sprint_name_used_count = 1;
        board.next_sprint_number = 7;

        let resp = BoardResponse::from(&board);
        assert_eq!(resp.sprint_names, board.sprint_names);
        assert_eq!(resp.sprint_name_used_count, board.sprint_name_used_count);
        assert_eq!(resp.next_sprint_number, board.next_sprint_number);

        let json = serde_json::to_string(&resp).unwrap();
        let back: BoardResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(back, resp);
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
        let archived = BoardResponse::with_archived_at(&board, Some(at));
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
        let archived =
            BoardResponse::with_archived_at(&Board::new("B", Some("KAN")), Some(Utc::now()));
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
        let resp = BoardResponse::with_archived_at(&board, Some(at));

        assert_eq!(resp.id, board_id);
        assert_eq!(resp.name, "Archived");
        assert_eq!(resp.archived_at, Some(at));

        let json = serde_json::to_string(&resp).unwrap();
        let back: BoardResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(back, resp);
    }
}
