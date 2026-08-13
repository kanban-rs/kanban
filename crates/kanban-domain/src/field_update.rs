/// Represents a field update operation for partial updates
///
/// This type provides a clear, three-state pattern for updating optional fields:
/// - `NoChange`: Field keeps its existing value
/// - `Set(value)`: Field is updated to the provided value
/// - `Clear`: Field is cleared (set to None)
///
/// # Example
///
/// ```
/// use kanban_domain::FieldUpdate;
///
/// let title_update = FieldUpdate::Set("New Title".to_string());
/// let description_update: FieldUpdate<String> = FieldUpdate::Clear;
/// let priority_update: FieldUpdate<i32> = FieldUpdate::NoChange;
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum FieldUpdate<T> {
    /// Do not modify this field (keep existing value)
    #[default]
    NoChange,
    /// Set the field to the provided value
    Set(T),
    /// Clear the field (set to None)
    Clear,
}

impl<T> FieldUpdate<T> {
    /// Apply this update to an optional field
    ///
    /// # Example
    ///
    /// ```
    /// use kanban_domain::FieldUpdate;
    ///
    /// let mut field = Some("old value".to_string());
    /// let update = FieldUpdate::Set("new value".to_string());
    /// update.apply_to(&mut field);
    /// assert_eq!(field, Some("new value".to_string()));
    ///
    /// let clear = FieldUpdate::Clear;
    /// clear.apply_to(&mut field);
    /// assert_eq!(field, None);
    /// ```
    pub fn apply_to(self, field: &mut Option<T>) {
        match self {
            FieldUpdate::NoChange => {}
            FieldUpdate::Set(value) => *field = Some(value),
            FieldUpdate::Clear => *field = None,
        }
    }

    /// Check if this represents a change (not NoChange)
    pub fn is_change(&self) -> bool {
        !matches!(self, FieldUpdate::NoChange)
    }
}

impl<T> From<Option<T>> for FieldUpdate<T> {
    /// Convert Option<T> to FieldUpdate<T>
    /// - Some(value) becomes Set(value)
    /// - None becomes Clear
    fn from(opt: Option<T>) -> Self {
        match opt {
            Some(value) => FieldUpdate::Set(value),
            None => FieldUpdate::Clear,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_field_update_serde_roundtrip_set() {
        let update = FieldUpdate::Set("hello".to_string());
        let json = serde_json::to_string(&update).unwrap();
        let back: FieldUpdate<String> = serde_json::from_str(&json).unwrap();
        assert_eq!(update, back);
    }

    #[test]
    fn test_field_update_serde_roundtrip_no_change() {
        let update: FieldUpdate<String> = FieldUpdate::NoChange;
        let json = serde_json::to_string(&update).unwrap();
        let back: FieldUpdate<String> = serde_json::from_str(&json).unwrap();
        assert_eq!(update, back);
    }

    #[test]
    fn test_field_update_serde_roundtrip_clear() {
        let update: FieldUpdate<String> = FieldUpdate::Clear;
        let json = serde_json::to_string(&update).unwrap();
        let back: FieldUpdate<String> = serde_json::from_str(&json).unwrap();
        assert_eq!(update, back);
    }

    #[test]
    fn test_field_update_serde_roundtrip_numeric() {
        let update = FieldUpdate::Set(42u32);
        let json = serde_json::to_string(&update).unwrap();
        let back: FieldUpdate<u32> = serde_json::from_str(&json).unwrap();
        assert_eq!(update, back);
    }

    #[test]
    fn test_field_update_serde_roundtrip_uuid() {
        let id = uuid::Uuid::new_v4();
        let update = FieldUpdate::Set(id);
        let json = serde_json::to_string(&update).unwrap();
        let back: FieldUpdate<uuid::Uuid> = serde_json::from_str(&json).unwrap();
        assert_eq!(update, back);
    }

    #[test]
    fn test_board_update_serde_roundtrip() {
        let update = crate::BoardUpdate {
            name: Some("Test".to_string()),
            description: FieldUpdate::Set("desc".to_string()),
            sprint_prefix: FieldUpdate::Clear,
            card_prefix: FieldUpdate::NoChange,
            ..Default::default()
        };
        let json = serde_json::to_string(&update).unwrap();
        let back: crate::BoardUpdate = serde_json::from_str(&json).unwrap();
        assert_eq!(back.name, Some("Test".to_string()));
    }

    #[test]
    fn test_card_update_serde_roundtrip() {
        let update = crate::CardUpdate {
            title: Some("Card".to_string()),
            description: FieldUpdate::Set("desc".to_string()),
            priority: Some(crate::CardPriority::High),
            points: FieldUpdate::Set(5),
            ..Default::default()
        };
        let json = serde_json::to_string(&update).unwrap();
        let back: crate::CardUpdate = serde_json::from_str(&json).unwrap();
        assert_eq!(back.title, Some("Card".to_string()));
    }

    #[test]
    fn test_create_card_options_serde_roundtrip() {
        let opts = crate::CreateCardOptions {
            description: Some("desc".to_string()),
            priority: Some(crate::CardPriority::Medium),
            points: Some(3),
            due_date: None,
            sprint_id: None,
        };
        let json = serde_json::to_string(&opts).unwrap();
        let back: crate::CreateCardOptions = serde_json::from_str(&json).unwrap();
        assert_eq!(back.description, Some("desc".to_string()));
    }

    #[test]
    fn test_column_update_serde_roundtrip() {
        let update = crate::ColumnUpdate {
            name: Some("Col".to_string()),
            position: Some(1),
            wip_limit: FieldUpdate::Set(5),
            default_status: None,
        };
        let json = serde_json::to_string(&update).unwrap();
        let back: crate::ColumnUpdate = serde_json::from_str(&json).unwrap();
        assert_eq!(back.name, Some("Col".to_string()));
    }

    #[test]
    fn test_sprint_update_serde_roundtrip() {
        let update = crate::SprintUpdate {
            name: Some("Sprint 1".to_string()),
            name_index: FieldUpdate::Set(0),
            prefix: FieldUpdate::Set("SPR".to_string()),
            card_prefix: FieldUpdate::Clear,
            ..Default::default()
        };
        let json = serde_json::to_string(&update).unwrap();
        let back: crate::SprintUpdate = serde_json::from_str(&json).unwrap();
        assert_eq!(back.name, Some("Sprint 1".to_string()));
    }
}
