//! Global label registry.
//!
//! A `Label` is a named, optional-coloured, optional-descriptive taxonomy
//! entry shared across the workspace. It is distinct from `Card.labels`, which
//! are free-form strings on a card: the `labels` table is the canonical
//! registry, while `Card.labels` is the per-card association (the set of label
//! names applied to that card).
//!
//! Backed by the `labels` table in the SQLite schema and the top-level
//! `labels` array in the JSON board file.

use serde::{Deserialize, Serialize};

/// A global label (taxonomy entry).
///
/// `name` is the natural primary key. No UUID is generated: the name is the
/// identity, so two callers that both `upsert_label("bug")` create/update the
/// same row, exactly like `Prefix::new` collapses by normalised name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Label {
    /// Uniqueness key. Matching is case-sensitive to allow
    /// `Bug` and `bug` to coexist; the resolver is case-sensitive.
    pub name: String,
    /// CSS-style colour (`"#rrggbb"` or a named colour), optional.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    /// Free-text description, optional.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

impl Label {
    /// Construct with all three fields.
    pub fn new(
        name: impl Into<String>,
        color: Option<String>,
        description: Option<String>,
    ) -> Self {
        Self {
            name: name.into(),
            color,
            description,
        }
    }

    /// Construct a label with only a name (used when migrating existing
    /// `Card.labels` values to the new registry).
    pub fn named(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            color: None,
            description: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_label_new_accepts_str_args() {
        let label = Label::new(
            "backend",
            Some("#aa0000".to_string()),
            Some("Server-side work".to_string()),
        );
        assert_eq!(label.name, "backend");
        assert_eq!(label.color.as_deref(), Some("#aa0000"));
        assert_eq!(label.description.as_deref(), Some("Server-side work"));
    }

    #[test]
    fn test_label_named_minimal() {
        let label = Label::named("bug");
        assert_eq!(label.name, "bug");
        assert!(label.color.is_none());
        assert!(label.description.is_none());
    }

    #[test]
    fn test_label_serde_roundtrip() {
        let label = Label::new("security", Some("#00aa00".to_string()), None);
        let json = serde_json::to_string(&label).unwrap();
        let back: Label = serde_json::from_str(&json).unwrap();
        assert_eq!(back, label);
    }

    #[test]
    fn test_label_serde_skips_none_color_and_description() {
        let label = Label::named("bug");
        let value: serde_json::Value = serde_json::to_value(&label).unwrap();
        assert!(value.get("color").is_none(), "None color must be skipped");
        assert!(
            value.get("description").is_none(),
            "None description must be skipped"
        );
    }
}
