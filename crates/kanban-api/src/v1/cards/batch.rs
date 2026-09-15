use super::requests::UpdateCardRequest;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct BatchArchiveRequest {
    pub ids: Vec<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct BatchMoveRequest {
    pub ids: Vec<Uuid>,
    pub column_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct BatchAssignSprintRequest {
    pub ids: Vec<Uuid>,
    pub sprint_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchUpdateItem {
    pub id: Uuid,
    #[serde(flatten)]
    pub update: UpdateCardRequest,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchUpdateRequest {
    pub updates: Vec<BatchUpdateItem>,
}

#[non_exhaustive]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchFailure {
    pub id: Uuid,
    pub error: String,
}

impl BatchFailure {
    pub fn new(id: Uuid, error: impl Into<String>) -> Self {
        Self {
            id,
            error: error.into(),
        }
    }
}

#[non_exhaustive]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchOperationResponse {
    pub succeeded: Vec<Uuid>,
    pub failed: Vec<BatchFailure>,
}

impl BatchOperationResponse {
    pub fn new(succeeded: Vec<Uuid>, failed: Vec<BatchFailure>) -> Self {
        Self { succeeded, failed }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::v1::Patch;
    use uuid::Uuid;

    #[test]
    fn test_batch_update_item_flattens_update_fields() {
        let id = Uuid::new_v4();
        let json = serde_json::json!({
            "id": id,
            "title": "T",
            "points": 3,
            "description": null,
        });

        let item: BatchUpdateItem = serde_json::from_value(json).unwrap();

        assert_eq!(item.id, id);
        assert_eq!(item.update.title, Some("T".to_string()));
        assert_eq!(item.update.points, Patch::Set(3));
        assert_eq!(item.update.description, Patch::Clear);
        assert_eq!(item.update.due_date, Patch::NoChange);
    }

    #[test]
    fn test_batch_operation_response_serde_round_trip() {
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        let response = BatchOperationResponse::new(
            vec![a],
            vec![BatchFailure::new(b, "Card ... not found".to_string())],
        );

        let value = serde_json::to_value(&response).unwrap();
        assert_eq!(value["succeeded"][0], a.to_string());
        assert_eq!(value["failed"][0]["id"], b.to_string());
        assert!(value["failed"][0]["error"].is_string());

        let round_tripped: BatchOperationResponse = serde_json::from_value(value).unwrap();
        assert_eq!(round_tripped.succeeded, response.succeeded);
        assert_eq!(round_tripped.failed[0].id, response.failed[0].id);
        assert_eq!(round_tripped.failed[0].error, response.failed[0].error);
    }
}
