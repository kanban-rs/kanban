//! Wire-to-domain conversions for the sprint request DTOs. Kept separate from
//! the struct definitions in `requests.rs`: the wire shape and the mapping
//! policy (server-managed exclusion, create-then-mint, true-replace) change for
//! different reasons. Each conversion destructures and constructs exhaustively
//! (no `..`) so a new field is a compile error.
//!
//! Asymmetry vs Board/Card: the create DTO carries a `name` STRING, but the
//! domain stores a `name_index`, and `NewSprint` additionally requires the
//! minted `sprint_number`. Both are allocated by the SERVICE against the owning
//! board. The DTO→domain seam therefore stops at [`CreateSprintParts`]; slice D
//! mints `sprint_number` + `name_index` and assembles the final `NewSprint`.

use super::requests::{CreateSprintRequest, ReplaceSprintRequest, UpdateSprintRequest};
use kanban_domain::{FieldUpdate, SprintUpdate};
use uuid::Uuid;

/// Intermediate split of a [`CreateSprintRequest`]: identity (optional client
/// `id`) plus the still-unminted content. The service consumes this, mints
/// `sprint_number`/`name_index` from the board, and assembles the final
/// `NewSprint`. `id` is identity, not content, so it travels separately.
#[derive(Debug, Clone, PartialEq)]
pub struct CreateSprintParts {
    pub id: Option<Uuid>,
    pub name: Option<String>,
    pub prefix: Option<String>,
    pub card_prefix: Option<String>,
}

impl From<CreateSprintRequest> for CreateSprintParts {
    fn from(req: CreateSprintRequest) -> Self {
        let CreateSprintRequest {
            id,
            name,
            prefix,
            card_prefix,
        } = req;
        CreateSprintParts {
            id,
            name,
            prefix,
            card_prefix,
        }
    }
}

impl From<UpdateSprintRequest> for SprintUpdate {
    fn from(req: UpdateSprintRequest) -> Self {
        let UpdateSprintRequest {
            name,
            prefix,
            card_prefix,
        } = req;
        SprintUpdate {
            name,
            prefix: prefix.into(),
            card_prefix: card_prefix.into(),
            // Server-managed / lifecycle — never accepted from a PATCH body;
            // sprint name allocation and lifecycle transitions are dedicated ops:
            name_index: FieldUpdate::NoChange,
            status: None,
            start_date: FieldUpdate::NoChange,
            end_date: FieldUpdate::NoChange,
        }
    }
}

impl From<ReplaceSprintRequest> for SprintUpdate {
    fn from(req: ReplaceSprintRequest) -> Self {
        let ReplaceSprintRequest {
            name,
            prefix,
            card_prefix,
        } = req;
        // True full replace: present nullable fields → Set, absent → Clear;
        // lifecycle/server fields untouched (NoChange / None).
        SprintUpdate {
            name,
            prefix: option_to_field_update(prefix),
            card_prefix: option_to_field_update(card_prefix),
            name_index: FieldUpdate::NoChange,
            status: None,
            start_date: FieldUpdate::NoChange,
            end_date: FieldUpdate::NoChange,
        }
    }
}

fn option_to_field_update<T>(value: Option<T>) -> FieldUpdate<T> {
    match value {
        Some(v) => FieldUpdate::Set(v),
        None => FieldUpdate::Clear,
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::Patch;
    use super::*;

    #[test]
    fn test_create_sprint_request_into_parts_carries_id_and_content() {
        let id = Uuid::new_v4();
        let req = CreateSprintRequest {
            id: Some(id),
            name: Some("S1".to_string()),
            prefix: Some("SPR".to_string()),
            card_prefix: Some("KAN".to_string()),
        };
        let parts: CreateSprintParts = req.into();
        assert_eq!(
            parts,
            CreateSprintParts {
                id: Some(id),
                name: Some("S1".to_string()),
                prefix: Some("SPR".to_string()),
                card_prefix: Some("KAN".to_string()),
            }
        );
    }

    #[test]
    fn test_create_sprint_request_into_parts_preserves_absent_id() {
        let req: CreateSprintRequest = serde_json::from_str(r#"{"name":"S1"}"#).unwrap();
        let parts: CreateSprintParts = req.into();
        assert_eq!(parts.id, None);
        assert_eq!(parts.name, Some("S1".to_string()));
    }

    #[test]
    fn test_update_request_to_sprint_update_leaves_lifecycle_fields_unchanged() {
        let req = UpdateSprintRequest {
            name: Some("N".to_string()),
            prefix: Patch::Set("SPR".to_string()),
            card_prefix: Patch::Clear,
        };
        let update: SprintUpdate = req.into();
        assert_eq!(update.name, Some("N".to_string()));
        assert_eq!(update.prefix, FieldUpdate::Set("SPR".to_string()));
        assert_eq!(update.card_prefix, FieldUpdate::Clear);
        // Lifecycle / server-managed untouched:
        assert_eq!(update.name_index, FieldUpdate::NoChange);
        assert_eq!(update.status, None);
        assert_eq!(update.start_date, FieldUpdate::NoChange);
        assert_eq!(update.end_date, FieldUpdate::NoChange);
    }

    #[test]
    fn test_replace_sprint_request_clears_omitted_nullable_fields() {
        let req: ReplaceSprintRequest =
            serde_json::from_str(r#"{"name":"Fresh","prefix":"SPR"}"#).unwrap();
        let update: SprintUpdate = req.into();
        assert_eq!(update.name, Some("Fresh".to_string()));
        assert_eq!(update.prefix, FieldUpdate::Set("SPR".to_string()));
        // Omitted nullable → Clear (wholesale replace):
        assert_eq!(update.card_prefix, FieldUpdate::Clear);
        assert_eq!(update.name_index, FieldUpdate::NoChange);
        assert_eq!(update.status, None);
        assert_eq!(update.start_date, FieldUpdate::NoChange);
        assert_eq!(update.end_date, FieldUpdate::NoChange);
    }

    #[test]
    fn test_sprint_conversions_destructure_exhaustively() {
        // Marker: every conversion in this module destructures the request and
        // constructs the domain target naming every field (no `..`,
        // no `Default::default()`). A CI grep over this file + `response.rs`
        // enforces the absence of those tokens; adding a `NewSprint`/
        // `SprintUpdate`/`Sprint` field then breaks the corresponding arm at
        // compile time.
    }
}
