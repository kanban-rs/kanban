use chrono::{DateTime, Utc};
use kanban_domain::ArchivedCard;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Response body for a board-scoped archived-card marker. Mirrors
/// `ArchivedCard`'s own flat wire shape: the card is never embedded, only
/// referenced by `entity_id`, and there is no restore-position payload since
/// an archived card stays live in place under the reference-marker model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArchivedCardResponse {
    pub entity_id: Uuid,
    pub archived_at: DateTime<Utc>,
    pub board_id: Uuid,
}

impl From<&ArchivedCard> for ArchivedCardResponse {
    fn from(ac: &ArchivedCard) -> Self {
        Self {
            entity_id: ac.entity_id,
            archived_at: ac.metadata.archived_at,
            board_id: ac.context.board_id,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> ArchivedCard {
        ArchivedCard::new(Uuid::new_v4(), Uuid::new_v4())
    }

    #[test]
    fn test_archived_card_response_round_trips_and_flattens_context() {
        let ac = sample();
        let resp = ArchivedCardResponse::from(&ac);

        let value = serde_json::to_value(&resp).unwrap();
        assert!(value.get("entity_id").is_some());
        assert!(value.get("archived_at").is_some());
        assert!(value.get("board_id").is_some());
        assert!(value.get("context").is_none(), "context must not be nested");
        assert!(
            value.get("metadata").is_none(),
            "metadata must not be nested"
        );

        let back: ArchivedCardResponse = serde_json::from_value(value).unwrap();
        assert_eq!(back, resp);
    }

    #[test]
    fn test_archived_card_response_from_archived_card_preserves_fields() {
        let ac = sample();
        let resp = ArchivedCardResponse::from(&ac);

        assert_eq!(resp.entity_id, ac.entity_id);
        assert_eq!(resp.archived_at, ac.metadata.archived_at);
        assert_eq!(resp.board_id, ac.context.board_id);
    }
}
