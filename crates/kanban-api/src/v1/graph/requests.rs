use crate::v1::enums::{RelatesKindDto, SeverityDto};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct AttachChildrenRequest {
    pub children: Vec<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct AddBlockRequest {
    pub blocked: Uuid,
    #[serde(default)]
    pub severity: SeverityDto,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct AddRelatedRequest {
    pub other: Uuid,
    #[serde(default)]
    pub kind: RelatesKindDto,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_attach_children_request_requires_children_field() {
        let err =
            serde_json::from_value::<AttachChildrenRequest>(serde_json::json!({})).unwrap_err();
        assert!(err.to_string().contains("children"));
    }

    #[test]
    fn test_add_block_request_defaults_severity_to_medium() {
        let req: AddBlockRequest =
            serde_json::from_value(serde_json::json!({"blocked": Uuid::nil()})).unwrap();
        assert_eq!(req.severity, SeverityDto::Medium);
    }

    #[test]
    fn test_add_related_request_defaults_kind_to_general() {
        let req: AddRelatedRequest =
            serde_json::from_value(serde_json::json!({"other": Uuid::nil()})).unwrap();
        assert_eq!(req.kind, RelatesKindDto::General);
    }
}
