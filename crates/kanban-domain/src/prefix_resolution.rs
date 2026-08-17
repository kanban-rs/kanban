//! The one rule for resolving a prefix, on either axis.
//!
//! A prefix is chosen by walking from the most specific entity that names one
//! out to the least: sprint, then board, then the workspace default, then the
//! axis's built-in. Cards and sprints differ only in which fields they offer
//! and which built-in ends the chain.
//!
//! The workspace default is an `Option` so that a caller with no configuration
//! to read cannot accidentally substitute a different constant: it passes
//! `None` and lands on the same built-in every other caller would.

use crate::{DEFAULT_CARD_PREFIX, DEFAULT_SPRINT_PREFIX};

/// The namespace a prefix names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrefixAxis {
    Card,
    Sprint,
}

impl PrefixAxis {
    /// The prefix used when neither an entity nor the workspace names one.
    pub const fn builtin(self) -> &'static str {
        match self {
            Self::Card => DEFAULT_CARD_PREFIX,
            Self::Sprint => DEFAULT_SPRINT_PREFIX,
        }
    }
}

/// The first override that is set, else the workspace default, else the
/// axis's built-in.
///
/// `overrides` is ordered most specific first.
pub fn resolve<'a>(
    axis: PrefixAxis,
    overrides: impl IntoIterator<Item = Option<&'a str>>,
    configured: Option<&'a str>,
) -> &'a str {
    overrides
        .into_iter()
        .flatten()
        .next()
        .or(configured)
        .unwrap_or(axis.builtin())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_the_most_specific_override_wins() {
        assert_eq!(
            resolve(PrefixAxis::Card, [Some("SPR"), Some("BRD")], Some("cfg")),
            "SPR"
        );
    }

    #[test]
    fn test_a_less_specific_override_is_used_when_the_first_is_unset() {
        assert_eq!(
            resolve(PrefixAxis::Card, [None, Some("BRD")], Some("cfg")),
            "BRD"
        );
    }

    #[test]
    fn test_the_configured_default_is_used_when_no_entity_names_one() {
        assert_eq!(resolve(PrefixAxis::Card, [None, None], Some("cfg")), "cfg");
    }

    #[test]
    fn test_the_builtin_ends_the_chain_when_nothing_else_names_one() {
        assert_eq!(resolve(PrefixAxis::Card, [None, None], None), "task");
        assert_eq!(resolve(PrefixAxis::Sprint, [None, None], None), "sprint");
    }

    #[test]
    fn test_the_axis_only_changes_the_builtin() {
        let overrides = [None, Some("BRD")];
        assert_eq!(
            resolve(PrefixAxis::Card, overrides, Some("cfg")),
            resolve(PrefixAxis::Sprint, overrides, Some("cfg")),
            "the two axes must walk the same chain"
        );
        assert_ne!(
            resolve(PrefixAxis::Card, [None, None], None),
            resolve(PrefixAxis::Sprint, [None, None], None),
            "the axis must still decide which builtin ends the chain"
        );
    }
}
