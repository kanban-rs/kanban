use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::board::BoardId;
use crate::card::{Card, CardId, CardPriority, CardStatus};
use crate::column::ColumnId;
use crate::error::KanbanResult;
use crate::SprintLog;

/// Client-settable CREATE fields only. Default-free. Never built with `..`.
///
/// `column_id` and `sprint_id` are FK values (not objects); FK existence is
/// validated at the service tier. `card_number` is NOT a field here: it is a
/// `Card::create` parameter the service mints from the Board counter, keeping
/// `create` Board-free.
#[derive(Debug, Clone, PartialEq)]
pub struct NewCard {
    pub column_id: ColumnId,
    pub title: String,
    pub description: Option<String>,
    pub priority: CardPriority,
    pub due_date: Option<DateTime<Utc>>,
    pub points: Option<u8>,
    pub sprint_id: Option<Uuid>,
}

/// COMPLETE field set. The Card type carrying the persistence wire shape.
/// Default-free, distinct from `Card`; exhaustively destructured in
/// `reconstitute` + `From<&Card>`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CardRecord {
    pub id: CardId,
    pub column_id: ColumnId,
    #[serde(default)]
    pub board_id: BoardId,
    pub title: String,
    pub description: Option<String>,
    pub priority: CardPriority,
    pub status: CardStatus,
    pub position: i32,
    pub due_date: Option<DateTime<Utc>>,
    pub points: Option<u8>,
    #[serde(default)]
    pub card_number: u32,
    /// Defaulted so records written before cards carried a prefix still
    /// deserialize; the migration backfills the real value.
    #[serde(default)]
    pub prefix: String,
    #[serde(default)]
    pub sprint_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[serde(default)]
    pub completed_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub sprint_logs: Vec<SprintLog>,
}

impl Card {
    /// Construct a brand-new card from client-supplied CREATE fields plus an
    /// injected id, server-minted `card_number`, clock, and the owning
    /// `board_id` (resolved from `spec.column_id` by the caller — a durable
    /// value stored directly on `Card`, not re-derived from the column on
    /// every read; see KAN-963). No internal `Uuid::new_v4`/`Utc::now`/`Board`
    /// access. Seeds `status = Todo`, `completed_at = None`, an empty
    /// `sprint_logs`, and `created_at == updated_at == now`. Sprint-log
    /// seeding (which needs a Sprint object) stays in the service tier even
    /// when `sprint_id` is set.
    pub fn create(
        spec: NewCard,
        id: CardId,
        card_number: u32,
        prefix: String,
        now: DateTime<Utc>,
        board_id: BoardId,
    ) -> KanbanResult<Card> {
        let NewCard {
            column_id,
            title,
            description,
            priority,
            due_date,
            points,
            sprint_id,
        } = spec;
        Ok(Card {
            id,
            column_id,
            board_id,
            title,
            description,
            priority,
            status: CardStatus::Todo,
            position: 0,
            due_date,
            points,
            card_number,
            prefix: crate::prefix::Prefix::normalize(&prefix),
            sprint_id,
            created_at: now,
            updated_at: now,
            completed_at: None,
            sprint_logs: Vec::new(),
        })
    }

    /// Rebuild a card from a persisted record. Structural validation only;
    /// invariants are assumed to have held when the record was written.
    pub fn reconstitute(record: CardRecord) -> KanbanResult<Card> {
        let CardRecord {
            id,
            column_id,
            board_id,
            title,
            description,
            priority,
            status,
            position,
            due_date,
            points,
            card_number,
            prefix,
            sprint_id,
            created_at,
            updated_at,
            completed_at,
            sprint_logs,
        } = record;
        Ok(Card {
            id,
            column_id,
            board_id,
            title,
            description,
            priority,
            status,
            position,
            due_date,
            points,
            card_number,
            prefix,
            sprint_id,
            created_at,
            updated_at,
            completed_at,
            sprint_logs,
        })
    }
}

impl From<&Card> for CardRecord {
    fn from(card: &Card) -> Self {
        let Card {
            id,
            column_id,
            board_id,
            title,
            description,
            priority,
            status,
            position,
            due_date,
            points,
            card_number,
            prefix,
            sprint_id,
            created_at,
            updated_at,
            completed_at,
            sprint_logs,
        } = card;
        CardRecord {
            id: *id,
            column_id: *column_id,
            board_id: *board_id,
            title: title.clone(),
            description: description.clone(),
            priority: *priority,
            status: *status,
            position: *position,
            due_date: *due_date,
            points: *points,
            card_number: *card_number,
            prefix: prefix.clone(),
            sprint_id: *sprint_id,
            created_at: *created_at,
            updated_at: *updated_at,
            completed_at: *completed_at,
            sprint_logs: sprint_logs.clone(),
        }
    }
}

/// Serde adapter for a single `Card` field, routing bytes through `CardRecord`
/// so construction always funnels through `Card::reconstitute` and
/// serialization through the `CardRecord` decompose. Used via
/// `#[serde(with = "card_serde")]`.
pub mod card_serde {
    use super::{Card, CardRecord};
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S>(card: &Card, s: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        CardRecord::from(card).serialize(s)
    }

    pub fn deserialize<'de, D>(d: D) -> Result<Card, D::Error>
    where
        D: Deserializer<'de>,
    {
        let record = CardRecord::deserialize(d)?;
        Card::reconstitute(record).map_err(serde::de::Error::custom)
    }
}

/// Serde adapter for a `Vec<Card>` field, routing every element through
/// `CardRecord`. Used via `#[serde(with = "card_vec_serde")]`.
pub mod card_vec_serde {
    use super::{Card, CardRecord};
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S>(cards: &[Card], s: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let records: Vec<CardRecord> = cards.iter().map(CardRecord::from).collect();
        records.serialize(s)
    }

    pub fn deserialize<'de, D>(d: D) -> Result<Vec<Card>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let records = Vec::<CardRecord>::deserialize(d)?;
        records
            .into_iter()
            .map(Card::reconstitute)
            .collect::<crate::error::KanbanResult<Vec<Card>>>()
            .map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod factory_tests {
    use super::*;

    fn fixed_now() -> DateTime<Utc> {
        "2024-01-01T00:00:00Z".parse::<DateTime<Utc>>().unwrap()
    }

    fn spec() -> NewCard {
        NewCard {
            column_id: Uuid::new_v4(),
            title: "Implement feature".to_string(),
            description: Some("details".to_string()),
            priority: CardPriority::High,
            due_date: Some("2024-06-01T00:00:00Z".parse().unwrap()),
            points: Some(5),
            sprint_id: None,
        }
    }

    #[test]
    fn test_card_create_seeds_server_managed_defaults() -> KanbanResult<()> {
        let id = Uuid::new_v4();
        let now = fixed_now();
        let card = Card::create(spec(), id, 7, "task".to_string(), now, Uuid::new_v4())?;
        assert_eq!(card.status, CardStatus::Todo);
        assert_eq!(card.completed_at, None);
        assert!(card.sprint_logs.is_empty());
        assert_eq!(card.created_at, now);
        assert_eq!(card.updated_at, now);
        assert_eq!(card.id, id);
        assert_eq!(card.card_number, 7);
        Ok(())
    }

    #[test]
    fn test_card_create_applies_client_fields_from_spec() -> KanbanResult<()> {
        let column_id = Uuid::new_v4();
        let board_id = Uuid::new_v4();
        let sprint_id = Uuid::new_v4();
        let due = "2024-06-01T00:00:00Z".parse::<DateTime<Utc>>().unwrap();
        let card = Card::create(
            NewCard {
                column_id,
                title: "Title".to_string(),
                description: Some("desc".to_string()),
                priority: CardPriority::Critical,
                due_date: Some(due),
                points: Some(8),
                sprint_id: Some(sprint_id),
            },
            Uuid::new_v4(),
            1,
            "task".to_string(),
            fixed_now(),
            board_id,
        )?;
        assert_eq!(card.column_id, column_id);
        assert_eq!(
            card.board_id, board_id,
            "board_id is carried through, not derived from spec"
        );
        assert_eq!(card.title, "Title");
        assert_eq!(card.description, Some("desc".to_string()));
        assert_eq!(card.priority, CardPriority::Critical);
        assert_eq!(card.due_date, Some(due));
        assert_eq!(card.points, Some(8));
        assert_eq!(card.sprint_id, Some(sprint_id));
        Ok(())
    }

    #[test]
    fn test_card_create_does_not_seed_sprint_log_for_present_sprint_id() -> KanbanResult<()> {
        let sprint_id = Uuid::new_v4();
        let card = Card::create(
            NewCard {
                column_id: Uuid::new_v4(),
                title: "t".to_string(),
                description: None,
                priority: CardPriority::Medium,
                due_date: None,
                points: None,
                sprint_id: Some(sprint_id),
            },
            Uuid::new_v4(),
            1,
            "task".to_string(),
            fixed_now(),
            Uuid::new_v4(),
        )?;
        assert_eq!(card.sprint_id, Some(sprint_id));
        assert!(card.sprint_logs.is_empty());
        Ok(())
    }

    #[test]
    fn test_card_create_is_deterministic_for_fixed_id_number_and_clock() -> KanbanResult<()> {
        let id = Uuid::new_v4();
        let now = fixed_now();
        let s = spec();
        let board_id = Uuid::new_v4();
        let a = Card::create(s.clone(), id, 3, "task".to_string(), now, board_id)?;
        let b = Card::create(s, id, 3, "task".to_string(), now, board_id)?;
        assert_eq!(a, b);
        Ok(())
    }

    fn populated_record() -> CardRecord {
        let sprint_id = Uuid::new_v4();
        CardRecord {
            id: Uuid::new_v4(),
            column_id: Uuid::new_v4(),
            board_id: Uuid::new_v4(),
            title: "Done card".to_string(),
            description: Some("finished".to_string()),
            priority: CardPriority::Low,
            status: CardStatus::Done,
            position: 7,
            due_date: Some("2024-05-05T00:00:00Z".parse().unwrap()),
            points: Some(3),
            card_number: 42,
            sprint_id: Some(sprint_id),
            created_at: "2024-01-01T00:00:00Z".parse().unwrap(),
            updated_at: "2024-02-02T00:00:00Z".parse().unwrap(),
            completed_at: Some("2024-03-03T00:00:00Z".parse().unwrap()),
            sprint_logs: vec![
                SprintLog {
                    sprint_id,
                    sprint_number: 1,
                    sprint_name: Some("Sprint 1".to_string()),
                    started_at: "2024-01-10T00:00:00Z".parse().unwrap(),
                    ended_at: Some("2024-01-20T00:00:00Z".parse().unwrap()),
                    status: "Completed".to_string(),
                },
                SprintLog {
                    sprint_id,
                    sprint_number: 2,
                    sprint_name: Some("Sprint 2".to_string()),
                    started_at: "2024-02-01T00:00:00Z".parse().unwrap(),
                    ended_at: None,
                    status: "Active".to_string(),
                },
            ],
            prefix: String::new(),
        }
    }

    #[test]
    fn test_card_reconstitute_restores_all_fields_verbatim() -> KanbanResult<()> {
        let rec = populated_record();
        let expected = rec.clone();
        let card = Card::reconstitute(rec)?;
        assert_eq!(card.id, expected.id);
        assert_eq!(card.column_id, expected.column_id);
        assert_eq!(card.title, expected.title);
        assert_eq!(card.description, expected.description);
        assert_eq!(card.priority, expected.priority);
        assert_eq!(card.status, expected.status);
        assert_eq!(card.position, expected.position);
        assert_eq!(card.due_date, expected.due_date);
        assert_eq!(card.points, expected.points);
        assert_eq!(card.card_number, expected.card_number);
        assert_eq!(card.sprint_id, expected.sprint_id);
        assert_eq!(card.created_at, expected.created_at);
        assert_eq!(card.updated_at, expected.updated_at);
        assert_eq!(card.completed_at, expected.completed_at);
        assert_eq!(card.sprint_logs, expected.sprint_logs);
        Ok(())
    }

    #[test]
    fn test_card_decompose_round_trip_via_record() -> KanbanResult<()> {
        let card = Card::reconstitute(populated_record())?;
        let r = CardRecord::from(&card);
        let c2 = Card::reconstitute(r)?;
        assert_eq!(card, c2);
        Ok(())
    }

    #[test]
    fn test_card_record_serde_round_trip() -> KanbanResult<()> {
        let rec = populated_record();
        let json = serde_json::to_string(&rec).unwrap();
        let back: CardRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(rec, back);
        Ok(())
    }

    #[test]
    fn test_new_card_has_no_default_impl() {
        // Compile-lock: constructing NewCard/CardRecord requires naming every
        // field (no `Default`, no `..`). A `NewCard::default()` must NOT compile;
        // the CI grep guard forbids a Default derive on these boundary types.
        let new_card = NewCard {
            column_id: Uuid::new_v4(),
            title: "x".to_string(),
            description: None,
            priority: CardPriority::Medium,
            due_date: None,
            points: None,
            sprint_id: None,
        };
        assert_eq!(new_card.title, "x");
    }
}
