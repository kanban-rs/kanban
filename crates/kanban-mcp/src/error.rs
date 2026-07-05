use kanban_domain::KanbanError;
use rmcp::model::ErrorData as McpError;
use thiserror::Error;

/// MCP-boundary error type.
///
/// Wraps [`KanbanError`] for the domain layer plus MCP-specific
/// concerns (handler-enriched messages). Tool handlers thread this
/// through with `?`; the boundary converts to the rmcp [`McpError`]
/// for transmission to the client.
///
/// Handler-enriched messages (built from the user's raw identifiers
/// to produce hints like "cycle detected: making KAN-5 a parent of
/// KAN-7 would create a cycle") flow through the `Resolution`
/// variant, which renders the hint verbatim. This is symmetric with
/// the CLI-side `KanbanCliError::Resolution`: both surfaces use the
/// same `messages::*` helpers and must render the result the same
/// way. The `INVALID_PARAMS` error code on the wire encodes the
/// semantic category; the human-readable message stays clean.
#[derive(Error, Debug)]
pub enum KanbanMcpError {
    #[error(transparent)]
    Domain(#[from] KanbanError),
    /// Handler-enriched user-facing message at the MCP boundary.
    /// Display renders the hint verbatim, no prefix.
    #[error("{hint}")]
    Resolution { hint: String },
}

pub type KanbanMcpResult<T> = Result<T, KanbanMcpError>;

impl From<KanbanMcpError> for McpError {
    fn from(e: KanbanMcpError) -> Self {
        match e {
            KanbanMcpError::Domain(d) => {
                // INVALID_PARAMS for anything that's "the inputs the client
                // gave us are wrong": validation/lookup failures (Domain) and
                // version mismatches on the data file the client pointed at
                // (UnsupportedFutureVersion). Everything else is a server-side
                // failure the client can't fix by rewording its request.
                if matches!(
                    &d,
                    KanbanError::Domain(_) | KanbanError::UnsupportedFutureVersion { .. }
                ) {
                    McpError::invalid_params(d.to_string(), None)
                } else {
                    McpError::internal_error(d.to_string(), None)
                }
            }
            KanbanMcpError::Resolution { .. } => McpError::invalid_params(e.to_string(), None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kanban_mcp_error_wraps_kanban_error() {
        let domain = KanbanError::validation("bad");
        let mcp: KanbanMcpError = domain.into();
        assert!(matches!(mcp, KanbanMcpError::Domain(_)));
    }

    #[test]
    fn test_kanban_mcp_error_resolution_displays_hint() {
        let err = KanbanMcpError::Resolution {
            hint: "no match".into(),
        };
        assert!(err.to_string().contains("no match"));
    }

    #[test]
    fn test_into_mcp_error_maps_domain_validation_to_invalid_params() {
        let err: KanbanMcpError = KanbanError::validation("bad").into();
        let mcp: McpError = err.into();
        assert!(format!("{:?}", mcp).contains("bad"));
    }

    /// Pins the absence of double-hint interpolation: the rendered
    /// `McpError.message` must contain the hint string exactly once.
    /// A regression where Display already includes the hint AND the
    /// conversion prepends it again would produce two occurrences and
    /// fail this assertion.
    #[test]
    fn test_into_mcp_error_resolution_renders_hint_exactly_once() {
        let hint = "no card matches 'foo'";
        let err = KanbanMcpError::Resolution {
            hint: hint.to_string(),
        };
        let mcp: McpError = err.into();
        let occurrences = mcp.message.matches(hint).count();
        assert_eq!(
            occurrences, 1,
            "hint should appear exactly once in rendered message; got {occurrences} in {:?}",
            mcp.message
        );
    }

    /// MCP `Resolution` must render the bare hint — no
    /// `identifier resolution failed:` prefix. The CLI side strips
    /// the same prefix; this pins the symmetry. INVALID_PARAMS
    /// already encodes the category in the error code; the
    /// human-readable message stays consistent across surfaces.
    #[test]
    fn test_into_mcp_error_resolution_renders_no_prefix() {
        let hint = "Card 'KAN-99999' not found";
        let err = KanbanMcpError::Resolution {
            hint: hint.to_string(),
        };
        let mcp: McpError = err.into();
        assert_eq!(
            mcp.message, hint,
            "Resolution rendered with prefix: {:?}",
            mcp.message
        );
    }

    /// A backend gap (`KanbanError::unsupported`) is a server fault: it must map
    /// to `INTERNAL_ERROR`, NOT `INVALID_PARAMS` (which would tell the client its
    /// inputs were wrong). This holds because `Unsupported` is a top-level
    /// `KanbanError`, not a `DomainError` (the `Domain(_)` blanket above routes
    /// to `INVALID_PARAMS`).
    #[test]
    fn test_into_mcp_error_unsupported_maps_to_internal_error() {
        let err: KanbanMcpError = KanbanError::unsupported("archive_board").into();
        let mcp: McpError = err.into();
        assert_eq!(mcp.code, rmcp::model::ErrorCode::INTERNAL_ERROR);
    }
}
