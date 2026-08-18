use super::super::enums::SprintStatusDto;
use chrono::{DateTime, Utc};
use kanban_domain::Sprint;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Response body for sprint reads. Hides the internal allocation state
/// (`name_index`); instead it exposes the resolved sprint `name`. `sprint_number`
/// IS exposed (it is the human-facing read-only identifier, unlike a board's
/// hidden counters). Lifecycle (`status`/dates) is exposed read-only;
/// transitions go through dedicated activate/complete/cancel endpoints.
/// `Deserialize` is derived intentionally (test round-trips / client use); the
/// server only serializes it.
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
    /// Project a sprint into its wire response given an already-resolved
    /// `name` (`name_index` is never exposed). Exhaustive destructure, no
    /// `..`, so a new `Sprint` field is a compile error here.
    pub fn new(sprint: &Sprint, name: Option<String>) -> Self {
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
    fn test_sprint_response_new_carries_the_supplied_resolved_name() {
        let sprint = Sprint::new(Uuid::new_v4(), 1, Some(0), Some("SPR"));

        let resp = SprintResponse::new(&sprint, Some("Alpha".to_string()));
        assert_eq!(resp.name, Some("Alpha".to_string()));
        assert_eq!(resp.sprint_number, 1);
        assert_eq!(resp.board_id, sprint.board_id);

        let json = serde_json::to_string(&resp).unwrap();
        assert!(
            !json.contains("name_index"),
            "SprintResponse leaked name_index: {json}"
        );
    }

    #[test]
    fn test_sprint_response_new_with_no_resolved_name_leaves_name_none() {
        let sprint = Sprint::new(Uuid::new_v4(), 2, None, None::<String>);

        let resp = SprintResponse::new(&sprint, None);
        assert_eq!(resp.name, None);
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"name\":null"), "json: {json}");
    }

    #[test]
    fn test_sprint_response_hides_name_index_and_exposes_resolved_name() {
        let board_id = Uuid::new_v4();
        let mut sprint = Sprint::new(board_id, 1, Some(0), Some("SPR"));
        sprint.card_prefix = Some("KAN".to_string());

        let resp = SprintResponse::new(&sprint, Some("Alpha".to_string()));
        assert_eq!(resp.name, Some("Alpha".to_string()));
        assert_eq!(resp.sprint_number, 1);
        assert_eq!(resp.board_id, board_id);

        let json = serde_json::to_string(&resp).unwrap();
        assert!(
            !json.contains("name_index"),
            "SprintResponse leaked name_index: {json}"
        );
    }

    #[test]
    fn test_sprint_response_uses_snake_case_status() {
        let mut sprint = Sprint::new(Uuid::new_v4(), 1, None, None::<String>);
        sprint.status = SprintStatus::Active;

        let resp = SprintResponse::new(&sprint, None);
        assert_eq!(resp.status, SprintStatusDto::Active);
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"status\":\"active\""), "json: {json}");
    }

    #[test]
    fn test_sprint_response_resolves_no_name_when_name_index_absent() {
        let sprint = Sprint::new(Uuid::new_v4(), 2, None, None::<String>);
        let resp = SprintResponse::new(&sprint, None);
        assert_eq!(resp.name, None);
    }
}
