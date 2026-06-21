use crate::error;
use kanban_domain::KanbanError;
use rmcp::model::ErrorData as McpError;

pub(crate) fn kanban_err_to_mcp(e: KanbanError) -> McpError {
    error::KanbanMcpError::Domain(e).into()
}

/// Symmetric with the CLI handler's `enrich_add_error`: rewrite an
/// anonymous DependencyError from a parent-edge add into a message
/// that names both sides of the edge, using the user's raw
/// identifiers. Non-dependency errors pass through to Domain.
///
/// Enriched messages flow through the `Resolution` variant so the
/// rendered hint is verbatim (no "invalid parameter: " prefix),
/// matching the CLI's `KanbanCliError::Resolution` rendering — both
/// sides share the same `messages::*` helpers, so symmetrical
/// rendering is the only way the two surfaces stay in step.
pub(crate) fn mcp_enrich_add_error(
    e: KanbanError,
    parent_raw: &str,
    child_raw: &str,
) -> error::KanbanMcpError {
    use kanban_domain::dependencies::messages;
    use kanban_domain::error::{DependencyError, DomainError};
    match e {
        KanbanError::Domain(DomainError::Dependency(DependencyError::CycleDetected)) => {
            error::KanbanMcpError::Resolution {
                hint: messages::parent_cycle(parent_raw, child_raw),
            }
        }
        KanbanError::Domain(DomainError::Dependency(DependencyError::SelfReference)) => {
            error::KanbanMcpError::Resolution {
                hint: messages::parent_self_reference(parent_raw),
            }
        }
        KanbanError::Domain(DomainError::Dependency(DependencyError::DuplicateEdge)) => {
            error::KanbanMcpError::Resolution {
                hint: messages::parent_duplicate(parent_raw, child_raw),
            }
        }
        KanbanError::Domain(DomainError::Dependency(DependencyError::EdgeNotFound)) => e.into(),
        other => other.into(),
    }
}

pub(crate) fn mcp_enrich_remove_error(
    e: KanbanError,
    parent_raw: &str,
    child_raw: &str,
) -> error::KanbanMcpError {
    use kanban_domain::dependencies::messages;
    use kanban_domain::error::{DependencyError, DomainError};
    match e {
        KanbanError::Domain(DomainError::Dependency(DependencyError::EdgeNotFound)) => {
            error::KanbanMcpError::Resolution {
                hint: messages::parent_edge_not_found(parent_raw, child_raw),
            }
        }
        KanbanError::Domain(DomainError::Dependency(DependencyError::CycleDetected)) => e.into(),
        KanbanError::Domain(DomainError::Dependency(DependencyError::SelfReference)) => e.into(),
        KanbanError::Domain(DomainError::Dependency(DependencyError::DuplicateEdge)) => e.into(),
        other => other.into(),
    }
}

pub(crate) fn core_err_to_mcp(e: kanban_core::CoreError) -> McpError {
    kanban_err_to_mcp(KanbanError::from(e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use kanban_domain::dependencies::messages;
    use kanban_domain::error::{DependencyError, DomainError};
    use rmcp::model::ErrorCode;

    // ─── Enrich helpers: symmetry with CLI ────────────────────────
    //
    // Both the CLI handler's enrich_*_error and the MCP enrich
    // helpers feed user-facing hints from the same `messages::*`
    // string-builders. The MCP path renders those hints verbatim
    // (no "invalid parameter: " prefix) so the two surfaces stay in
    // step. INVALID_PARAMS on the wire carries the semantic category.
    //
    // Pin both: the rendered McpError.message must equal the bare
    // hint produced by the shared message helper, and the error
    // code must be INVALID_PARAMS (the resolution category).

    #[test]
    fn test_mcp_enrich_add_error_cycle_renders_hint_verbatim() {
        let err: McpError = mcp_enrich_add_error(
            KanbanError::Domain(DomainError::Dependency(DependencyError::CycleDetected)),
            "KAN-5",
            "KAN-7",
        )
        .into();
        assert_eq!(err.message, messages::parent_cycle("KAN-5", "KAN-7"));
        assert_eq!(err.code, ErrorCode::INVALID_PARAMS);
    }

    #[test]
    fn test_mcp_enrich_add_error_self_reference_renders_hint_verbatim() {
        let err: McpError = mcp_enrich_add_error(
            KanbanError::Domain(DomainError::Dependency(DependencyError::SelfReference)),
            "KAN-5",
            "KAN-5",
        )
        .into();
        assert_eq!(err.message, messages::parent_self_reference("KAN-5"));
        assert_eq!(err.code, ErrorCode::INVALID_PARAMS);
    }

    #[test]
    fn test_mcp_enrich_add_error_duplicate_renders_hint_verbatim() {
        let err: McpError = mcp_enrich_add_error(
            KanbanError::Domain(DomainError::Dependency(DependencyError::DuplicateEdge)),
            "KAN-5",
            "KAN-7",
        )
        .into();
        assert_eq!(err.message, messages::parent_duplicate("KAN-5", "KAN-7"));
        assert_eq!(err.code, ErrorCode::INVALID_PARAMS);
    }

    #[test]
    fn test_mcp_enrich_remove_error_edge_not_found_renders_hint_verbatim() {
        let err: McpError = mcp_enrich_remove_error(
            KanbanError::Domain(DomainError::Dependency(DependencyError::EdgeNotFound)),
            "KAN-5",
            "KAN-7",
        )
        .into();
        assert_eq!(
            err.message,
            messages::parent_edge_not_found("KAN-5", "KAN-7")
        );
        assert_eq!(err.code, ErrorCode::INVALID_PARAMS);
    }

    #[test]
    fn err_not_found_maps_to_invalid_params() {
        use rmcp::model::ErrorCode;
        let err = kanban_err_to_mcp(KanbanError::not_found("Board", uuid::Uuid::new_v4()));
        assert_eq!(err.code, ErrorCode::INVALID_PARAMS);
        assert!(err.message.contains("Board"));
    }

    #[test]
    fn err_validation_maps_to_invalid_params() {
        use rmcp::model::ErrorCode;
        let err = kanban_err_to_mcp(KanbanError::validation("bad input"));
        assert_eq!(err.code, ErrorCode::INVALID_PARAMS);
    }

    #[test]
    fn err_cycle_maps_to_invalid_params() {
        use kanban_domain::DependencyError;
        use rmcp::model::ErrorCode;
        let err = kanban_err_to_mcp(KanbanError::from(DependencyError::CycleDetected));
        assert_eq!(err.code, ErrorCode::INVALID_PARAMS);
    }

    #[test]
    fn err_self_ref_maps_to_invalid_params() {
        use kanban_domain::DependencyError;
        use rmcp::model::ErrorCode;
        let err = kanban_err_to_mcp(KanbanError::from(DependencyError::SelfReference));
        assert_eq!(err.code, ErrorCode::INVALID_PARAMS);
    }

    #[test]
    fn err_edge_not_found_maps_to_invalid_params() {
        use kanban_domain::DependencyError;
        use rmcp::model::ErrorCode;
        let err = kanban_err_to_mcp(KanbanError::from(DependencyError::EdgeNotFound));
        assert_eq!(err.code, ErrorCode::INVALID_PARAMS);
    }

    #[test]
    fn err_internal_maps_to_internal_error() {
        use rmcp::model::ErrorCode;
        let err = kanban_err_to_mcp(KanbanError::Internal("boom".into()));
        assert_eq!(err.code, ErrorCode::INTERNAL_ERROR);
    }

    #[test]
    fn err_io_maps_to_internal_error() {
        use rmcp::model::ErrorCode;
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file gone");
        let err = kanban_err_to_mcp(KanbanError::Io(io_err));
        assert_eq!(err.code, ErrorCode::INTERNAL_ERROR);
    }
}
