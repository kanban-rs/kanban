use std::fmt;

use thiserror::Error;
use uuid::Uuid;

/// One element of an `Ambiguous` error's match list. Carries both a
/// human-readable label (what makes this match distinguishable from the
/// others) and the entity's UUID (always copy-pasteable). Display always
/// renders as `{label} ({uuid})` so users can disambiguate by either.
#[derive(Debug, Clone)]
pub struct AmbiguousMatch {
    /// Human-readable label that distinguishes this match. Examples:
    ///   - board name (when two boards share a name)
    ///   - `"on board 'X'"` (when a column name appears on multiple boards)
    ///   - `"#15 'yarara-release' on board 'Project A'"` (sprint global match)
    ///   - card title
    pub label: String,
    /// The matched entity's UUID.
    pub id: Uuid,
}

impl fmt::Display for AmbiguousMatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({})", self.label, self.id)
    }
}

/// One element of a `BatchResolutionFailed` error. Carries the raw input
/// the caller passed and the typed reason it couldn't be resolved.
#[derive(Debug, Clone)]
pub struct BatchResolutionFailure {
    /// The string the caller passed for this slot (UUID, identifier, name, etc.).
    pub raw_input: String,
    /// Why this input couldn't be resolved.
    pub cause: BatchResolutionCause,
}

impl fmt::Display for BatchResolutionFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "'{}' ({})", self.raw_input, self.cause)
    }
}

/// Why a single input in a batch resolver call failed.
#[derive(Debug, Clone)]
pub enum BatchResolutionCause {
    /// No entity matched the input.
    NotFound,
    /// More than one entity matched. Carries the same match data an
    /// `Ambiguous` single-resolver error would.
    Ambiguous(Vec<AmbiguousMatch>),
}

impl fmt::Display for BatchResolutionCause {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound => write!(f, "not found"),
            Self::Ambiguous(matches) => {
                f.write_str("ambiguous: ")?;
                for (i, m) in matches.iter().enumerate() {
                    if i > 0 {
                        f.write_str(", ")?;
                    }
                    write!(f, "{}", m)?;
                }
                Ok(())
            }
        }
    }
}

#[derive(Error, Debug)]
pub enum DependencyError {
    #[error("cycle detected: adding this edge would create a circular dependency")]
    CycleDetected,
    #[error("self-reference not allowed")]
    SelfReference,
    #[error("edge not found")]
    EdgeNotFound,
    #[error("edge already exists between the two cards")]
    DuplicateEdge,
}

#[derive(Error, Debug)]
pub enum DomainError {
    /// `entity` is rendered as a sentence-leading noun (e.g. `"Card"`,
    /// `"Sprint"`). All call sites of `KanbanError::not_found(entity, id)`
    /// must pass a capitalized noun so the rendered message reads
    /// `"Card <uuid> not found"`, matching the sibling `NotFoundByName`
    /// convention. See KAN-659 for the normalization history.
    #[error("{entity} {id} not found")]
    NotFound { entity: &'static str, id: Uuid },

    /// Returned when an entity is created with a client-supplied id that already
    /// exists (the POST-create collision path; PUT-create replaces instead).
    /// Distinct from [`KanbanError::ConflictDetected`], which is file-level
    /// optimistic-concurrency and whose wire message is deliberately scrubbed.
    /// Here the entity + id are safe and useful to echo back to the client, so
    /// the message names them. `entity` follows the same capitalized-noun
    /// convention as [`NotFound`](Self::NotFound).
    #[error("{entity} {id} already exists")]
    AlreadyExists { entity: &'static str, id: Uuid },

    /// Returned when a name- or identifier-based lookup misses. The `available`
    /// vector is appended to the message so users see what they could have typed.
    /// `entity` follows the same capitalized-noun convention as
    /// [`NotFound`](Self::NotFound).
    #[error("{}", DomainError::fmt_not_found_by_name(entity, name, available))]
    NotFoundByName {
        entity: &'static str,
        name: String,
        available: Vec<String>,
    },

    /// Returned when a name- or identifier-based lookup matches more than one
    /// entity. Each match carries both a human-readable label and a UUID so
    /// users can disambiguate by either.
    #[error("{}", DomainError::fmt_ambiguous(entity, name, matches))]
    Ambiguous {
        entity: &'static str,
        name: String,
        matches: Vec<AmbiguousMatch>,
    },

    /// Returned by batch resolvers (`resolve_card_ids`, future siblings) when
    /// one or more inputs in the batch couldn't be resolved. Carries per-input
    /// typed causes so callers can introspect.
    #[error("{}", DomainError::fmt_batch_resolution_failed(entity, failures))]
    BatchResolutionFailed {
        entity: &'static str,
        failures: Vec<BatchResolutionFailure>,
    },

    #[error("validation error: {0}")]
    Validation(String),

    #[error(transparent)]
    Dependency(#[from] DependencyError),

    #[error("column {column_id} has reached its WIP limit of {limit}")]
    WipLimitExceeded { column_id: Uuid, limit: u32 },

    #[error(
        "sprint {sprint_id} belongs to board {sprint_board} but card is being created on board {card_board}"
    )]
    SprintBoardMismatch {
        sprint_id: Uuid,
        sprint_board: Uuid,
        card_board: Uuid,
    },

    /// A `DataStore`/backend method that a backend has not implemented yet.
    /// The shared affordance behind default trait bodies added by later slices
    /// (a backend gap, surfaced as a server fault at the wire boundary).
    #[error("operation not supported by this backend: {operation}")]
    Unsupported { operation: &'static str },
}

impl DomainError {
    // ----- Display formatters for the name/identifier resolver variants -----

    fn fmt_not_found_by_name(entity: &str, name: &str, available: &[String]) -> String {
        if available.is_empty() {
            format!("{} '{}' not found", entity, name)
        } else {
            format!(
                "{} '{}' not found. Available: {}",
                entity,
                name,
                available
                    .iter()
                    .map(|s| format!("'{}'", s))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        }
    }

    fn fmt_ambiguous(entity: &str, name: &str, matches: &[AmbiguousMatch]) -> String {
        let rendered = matches
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(", ");
        format!("{} '{}' is ambiguous: {}.", entity, name, rendered)
    }

    fn fmt_batch_resolution_failed(entity: &str, failures: &[BatchResolutionFailure]) -> String {
        let parts = failures
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(", ");
        format!(
            "Could not resolve {} {}: {}",
            failures.len(),
            pluralize(entity, failures.len()),
            parts
        )
    }

    pub fn wip_limit_exceeded(column_id: Uuid, limit: u32) -> Self {
        Self::WipLimitExceeded { column_id, limit }
    }
}

#[derive(Error, Debug)]
pub enum KanbanError {
    #[error(transparent)]
    Domain(#[from] DomainError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("serialization error: {0}")]
    Serialization(String),

    #[error("file conflict: {path} was modified by another instance")]
    ConflictDetected {
        path: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    #[error("database error: {0}")]
    Database(String),

    #[error("internal error: {0}")]
    Internal(String),

    #[error(
        "file format v{file_version} is newer than this binary's max v{binary_max}; \
         please upgrade kanban"
    )]
    UnsupportedFutureVersion { file_version: u32, binary_max: u32 },
}

/// Return `noun` (when count is 1) or `noun + "s"` (otherwise).
/// Trivial English helper used by error message formatters.
fn pluralize(noun: &str, count: usize) -> String {
    if count == 1 {
        noun.to_string()
    } else {
        format!("{}s", noun)
    }
}

pub type KanbanResult<T> = Result<T, KanbanError>;

impl KanbanError {
    pub fn not_found(entity: &'static str, id: Uuid) -> Self {
        Self::Domain(DomainError::NotFound { entity, id })
    }

    pub fn already_exists(entity: &'static str, id: Uuid) -> Self {
        Self::Domain(DomainError::AlreadyExists { entity, id })
    }

    pub fn not_found_by_name(
        entity: &'static str,
        name: impl Into<String>,
        available: Vec<String>,
    ) -> Self {
        Self::Domain(DomainError::NotFoundByName {
            entity,
            name: name.into(),
            available,
        })
    }

    pub fn ambiguous(
        entity: &'static str,
        name: impl Into<String>,
        matches: Vec<AmbiguousMatch>,
    ) -> Self {
        Self::Domain(DomainError::Ambiguous {
            entity,
            name: name.into(),
            matches,
        })
    }

    pub fn batch_resolution_failed(
        entity: &'static str,
        failures: Vec<BatchResolutionFailure>,
    ) -> Self {
        Self::Domain(DomainError::BatchResolutionFailed { entity, failures })
    }

    pub fn validation(msg: impl Into<String>) -> Self {
        Self::Domain(DomainError::Validation(msg.into()))
    }

    /// A backend method that has not been implemented yet. The shared affordance
    /// consumed by default `DataStore` trait bodies added in later slices (D6).
    pub fn unsupported(operation: &'static str) -> Self {
        Self::Domain(DomainError::Unsupported { operation })
    }

    pub fn is_unsupported(&self) -> bool {
        matches!(self, KanbanError::Domain(DomainError::Unsupported { .. }))
    }

    /// True for both `NotFound` (by UUID) and `NotFoundByName`.
    pub fn is_not_found(&self) -> bool {
        matches!(
            self,
            KanbanError::Domain(DomainError::NotFound { .. })
                | KanbanError::Domain(DomainError::NotFoundByName { .. })
        )
    }

    pub fn is_already_exists(&self) -> bool {
        matches!(self, KanbanError::Domain(DomainError::AlreadyExists { .. }))
    }

    pub fn is_not_found_by_name(&self) -> bool {
        matches!(
            self,
            KanbanError::Domain(DomainError::NotFoundByName { .. })
        )
    }

    pub fn is_ambiguous(&self) -> bool {
        matches!(self, KanbanError::Domain(DomainError::Ambiguous { .. }))
    }

    pub fn is_batch_resolution_failed(&self) -> bool {
        matches!(
            self,
            KanbanError::Domain(DomainError::BatchResolutionFailed { .. })
        )
    }

    pub fn is_validation(&self) -> bool {
        matches!(self, KanbanError::Domain(DomainError::Validation(_)))
    }

    pub fn is_cycle_detected(&self) -> bool {
        matches!(
            self,
            KanbanError::Domain(DomainError::Dependency(DependencyError::CycleDetected))
        )
    }

    pub fn is_self_reference(&self) -> bool {
        matches!(
            self,
            KanbanError::Domain(DomainError::Dependency(DependencyError::SelfReference))
        )
    }

    pub fn is_edge_not_found(&self) -> bool {
        matches!(
            self,
            KanbanError::Domain(DomainError::Dependency(DependencyError::EdgeNotFound))
        )
    }

    pub fn is_duplicate_edge(&self) -> bool {
        matches!(
            self,
            KanbanError::Domain(DomainError::Dependency(DependencyError::DuplicateEdge))
        )
    }

    pub fn is_conflict_detected(&self) -> bool {
        matches!(self, KanbanError::ConflictDetected { .. })
    }

    pub fn is_wip_limit_exceeded(&self) -> bool {
        matches!(
            self,
            KanbanError::Domain(DomainError::WipLimitExceeded { .. })
        )
    }

    pub fn is_sprint_board_mismatch(&self) -> bool {
        matches!(
            self,
            KanbanError::Domain(DomainError::SprintBoardMismatch { .. })
        )
    }

    pub fn is_unsupported_future_version(&self) -> bool {
        matches!(self, KanbanError::UnsupportedFutureVersion { .. })
    }

    pub fn serialization(msg: impl Into<String>) -> Self {
        Self::Serialization(msg.into())
    }
}

impl From<DependencyError> for KanbanError {
    fn from(e: DependencyError) -> Self {
        KanbanError::Domain(DomainError::Dependency(e))
    }
}

impl From<kanban_core::CoreError> for KanbanError {
    fn from(e: kanban_core::CoreError) -> Self {
        match e {
            kanban_core::CoreError::Validation(msg) => KanbanError::validation(msg),
            kanban_core::CoreError::Config(msg) => KanbanError::Internal(msg),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn test_is_not_found_returns_true_for_card_not_found() {
        let err = KanbanError::not_found("Card", Uuid::new_v4());
        assert!(err.is_not_found());
    }

    #[test]
    fn test_unsupported_error_is_unsupported_and_not_not_found() {
        let err = KanbanError::unsupported("archive_board");
        assert!(err.is_unsupported());
        assert!(!err.is_not_found());
        assert!(err.to_string().contains("archive_board"));
    }

    #[test]
    fn test_is_not_found_returns_false_for_validation_error() {
        let err = KanbanError::validation("bad input");
        assert!(!err.is_not_found());
    }

    #[test]
    fn test_is_validation_returns_true_for_validation_error() {
        let err = KanbanError::validation("bad input");
        assert!(err.is_validation());
    }

    #[test]
    fn test_is_cycle_detected_returns_true() {
        let err = KanbanError::from(DependencyError::CycleDetected);
        assert!(err.is_cycle_detected());
    }

    #[test]
    fn test_is_self_reference_returns_true() {
        let err = KanbanError::from(DependencyError::SelfReference);
        assert!(err.is_self_reference());
    }

    #[test]
    fn test_is_edge_not_found_returns_true() {
        let err = KanbanError::from(DependencyError::EdgeNotFound);
        assert!(err.is_edge_not_found());
    }

    #[test]
    fn test_is_self_reference_returns_false_for_other_error() {
        let err = KanbanError::not_found("Card", Uuid::new_v4());
        assert!(!err.is_self_reference());
    }

    #[test]
    fn test_is_edge_not_found_returns_false_for_other_error() {
        let err = KanbanError::not_found("Card", Uuid::new_v4());
        assert!(!err.is_edge_not_found());
    }

    #[test]
    fn test_is_conflict_detected_returns_true() {
        let err = KanbanError::ConflictDetected {
            path: "test.json".to_string(),
            source: None,
        };
        assert!(err.is_conflict_detected());
    }

    #[test]
    fn test_is_wip_limit_exceeded_returns_true() {
        let id = Uuid::new_v4();
        let err = KanbanError::Domain(DomainError::wip_limit_exceeded(id, 3));
        assert!(err.is_wip_limit_exceeded());
    }

    #[test]
    fn test_sprint_board_mismatch_display_includes_all_three_ids() {
        let sprint_id = Uuid::new_v4();
        let sprint_board = Uuid::new_v4();
        let card_board = Uuid::new_v4();
        let err = KanbanError::Domain(DomainError::SprintBoardMismatch {
            sprint_id,
            sprint_board,
            card_board,
        });
        let msg = err.to_string();
        assert!(msg.contains(&sprint_id.to_string()), "msg: {msg}");
        assert!(msg.contains(&sprint_board.to_string()), "msg: {msg}");
        assert!(msg.contains(&card_board.to_string()), "msg: {msg}");
        assert!(msg.contains("belongs to board"), "msg: {msg}");
    }

    #[test]
    fn test_is_sprint_board_mismatch_predicate() {
        let err = KanbanError::Domain(DomainError::SprintBoardMismatch {
            sprint_id: Uuid::new_v4(),
            sprint_board: Uuid::new_v4(),
            card_board: Uuid::new_v4(),
        });
        assert!(err.is_sprint_board_mismatch());
        assert!(!err.is_validation());
        assert!(!err.is_not_found());
    }

    #[test]
    fn test_unsupported_future_version_display_mentions_both_versions() {
        let err = KanbanError::UnsupportedFutureVersion {
            file_version: 99,
            binary_max: 6,
        };
        let msg = err.to_string();
        assert!(msg.contains("99"), "msg should mention file version: {msg}");
        assert!(msg.contains('6'), "msg should mention binary max: {msg}");
    }

    #[test]
    fn test_is_unsupported_future_version_returns_true() {
        let err = KanbanError::UnsupportedFutureVersion {
            file_version: 99,
            binary_max: 6,
        };
        assert!(err.is_unsupported_future_version());
    }

    #[test]
    fn test_is_unsupported_future_version_returns_false_for_other_error() {
        let err = KanbanError::not_found("Card", Uuid::new_v4());
        assert!(!err.is_unsupported_future_version());
    }

    #[test]
    fn test_not_found_by_name_display_lists_available() {
        let err = KanbanError::not_found_by_name(
            "Column",
            "done",
            vec!["TODO".into(), "Doing".into(), "Complete".into()],
        );
        let msg = err.to_string();
        assert!(msg.contains("'done'"), "msg: {msg}");
        assert!(msg.contains("not found"), "msg: {msg}");
        assert!(msg.contains("'TODO'"), "msg: {msg}");
        assert!(msg.contains("'Doing'"), "msg: {msg}");
        assert!(msg.contains("'Complete'"), "msg: {msg}");
    }

    #[test]
    fn test_not_found_by_name_display_with_empty_available_omits_list() {
        let err = KanbanError::not_found_by_name("Card", "KAN-999", Vec::new());
        let msg = err.to_string();
        assert!(msg.contains("'KAN-999' not found"), "msg: {msg}");
        assert!(!msg.contains("Available:"), "msg: {msg}");
    }

    #[test]
    fn test_ambiguous_display_includes_label_and_uuid_per_match() {
        // KAN-400 polish: every match carries both a human label and a UUID
        // so users can disambiguate by either.
        let a_id = Uuid::new_v4();
        let b_id = Uuid::new_v4();
        let err = KanbanError::ambiguous(
            "Sprint",
            "13",
            vec![
                AmbiguousMatch {
                    label: "on board 'Project A'".into(),
                    id: a_id,
                },
                AmbiguousMatch {
                    label: "on board 'Project B'".into(),
                    id: b_id,
                },
            ],
        );
        let msg = err.to_string();
        assert!(msg.contains("'13' is ambiguous"), "msg: {msg}");
        assert!(msg.contains("'Project A'"), "msg: {msg}");
        assert!(msg.contains("'Project B'"), "msg: {msg}");
        assert!(
            msg.contains(&a_id.to_string()),
            "label-only is not enough: {msg}"
        );
        assert!(
            msg.contains(&b_id.to_string()),
            "label-only is not enough: {msg}"
        );
    }

    #[test]
    fn test_ambiguous_display_single_match_renders_cleanly() {
        // Degenerate case; can happen if a different resolver path produces
        // a single-match Ambiguous (shouldn't, but be defensive).
        let id = Uuid::new_v4();
        let err = KanbanError::ambiguous(
            "Card",
            "5",
            vec![AmbiguousMatch {
                label: "Some title".into(),
                id,
            }],
        );
        let msg = err.to_string();
        assert!(msg.contains("'5' is ambiguous"), "msg: {msg}");
        assert!(msg.contains("Some title"), "msg: {msg}");
        assert!(msg.contains(&id.to_string()), "msg: {msg}");
    }

    #[test]
    fn test_ambiguous_message_drops_specify_by_uuid_coda() {
        // The old wording "Specify by UUID" was redundant once the UUID is
        // already in the message. New message doesn't repeat it.
        let err = KanbanError::ambiguous(
            "Board",
            "shared",
            vec![AmbiguousMatch {
                label: "shared".into(),
                id: Uuid::new_v4(),
            }],
        );
        let msg = err.to_string();
        assert!(!msg.contains("Specify by UUID"), "msg: {msg}");
    }

    #[test]
    fn test_is_not_found_by_name_predicate() {
        let err = KanbanError::not_found_by_name("Column", "foo", Vec::new());
        assert!(err.is_not_found_by_name());
        // is_not_found is the umbrella predicate covering both shapes.
        assert!(err.is_not_found());
    }

    #[test]
    fn test_is_ambiguous_predicate() {
        let err = KanbanError::ambiguous(
            "Card",
            "5",
            vec![
                AmbiguousMatch {
                    label: "x".into(),
                    id: Uuid::new_v4(),
                },
                AmbiguousMatch {
                    label: "y".into(),
                    id: Uuid::new_v4(),
                },
            ],
        );
        assert!(err.is_ambiguous());
        assert!(!err.is_not_found());
    }

    #[test]
    fn test_batch_resolution_failed_display_includes_each_input_and_cause() {
        let err = KanbanError::batch_resolution_failed(
            "Card",
            vec![
                BatchResolutionFailure {
                    raw_input: "KAN-999".into(),
                    cause: BatchResolutionCause::NotFound,
                },
                BatchResolutionFailure {
                    raw_input: "KAN-998".into(),
                    cause: BatchResolutionCause::Ambiguous(vec![AmbiguousMatch {
                        label: "'one'".into(),
                        id: Uuid::new_v4(),
                    }]),
                },
            ],
        );
        let msg = err.to_string();
        assert!(msg.contains("2 Cards"), "msg: {msg}");
        assert!(msg.contains("'KAN-999'"), "msg: {msg}");
        assert!(msg.contains("'KAN-998'"), "msg: {msg}");
        assert!(msg.contains("not found"), "msg: {msg}");
        assert!(msg.contains("ambiguous"), "msg: {msg}");
    }

    #[test]
    fn test_batch_resolution_failed_display_singularizes_for_one_failure() {
        // KAN-400 review-3 fix: "1 card(s)" was grammatically awkward. Now
        // the noun agrees in number with the count, and entity casing matches
        // sibling error variants (NotFoundByName / Ambiguous both keep
        // "Card" capitalized).
        let err = KanbanError::batch_resolution_failed(
            "Card",
            vec![BatchResolutionFailure {
                raw_input: "KAN-999".into(),
                cause: BatchResolutionCause::NotFound,
            }],
        );
        let msg = err.to_string();
        assert!(msg.contains("1 Card"), "expected '1 Card' singular: {msg}");
        assert!(
            !msg.contains("1 Cards") && !msg.contains("card(s)"),
            "no plural or parenthetical: {msg}"
        );
        assert!(msg.contains('('), "still wraps cause in parens: {msg}");
    }

    #[test]
    fn test_batch_resolution_failed_display_preserves_entity_capitalization() {
        // Sibling variants render "Card '...' not found"; the batch variant
        // must match. No to_lowercase.
        let err = KanbanError::batch_resolution_failed(
            "Card",
            vec![
                BatchResolutionFailure {
                    raw_input: "x".into(),
                    cause: BatchResolutionCause::NotFound,
                },
                BatchResolutionFailure {
                    raw_input: "y".into(),
                    cause: BatchResolutionCause::NotFound,
                },
            ],
        );
        let msg = err.to_string();
        assert!(msg.contains("Cards"), "got: {msg}");
        assert!(!msg.contains(" cards"), "lowercased entity: {msg}");
    }

    #[test]
    fn test_ambiguous_match_display_is_label_then_uuid_in_parens() {
        // Standalone Display impl on AmbiguousMatch so callers can render
        // a match without re-implementing the format.
        let id = Uuid::new_v4();
        let m = AmbiguousMatch {
            label: "'Alpha'".into(),
            id,
        };
        let rendered = format!("{}", m);
        assert_eq!(rendered, format!("'Alpha' ({})", id));
    }

    #[test]
    fn test_batch_resolution_cause_display_renders_not_found_and_ambiguous() {
        // Standalone Display impl on BatchResolutionCause for symmetry.
        let nf = BatchResolutionCause::NotFound;
        assert_eq!(format!("{}", nf), "not found");

        let id = Uuid::new_v4();
        let amb = BatchResolutionCause::Ambiguous(vec![
            AmbiguousMatch {
                label: "'A'".into(),
                id,
            },
            AmbiguousMatch {
                label: "'B'".into(),
                id,
            },
        ]);
        let rendered = format!("{}", amb);
        assert!(rendered.starts_with("ambiguous: "), "got: {rendered}");
        assert!(rendered.contains("'A'"), "got: {rendered}");
        assert!(rendered.contains("'B'"), "got: {rendered}");
        assert!(
            rendered.matches(&id.to_string()).count() == 2,
            "got: {rendered}"
        );
    }

    #[test]
    fn test_is_batch_resolution_failed_predicate() {
        let err = KanbanError::batch_resolution_failed("Card", Vec::new());
        assert!(err.is_batch_resolution_failed());
        assert!(!err.is_not_found(), "not the same as a single not-found");
    }

    #[test]
    fn test_is_not_found_true_for_uuid_variant_too() {
        let err = KanbanError::not_found("Card", Uuid::new_v4());
        assert!(err.is_not_found(), "umbrella predicate covers Uuid variant");
        assert!(!err.is_not_found_by_name());
    }

    #[test]
    fn test_not_found_display_includes_entity_and_id() {
        let id = Uuid::new_v4();
        let err = KanbanError::not_found("Card", id);
        let msg = err.to_string();
        assert!(msg.contains("Card"));
        assert!(msg.contains(&id.to_string()));
    }

    #[test]
    fn test_already_exists_display_includes_entity_and_id() {
        let id = Uuid::new_v4();
        let err = KanbanError::already_exists("Board", id);
        let msg = err.to_string();
        assert!(msg.contains("Board"), "msg: {msg}");
        assert!(msg.contains(&id.to_string()), "msg: {msg}");
        assert!(msg.contains("already exists"), "msg: {msg}");
    }

    #[test]
    fn test_is_already_exists_returns_true() {
        let err = KanbanError::already_exists("Card", Uuid::new_v4());
        assert!(err.is_already_exists());
    }

    #[test]
    fn test_is_already_exists_returns_false_for_other_error() {
        let err = KanbanError::not_found("Card", Uuid::new_v4());
        assert!(!err.is_already_exists());
        assert!(!err.is_conflict_detected());
    }

    #[test]
    fn test_from_dependency_error_converts_to_kanban_domain() {
        let dep_err = DependencyError::CycleDetected;
        let kanban_err = KanbanError::from(dep_err);
        assert!(matches!(
            kanban_err,
            KanbanError::Domain(DomainError::Dependency(DependencyError::CycleDetected))
        ));
    }

    #[test]
    fn test_from_core_error_validation_converts_to_kanban_validation() {
        let core_err = kanban_core::CoreError::Validation("bad".to_string());
        let kanban_err = KanbanError::from(core_err);
        assert!(kanban_err.is_validation());
    }

    #[test]
    fn test_from_core_error_config_converts_to_internal() {
        let core_err = kanban_core::CoreError::Config("cfg error".to_string());
        let kanban_err = KanbanError::from(core_err);
        assert!(matches!(kanban_err, KanbanError::Internal(_)));
    }
}
