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
}
