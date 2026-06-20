use kanban_domain::FieldUpdate;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// A JSON Merge Patch (RFC 7386) field for PATCH request bodies. On the wire:
///
/// - **absent** → [`Patch::NoChange`] (the field uses `#[serde(default)]`)
/// - **`null`** → [`Patch::Clear`]
/// - **value** → [`Patch::Set`]
///
/// Converts to the domain [`FieldUpdate`] via `From`. Fields carrying a `Patch`
/// must be tagged `#[serde(default, skip_serializing_if = "Patch::is_no_change")]`
/// so the absent and serialized-omitted cases line up.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Patch<T> {
    /// Field absent from the body — leave the existing value untouched.
    #[default]
    NoChange,
    /// Field present and `null` — clear the existing value.
    Clear,
    /// Field present with a value — set it.
    Set(T),
}

impl<T> Patch<T> {
    /// True for [`Patch::NoChange`]; used as `skip_serializing_if` so an
    /// untouched field is omitted from the serialized body.
    pub fn is_no_change(&self) -> bool {
        matches!(self, Patch::NoChange)
    }
}

impl<T> From<Patch<T>> for FieldUpdate<T> {
    fn from(patch: Patch<T>) -> Self {
        match patch {
            Patch::NoChange => FieldUpdate::NoChange,
            Patch::Clear => FieldUpdate::Clear,
            Patch::Set(value) => FieldUpdate::Set(value),
        }
    }
}

// Only ever called for a *present* field (absent is handled by `#[serde(default)]`),
// so `null` maps to `Clear` and any value maps to `Set`.
impl<'de, T: Deserialize<'de>> Deserialize<'de> for Patch<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(match Option::<T>::deserialize(deserializer)? {
            None => Patch::Clear,
            Some(value) => Patch::Set(value),
        })
    }
}

impl<T: Serialize> Serialize for Patch<T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Patch::Set(value) => value.serialize(serializer),
            // `NoChange` is normally skipped via `skip_serializing_if`; if it
            // reaches here it serializes the same as `Clear` (null).
            Patch::NoChange | Patch::Clear => serializer.serialize_none(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kanban_domain::FieldUpdate;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Serialize, Deserialize)]
    struct Probe {
        #[serde(default, skip_serializing_if = "Patch::is_no_change")]
        field: Patch<String>,
    }

    #[test]
    fn test_absent_field_deserializes_to_no_change() {
        let p: Probe = serde_json::from_str("{}").unwrap();
        assert_eq!(p.field, Patch::NoChange);
    }

    #[test]
    fn test_null_field_deserializes_to_clear() {
        let p: Probe = serde_json::from_str(r#"{"field":null}"#).unwrap();
        assert_eq!(p.field, Patch::Clear);
    }

    #[test]
    fn test_present_value_deserializes_to_set() {
        let p: Probe = serde_json::from_str(r#"{"field":"hello"}"#).unwrap();
        assert_eq!(p.field, Patch::Set("hello".to_string()));
    }

    #[test]
    fn test_no_change_is_omitted_on_serialize() {
        let json = serde_json::to_string(&Probe {
            field: Patch::NoChange,
        })
        .unwrap();
        assert_eq!(json, "{}");
    }

    #[test]
    fn test_clear_serializes_to_null() {
        let json = serde_json::to_string(&Probe {
            field: Patch::Clear,
        })
        .unwrap();
        assert_eq!(json, r#"{"field":null}"#);
    }

    #[test]
    fn test_set_serializes_to_value() {
        let json = serde_json::to_string(&Probe {
            field: Patch::Set("x".to_string()),
        })
        .unwrap();
        assert_eq!(json, r#"{"field":"x"}"#);
    }

    #[test]
    fn test_into_field_update_maps_all_three_states() {
        assert_eq!(
            FieldUpdate::<String>::from(Patch::NoChange),
            FieldUpdate::NoChange
        );
        assert_eq!(
            FieldUpdate::<String>::from(Patch::Clear),
            FieldUpdate::Clear
        );
        assert_eq!(
            FieldUpdate::from(Patch::Set("v".to_string())),
            FieldUpdate::Set("v".to_string())
        );
    }

    #[test]
    fn test_default_is_no_change() {
        assert_eq!(Patch::<String>::default(), Patch::NoChange);
    }
}
