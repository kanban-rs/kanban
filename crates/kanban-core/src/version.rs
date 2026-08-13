//! Shared version constants for kanban binaries.
//!
//! Three consts, two pieces of information:
//! - [`KANBAN_VERSION`] / [`KANBAN_COMMIT`] are the raw components, consumed by
//!   the persistence layer when stamping a file's writer identity.
//! - [`CLI_VERSION_DISPLAY`] is the multi-line string clap renders for
//!   `--version`. It looks like `"0.6.0\ncommit: 18e98c4..."`.
//!
//! The redundancy is structural: Rust's `concat!` only accepts literal
//! expressions, so it must read the env vars directly rather than reuse the
//! component consts. Moving the concat into the binary crates would require
//! either duplicating `build.rs` (no thanks) or runtime `Box::leak` (worse).

/// Semver-style version of the kanban crate that produced this binary.
/// Stamped into persistence metadata so a file remembers which kanban wrote it.
pub const KANBAN_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Git commit hash of the kanban build that produced this binary.
/// Resolves to `"unknown"` when built outside a git checkout.
pub const KANBAN_COMMIT: &str = env!("GIT_COMMIT_HASH");

/// Multi-line display string passed to clap's `version = ...` attribute on the
/// `kanban` and `kanban-mcp` binaries. Renders `"{KANBAN_VERSION}\ncommit:
/// {KANBAN_COMMIT}"` for an exact-tag build, `"unstable\ncommit:
/// {KANBAN_COMMIT}"` for a git checkout with no exact tag, and bare
/// `KANBAN_VERSION` when no git commit is available at all (crates.io/AUR
/// tarball builds).
#[cfg(all(has_git_commit, not(is_unstable_build)))]
pub const CLI_VERSION_DISPLAY: &str = concat!(
    env!("CARGO_PKG_VERSION"),
    "\ncommit: ",
    env!("GIT_COMMIT_HASH")
);

#[cfg(all(has_git_commit, is_unstable_build))]
pub const CLI_VERSION_DISPLAY: &str = concat!("unstable\ncommit: ", env!("GIT_COMMIT_HASH"));

#[cfg(not(has_git_commit))]
pub const CLI_VERSION_DISPLAY: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_version_display_is_non_empty() {
        assert!(!CLI_VERSION_DISPLAY.is_empty());
    }

    #[test]
    fn test_kanban_version_const_is_non_empty() {
        assert!(!KANBAN_VERSION.is_empty());
    }

    #[test]
    fn test_kanban_commit_const_is_non_empty() {
        assert!(!KANBAN_COMMIT.is_empty());
    }
}
