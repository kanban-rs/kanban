use serde::{Deserialize, Serialize};

/// HTTP error response envelope returned by all kanban-server routes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiError {
    pub code: String,
    pub message: String,
}

impl ApiError {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self { code: code.into(), message: message.into() }
    }
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_error_new_sets_fields() {
        let e = ApiError::new("NOT_FOUND", "board not found");
        assert_eq!(e.code, "NOT_FOUND");
        assert_eq!(e.message, "board not found");
    }

    #[test]
    fn test_api_error_display() {
        let e = ApiError::new("INVALID", "bad input");
        assert_eq!(e.to_string(), "INVALID: bad input");
    }

    #[test]
    fn test_api_error_serde_round_trip() {
        let e = ApiError::new("SERVER_ERROR", "something went wrong");
        let json = serde_json::to_string(&e).unwrap();
        let parsed: ApiError = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.code, e.code);
        assert_eq!(parsed.message, e.message);
    }
}
