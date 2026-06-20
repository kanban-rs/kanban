//! Wire-to-domain conversions + validation for the column request DTOs. Kept
//! separate from the struct definitions in `requests.rs` (wire shape vs mapping
//! policy). Conversions are the validation boundary and destructure/construct
//! exhaustively (no `..`).

use super::super::conv::set_or_no_change;
use super::super::Patch;
use super::requests::{CreateColumnRequest, ReplaceColumnRequest, UpdateColumnRequest};
use kanban_domain::{ColumnUpdate, KanbanError, KanbanResult};

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

impl CreateColumnRequest {
    /// Split into the `create_column(_, name, _)` arg plus a follow-up
    /// [`ColumnUpdate`] for `wip_limit` (the handler runs create-then-update;
    /// position is server-assigned on append). Present `wip_limit` → `Set`,
    /// absent → `NoChange`; validates non-negativity.
    pub fn into_parts(self) -> KanbanResult<(String, ColumnUpdate)> {
        let CreateColumnRequest { name, wip_limit } = self;
        if let Some(limit) = wip_limit {
            validate_wip_limit(limit)?;
        }
        let follow_up = ColumnUpdate {
            name: None,
            position: None,
            wip_limit: set_or_no_change(wip_limit),
        };
        Ok((name, follow_up))
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
    fn test_create_column_request_into_parts_sets_wip_limit() {
        let req = CreateColumnRequest {
            name: "Doing".to_string(),
            wip_limit: Some(3),
        };
        let (name, follow_up) = req.into_parts().unwrap();
        assert_eq!(name, "Doing");
        assert_eq!(follow_up.wip_limit, FieldUpdate::Set(3));
        assert_eq!(follow_up.name, None);
        assert_eq!(follow_up.position, None);
    }

    #[test]
    fn test_create_column_request_into_parts_absent_wip_is_no_change() {
        let req = CreateColumnRequest {
            name: "Backlog".to_string(),
            wip_limit: None,
        };
        let (_, follow_up) = req.into_parts().unwrap();
        assert_eq!(follow_up.wip_limit, FieldUpdate::NoChange); // not Clear
    }

    #[test]
    fn test_create_column_request_into_parts_rejects_negative_wip() {
        let req = CreateColumnRequest {
            name: "X".to_string(),
            wip_limit: Some(-5),
        };
        assert!(req.into_parts().is_err());
    }
}
