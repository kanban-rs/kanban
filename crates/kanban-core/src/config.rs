use crate::CoreResult;
use serde::{Deserialize, Serialize};

pub const DEFAULT_STORAGE_BACKEND: &str = "json";
pub const DEFAULT_JSON_FILENAME: &str = "boards.json";
pub const DEFAULT_SQLITE_FILENAME: &str = "boards.sqlite";

pub fn validate_branch_prefix(prefix: &str) -> bool {
    if prefix.is_empty() {
        return false;
    }
    if prefix.starts_with('-') || prefix.ends_with('-') {
        return false;
    }
    prefix
        .chars()
        .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub configuration_format: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub configuration_location: Option<String>,
    #[serde(
        default,
        alias = "default_branch_prefix",
        skip_serializing_if = "Option::is_none"
    )]
    pub default_card_prefix: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_sprint_prefix: Option<String>,
    #[serde(
        default,
        alias = "default_format",
        skip_serializing_if = "Option::is_none"
    )]
    pub editing_format: Option<String>,
    #[serde(
        default,
        alias = "default_db_mode",
        skip_serializing_if = "Option::is_none"
    )]
    pub storage_backend: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub storage_location: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub board_sort_field: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub board_sort_order: Option<String>,
}

impl AppConfig {
    pub fn effective_default_card_prefix(&self) -> &str {
        self.default_card_prefix.as_deref().unwrap_or("task")
    }

    pub fn effective_default_sprint_prefix(&self) -> &str {
        self.default_sprint_prefix.as_deref().unwrap_or("sprint")
    }

    pub fn effective_storage_backend(&self) -> &str {
        self.storage_backend
            .as_deref()
            .unwrap_or(DEFAULT_STORAGE_BACKEND)
    }

    pub fn effective_editing_format(&self) -> &str {
        self.editing_format.as_deref().unwrap_or("json")
    }

    pub fn effective_configuration_format(&self) -> &str {
        self.configuration_format.as_deref().unwrap_or("toml")
    }

    pub fn effective_storage_location(&self) -> String {
        self.storage_location.clone().unwrap_or_else(|| {
            match self.effective_storage_backend() {
                "sqlite" => DEFAULT_SQLITE_FILENAME,
                _ => DEFAULT_JSON_FILENAME,
            }
            .to_string()
        })
    }

    pub fn validate_values(&self) -> CoreResult<()> {
        if let Some(ref v) = self.storage_backend {
            if !matches!(v.as_str(), "json" | "sqlite") {
                return Err(crate::CoreError::Validation(format!(
                    "Invalid storage_backend '{}': must be 'json' or 'sqlite'",
                    v
                )));
            }
        }
        if let Some(ref v) = self.editing_format {
            if !matches!(v.as_str(), "json" | "toml") {
                return Err(crate::CoreError::Validation(format!(
                    "Invalid editing_format '{}': must be 'json' or 'toml'",
                    v
                )));
            }
        }
        if let Some(ref v) = self.configuration_format {
            if !matches!(v.as_str(), "json" | "toml") {
                return Err(crate::CoreError::Validation(format!(
                    "Invalid configuration_format '{}': must be 'json' or 'toml'",
                    v
                )));
            }
        }
        if let Some(ref v) = self.default_card_prefix {
            if !validate_branch_prefix(v) {
                return Err(crate::CoreError::Validation(format!(
                    "Invalid default_card_prefix '{}': must be non-empty, alphanumeric with hyphens/underscores, no leading/trailing hyphens",
                    v
                )));
            }
        }
        if let Some(ref v) = self.default_sprint_prefix {
            if !validate_branch_prefix(v) {
                return Err(crate::CoreError::Validation(format!(
                    "Invalid default_sprint_prefix '{}': must be non-empty, alphanumeric with hyphens/underscores, no leading/trailing hyphens",
                    v
                )));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_effective_storage_backend() {
        let config = AppConfig::default();
        assert_eq!(config.effective_storage_backend(), "json");

        let config = AppConfig {
            storage_backend: Some("sqlite".into()),
            ..Default::default()
        };
        assert_eq!(config.effective_storage_backend(), "sqlite");
    }

    #[test]
    fn test_effective_editing_format() {
        let config = AppConfig::default();
        assert_eq!(config.effective_editing_format(), "json");

        let config = AppConfig {
            editing_format: Some("toml".into()),
            ..Default::default()
        };
        assert_eq!(config.effective_editing_format(), "toml");
    }

    #[test]
    fn test_effective_default_card_prefix() {
        let config = AppConfig::default();
        assert_eq!(config.effective_default_card_prefix(), "task");

        let config = AppConfig {
            default_card_prefix: Some("feat".into()),
            ..Default::default()
        };
        assert_eq!(config.effective_default_card_prefix(), "feat");
    }

    #[test]
    fn test_effective_default_sprint_prefix() {
        let config = AppConfig::default();
        assert_eq!(config.effective_default_sprint_prefix(), "sprint");

        let config = AppConfig {
            default_sprint_prefix: Some("iteration".into()),
            ..Default::default()
        };
        assert_eq!(config.effective_default_sprint_prefix(), "iteration");
    }

    #[test]
    fn test_effective_configuration_format() {
        let config = AppConfig::default();
        assert_eq!(config.effective_configuration_format(), "toml");

        let config = AppConfig {
            configuration_format: Some("json".into()),
            ..Default::default()
        };
        assert_eq!(config.effective_configuration_format(), "json");
    }

    #[test]
    fn test_effective_storage_location_defaults_to_json() {
        let config = AppConfig::default();
        assert_eq!(config.effective_storage_location(), "boards.json");
    }

    #[test]
    fn test_effective_storage_location_sqlite_when_backend_sqlite() {
        let config = AppConfig {
            storage_backend: Some("sqlite".into()),
            ..Default::default()
        };
        assert_eq!(config.effective_storage_location(), "boards.sqlite");
    }

    #[test]
    fn test_effective_storage_location_custom_relative() {
        let config = AppConfig {
            storage_location: Some("my_data.json".into()),
            ..Default::default()
        };
        assert_eq!(config.effective_storage_location(), "my_data.json");
    }

    #[test]
    fn test_effective_storage_location_custom_absolute() {
        let config = AppConfig {
            storage_location: Some("/tmp/my_data.json".into()),
            ..Default::default()
        };
        assert_eq!(config.effective_storage_location(), "/tmp/my_data.json");
    }

    #[test]
    fn test_validate_branch_prefix_valid() {
        assert!(validate_branch_prefix("task"));
        assert!(validate_branch_prefix("feat"));
        assert!(validate_branch_prefix("FEAT-123"));
        assert!(validate_branch_prefix("my_prefix"));
        assert!(validate_branch_prefix("a"));
    }

    #[test]
    fn test_validate_branch_prefix_invalid() {
        assert!(!validate_branch_prefix(""));
        assert!(!validate_branch_prefix("-feat"));
        assert!(!validate_branch_prefix("feat-"));
        assert!(!validate_branch_prefix("feat/bad"));
        assert!(!validate_branch_prefix("feat bad"));
        assert!(!validate_branch_prefix("feat@123"));
    }

    #[test]
    fn test_validate_values_default_config_passes() {
        let config = AppConfig::default();
        config.validate_values().unwrap();
    }

    #[test]
    fn test_validate_values_valid_storage_backend_passes() {
        for backend in &["json", "sqlite"] {
            let config = AppConfig {
                storage_backend: Some(backend.to_string()),
                ..Default::default()
            };
            config.validate_values().unwrap();
        }
    }

    #[test]
    fn test_validate_values_invalid_storage_backend_fails() {
        let config = AppConfig {
            storage_backend: Some("yaml".into()),
            ..Default::default()
        };
        let err = config.validate_values().unwrap_err();
        assert!(err.to_string().contains("storage_backend"));
        assert!(err.to_string().contains("yaml"));
    }

    #[test]
    fn test_validate_values_valid_editing_format_passes() {
        for fmt in &["json", "toml"] {
            let config = AppConfig {
                editing_format: Some(fmt.to_string()),
                ..Default::default()
            };
            config.validate_values().unwrap();
        }
    }

    #[test]
    fn test_validate_values_valid_toml_editing_format_passes() {
        let config = AppConfig {
            editing_format: Some("toml".into()),
            ..Default::default()
        };
        config.validate_values().unwrap();
    }

    #[test]
    fn test_validate_values_invalid_editing_format_fails() {
        let config = AppConfig {
            editing_format: Some("yaml".into()),
            ..Default::default()
        };
        let err = config.validate_values().unwrap_err();
        assert!(err.to_string().contains("editing_format"));
    }

    #[test]
    fn test_validate_values_valid_configuration_format_passes() {
        for fmt in &["json", "toml"] {
            let config = AppConfig {
                configuration_format: Some(fmt.to_string()),
                ..Default::default()
            };
            config.validate_values().unwrap();
        }
    }

    #[test]
    fn test_validate_values_invalid_configuration_format_fails() {
        let config = AppConfig {
            configuration_format: Some("yaml".into()),
            ..Default::default()
        };
        let err = config.validate_values().unwrap_err();
        assert!(err.to_string().contains("configuration_format"));
        assert!(err.to_string().contains("yaml"));
    }

    #[test]
    fn test_validate_values_valid_card_prefix_passes() {
        for prefix in &["task", "feat", "FEAT-123", "my_prefix"] {
            let config = AppConfig {
                default_card_prefix: Some(prefix.to_string()),
                ..Default::default()
            };
            config.validate_values().unwrap();
        }
    }

    #[test]
    fn test_validate_values_invalid_card_prefix_fails() {
        for prefix in &["", "-feat", "feat-", "feat/bad", "feat bad"] {
            let config = AppConfig {
                default_card_prefix: Some(prefix.to_string()),
                ..Default::default()
            };
            let err = config.validate_values().unwrap_err();
            assert!(err.to_string().contains("default_card_prefix"));
        }
    }

    #[test]
    fn test_validate_values_valid_sprint_prefix_passes() {
        for prefix in &["sprint", "SP", "iteration-1"] {
            let config = AppConfig {
                default_sprint_prefix: Some(prefix.to_string()),
                ..Default::default()
            };
            config.validate_values().unwrap();
        }
    }

    #[test]
    fn test_validate_values_invalid_sprint_prefix_fails() {
        for prefix in &["", "sprint/1", "-sprint"] {
            let config = AppConfig {
                default_sprint_prefix: Some(prefix.to_string()),
                ..Default::default()
            };
            let err = config.validate_values().unwrap_err();
            assert!(err.to_string().contains("default_sprint_prefix"));
        }
    }
}
