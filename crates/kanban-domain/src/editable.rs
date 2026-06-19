use crate::{Board, Card};
use chrono::{DateTime, NaiveTime, Utc};
use kanban_core::{parse_datetime_input, Editable};
use serde::{de::Error as _, Deserialize, Deserializer, Serialize, Serializer};

// Case-insensitive enum parsers that normalize to canonical format
fn parse_card_priority_case_insensitive(s: &str) -> Option<String> {
    match s.to_lowercase().as_str() {
        "low" => Some("Low".to_string()),
        "medium" => Some("Medium".to_string()),
        "high" => Some("High".to_string()),
        "critical" => Some("Critical".to_string()),
        _ => None,
    }
}

fn parse_card_status_case_insensitive(s: &str) -> Option<String> {
    match s.to_lowercase().replace('_', "").as_str() {
        "todo" => Some("Todo".to_string()),
        "inprogress" => Some("InProgress".to_string()),
        "blocked" => Some("Blocked".to_string()),
        "done" => Some("Done".to_string()),
        _ => None,
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BoardSettingsDto {
    #[serde(alias = "branch_prefix")]
    pub sprint_prefix: Option<String>,
    pub card_prefix: Option<String>,
    pub sprint_duration_days: Option<u32>,
    pub sprint_names: Vec<String>,
    #[serde(default)]
    pub completion_column_id: Option<uuid::Uuid>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CardMetadataDto {
    pub priority: String,
    pub status: String,
    pub points: Option<u8>,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_date_input",
        serialize_with = "serialize_optional_date_input"
    )]
    pub due_date: Option<DateTime<Utc>>,
}

fn deserialize_optional_date_input<'de, D>(
    deserializer: D,
) -> Result<Option<DateTime<Utc>>, D::Error>
where
    D: Deserializer<'de>,
{
    let opt: Option<String> = Option::deserialize(deserializer)?;
    match opt {
        None => Ok(None),
        Some(s) => parse_datetime_input(&s).map(Some).map_err(D::Error::custom),
    }
}

fn serialize_optional_date_input<S>(
    value: &Option<DateTime<Utc>>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    match value {
        None => serializer.serialize_none(),
        Some(dt) => {
            let midnight = NaiveTime::from_hms_opt(0, 0, 0).expect("midnight is valid");
            let rendered = if dt.time() == midnight {
                dt.format("%Y-%m-%d").to_string()
            } else {
                dt.to_rfc3339()
            };
            serializer.serialize_some(&rendered)
        }
    }
}

impl Editable<Board> for BoardSettingsDto {
    fn from_entity(board: &Board) -> Self {
        Self {
            sprint_prefix: board.sprint_prefix.clone(),
            card_prefix: board.card_prefix.clone(),
            sprint_duration_days: board.sprint_duration_days,
            sprint_names: board.sprint_names.clone(),
            completion_column_id: board.completion_column_id,
        }
    }

    fn apply_to(self, board: &mut Board) {
        board.sprint_prefix = self.sprint_prefix;
        board.card_prefix = self.card_prefix;
        board.sprint_duration_days = self.sprint_duration_days;
        board.sprint_names = self.sprint_names;
        board.completion_column_id = self.completion_column_id;
        board.updated_at = chrono::Utc::now();
    }
}

impl Editable<Card> for CardMetadataDto {
    fn from_entity(card: &Card) -> Self {
        Self {
            priority: format!("{:?}", card.priority),
            status: format!("{:?}", card.status),
            points: card.points,
            due_date: card.due_date,
        }
    }

    fn apply_to(self, card: &mut Card) {
        if let Some(canonical_priority) = parse_card_priority_case_insensitive(&self.priority) {
            if let Ok(priority) = serde_json::from_value::<crate::CardPriority>(
                serde_json::Value::String(canonical_priority),
            ) {
                card.priority = priority;
            }
        }

        if let Some(canonical_status) = parse_card_status_case_insensitive(&self.status) {
            if let Ok(status) = serde_json::from_value::<crate::CardStatus>(
                serde_json::Value::String(canonical_status),
            ) {
                card.status = status;
            }
        }

        if let Some(points) = self.points {
            card.points = Some(points);
        }

        if let Some(due_date) = self.due_date {
            card.due_date = Some(due_date);
        }

        card.updated_at = chrono::Utc::now();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn metadata_json(due_date_value: &str) -> String {
        format!(
            r#"{{"priority":"High","status":"Todo","points":null,"due_date":{due_date_value}}}"#
        )
    }

    #[test]
    fn test_card_metadata_dto_deserializes_yyyy_mm_dd_as_midnight_utc() {
        let dto: CardMetadataDto = serde_json::from_str(&metadata_json(r#""2024-01-15""#)).unwrap();
        assert_eq!(
            dto.due_date,
            Some(chrono::Utc.with_ymd_and_hms(2024, 1, 15, 0, 0, 0).unwrap())
        );
    }

    #[test]
    fn test_card_metadata_dto_deserializes_full_rfc3339_preserved() {
        let dto: CardMetadataDto =
            serde_json::from_str(&metadata_json(r#""2024-01-15T14:30:00Z""#)).unwrap();
        assert_eq!(
            dto.due_date,
            Some(
                chrono::Utc
                    .with_ymd_and_hms(2024, 1, 15, 14, 30, 0)
                    .unwrap()
            )
        );
    }

    #[test]
    fn test_card_metadata_dto_deserializes_null_due_date() {
        let dto: CardMetadataDto = serde_json::from_str(&metadata_json("null")).unwrap();
        assert_eq!(dto.due_date, None);
    }

    #[test]
    fn test_card_metadata_dto_rejects_garbage_due_date_with_serde_error() {
        let err = serde_json::from_str::<CardMetadataDto>(&metadata_json(r#""yesterday""#))
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("yesterday"),
            "serde error should include the offending input, got: {err}"
        );
        assert!(
            err.contains("YYYY-MM-DD"),
            "serde error should mention the supported format, got: {err}"
        );
    }

    #[test]
    fn test_card_metadata_dto_serializes_midnight_utc_as_yyyy_mm_dd() {
        let dto = CardMetadataDto {
            priority: "High".to_string(),
            status: "Todo".to_string(),
            points: None,
            due_date: Some(chrono::Utc.with_ymd_and_hms(2024, 1, 15, 0, 0, 0).unwrap()),
        };
        let json = serde_json::to_string(&dto).unwrap();
        assert!(
            json.contains(r#""due_date":"2024-01-15""#),
            "midnight UTC should serialize as YYYY-MM-DD for round-trip-friendly display, got: {json}"
        );
    }

    #[test]
    fn test_card_metadata_dto_serializes_non_midnight_as_rfc3339() {
        let dto = CardMetadataDto {
            priority: "High".to_string(),
            status: "Todo".to_string(),
            points: None,
            due_date: Some(
                chrono::Utc
                    .with_ymd_and_hms(2024, 1, 15, 14, 30, 0)
                    .unwrap(),
            ),
        };
        let json = serde_json::to_string(&dto).unwrap();
        assert!(
            json.contains(r#""due_date":"2024-01-15T14:30:00+00:00""#),
            "non-midnight should round-trip as full RFC3339, got: {json}"
        );
    }

    #[test]
    fn test_card_metadata_dto_serializes_none_as_json_null() {
        let dto = CardMetadataDto {
            priority: "High".to_string(),
            status: "Todo".to_string(),
            points: None,
            due_date: None,
        };
        let json = serde_json::to_string(&dto).unwrap();
        assert!(json.contains(r#""due_date":null"#), "got: {json}");
    }

    #[test]
    fn test_apply_to_writes_due_date_when_dto_is_some() {
        let mut card = Card {
            due_date: None,
            ..fresh_card_for_tests()
        };
        let dto = CardMetadataDto {
            priority: "High".to_string(),
            status: "Todo".to_string(),
            points: None,
            due_date: Some(chrono::Utc.with_ymd_and_hms(2024, 1, 15, 0, 0, 0).unwrap()),
        };
        dto.apply_to(&mut card);
        assert_eq!(
            card.due_date,
            Some(chrono::Utc.with_ymd_and_hms(2024, 1, 15, 0, 0, 0).unwrap())
        );
    }

    #[test]
    fn test_apply_to_preserves_existing_due_date_when_dto_is_none() {
        let preset = chrono::Utc.with_ymd_and_hms(2024, 1, 15, 0, 0, 0).unwrap();
        let mut card = Card {
            due_date: Some(preset),
            ..fresh_card_for_tests()
        };
        let dto = CardMetadataDto {
            priority: "High".to_string(),
            status: "Todo".to_string(),
            points: None,
            due_date: None,
        };
        dto.apply_to(&mut card);
        assert_eq!(
            card.due_date,
            Some(preset),
            "null in editor must leave the existing due_date untouched (consistent with priority/status/points behaviour)"
        );
    }

    fn fresh_card_for_tests() -> Card {
        let mut board = crate::Board::new("B", None::<String>);
        crate::Card::new(&mut board, uuid::Uuid::new_v4(), "title", 0)
    }

    #[test]
    fn test_deserialize_old_branch_prefix_format() {
        // Test that old JSON with branch_prefix field is correctly deserialized
        let old_format = r#"{
            "branch_prefix": "FEAT",
            "sprint_duration_days": null,
            "sprint_names": []
        }"#;

        let settings: BoardSettingsDto = serde_json::from_str(old_format)
            .expect("Failed to deserialize BoardSettingsDto with old branch_prefix field");

        // Via the serde alias, branch_prefix should map to sprint_prefix
        assert_eq!(settings.sprint_prefix, Some("FEAT".to_string()));
        // card_prefix should be None since it wasn't in the old format
        assert_eq!(settings.card_prefix, None);
    }

    #[test]
    fn test_deserialize_new_separate_prefixes() {
        // Test that new JSON with both sprint_prefix and card_prefix works
        let new_format = r#"{
            "sprint_prefix": "SPRINT",
            "card_prefix": "TASK",
            "sprint_duration_days": 14,
            "sprint_names": []
        }"#;

        let settings: BoardSettingsDto = serde_json::from_str(new_format)
            .expect("Failed to deserialize BoardSettingsDto with separate prefixes");

        assert_eq!(settings.sprint_prefix, Some("SPRINT".to_string()));
        assert_eq!(settings.card_prefix, Some("TASK".to_string()));
        assert_eq!(settings.sprint_duration_days, Some(14));
    }

    #[test]
    fn test_sprint_deserialization_without_card_prefix() {
        // Test that old Sprint JSON without card_prefix field deserializes correctly
        let old_sprint_json = r#"{
            "id": "550e8400-e29b-41d4-a716-446655440000",
            "board_id": "550e8400-e29b-41d4-a716-446655440001",
            "sprint_number": 1,
            "name_index": null,
            "prefix": null,
            "status": "Planning",
            "start_date": null,
            "end_date": null,
            "created_at": "2024-01-01T00:00:00Z",
            "updated_at": "2024-01-01T00:00:00Z"
        }"#;

        let sprint: crate::Sprint = serde_json::from_str(old_sprint_json)
            .expect("Failed to deserialize Sprint without card_prefix field");

        // card_prefix should default to None via #[serde(default)]
        assert_eq!(sprint.card_prefix, None);
        assert_eq!(sprint.sprint_number, 1);
    }

    #[test]
    fn test_card_deserialization_from_old_format() {
        // Test that old Card JSON with legacy fields (assigned_prefix, card_prefix)
        // still deserializes correctly — unknown fields are silently ignored.
        let old_card_json = r#"{
            "id": "550e8400-e29b-41d4-a716-446655440000",
            "column_id": "550e8400-e29b-41d4-a716-446655440002",
            "title": "Test Card",
            "description": null,
            "priority": "Medium",
            "status": "Todo",
            "position": 0,
            "due_date": null,
            "points": null,
            "card_number": 1,
            "sprint_id": null,
            "assigned_prefix": "task",
            "card_prefix": null,
            "created_at": "2024-01-01T00:00:00Z",
            "updated_at": "2024-01-01T00:00:00Z",
            "completed_at": null,
            "sprint_logs": []
        }"#;

        let card: Card = serde_json::from_str(old_card_json)
            .expect("Failed to deserialize Card from old format");

        assert_eq!(card.title, "Test Card");
        assert_eq!(card.card_number, 1);
    }
}
