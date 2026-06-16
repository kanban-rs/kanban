use serde::{Deserialize, Serialize};

/// Identifies which application surface issued a command.
/// Used in CommandBatch to attribute mutations in the audit log.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppType {
    #[default]
    Unknown,
    Tui,
    Cli,
    Mcp,
    HttpClient,
}

impl std::fmt::Display for AppType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Tui => write!(f, "tui"),
            Self::Cli => write!(f, "cli"),
            Self::Mcp => write!(f, "mcp"),
            Self::HttpClient => write!(f, "http_client"),
            Self::Unknown => write!(f, "unknown"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_type_default_is_unknown() {
        assert_eq!(AppType::default(), AppType::Unknown);
    }

    #[test]
    fn test_app_type_display_variants() {
        assert_eq!(AppType::Tui.to_string(), "tui");
        assert_eq!(AppType::Cli.to_string(), "cli");
        assert_eq!(AppType::Mcp.to_string(), "mcp");
        assert_eq!(AppType::HttpClient.to_string(), "http_client");
        assert_eq!(AppType::Unknown.to_string(), "unknown");
    }

    #[test]
    fn test_app_type_serde_round_trip() {
        let variants = [
            AppType::Tui,
            AppType::Cli,
            AppType::Mcp,
            AppType::HttpClient,
            AppType::Unknown,
        ];
        for variant in variants {
            let json = serde_json::to_string(&variant).unwrap();
            let parsed: AppType = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed, variant);
        }
    }

    #[test]
    fn test_app_type_snake_case_serialization() {
        let json = serde_json::to_string(&AppType::HttpClient).unwrap();
        assert_eq!(json, "\"http_client\"");
    }
}
