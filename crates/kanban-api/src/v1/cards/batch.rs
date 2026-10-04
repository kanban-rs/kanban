use super::requests::UpdateCardRequest;
use crate::v1::invalidation::invalidation_all;
use crate::v1::{ApiError, ErrorCode};
use crate::InvalidationDto;
use kanban_domain::Invalidation;
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
    /// Absent from servers that predate per-failure codes, and `None` for a code
    /// this build does not know; read it through [`BatchFailure::to_api_error`].
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "lenient_error_code"
    )]
    pub code: Option<ErrorCode>,
}

impl BatchFailure {
    pub fn new(id: Uuid, error: impl Into<String>) -> Self {
        Self {
            id,
            error: error.into(),
            code: None,
        }
    }

    pub fn from_api_error(id: Uuid, err: &ApiError) -> Self {
        Self {
            id,
            error: err.message.clone(),
            code: Some(err.code),
        }
    }

    pub fn to_api_error(&self) -> ApiError {
        ApiError::new(
            self.code.unwrap_or(ErrorCode::ValidationFailed),
            self.error.clone(),
        )
    }
}

fn lenient_error_code<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> Result<Option<ErrorCode>, D::Error> {
    use serde::de::value::{Error as ValueError, StringDeserializer};
    use serde::de::IntoDeserializer;
    let raw: Option<String> = Option::deserialize(d)?;
    Ok(raw.and_then(|s| {
        let de: StringDeserializer<ValueError> = s.into_deserializer();
        ErrorCode::deserialize(de).ok()
    }))
}

#[non_exhaustive]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchOperationResponse {
    pub succeeded: Vec<Uuid>,
    pub failed: Vec<BatchFailure>,
    #[serde(default = "invalidation_all")]
    pub invalidation: InvalidationDto,
}

impl BatchOperationResponse {
    pub fn new(
        succeeded: Vec<Uuid>,
        failed: Vec<BatchFailure>,
        invalidation: &Invalidation,
    ) -> Self {
        Self {
            succeeded,
            failed,
            invalidation: InvalidationDto::from(invalidation),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::v1::Patch;
    use kanban_domain::EntityIds;
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
        let invalidation = Invalidation::All;
        let response = BatchOperationResponse::new(
            vec![a],
            vec![BatchFailure::new(b, "Card ... not found".to_string())],
            &invalidation,
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

    #[test]
    fn test_batch_operation_response_carries_the_invalidation_on_the_wire() {
        let card_id = Uuid::new_v4();
        let invalidation = Invalidation::Entities(EntityIds::cards([card_id]));
        let response = BatchOperationResponse::new(vec![card_id], vec![], &invalidation);

        let value = serde_json::to_value(&response).unwrap();
        assert_eq!(value["invalidation"]["scope"], "entities");
        assert!(value["invalidation"]["entities"]["cards"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!(card_id)));
    }

    #[test]
    fn test_batch_operation_response_without_invalidation_deserializes_as_all() {
        let value = serde_json::json!({ "succeeded": [], "failed": [] });

        let response: BatchOperationResponse = serde_json::from_value(value).unwrap();

        assert_eq!(response.invalidation, InvalidationDto::All);
    }

    #[test]
    fn test_batch_failure_from_api_error_serializes_its_code() {
        let id = Uuid::new_v4();
        let api_err = ApiError::new(ErrorCode::NotFound, "Card x not found");

        let failure = BatchFailure::from_api_error(id, &api_err);
        let value = serde_json::to_value(&failure).unwrap();

        assert_eq!(value["code"], "NOT_FOUND");
        assert_eq!(value["error"], "Card x not found");
    }

    #[test]
    fn test_batch_failure_without_a_code_deserializes_and_falls_back_to_validation_failed() {
        let id = Uuid::new_v4();
        let value = serde_json::json!({ "id": id, "error": "boom" });

        let failure: BatchFailure = serde_json::from_value(value).unwrap();

        assert_eq!(failure.code, None);
        assert_eq!(
            failure.to_api_error(),
            ApiError::new(ErrorCode::ValidationFailed, "boom")
        );
    }

    #[test]
    fn test_batch_failure_with_an_unknown_code_deserializes_with_no_code() {
        let id = Uuid::new_v4();
        let value = serde_json::json!({ "id": id, "error": "boom", "code": "SOME_FUTURE_CODE" });

        let failure: BatchFailure = serde_json::from_value(value).unwrap();

        assert_eq!(failure.code, None);
        assert_eq!(failure.to_api_error().code, ErrorCode::ValidationFailed);
    }
}
