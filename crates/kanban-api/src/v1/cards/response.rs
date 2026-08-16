use super::super::enums::{CardPriorityDto, CardStatusDto};
use chrono::{DateTime, Utc};
use kanban_domain::Card;
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
    /// `Some` iff this card is archived (the marker's `archived_at`); `None` for
    /// a live card. Skipped on the wire when `None` so live-card payloads are
    /// byte-identical to before this field existed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub archived_at: Option<DateTime<Utc>>,
}

impl CardResponse {
    /// Build a card projection stamping the archival marker's `archived_at`.
    /// `None` yields the live projection (the wire key is skipped); `Some`
    /// marks the card archived. `from(&Card)` is `with_archived_at(card, None)`.
    pub fn with_archived_at(card: &Card, archived_at: Option<DateTime<Utc>>) -> Self {
        Self {
            archived_at,
            ..Self::from(card)
        }
    }
}

impl From<&Card> for CardResponse {
    fn from(card: &Card) -> Self {
        let Card {
            id,
            column_id,
            board_id: _,
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
            prefix: _,
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
            archived_at: None,
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
        Card::create(
            spec,
            Uuid::new_v4(),
            5,
            "task".to_string(),
            Utc::now(),
            Uuid::new_v4(),
        )
        .unwrap()
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

    // D1 (KAN-879): CardResponse gains an optional `archived_at` so the live
    // response is the single wire type for both live and archived cards. Live
    // payloads stay byte-identical (the key is skipped when absent).
    #[test]
    fn test_card_response_from_card_has_null_archived_at() {
        let resp = CardResponse::from(&sample_card());
        assert_eq!(resp.archived_at, None);
    }

    #[test]
    fn test_card_response_archived_stamps_archived_at() {
        let card = sample_card();
        let at = Utc::now();
        // Production stamps the marker's `archived_at` onto the live projection.
        let archived = CardResponse {
            archived_at: Some(at),
            ..CardResponse::from(&card)
        };
        assert_eq!(archived.archived_at, Some(at));
        // Every other field matches the live projection.
        assert_eq!(
            CardResponse {
                archived_at: None,
                ..archived.clone()
            },
            CardResponse::from(&card)
        );
    }

    #[test]
    fn test_card_response_archived_at_serde_round_trip() {
        let archived = CardResponse {
            archived_at: Some(Utc::now()),
            ..CardResponse::from(&sample_card())
        };
        let json = serde_json::to_string(&archived).unwrap();
        assert!(json.contains("archived_at"));
        let back: CardResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(back, archived);
    }

    #[test]
    fn test_card_response_live_omits_archived_at_key() {
        let live = CardResponse::from(&sample_card());
        let value = serde_json::to_value(&live).unwrap();
        assert!(
            value.get("archived_at").is_none(),
            "a live card payload must not carry an archived_at key"
        );
    }
}
