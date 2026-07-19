//! Domain-error → wire `ApiError` mapping. Separate from the wire error types in
//! `error.rs`: classifying domain errors and choosing client-safe messages is a
//! distinct concern (and pulls in a `kanban-domain` dependency the types don't need).

use super::{ApiError, ErrorCode};
use kanban_domain::{DependencyError, DomainError, KanbanError};

impl From<&KanbanError> for ApiError {
    /// Map a domain error onto its wire `ErrorCode` + a **client-safe** message.
    ///
    /// The code match is exhaustive (no `_`) so a new domain error variant must be
    /// classified before it compiles. The message match is likewise exhaustive over
    /// `ErrorCode`: server-fault codes return a fixed generic message and never echo
    /// the underlying detail (which can include DB text, OS/IO messages, or file
    /// paths) to the client. The server handler is expected to **log the original
    /// `KanbanError`** before calling this, so detail is preserved for operators.
    fn from(err: &KanbanError) -> Self {
        let code = match err {
            KanbanError::Domain(domain) => match domain {
                DomainError::NotFound { .. } => ErrorCode::NotFound,
                DomainError::AlreadyExists { .. } => ErrorCode::AlreadyExists,
                DomainError::NotFoundByName { .. } => ErrorCode::NotFoundByName,
                DomainError::Ambiguous { .. } => ErrorCode::Ambiguous,
                DomainError::BatchResolutionFailed { .. } => ErrorCode::BatchResolutionFailed,
                DomainError::Validation(_) => ErrorCode::ValidationFailed,
                DomainError::Dependency(dep) => match dep {
                    DependencyError::CycleDetected => ErrorCode::CycleDetected,
                    DependencyError::SelfReference => ErrorCode::SelfReference,
                    DependencyError::EdgeNotFound => ErrorCode::EdgeNotFound,
                    DependencyError::DuplicateEdge => ErrorCode::DuplicateEdge,
                },
                DomainError::WipLimitExceeded { .. } => ErrorCode::WipLimitExceeded,
                DomainError::SprintBoardMismatch { .. } => ErrorCode::SprintBoardMismatch,
                DomainError::BoardArchived { .. } => ErrorCode::BoardArchived,
            },
            KanbanError::Io(_) => ErrorCode::IoError,
            KanbanError::Serialization(_) => ErrorCode::SerializationError,
            KanbanError::ConflictDetected { .. } => ErrorCode::ConflictDetected,
            KanbanError::Database(_) => ErrorCode::DatabaseError,
            KanbanError::Internal(_) => ErrorCode::InternalError,
            // A backend gap is a server fault, not a client error.
            KanbanError::Unsupported { .. } => ErrorCode::InternalError,
            KanbanError::UnsupportedFutureVersion { .. } => ErrorCode::UnsupportedVersion,
        };
        // Public-message policy — exhaustive over `ErrorCode` so a new code must
        // declare whether it may expose the underlying error detail.
        let message = match code {
            // Client errors: the domain `Display` is user-facing and safe.
            ErrorCode::NotFound
            | ErrorCode::AlreadyExists
            | ErrorCode::NotFoundByName
            | ErrorCode::Ambiguous
            | ErrorCode::BatchResolutionFailed
            | ErrorCode::ValidationFailed
            | ErrorCode::WipLimitExceeded
            | ErrorCode::SprintBoardMismatch
            | ErrorCode::BoardArchived
            | ErrorCode::DependencyError
            | ErrorCode::CycleDetected
            | ErrorCode::SelfReference
            | ErrorCode::EdgeNotFound
            | ErrorCode::DuplicateEdge
            | ErrorCode::UnsupportedVersion => err.to_string(),
            // Conflict is a 409 the client can act on, but its `Display` leaks a
            // server path — give a useful message without the detail.
            ErrorCode::ConflictDetected => {
                "resource was modified by another writer; reload and retry".to_string()
            }
            // Server faults: never echo internal detail to the client.
            ErrorCode::IoError
            | ErrorCode::SerializationError
            | ErrorCode::DatabaseError
            | ErrorCode::InternalError => "internal server error".to_string(),
        };
        ApiError::new(code, message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kanban_error_maps_every_family_to_expected_code() {
        use kanban_domain::{DependencyError, DomainError, KanbanError};
        use uuid::Uuid;
        let d = |e: DomainError| KanbanError::Domain(e);
        let cases: Vec<(KanbanError, ErrorCode)> = vec![
            (
                d(DomainError::NotFound {
                    entity: "Board",
                    id: Uuid::nil(),
                }),
                ErrorCode::NotFound,
            ),
            (
                d(DomainError::NotFoundByName {
                    entity: "Board",
                    name: "x".into(),
                    available: vec![],
                }),
                ErrorCode::NotFoundByName,
            ),
            (
                d(DomainError::Ambiguous {
                    entity: "Board",
                    name: "x".into(),
                    matches: vec![],
                }),
                ErrorCode::Ambiguous,
            ),
            (
                d(DomainError::BatchResolutionFailed {
                    entity: "Card",
                    failures: vec![],
                }),
                ErrorCode::BatchResolutionFailed,
            ),
            (
                d(DomainError::Validation("bad".into())),
                ErrorCode::ValidationFailed,
            ),
            (
                d(DomainError::Dependency(DependencyError::CycleDetected)),
                ErrorCode::CycleDetected,
            ),
            (
                d(DomainError::Dependency(DependencyError::SelfReference)),
                ErrorCode::SelfReference,
            ),
            (
                d(DomainError::Dependency(DependencyError::EdgeNotFound)),
                ErrorCode::EdgeNotFound,
            ),
            (
                d(DomainError::Dependency(DependencyError::DuplicateEdge)),
                ErrorCode::DuplicateEdge,
            ),
            (
                d(DomainError::WipLimitExceeded {
                    column_id: Uuid::nil(),
                    limit: 5,
                }),
                ErrorCode::WipLimitExceeded,
            ),
            (
                d(DomainError::SprintBoardMismatch {
                    sprint_id: Uuid::nil(),
                    sprint_board: Uuid::nil(),
                    card_board: Uuid::nil(),
                }),
                ErrorCode::SprintBoardMismatch,
            ),
            (
                KanbanError::Io(std::io::Error::other("disk gone")),
                ErrorCode::IoError,
            ),
            (
                KanbanError::Serialization("ser".into()),
                ErrorCode::SerializationError,
            ),
            (
                KanbanError::ConflictDetected {
                    path: "secret/path".into(),
                    source: None,
                },
                ErrorCode::ConflictDetected,
            ),
            (
                KanbanError::Database("schema x".into()),
                ErrorCode::DatabaseError,
            ),
            (
                KanbanError::Internal("oops".into()),
                ErrorCode::InternalError,
            ),
            (
                // A backend gap is a server fault (and must NOT echo the
                // operation string to the client - see the message policy).
                KanbanError::unsupported("archive_board"),
                ErrorCode::InternalError,
            ),
            (
                KanbanError::UnsupportedFutureVersion {
                    file_version: 9,
                    binary_max: 8,
                },
                ErrorCode::UnsupportedVersion,
            ),
        ];
        for (err, expected) in cases {
            assert_eq!(ApiError::from(&err).code, expected, "for error: {err}");
        }
    }

    #[test]
    fn test_already_exists_maps_to_error_code_already_exists() {
        use kanban_domain::KanbanError;
        use uuid::Uuid;
        let err = KanbanError::already_exists("Board", Uuid::nil());
        assert_eq!(ApiError::from(&err).code, ErrorCode::AlreadyExists);
    }

    #[test]
    fn test_already_exists_message_names_entity_not_scrubbed() {
        use kanban_domain::KanbanError;
        use uuid::Uuid;
        // Unlike ConflictDetected (file-level optimistic concurrency, scrubbed),
        // the already-exists message is client-actionable: it must name the
        // entity and id rather than a generic placeholder.
        let id = Uuid::new_v4();
        let err = KanbanError::already_exists("Card", id);
        let api = ApiError::from(&err);
        assert!(api.message.contains("Card"), "msg: {}", api.message);
        assert!(
            api.message.contains(&id.to_string()),
            "msg: {}",
            api.message
        );
        assert!(
            api.message.contains("already exists"),
            "msg: {}",
            api.message
        );
    }

    #[test]
    fn test_api_error_does_not_leak_internal_detail() {
        use kanban_domain::KanbanError;
        // Server faults must never echo internal text to the client.
        for err in [
            KanbanError::Database("secret schema name".into()),
            KanbanError::Serialization("internal serde detail".into()),
            KanbanError::Internal("stack trace bits".into()),
            KanbanError::Io(std::io::Error::other("/var/secret/path")),
        ] {
            let api = ApiError::from(&err);
            assert_eq!(api.message, "internal server error", "leaked via {err}");
        }
        // Conflict is actionable but must not leak the file path.
        let conflict = KanbanError::ConflictDetected {
            path: "/srv/data/board.json".into(),
            source: None,
        };
        assert!(
            !ApiError::from(&conflict).message.contains("/srv/data"),
            "leaked path"
        );
        // Client errors keep their helpful Display message.
        let validation = KanbanError::validation("title must not be empty");
        assert!(ApiError::from(&validation)
            .message
            .contains("title must not be empty"));
    }
}
