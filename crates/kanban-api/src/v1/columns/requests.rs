use super::super::enums::CardStatusDto;
use super::super::Patch;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Request body for `POST /v1/boards/:id/columns` (and `PUT` create arm).
///
/// Carries the client-settable CREATE fields plus an optional client-supplied
/// `id` for idempotent PUT-create; the service mints the id when absent and
/// funnels the content through `NewColumn` + `Column::create`. `board_id` is
/// path-supplied (not a body field) and `position` is server-assigned on
/// append, so neither appears here.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct CreateColumnRequest {
    /// Client-supplied id (idempotent PUT-create); read by the service tier.
    #[serde(default)]
    pub id: Option<Uuid>,
    pub name: String,
    #[serde(default)]
    pub wip_limit: Option<i32>,
    #[serde(default)]
    pub default_status: Option<CardStatusDto>,
}

/// Request body for `PATCH /v1/columns/:id` — JSON Merge Patch (RFC 7386):
/// absent = no change, `null` = clear, value = set (see [`Patch`]).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UpdateColumnRequest {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub position: Option<i32>,
    #[serde(default, skip_serializing_if = "Patch::is_no_change")]
    pub wip_limit: Patch<i32>,
    #[serde(default, skip_serializing_if = "Patch::is_no_change")]
    pub default_status: Patch<CardStatusDto>,
}

/// Request body for `PUT /v1/columns/:id` — full replace of client-editable
/// fields. An omitted `wip_limit` is cleared (wholesale replace).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ReplaceColumnRequest {
    pub name: String,
    pub position: i32,
    #[serde(default)]
    pub wip_limit: Option<i32>,
    #[serde(default)]
    pub default_status: Option<CardStatusDto>,
}

/// Request body for `POST /v1/columns/:id/reorder`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReorderColumnRequest {
    pub position: i32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_column_request_serde_round_trip() {
        let req = CreateColumnRequest {
            id: None,
            name: "In Review".to_string(),
            wip_limit: Some(3),
            default_status: None,
        };
        let json = serde_json::to_string(&req).unwrap();
        let back: CreateColumnRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(back.id, req.id);
        assert_eq!(back.name, req.name);
        assert_eq!(back.wip_limit, req.wip_limit);
    }

    #[test]
    fn test_create_column_request_serde_round_trip_with_id() {
        let req = CreateColumnRequest {
            id: Some(Uuid::new_v4()),
            name: "In Review".to_string(),
            wip_limit: Some(3),
            default_status: None,
        };
        let json = serde_json::to_string(&req).unwrap();
        let back: CreateColumnRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(back.id, req.id);
        assert_eq!(back.name, req.name);
        assert_eq!(back.wip_limit, req.wip_limit);
    }

    #[test]
    fn test_create_column_request_minimal_omits_wip_limit() {
        let json = r#"{"name":"Backlog"}"#;
        let back: CreateColumnRequest = serde_json::from_str(json).unwrap();
        assert_eq!(back.name, "Backlog");
        assert_eq!(back.wip_limit, None);
    }

    #[test]
    fn test_create_column_request_absent_id_is_none() {
        let json = r#"{"name":"Backlog"}"#;
        let back: CreateColumnRequest = serde_json::from_str(json).unwrap();
        assert_eq!(back.id, None);
    }

    #[test]
    fn test_update_column_request_omits_no_change_wip_limit_on_serialize() {
        // Guards the Patch footgun: the wip_limit Patch field must carry
        // skip_serializing_if so a default (NoChange) request omits it, not null.
        let v = serde_json::to_value(UpdateColumnRequest::default()).unwrap();
        assert!(
            v.get("wip_limit").is_none(),
            "NoChange wip_limit must be omitted, got: {v}"
        );
    }

    #[test]
    fn test_update_column_request_merge_patch_round_trip() {
        let req = UpdateColumnRequest {
            name: Some("Done".to_string()),
            position: Some(4),
            wip_limit: Patch::Clear,
            default_status: Patch::NoChange,
        };
        let json = serde_json::to_string(&req).unwrap();
        let back: UpdateColumnRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(back.name, Some("Done".to_string()));
        assert_eq!(back.position, Some(4));
        assert_eq!(back.wip_limit, Patch::Clear);
    }

    #[test]
    fn test_update_column_request_absent_is_no_change_null_is_clear() {
        let absent: UpdateColumnRequest = serde_json::from_str("{}").unwrap();
        assert_eq!(absent.wip_limit, Patch::NoChange);
        let null: UpdateColumnRequest = serde_json::from_str(r#"{"wip_limit":null}"#).unwrap();
        assert_eq!(null.wip_limit, Patch::Clear);
        let set: UpdateColumnRequest = serde_json::from_str(r#"{"wip_limit":5}"#).unwrap();
        assert_eq!(set.wip_limit, Patch::Set(5));
    }

    #[test]
    fn test_reorder_column_request_serde_round_trip() {
        let req = ReorderColumnRequest { position: 2 };
        let json = serde_json::to_string(&req).unwrap();
        let back: ReorderColumnRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(back.position, req.position);
    }
}
