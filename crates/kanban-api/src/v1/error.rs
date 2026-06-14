use serde::{Deserialize, Serialize};

/// Machine-readable error code included in every [`ApiError`] response body.
///
/// Variants map to the `KanbanError`/`DomainError` taxonomy and serialise as
/// `SCREAMING_SNAKE_CASE` so JSON clients can branch on them without parsing
/// the human-readable `message` field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
    NotFound,
    NotFoundByName,
    Ambiguous,
    WipLimitExceeded,
    SprintBoardMismatch,
    ValidationFailed,
    DependencyError,
    ConflictDetected,
    UnsupportedVersion,
    IoError,
    SerializationError,
    DatabaseError,
    InternalError,
}

impl std::fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::NotFound => "NOT_FOUND",
            Self::NotFoundByName => "NOT_FOUND_BY_NAME",
            Self::Ambiguous => "AMBIGUOUS",
            Self::WipLimitExceeded => "WIP_LIMIT_EXCEEDED",
            Self::SprintBoardMismatch => "SPRINT_BOARD_MISMATCH",
            Self::ValidationFailed => "VALIDATION_FAILED",
            Self::DependencyError => "DEPENDENCY_ERROR",
            Self::ConflictDetected => "CONFLICT_DETECTED",
            Self::UnsupportedVersion => "UNSUPPORTED_VERSION",
            Self::IoError => "IO_ERROR",
            Self::SerializationError => "SERIALIZATION_ERROR",
            Self::DatabaseError => "DATABASE_ERROR",
            Self::InternalError => "INTERNAL_ERROR",
        };
        f.write_str(s)
    }
}

/// HTTP error response envelope returned by all kanban-server routes.
#[non_exhaustive]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ApiError {
    pub code: ErrorCode,
    pub message: String,
}

impl ApiError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for ApiError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_error_new_sets_fields() {
        let e = ApiError::new(ErrorCode::NotFound, "board not found");
        assert_eq!(e.code, ErrorCode::NotFound);
        assert_eq!(e.message, "board not found");
    }

    #[test]
    fn test_api_error_display() {
        let e = ApiError::new(ErrorCode::ValidationFailed, "bad input");
        assert_eq!(e.to_string(), "VALIDATION_FAILED: bad input");
    }

    #[test]
    fn test_api_error_serde_round_trip() {
        let e = ApiError::new(ErrorCode::InternalError, "something went wrong");
        let json = serde_json::to_string(&e).unwrap();
        let parsed: ApiError = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.code, e.code);
        assert_eq!(parsed.message, e.message);
    }

    #[test]
    fn test_api_error_implements_std_error() {
        let e = ApiError::new(ErrorCode::InternalError, "msg");
        let _: &dyn std::error::Error = &e;
    }

    #[test]
    fn test_error_code_serializes_as_screaming_snake_case() {
        assert_eq!(
            serde_json::to_string(&ErrorCode::NotFound).unwrap(),
            "\"NOT_FOUND\""
        );
        assert_eq!(
            serde_json::to_string(&ErrorCode::WipLimitExceeded).unwrap(),
            "\"WIP_LIMIT_EXCEEDED\""
        );
        assert_eq!(
            serde_json::to_string(&ErrorCode::InternalError).unwrap(),
            "\"INTERNAL_ERROR\""
        );
    }

    #[test]
    fn test_error_code_deserializes_from_screaming_snake_case() {
        let code: ErrorCode = serde_json::from_str("\"CONFLICT_DETECTED\"").unwrap();
        assert_eq!(code, ErrorCode::ConflictDetected);
    }

    #[test]
    fn test_api_error_code_field_serializes_as_screaming_snake_case() {
        let e = ApiError::new(ErrorCode::NotFoundByName, "no match");
        let json = serde_json::to_string(&e).unwrap();
        assert!(json.contains("\"NOT_FOUND_BY_NAME\""), "json: {json}");
    }
}
