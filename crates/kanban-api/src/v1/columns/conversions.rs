//! Wire-to-domain conversions + validation for the column request DTOs. Kept
//! separate from the struct definitions in `requests.rs` (wire shape vs mapping
//! policy). Conversions are the validation boundary and destructure/construct
//! exhaustively (no `..`).

use super::super::Patch;
use super::requests::{
    CreateColumnRequest, ReorderColumnRequest, ReplaceColumnRequest, UpdateColumnRequest,
};
use kanban_domain::{BoardId, ColumnUpdate, KanbanError, KanbanResult, NewColumn};
use uuid::Uuid;

impl TryFrom<UpdateColumnRequest> for ColumnUpdate {
    type Error = KanbanError;

    fn try_from(req: UpdateColumnRequest) -> KanbanResult<Self> {
        let UpdateColumnRequest {
            name,
            position,
            wip_limit,
        } = req;
        validate_position(position)?;
        if let Patch::Set(limit) = &wip_limit {
            validate_wip_limit(*limit)?;
        }
        Ok(ColumnUpdate {
            name,
            position,
            wip_limit: wip_limit.into(),
        })
    }
}

impl CreateColumnRequest {
    /// Split the identity (optional client id) from the domain create spec. The
    /// service mints the id when `None` and calls
    /// `Column::create(spec, id, position, now)` with a server-assigned append
    /// `position` (NOT carried here). `board_id` is path-supplied (nested
    /// `POST /boards/:id/columns` route), so it is a parameter rather than a
    /// body field. Validates `wip_limit >= 0`; exhaustive destructure — no `..`
    /// — so a new field is a compile error.
    pub fn into_new_column(self, board_id: BoardId) -> KanbanResult<(Option<Uuid>, NewColumn)> {
        let CreateColumnRequest {
            id,
            name,
            wip_limit,
        } = self;
        if let Some(limit) = wip_limit {
            validate_wip_limit(limit)?;
        }
        let spec = NewColumn {
            board_id,
            name,
            wip_limit,
        };
        Ok((id, spec))
    }
}

impl ReplaceColumnRequest {
    /// Split the full replace spec (content + position) into a domain spec and
    /// the server-managed position to apply on the replace arm. The position is
    /// part of the full-replace contract of PUT — the client sends back what
    /// they read, or a new position to move the column. `board_id` is
    /// path-supplied (nested `PUT /boards/:id/columns/:id` route), so it is a
    /// parameter rather than a body field. Validates `name`, `position >= 0`,
    /// and `wip_limit >= 0`; exhaustive destructure — no `..` — so a new field
    /// is a compile error.
    pub fn into_new_column(self, board_id: BoardId) -> KanbanResult<(NewColumn, i32)> {
        let ReplaceColumnRequest {
            name,
            position,
            wip_limit,
        } = self;
        validate_position(Some(position))?;
        if let Some(limit) = wip_limit {
            validate_wip_limit(limit)?;
        }
        let spec = NewColumn {
            board_id,
            name,
            wip_limit,
        };
        Ok((spec, position))
    }
}

impl ReorderColumnRequest {
    /// Validate and unwrap the target position (`>= 0`), same rule and message
    /// every other column DTO validates through this module.
    pub fn validated_position(self) -> KanbanResult<i32> {
        validate_position(Some(self.position))?;
        Ok(self.position)
    }
}

// Non-negativity is the only domain invariant for these `i32` fields; no upper
// bound is enforced by design (a maximum is not a domain rule).
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
    fn test_create_column_request_into_new_column_maps_fields() {
        let board_id = Uuid::new_v4();
        let req = CreateColumnRequest {
            id: None,
            name: "Doing".to_string(),
            wip_limit: Some(3),
        };
        let (id, spec) = req.into_new_column(board_id).unwrap();
        assert_eq!(id, None);
        assert_eq!(
            spec,
            NewColumn {
                board_id,
                name: "Doing".to_string(),
                wip_limit: Some(3),
            }
        );
    }

    #[test]
    fn test_create_column_request_carries_client_id() {
        let board_id = Uuid::new_v4();
        let client_id = Uuid::new_v4();
        let req = CreateColumnRequest {
            id: Some(client_id),
            name: "Doing".to_string(),
            wip_limit: None,
        };
        let (id, _) = req.into_new_column(board_id).unwrap();
        assert_eq!(id, Some(client_id));
    }

    #[test]
    fn test_create_column_request_absent_id_is_none() {
        let board_id = Uuid::new_v4();
        let req: CreateColumnRequest = serde_json::from_str(r#"{"name":"Backlog"}"#).unwrap();
        let (id, _) = req.into_new_column(board_id).unwrap();
        assert_eq!(id, None);
    }

    #[test]
    fn test_create_column_request_rejects_negative_wip_limit() {
        let board_id = Uuid::new_v4();
        let req = CreateColumnRequest {
            id: None,
            name: "X".to_string(),
            wip_limit: Some(-5),
        };
        assert!(req.into_new_column(board_id).is_err());
    }

    #[test]
    fn test_create_column_request_omits_position() {
        // Compile-lock: NewColumn carries no `position` field — the server
        // assigns the append index, so the create spec must not name it.
        let board_id = Uuid::new_v4();
        let req = CreateColumnRequest {
            id: None,
            name: "Backlog".to_string(),
            wip_limit: None,
        };
        let (_, spec) = req.into_new_column(board_id).unwrap();
        let NewColumn {
            board_id: _,
            name: _,
            wip_limit: _,
        } = spec;
    }

    #[test]
    fn test_replace_column_request_into_new_column_maps_fields_and_position() {
        let board_id = Uuid::new_v4();
        let req = ReplaceColumnRequest {
            name: "Doing".to_string(),
            position: 3,
            wip_limit: Some(2),
        };
        let (spec, position) = req.into_new_column(board_id).unwrap();
        assert_eq!(
            spec,
            NewColumn {
                board_id,
                name: "Doing".to_string(),
                wip_limit: Some(2),
            }
        );
        assert_eq!(position, 3);
    }

    #[test]
    fn test_replace_column_request_rejects_negative_position() {
        let board_id = Uuid::new_v4();
        let req = ReplaceColumnRequest {
            name: "X".to_string(),
            position: -1,
            wip_limit: None,
        };
        assert!(req.into_new_column(board_id).is_err());
    }

    #[test]
    fn test_replace_column_request_rejects_negative_wip_limit() {
        let board_id = Uuid::new_v4();
        let req = ReplaceColumnRequest {
            name: "X".to_string(),
            position: 0,
            wip_limit: Some(-1),
        };
        assert!(req.into_new_column(board_id).is_err());
    }

    #[test]
    fn test_reorder_column_request_validated_position_accepts_non_negative() {
        let req = ReorderColumnRequest { position: 5 };
        assert_eq!(req.validated_position().unwrap(), 5);
    }

    #[test]
    fn test_reorder_column_request_validated_position_rejects_negative() {
        let req = ReorderColumnRequest { position: -1 };
        assert!(req.validated_position().is_err());
    }
}
