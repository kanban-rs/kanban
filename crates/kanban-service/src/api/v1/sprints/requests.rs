use super::super::Patch;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Request body for `POST /v1/boards/:board_id/sprints` (and the
/// `PUT /v1/sprints/:id` create arm).
///
/// Carries the client-settable CREATE fields plus an optional client-supplied
/// `id` for idempotent PUT-create. The client sends a sprint `name` (a string);
/// the service allocates the domain `name_index` against the owning board's
/// name pool. `board_id` is path-supplied (nested route), so it is not a body
/// field. Server-managed fields (`sprint_number`, `name_index`, `status`,
/// dates, timestamps) are never on the wire.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct CreateSprintRequest {
    /// Client-supplied id (idempotent PUT-create); read by the service tier.
    #[serde(default)]
    pub id: Option<Uuid>,
    /// Client sends a NAME; the service allocates `name_index` from the board.
    #[serde(default)]
    pub name: Option<String>,
    /// Explicit sprint prefix override.
    #[serde(default)]
    pub prefix: Option<String>,
    #[serde(default)]
    pub card_prefix: Option<String>,
}

/// Request body for `PATCH /v1/sprints/:id` — JSON Merge Patch (RFC 7386):
/// absent field = no change, `null` = clear, value = set (see [`Patch`]).
///
/// Server-managed fields (`sprint_number`, `name_index`, `created_at`,
/// `updated_at`) are excluded. `status` and the dates are excluded too:
/// lifecycle transitions go through dedicated activate/complete/cancel
/// endpoints, not PATCH.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UpdateSprintRequest {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Patch::is_no_change")]
    pub prefix: Patch<String>,
    #[serde(default, skip_serializing_if = "Patch::is_no_change")]
    pub card_prefix: Patch<String>,
}

/// Request body for `PUT /v1/sprints/:id` — a true full replace per
/// [RFC 9110 §9.3.4](https://www.rfc-editor.org/info/rfc9110/): the body is the
/// complete client-editable state. The nullable fields are cleared when
/// omitted. Lifecycle/server-managed fields are excluded as in
/// [`UpdateSprintRequest`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplaceSprintRequest {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub prefix: Option<String>,
    #[serde(default)]
    pub card_prefix: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_sprint_request_serde_round_trip() {
        let id = Uuid::new_v4();
        let req = CreateSprintRequest {
            id: Some(id),
            name: Some("Sprint 1".to_string()),
            prefix: Some("SPR".to_string()),
            card_prefix: Some("KAN".to_string()),
        };
        let json = serde_json::to_string(&req).unwrap();
        let back: CreateSprintRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(back.id, Some(id));
        assert_eq!(back.name, Some("Sprint 1".to_string()));
        assert_eq!(back.prefix, Some("SPR".to_string()));
        assert_eq!(back.card_prefix, Some("KAN".to_string()));
    }

    #[test]
    fn test_create_sprint_request_minimal_omits_optionals() {
        let empty: CreateSprintRequest = serde_json::from_str("{}").unwrap();
        assert_eq!(empty.id, None);
        assert_eq!(empty.name, None);
        assert_eq!(empty.prefix, None);
        assert_eq!(empty.card_prefix, None);

        let named: CreateSprintRequest = serde_json::from_str(r#"{"name":"S1"}"#).unwrap();
        assert_eq!(named.id, None);
        assert_eq!(named.name, Some("S1".to_string()));
    }

    #[test]
    fn test_create_sprint_request_carries_optional_client_id() {
        let id = Uuid::new_v4();
        let json = format!(r#"{{"id":"{id}","name":"S1"}}"#);
        let req: CreateSprintRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(req.id, Some(id));
    }

    #[test]
    fn test_update_sprint_request_merge_patch_round_trip() {
        let req = UpdateSprintRequest {
            name: Some("Renamed".to_string()),
            prefix: Patch::Set("SPR".to_string()),
            card_prefix: Patch::Clear,
        };
        let json = serde_json::to_string(&req).unwrap();
        let back: UpdateSprintRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(back.name, Some("Renamed".to_string()));
        assert_eq!(back.prefix, Patch::Set("SPR".to_string()));
        assert_eq!(back.card_prefix, Patch::Clear);
    }

    #[test]
    fn test_update_sprint_request_absent_is_no_change_null_is_clear() {
        let back: UpdateSprintRequest = serde_json::from_str(r#"{"prefix":null}"#).unwrap();
        assert_eq!(back.name, None);
        assert_eq!(back.prefix, Patch::Clear); // explicit null → clear
        assert_eq!(back.card_prefix, Patch::NoChange); // absent → no change
    }

    #[test]
    fn test_update_sprint_request_omits_no_change_patch_fields_on_serialize() {
        // Guards the Patch footgun: every Patch field must carry
        // skip_serializing_if, so a default (all-NoChange) request omits them
        // rather than emitting null (= clear).
        let v = serde_json::to_value(UpdateSprintRequest::default()).unwrap();
        for field in ["prefix", "card_prefix"] {
            assert!(
                v.get(field).is_none(),
                "NoChange patch field `{field}` must be omitted, got: {v}"
            );
        }
    }

    #[test]
    fn test_replace_sprint_request_round_trips() {
        let req = ReplaceSprintRequest {
            name: Some("Fresh".to_string()),
            prefix: Some("SPR".to_string()),
            card_prefix: None,
        };
        let json = serde_json::to_string(&req).unwrap();
        let back: ReplaceSprintRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(back.name, Some("Fresh".to_string()));
        assert_eq!(back.prefix, Some("SPR".to_string()));
        assert_eq!(back.card_prefix, None);
    }
}
