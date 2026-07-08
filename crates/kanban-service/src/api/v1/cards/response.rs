use super::super::enums::{CardPriorityDto, CardStatusDto};
use chrono::{DateTime, Utc};
use kanban_domain::{ArchivedCard, Card};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Response body for card reads. `Deserialize` is derived intentionally (for
/// test round-trips and client/consumer use), though the server only serializes
/// it. Ids are plain `Uuid`, decoupled from the domain id aliases.
///
/// `card_number` is exposed (it is the user-facing card identifier driving
/// `KAN-5`/branch names). `sprint_logs` is intentionally hidden (internal
/// history; a history endpoint, if ever needed, gets its own DTO).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CardResponse {
    pub id: Uuid,
    pub column_id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub priority: CardPriorityDto,
    pub status: CardStatusDto,
    pub position: i32,
    pub due_date: Option<DateTime<Utc>>,
    pub points: Option<u8>,
    pub card_number: u32,
    pub sprint_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

impl From<&Card> for CardResponse {
    fn from(card: &Card) -> Self {
        let Card {
            id,
            column_id,
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
            sprint_logs: _,
        } = card;
        Self {
            id: *id,
            column_id: *column_id,
            title: title.clone(),
            description: description.clone(),
            priority: (*priority).into(),
            status: (*status).into(),
            position: *position,
            due_date: *due_date,
            points: *points,
            card_number: *card_number,
            sprint_id: *sprint_id,
            created_at: *created_at,
            updated_at: *updated_at,
            completed_at: *completed_at,
        }
    }
}

/// Response body for an archived card. Nests the rich [`CardResponse`] (not the
/// lean domain summary) and surfaces the first-class `board_id` plus the archival
/// metadata as a stable v1 contract.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArchivedCardResponse {
    pub card: CardResponse,
    /// The board the card belonged to. `None` when unknown — the domain uses a
    /// nil UUID sentinel for an archived card whose original column was already
    /// gone at backfill time; surfacing that zero-UUID would read as a real
    /// board, so it maps to `null` here.
    pub board_id: Option<Uuid>,
    pub archived_at: DateTime<Utc>,
    pub original_column_id: Uuid,
    pub original_position: i32,
}

impl From<&ArchivedCard> for ArchivedCardResponse {
    fn from(archived: &ArchivedCard) -> Self {
        // Exhaustive destructure: a future `ArchivedCard` field fails to compile
        // here until it is deliberately mapped (or omitted) in the DTO.
        let ArchivedCard {
            card,
            metadata,
            board_id,
            original_column_id,
            original_position,
        } = archived;
        Self {
            card: CardResponse::from(card),
            board_id: (!board_id.is_nil()).then_some(*board_id),
            archived_at: metadata.archived_at,
            original_column_id: *original_column_id,
            original_position: *original_position,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kanban_domain::{Card, CardPriority, NewCard};

    fn sample_card() -> Card {
        let column_id = Uuid::new_v4();
        let spec = NewCard {
            column_id,
            title: "Ship it".to_string(),
            description: Some("a card".to_string()),
            priority: CardPriority::High,
            due_date: None,
            points: Some(3),
            sprint_id: None,
        };
        Card::create(spec, Uuid::new_v4(), 5, Utc::now()).unwrap()
    }

    #[test]
    fn test_card_response_from_card_exposes_card_number_hides_sprint_logs() {
        let card = sample_card();
        let resp = CardResponse::from(&card);
        assert_eq!(resp.id, card.id);
        assert_eq!(resp.column_id, card.column_id);
        assert_eq!(resp.title, "Ship it");
        assert_eq!(resp.description, Some("a card".to_string()));
        assert_eq!(resp.priority, CardPriorityDto::High);
        assert_eq!(resp.status, CardStatusDto::Todo);
        assert_eq!(resp.position, card.position);
        assert_eq!(resp.points, Some(3));
        assert_eq!(resp.card_number, 5);
        assert_eq!(resp.completed_at, None);

        // serde round-trip equal; the serialized form has no `sprint_logs` key.
        let json = serde_json::to_string(&resp).unwrap();
        assert!(!json.contains("sprint_logs"));
        let back: CardResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(back, resp);
    }

    #[test]
    fn test_archived_card_response_carries_board_id_and_archived_meta() {
        use kanban_domain::ArchivedCard;
        let card = sample_card();
        let board_id = Uuid::new_v4();
        let original_column_id = Uuid::new_v4();
        let ac = ArchivedCard::new(card, board_id, original_column_id, 7);

        let resp = ArchivedCardResponse::from(&ac);

        // First-class board_id (B1) + the archival metadata are surfaced, and the
        // nested card is the rich CardResponse projection (not the lean summary).
        assert_eq!(resp.board_id, Some(board_id));
        assert_eq!(resp.archived_at, ac.metadata.archived_at);
        assert_eq!(resp.original_column_id, original_column_id);
        assert_eq!(resp.original_position, 7);
        assert_eq!(resp.card, CardResponse::from(&ac.card));
        // The rich card projection carries `description` (the lean summary did not).
        let json = serde_json::to_value(&resp).unwrap();
        assert!(json["card"].get("description").is_some());
    }

    #[test]
    fn test_archived_card_response_maps_nil_board_id_to_null() {
        // A nil board_id (unknown board — original column gone at backfill time)
        // must NOT surface as a zero-UUID a client would misread as a real board;
        // it maps to `None` -> serialized `null`.
        use kanban_domain::ArchivedCard;
        let ac = ArchivedCard::new(sample_card(), Uuid::nil(), Uuid::new_v4(), 0);
        let resp = ArchivedCardResponse::from(&ac);
        assert_eq!(resp.board_id, None);
        assert!(serde_json::to_value(&resp).unwrap()["board_id"].is_null());
    }
}
