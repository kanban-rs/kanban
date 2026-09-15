use chrono::{DateTime, Utc};
use kanban_domain::ArchivedBoard;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Response body for an archived-board marker. A board is a scoping root, so
/// unlike `ArchivedCardResponse` there is no restore context (no `board_id`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArchivedBoardResponse {
    pub entity_id: Uuid,
    pub archived_at: DateTime<Utc>,
}

impl From<&ArchivedBoard> for ArchivedBoardResponse {
    fn from(ab: &ArchivedBoard) -> Self {
        Self {
            entity_id: ab.entity_id,
            archived_at: ab.metadata.archived_at,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ArchivedBoardResponse;
    use chrono::Utc;
    use kanban_domain::ArchivedBoard;
    use uuid::Uuid;

    #[test]
    fn test_archived_board_response_round_trips_and_flattens_metadata() {
        let id = Uuid::new_v4();
        let ts = Utc::now();
        let ab = ArchivedBoard::at(id, ts);
        let resp = ArchivedBoardResponse::from(&ab);

        let value = serde_json::to_value(&resp).unwrap();
        assert!(value.get("entity_id").is_some());
        assert!(value.get("archived_at").is_some());
        assert!(
            value.get("metadata").is_none(),
            "metadata must not be nested"
        );
        assert!(value.get("context").is_none(), "context must not be nested");
        assert!(
            value.get("board_id").is_none(),
            "a board marker is NoContext, unlike ArchivedCardResponse"
        );

        let back: ArchivedBoardResponse = serde_json::from_value(value).unwrap();
        assert_eq!(back, resp);
    }

    #[test]
    fn test_archived_board_response_from_marker_preserves_fields() {
        use chrono::{TimeZone, Utc};
        let id = Uuid::new_v4();
        let ts = Utc.timestamp_opt(1_700_000_000, 0).unwrap();
        let ab = ArchivedBoard::at(id, ts);
        let resp = ArchivedBoardResponse::from(&ab);

        assert_eq!(resp.entity_id, ab.entity_id);
        assert_eq!(resp.archived_at, ab.metadata.archived_at);
    }
}
