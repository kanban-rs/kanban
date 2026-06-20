use super::patch::Patch;
use chrono::{DateTime, Utc};
use kanban_domain::{BoardId, Column, ColumnId, ColumnUpdate, KanbanError, KanbanResult};
use serde::{Deserialize, Serialize};

/// Request body for `POST /v1/boards/:id/columns`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateColumnRequest {
    pub name: String,
    #[serde(default)]
    pub wip_limit: Option<i32>,
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
}

impl TryFrom<UpdateColumnRequest> for ColumnUpdate {
    type Error = KanbanError;

    fn try_from(req: UpdateColumnRequest) -> KanbanResult<Self> {
        // Exhaustive destructure (no `..`): a new request field is a compile error.
        let UpdateColumnRequest {
            name,
            position,
            wip_limit,
        } = req;
        validate_position(position)?;
        if let Patch::Set(limit) = &wip_limit {
            validate_wip_limit(*limit)?;
        }
        // Exhaustive construct (no `..Default::default()`): a new ColumnUpdate field
        // is a compile error.
        Ok(ColumnUpdate {
            name,
            position,
            wip_limit: wip_limit.into(),
        })
    }
}

/// Request body for `PUT /v1/columns/:id` — full replace of client-editable
/// fields. An omitted `wip_limit` is cleared (wholesale replace).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ReplaceColumnRequest {
    pub name: String,
    pub position: i32,
    #[serde(default)]
    pub wip_limit: Option<i32>,
}

impl TryFrom<ReplaceColumnRequest> for ColumnUpdate {
    type Error = KanbanError;

    fn try_from(req: ReplaceColumnRequest) -> KanbanResult<Self> {
        let ReplaceColumnRequest {
            name,
            position,
            wip_limit,
        } = req;
        validate_position(Some(position))?;
        if let Some(limit) = wip_limit {
            validate_wip_limit(limit)?;
        }
        Ok(ColumnUpdate {
            name: Some(name),
            position: Some(position),
            wip_limit: wip_limit.into(),
        })
    }
}

/// Request body for `POST /v1/columns/:id/reorder`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReorderColumnRequest {
    pub position: i32,
}

/// Response body for column reads.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ColumnResponse {
    pub id: ColumnId,
    pub board_id: BoardId,
    pub name: String,
    pub position: i32,
    pub wip_limit: Option<i32>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<Column> for ColumnResponse {
    fn from(c: Column) -> Self {
        Self {
            id: c.id,
            board_id: c.board_id,
            name: c.name,
            position: c.position,
            wip_limit: c.wip_limit,
            created_at: c.created_at,
            updated_at: c.updated_at,
        }
    }
}

fn validate_position(position: Option<i32>) -> KanbanResult<()> {
    match position {
        Some(p) if p < 0 => Err(KanbanError::validation(format!(
            "column position must be >= 0, got {p}"
        ))),
        _ => Ok(()),
    }
}

fn validate_wip_limit(limit: i32) -> KanbanResult<()> {
    if limit < 0 {
        return Err(KanbanError::validation(format!(
            "wip_limit must be >= 0, got {limit}"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use kanban_domain::FieldUpdate;
    use uuid::Uuid;

    #[test]
    fn test_create_column_request_serde_round_trip() {
        let req = CreateColumnRequest {
            name: "In Review".to_string(),
            wip_limit: Some(3),
        };
        let json = serde_json::to_string(&req).unwrap();
        let back: CreateColumnRequest = serde_json::from_str(&json).unwrap();
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
    fn test_update_column_request_merge_patch_round_trip() {
        let req = UpdateColumnRequest {
            name: Some("Done".to_string()),
            position: Some(4),
            wip_limit: Patch::Clear,
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
    fn test_update_column_request_into_column_update() {
        let req = UpdateColumnRequest {
            name: Some("Done".to_string()),
            position: Some(4),
            wip_limit: Patch::Clear,
        };
        let update = ColumnUpdate::try_from(req).unwrap();
        assert_eq!(update.name, Some("Done".to_string()));
        assert_eq!(update.position, Some(4));
        assert_eq!(update.wip_limit, FieldUpdate::Clear);
    }

    #[test]
    fn test_update_column_request_rejects_negative_wip_limit() {
        let req = UpdateColumnRequest {
            name: None,
            position: None,
            wip_limit: Patch::Set(-1),
        };
        assert!(ColumnUpdate::try_from(req).is_err());
    }

    #[test]
    fn test_replace_column_request_clears_omitted_wip_limit() {
        let req: ReplaceColumnRequest =
            serde_json::from_str(r#"{"name":"Done","position":2}"#).unwrap();
        let update = ColumnUpdate::try_from(req).unwrap();
        assert_eq!(update.name, Some("Done".to_string()));
        assert_eq!(update.position, Some(2));
        assert_eq!(update.wip_limit, FieldUpdate::Clear);
    }

    #[test]
    fn test_replace_column_request_rejects_negative_position() {
        let req = ReplaceColumnRequest {
            name: "X".to_string(),
            position: -1,
            wip_limit: None,
        };
        assert!(ColumnUpdate::try_from(req).is_err());
    }

    #[test]
    fn test_reorder_column_request_serde_round_trip() {
        let req = ReorderColumnRequest { position: 2 };
        let json = serde_json::to_string(&req).unwrap();
        let back: ReorderColumnRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(back.position, req.position);
    }

    #[test]
    fn test_column_response_from_column() {
        let board_id = Uuid::new_v4();
        let column = Column::new(board_id, "Doing", 1);
        let resp = ColumnResponse::from(column.clone());
        assert_eq!(resp.id, column.id);
        assert_eq!(resp.board_id, board_id);
        assert_eq!(resp.name, "Doing");
        assert_eq!(resp.position, 1);
        let back: ColumnResponse =
            serde_json::from_str(&serde_json::to_string(&resp).unwrap()).unwrap();
        assert_eq!(back, resp);
    }
}
