use super::super::enums::SprintStatusDto;
use chrono::{DateTime, Utc};
use kanban_domain::{Board, Sprint};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Response body for sprint reads. Hides the internal allocation state
/// (`name_index`); instead it exposes the resolved sprint `name`, looked up
/// against the owning board at projection time. `sprint_number` IS exposed (it
/// is the human-facing read-only identifier, unlike a board's hidden counters).
/// Lifecycle (`status`/dates) is exposed read-only; transitions go through
/// dedicated activate/complete/cancel endpoints. `Deserialize` is derived
/// intentionally (test round-trips / client use); the server only serializes it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SprintResponse {
    pub id: Uuid,
    pub board_id: Uuid,
    pub sprint_number: u32,
    pub name: Option<String>,
    pub prefix: Option<String>,
    pub card_prefix: Option<String>,
    pub status: SprintStatusDto,
    pub start_date: Option<DateTime<Utc>>,
    pub end_date: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl SprintResponse {
    /// Project a sprint into its wire response, resolving the `name` against the
    /// owning `board`'s name pool (`name_index` is never exposed). Exhaustive
    /// destructure — no `..` — so a new `Sprint` field is a compile error here.
    pub fn from_sprint(sprint: &Sprint, board: &Board) -> Self {
        let name = sprint.get_name(board).map(str::to_string);
        let Sprint {
            id,
            board_id,
            sprint_number,
            name_index: _,
            prefix,
            card_prefix,
            status,
            start_date,
            end_date,
            created_at,
            updated_at,
        } = sprint;
        Self {
            id: *id,
            board_id: *board_id,
            sprint_number: *sprint_number,
            name,
            prefix: prefix.clone(),
            card_prefix: card_prefix.clone(),
            status: (*status).into(),
            start_date: *start_date,
            end_date: *end_date,
            created_at: *created_at,
            updated_at: *updated_at,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kanban_domain::SprintStatus;

    #[test]
    fn test_sprint_response_hides_name_index_and_exposes_resolved_name() {
        let mut board = Board::new("Test", Some("KAN"));
        let idx = board.add_sprint_name_at_used_index("Alpha");
        let mut sprint = Sprint::new(board.id, 1, Some(idx), Some("SPR"));
        sprint.card_prefix = Some("KAN".to_string());

        let resp = SprintResponse::from_sprint(&sprint, &board);
        assert_eq!(resp.name, Some("Alpha".to_string()));
        assert_eq!(resp.sprint_number, 1);
        assert_eq!(resp.board_id, board.id);

        let json = serde_json::to_string(&resp).unwrap();
        assert!(
            !json.contains("name_index"),
            "SprintResponse leaked name_index: {json}"
        );
    }

    #[test]
    fn test_sprint_response_uses_snake_case_status() {
        let board = Board::new("Test", Some("KAN"));
        let mut sprint = Sprint::new(board.id, 1, None, None::<String>);
        sprint.status = SprintStatus::Active;

        let resp = SprintResponse::from_sprint(&sprint, &board);
        assert_eq!(resp.status, SprintStatusDto::Active);
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"status\":\"active\""), "json: {json}");
    }

    #[test]
    fn test_sprint_response_resolves_no_name_when_name_index_absent() {
        let board = Board::new("Test", Some("KAN"));
        let sprint = Sprint::new(board.id, 2, None, None::<String>);
        let resp = SprintResponse::from_sprint(&sprint, &board);
        assert_eq!(resp.name, None);
    }
}
