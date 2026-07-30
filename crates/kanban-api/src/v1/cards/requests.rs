use super::super::enums::{CardPriorityDto, CardStatusDto};
use super::super::Patch;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Request body for `POST /v1/columns/:column_id/cards` (and `PUT` create arm).
///
/// Carries the client-settable CREATE fields plus an optional client-supplied
/// `id` for idempotent PUT-create; the service mints the id when absent and
/// funnels the content through `NewCard` + `Card::create`. `column_id` is
/// path-supplied (not a body field) and `card_number` is server-minted from the
/// Board counter, so neither appears here. An omitted `priority` defaults to
/// `Medium` at conversion.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct CreateCardRequest {
    /// Client-supplied id (idempotent PUT-create); read by the service tier.
    #[serde(default)]
    pub id: Option<Uuid>,
    pub title: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub priority: Option<CardPriorityDto>,
    #[serde(default)]
    pub due_date: Option<DateTime<Utc>>,
    #[serde(default)]
    pub points: Option<u8>,
    #[serde(default)]
    pub sprint_id: Option<Uuid>,
}

/// Request body for `PATCH /v1/cards/:id` — JSON Merge Patch (RFC 7386):
/// absent = no change, `null` = clear, value = set (see [`Patch`]).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UpdateCardRequest {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub priority: Option<CardPriorityDto>,
    #[serde(default)]
    pub status: Option<CardStatusDto>,
    #[serde(default)]
    pub position: Option<i32>,
    #[serde(default)]
    pub column_id: Option<Uuid>,
    #[serde(default, skip_serializing_if = "Patch::is_no_change")]
    pub description: Patch<String>,
    #[serde(default, skip_serializing_if = "Patch::is_no_change")]
    pub due_date: Patch<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "Patch::is_no_change")]
    pub points: Patch<u8>,
    #[serde(default, skip_serializing_if = "Patch::is_no_change")]
    pub sprint_id: Patch<Uuid>,
}

/// Request body for `PUT /v1/cards/:id` — full replace of client-editable
/// fields. Omitted optional fields (`description`/`due_date`/`points`/
/// `sprint_id`) are cleared (wholesale replace).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplaceCardRequest {
    pub title: String,
    pub priority: CardPriorityDto,
    pub status: CardStatusDto,
    pub position: i32,
    pub column_id: Uuid,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub due_date: Option<DateTime<Utc>>,
    #[serde(default)]
    pub points: Option<u8>,
    #[serde(default)]
    pub sprint_id: Option<Uuid>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_card_request_minimal_omits_optionals() {
        let req: CreateCardRequest = serde_json::from_str(r#"{"title":"x"}"#).unwrap();
        assert_eq!(req.title, "x");
        assert_eq!(req.id, None);
        assert_eq!(req.description, None);
        assert_eq!(req.priority, None);
        assert_eq!(req.due_date, None);
        assert_eq!(req.points, None);
        assert_eq!(req.sprint_id, None);
    }

    #[test]
    fn test_create_card_request_accepts_client_id() {
        let uuid = Uuid::new_v4();
        let json = format!(r#"{{"title":"x","id":"{uuid}"}}"#);
        let req: CreateCardRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(req.id, Some(uuid));
    }

    #[test]
    fn test_update_card_request_absent_is_no_change_null_is_clear() {
        let absent: UpdateCardRequest = serde_json::from_str("{}").unwrap();
        assert_eq!(absent.description, Patch::NoChange);
        assert_eq!(absent.due_date, Patch::NoChange);
        assert_eq!(absent.points, Patch::NoChange);
        assert_eq!(absent.sprint_id, Patch::NoChange);

        let null: UpdateCardRequest = serde_json::from_str(
            r#"{"description":null,"due_date":null,"points":null,"sprint_id":null}"#,
        )
        .unwrap();
        assert_eq!(null.description, Patch::Clear);
        assert_eq!(null.due_date, Patch::Clear);
        assert_eq!(null.points, Patch::Clear);
        assert_eq!(null.sprint_id, Patch::Clear);

        let set: UpdateCardRequest =
            serde_json::from_str(r#"{"description":"d","points":3}"#).unwrap();
        assert_eq!(set.description, Patch::Set("d".to_string()));
        assert_eq!(set.points, Patch::Set(3));
    }

    #[test]
    fn test_update_card_request_omits_no_change_patch_fields_on_serialize() {
        let v = serde_json::to_value(UpdateCardRequest::default()).unwrap();
        for key in ["description", "due_date", "points", "sprint_id"] {
            assert!(
                v.get(key).is_none(),
                "NoChange {key} must be omitted, got: {v}"
            );
        }
    }
}
