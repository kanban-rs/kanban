//! Wire-to-domain conversions for the card request DTOs. Kept separate from the
//! struct definitions in `requests.rs` (wire shape vs mapping policy). The enum
//! mirrors carry the only validation (variant exhaustiveness); `points: u8` and
//! `position: i32` need no range check by design. All conversions destructure
//! the request and construct the domain target exhaustively (no `..`), so a new
//! field on either side is a compile error.

use super::requests::{CreateCardRequest, ReplaceCardRequest, UpdateCardRequest};
use kanban_domain::{
    CardPriority, CardUpdate, ColumnId, FieldUpdate, KanbanError, KanbanResult, NewCard,
};
use uuid::Uuid;

impl CreateCardRequest {
    /// Split the identity (optional client id) from the domain create spec. The
    /// service mints the id when `None` and calls `Card::create(spec, id,
    /// card_number, now)` with a server-minted `card_number` (NOT carried here).
    /// `column_id` is path-supplied (nested `POST /columns/:id/cards` route), so
    /// it is a parameter rather than a body field. An omitted `priority`
    /// defaults to `Medium`. Exhaustive destructure — no `..` — so a new field
    /// is a compile error.
    pub fn into_new_card(self, column_id: ColumnId) -> KanbanResult<(Option<Uuid>, NewCard)> {
        let CreateCardRequest {
            id,
            title,
            description,
            priority,
            due_date,
            points,
            sprint_id,
            tags,
        } = self;
        let spec = NewCard {
            column_id,
            title,
            description,
            priority: priority
                .map(CardPriority::from)
                .unwrap_or(CardPriority::Medium),
            due_date,
            points,
            sprint_id,
            tags: tags.unwrap_or_default(),
        };
        Ok((id, spec))
    }
}

impl TryFrom<UpdateCardRequest> for CardUpdate {
    type Error = KanbanError;

    fn try_from(req: UpdateCardRequest) -> KanbanResult<Self> {
        let UpdateCardRequest {
            title,
            priority,
            status,
            position,
            column_id,
            description,
            due_date,
            points,
            sprint_id,
            tags,
        } = req;
        Ok(CardUpdate {
            title,
            description: description.into(),
            priority: priority.map(Into::into),
            status: status.map(Into::into),
            position,
            column_id,
            due_date: due_date.into(),
            points: points.into(),
            sprint_id: sprint_id.into(),
            tags: tags.into(),
        })
    }
}

impl TryFrom<ReplaceCardRequest> for CardUpdate {
    type Error = KanbanError;

    fn try_from(req: ReplaceCardRequest) -> KanbanResult<Self> {
        let ReplaceCardRequest {
            title,
            priority,
            status,
            position,
            column_id,
            description,
            due_date,
            points,
            sprint_id,
            tags,
        } = req;
        Ok(CardUpdate {
            title: Some(title),
            description: description.into(),
            priority: Some(priority.into()),
            status: Some(status.into()),
            position: Some(position),
            column_id: Some(column_id),
            due_date: due_date.into(),
            points: points.into(),
            sprint_id: sprint_id.into(),
            tags: match tags {
                Some(v) => FieldUpdate::Set(v),
                None => FieldUpdate::Clear,
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::enums::{CardPriorityDto, CardStatusDto};
    use super::super::super::Patch;
    use super::*;
    use chrono::{TimeZone, Utc};
    use kanban_domain::FieldUpdate;

    #[test]
    fn test_create_card_request_into_new_card_maps_every_field() {
        let column_id = Uuid::new_v4();
        let client_id = Uuid::new_v4();
        let sprint_id = Uuid::new_v4();
        let due = Utc.with_ymd_and_hms(2026, 1, 2, 3, 4, 5).unwrap();
        let req = CreateCardRequest {
            id: Some(client_id),
            title: "Task".to_string(),
            description: Some("desc".to_string()),
            priority: Some(CardPriorityDto::High),
            due_date: Some(due),
            points: Some(5),
            sprint_id: Some(sprint_id),
            tags: Some(vec!["tag".to_string()]),
        };
        let (id, spec) = req.into_new_card(column_id).unwrap();
        assert_eq!(id, Some(client_id));
        assert_eq!(
            spec,
            NewCard {
                column_id,
                title: "Task".to_string(),
                description: Some("desc".to_string()),
                priority: CardPriority::High,
                due_date: Some(due),
                points: Some(5),
                sprint_id: Some(sprint_id),
                tags: vec!["tag".to_string()],
            }
        );
    }

    #[test]
    fn test_into_new_card_maps_some_tags_to_new_card() {
        let column_id = Uuid::new_v4();
        let req = CreateCardRequest {
            id: None,
            title: "Task".to_string(),
            description: None,
            priority: None,
            due_date: None,
            points: None,
            sprint_id: None,
            tags: Some(vec!["a".to_string()]),
        };
        let (_, spec) = req.into_new_card(column_id).unwrap();
        assert_eq!(spec.tags, vec!["a".to_string()]);
    }

    #[test]
    fn test_into_new_card_maps_none_tags_to_empty() {
        let column_id = Uuid::new_v4();
        let req: CreateCardRequest = serde_json::from_str(r#"{"title":"x"}"#).unwrap();
        let (_, spec) = req.into_new_card(column_id).unwrap();
        assert!(spec.tags.is_empty());
    }

    #[test]
    fn test_create_card_request_defaults_priority_to_medium_when_omitted() {
        let column_id = Uuid::new_v4();
        let req: CreateCardRequest = serde_json::from_str(r#"{"title":"x"}"#).unwrap();
        let (id, spec) = req.into_new_card(column_id).unwrap();
        assert_eq!(id, None);
        assert_eq!(spec.priority, CardPriority::Medium);
    }

    #[test]
    fn test_update_card_request_into_card_update_maps_patch_to_field_update() {
        let column_id = Uuid::new_v4();
        let req = UpdateCardRequest {
            title: Some("New".to_string()),
            priority: Some(CardPriorityDto::Low),
            status: Some(CardStatusDto::InProgress),
            position: Some(7),
            column_id: Some(column_id),
            description: Patch::Clear,
            due_date: Patch::NoChange,
            points: Patch::Set(2),
            sprint_id: Patch::NoChange,
            tags: Patch::NoChange,
        };
        let update = CardUpdate::try_from(req).unwrap();
        assert_eq!(update.title, Some("New".to_string()));
        assert_eq!(update.priority, Some(CardPriority::Low));
        assert_eq!(update.status, Some(kanban_domain::CardStatus::InProgress));
        assert_eq!(update.position, Some(7));
        assert_eq!(update.column_id, Some(column_id));
        assert_eq!(update.description, FieldUpdate::Clear);
        assert_eq!(update.due_date, FieldUpdate::NoChange);
        assert_eq!(update.points, FieldUpdate::Set(2));
        assert_eq!(update.sprint_id, FieldUpdate::NoChange);
        assert_eq!(update.tags, FieldUpdate::NoChange);
    }

    #[test]
    fn test_replace_card_request_clears_omitted_optionals() {
        let column_id = Uuid::new_v4();
        let json = format!(
            r#"{{"title":"R","priority":"medium","status":"done","position":1,"column_id":"{column_id}"}}"#
        );
        let req: ReplaceCardRequest = serde_json::from_str(&json).unwrap();
        let update = CardUpdate::try_from(req).unwrap();
        assert_eq!(update.title, Some("R".to_string()));
        assert_eq!(update.priority, Some(CardPriority::Medium));
        assert_eq!(update.status, Some(kanban_domain::CardStatus::Done));
        assert_eq!(update.position, Some(1));
        assert_eq!(update.column_id, Some(column_id));
        assert_eq!(update.description, FieldUpdate::Clear);
        assert_eq!(update.due_date, FieldUpdate::Clear);
        assert_eq!(update.points, FieldUpdate::Clear);
        assert_eq!(update.sprint_id, FieldUpdate::Clear);
    }

    #[test]
    fn test_update_request_tags_set_maps_to_field_update_set() {
        let req = UpdateCardRequest {
            title: None,
            priority: None,
            status: None,
            position: None,
            column_id: None,
            description: Patch::NoChange,
            due_date: Patch::NoChange,
            points: Patch::NoChange,
            sprint_id: Patch::NoChange,
            tags: Patch::Set(vec!["a".to_string()]),
        };
        let card_update = CardUpdate::try_from(req).unwrap();
        assert!(
            matches!(card_update.tags, FieldUpdate::Set(ref v) if v == &vec!["a".to_string()]),
            "Patch::Set([\"a\"]) must map to FieldUpdate::Set([\"a\"])"
        );
    }

    #[test]
    fn test_update_request_tags_clear_maps_to_field_update_clear() {
        let req = UpdateCardRequest {
            title: None,
            priority: None,
            status: None,
            position: None,
            column_id: None,
            description: Patch::NoChange,
            due_date: Patch::NoChange,
            points: Patch::NoChange,
            sprint_id: Patch::NoChange,
            tags: Patch::Clear,
        };
        let card_update = CardUpdate::try_from(req).unwrap();
        assert!(
            matches!(card_update.tags, FieldUpdate::Clear),
            "Patch::Clear must map to FieldUpdate::Clear"
        );
    }

    #[test]
    fn test_update_request_tags_nochange_maps_to_no_change() {
        let req = UpdateCardRequest {
            title: None,
            priority: None,
            status: None,
            position: None,
            column_id: None,
            description: Patch::NoChange,
            due_date: Patch::NoChange,
            points: Patch::NoChange,
            sprint_id: Patch::NoChange,
            tags: Patch::NoChange,
        };
        let card_update = CardUpdate::try_from(req).unwrap();
        assert!(
            matches!(card_update.tags, FieldUpdate::NoChange),
            "Patch::NoChange must map to FieldUpdate::NoChange"
        );
    }

    #[test]
    fn test_replace_request_tags_none_clears_some_sets() {
        // tags: None -> FieldUpdate::Clear (replace semantics: absent = cleared)
        let req_none = ReplaceCardRequest {
            title: "R".to_string(),
            priority: CardPriorityDto::Medium,
            status: CardStatusDto::Todo,
            position: 0,
            column_id: Uuid::new_v4(),
            description: None,
            due_date: None,
            points: None,
            sprint_id: None,
            tags: None,
        };
        let update_none = CardUpdate::try_from(req_none).unwrap();
        assert!(
            matches!(update_none.tags, FieldUpdate::Clear),
            "ReplaceCardRequest tags:None must map to FieldUpdate::Clear"
        );

        // tags: Some(["x"]) -> FieldUpdate::Set(["x"])
        let req_some = ReplaceCardRequest {
            title: "R".to_string(),
            priority: CardPriorityDto::Medium,
            status: CardStatusDto::Todo,
            position: 0,
            column_id: Uuid::new_v4(),
            description: None,
            due_date: None,
            points: None,
            sprint_id: None,
            tags: Some(vec!["x".to_string()]),
        };
        let update_some = CardUpdate::try_from(req_some).unwrap();
        assert!(
            matches!(update_some.tags, FieldUpdate::Set(ref v) if v == &vec!["x".to_string()]),
            "ReplaceCardRequest tags:Some([\"x\"]) must map to FieldUpdate::Set([\"x\"])"
        );
    }

    #[test]
    fn test_card_conversions_destructure_exhaustively() {
        // Marker: every conversion in this module destructures the request and
        // constructs the domain target naming every field (no `..`,
        // no `Default::default()`). A CI grep over this file + `response.rs`
        // enforces the absence of those tokens; adding a `NewCard`/`CardUpdate`
        // field then breaks the corresponding arm at compile time.
    }
}
