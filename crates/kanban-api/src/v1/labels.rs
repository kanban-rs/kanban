//! Wire types for the global label registry routes.
//!
//! Labels are identified by name, not UUID, so the routes carry the name in
//! the path and the bodies only describe the label's metadata or the batch
//! of cards it applies to.

use kanban_domain::Label;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A single label from the global registry. `name` is the identity key;
/// `color` and `description` are optional metadata.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct LabelResponse {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

impl From<&Label> for LabelResponse {
    fn from(label: &Label) -> Self {
        Self {
            name: label.name.clone(),
            color: label.color.clone(),
            description: label.description.clone(),
        }
    }
}

/// Body for `PUT /v1/labels/{name}`. The name travels in the path; the body
/// carries the metadata. The label is replaced in full, so a `color` or
/// `description` omitted from the body is cleared.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct UpsertLabelRequest {
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
}

impl UpsertLabelRequest {
    /// Assemble the domain label using the name the route path carried.
    pub fn into_label(self, name: String) -> Label {
        Label::new(name, self.color, self.description)
    }
}

/// Body for `POST /v1/labels/{name}/cards` (apply the label) and
/// `DELETE /v1/labels/{name}/cards` (remove the label). `card_ids` lists
/// the cards the label is applied to or removed from.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct LabelCardsRequest {
    pub card_ids: Vec<Uuid>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_label_response_from_domain_label() {
        let label = Label::new(
            "backend",
            Some("#aa0000".to_string()),
            Some("Server-side work".to_string()),
        );

        let resp = LabelResponse::from(&label);

        assert_eq!(resp.name, "backend");
        assert_eq!(resp.color.as_deref(), Some("#aa0000"));
        assert_eq!(resp.description.as_deref(), Some("Server-side work"));
    }

    #[test]
    fn test_label_response_omits_none_metadata_on_the_wire() {
        let label = Label::named("bug");
        let resp = LabelResponse::from(&label);
        let value = serde_json::to_value(&resp).unwrap();

        assert_eq!(value["name"], "bug");
        assert!(
            value.get("color").is_none(),
            "omitted color must not be serialized, got: {value}"
        );
        assert!(
            value.get("description").is_none(),
            "omitted description must not be serialized, got: {value}"
        );
    }

    #[test]
    fn test_upsert_label_request_into_label_uses_path_name() {
        let req = UpsertLabelRequest {
            color: Some("#00aa00".to_string()),
            description: None,
        };

        let label = req.into_label("feat".to_string());

        assert_eq!(label.name, "feat");
        assert_eq!(label.color.as_deref(), Some("#00aa00"));
        assert!(label.description.is_none());
    }

    #[test]
    fn test_upsert_label_request_empty_body_clears_metadata() {
        let req: UpsertLabelRequest = serde_json::from_value(serde_json::json!({})).unwrap();
        let label = req.into_label("clear".to_string());

        assert!(label.color.is_none());
        assert!(label.description.is_none());
    }

    #[test]
    fn test_label_cards_request_shape() {
        let id_a = Uuid::new_v4();
        let id_b = Uuid::new_v4();
        let json = serde_json::json!({ "card_ids": [id_a, id_b] });

        let req: LabelCardsRequest = serde_json::from_value(json).unwrap();

        assert_eq!(req.card_ids, vec![id_a, id_b]);
    }
}
