use axum::http::{header, HeaderName, HeaderValue, Method};
use std::time::Duration;
use tower_http::cors::{AllowOrigin, CorsLayer};

/// Cross-origin policy for browser clients. `Disabled` sends no CORS headers
/// at all; `Permissive` allows any origin (`*`); `Origins` echoes back only
/// the listed origins.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum CorsPolicy {
    #[default]
    Disabled,
    Permissive,
    Origins(Vec<HeaderValue>),
}

impl CorsPolicy {
    /// Parses a comma-separated `KANBAN_CORS_ORIGINS` value. Any entry equal
    /// to `*`, anywhere in the list, collapses the whole policy to
    /// `Permissive` rather than being passed through to `AllowOrigin::list`,
    /// which panics on a wildcard entry.
    pub fn parse(raw: Option<&str>) -> Self {
        let Some(raw) = raw else {
            return Self::Disabled;
        };

        let entries: Vec<&str> = raw
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .collect();

        if entries.contains(&"*") {
            return Self::Permissive;
        }

        let origins: Vec<HeaderValue> = entries
            .iter()
            .filter_map(|s| HeaderValue::from_str(s).ok())
            .collect();

        if origins.is_empty() {
            Self::Disabled
        } else {
            Self::Origins(origins)
        }
    }

    pub fn into_layer(self) -> Option<CorsLayer> {
        match self {
            Self::Disabled => None,
            Self::Permissive => Some(CorsLayer::permissive()),
            Self::Origins(origins) => Some(
                CorsLayer::new()
                    .allow_origin(AllowOrigin::list(origins))
                    .allow_methods([
                        Method::GET,
                        Method::POST,
                        Method::PUT,
                        Method::PATCH,
                        Method::DELETE,
                    ])
                    .allow_headers([
                        header::CONTENT_TYPE,
                        header::IF_MATCH,
                        header::IF_NONE_MATCH,
                        HeaderName::from_static(crate::client_ident::CLIENT_ID_HEADER),
                    ])
                    .expose_headers([header::ETAG]),
            ),
        }
    }
}

pub const DEFAULT_BODY_LIMIT_BYTES: usize = 2 * 1024 * 1024;
pub const DEFAULT_REQUEST_TIMEOUT_SECS: u64 = 30;
pub const IMPORT_BODY_LIMIT_BYTES: usize = 32 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct LayerConfig {
    pub body_limit_bytes: usize,
    pub cors: CorsPolicy,
    /// `None` applies no timeout layer at all.
    pub timeout: Option<Duration>,
    /// Cap for `POST /v1/import` only. Must not be lower than `body_limit_bytes`.
    pub import_body_limit_bytes: usize,
}

impl Default for LayerConfig {
    fn default() -> Self {
        Self {
            body_limit_bytes: DEFAULT_BODY_LIMIT_BYTES,
            cors: CorsPolicy::Disabled,
            timeout: Some(Duration::from_secs(DEFAULT_REQUEST_TIMEOUT_SECS)),
            import_body_limit_bytes: IMPORT_BODY_LIMIT_BYTES,
        }
    }
}

/// Parses `KANBAN_REQUEST_TIMEOUT_SECS`. `0` disables the timeout; an unset
/// or unparseable value falls back to [`DEFAULT_REQUEST_TIMEOUT_SECS`].
pub fn parse_request_timeout(raw: Option<&str>) -> Option<Duration> {
    match raw.and_then(|v| v.parse::<u64>().ok()) {
        Some(0) => None,
        Some(secs) => Some(Duration::from_secs(secs)),
        None => Some(Duration::from_secs(DEFAULT_REQUEST_TIMEOUT_SECS)),
    }
}

impl LayerConfig {
    pub fn from_env() -> Self {
        Self {
            cors: CorsPolicy::parse(std::env::var("KANBAN_CORS_ORIGINS").ok().as_deref()),
            timeout: parse_request_timeout(
                std::env::var("KANBAN_REQUEST_TIMEOUT_SECS").ok().as_deref(),
            ),
            ..Self::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_default_layer_config_request_timeout_is_thirty_seconds() {
        assert_eq!(
            LayerConfig::default().timeout,
            Some(Duration::from_secs(30))
        );
    }

    #[test]
    fn test_request_timeout_secs_unset_yields_the_default() {
        assert_eq!(
            parse_request_timeout(None),
            Some(Duration::from_secs(DEFAULT_REQUEST_TIMEOUT_SECS))
        );
    }

    #[test]
    fn test_request_timeout_secs_parses_as_whole_seconds() {
        assert_eq!(
            parse_request_timeout(Some("5")),
            Some(Duration::from_secs(5))
        );
    }

    #[test]
    fn test_request_timeout_secs_zero_disables_the_timeout() {
        assert_eq!(parse_request_timeout(Some("0")), None);
    }

    #[test]
    fn test_request_timeout_secs_unparseable_falls_back_to_the_default() {
        assert_eq!(
            parse_request_timeout(Some("soon")),
            Some(Duration::from_secs(30))
        );
    }

    #[test]
    fn test_cors_origins_star_yields_permissive_policy() {
        assert_eq!(CorsPolicy::parse(Some("*")), CorsPolicy::Permissive);
    }

    #[test]
    fn test_cors_origins_csv_yields_exact_origin_list() {
        assert_eq!(
            CorsPolicy::parse(Some("http://localhost:5173, https://app.example.com")),
            CorsPolicy::Origins(vec![
                HeaderValue::from_static("http://localhost:5173"),
                HeaderValue::from_static("https://app.example.com"),
            ])
        );
    }

    #[test]
    fn test_cors_origins_csv_containing_star_yields_permissive_policy() {
        assert_eq!(
            CorsPolicy::parse(Some("*,http://localhost:5173")),
            CorsPolicy::Permissive
        );
    }

    #[test]
    fn test_cors_origins_none_yields_disabled_policy() {
        assert_eq!(CorsPolicy::parse(None), CorsPolicy::Disabled);
    }
}
