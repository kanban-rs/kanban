use std::path::{Path, PathBuf};

/// Env var naming the local checkout to read git history from.
pub const REPO_PATH_ENV: &str = "KANBAN_REPO";

/// Resolves which local git checkout the client should read, by precedence:
/// 1. `explicit` — an explicitly configured path (CLI flag / config field).
/// 2. `env_repo` — the `KANBAN_REPO` override.
/// 3. the first ancestor of `start_dir` (inclusive) that contains a `.git` entry.
/// 4. `None` — nothing configured and no enclosing repository.
///
/// Pure: `env_repo` and `start_dir` are injected so callers can unit-test every
/// branch without touching the process environment or the real cwd.
pub fn resolve_repo_path(
    explicit: Option<PathBuf>,
    env_repo: Option<PathBuf>,
    start_dir: &Path,
) -> Option<PathBuf> {
    if let Some(path) = explicit {
        return Some(path);
    }
    if let Some(path) = env_repo {
        return Some(path);
    }
    find_enclosing_repo(start_dir)
}

/// Walks `start_dir` and its ancestors, returning the first directory that
/// contains a `.git` entry. `.git` may be a directory (normal clone) or a file
/// (worktree/submodule gitlink), so existence — not directory-ness — is the test.
fn find_enclosing_repo(start_dir: &Path) -> Option<PathBuf> {
    start_dir
        .ancestors()
        .find(|dir| dir.join(".git").exists())
        .map(Path::to_path_buf)
}

/// Impure entry point mirroring `path::validate_path`: reads `KANBAN_REPO` and
/// the process cwd, then delegates to the pure [`resolve_repo_path`]. Returns
/// `None` if the cwd cannot be read and no explicit/env path is set.
pub fn resolve_repo_path_from_env(explicit: Option<PathBuf>) -> Option<PathBuf> {
    let env_repo = std::env::var_os(REPO_PATH_ENV).map(PathBuf::from);
    if explicit.is_some() || env_repo.is_some() {
        return resolve_repo_path(explicit, env_repo, Path::new(""));
    }
    let cwd = std::env::current_dir().ok()?;
    resolve_repo_path(None, None, &cwd)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};

    #[test]
    fn test_resolve_prefers_explicit_over_env() {
        let result = resolve_repo_path(
            Some(PathBuf::from("/explicit")),
            Some(PathBuf::from("/from-env")),
            Path::new("/cwd"),
        );
        assert_eq!(result, Some(PathBuf::from("/explicit")));
    }

    #[test]
    fn test_resolve_uses_env_when_no_explicit() {
        let result = resolve_repo_path(
            None,
            Some(PathBuf::from("/from-env")),
            Path::new("/some/dir"),
        );
        assert_eq!(result, Some(PathBuf::from("/from-env")));
    }

    #[test]
    fn test_resolve_walks_up_to_git_dir() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();
        std::fs::create_dir(root.join(".git")).unwrap();
        let deep = root.join("a").join("b").join("c");
        std::fs::create_dir_all(&deep).unwrap();

        let found = resolve_repo_path(None, None, &deep);

        assert_eq!(found.as_deref(), Some(root));
    }

    #[test]
    fn test_resolve_returns_none_when_nothing_found() {
        let tmp = tempfile::TempDir::new().unwrap();
        assert!(tmp.path().ancestors().all(|d| !d.join(".git").exists()));

        assert!(resolve_repo_path(None, None, tmp.path()).is_none());
    }

    #[test]
    fn test_resolve_walk_matches_git_file_not_just_dir() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();
        std::fs::write(root.join(".git"), "gitdir: /elsewhere").unwrap();
        let deep = root.join("nested");
        std::fs::create_dir_all(&deep).unwrap();

        let found = resolve_repo_path(None, None, &deep);

        assert_eq!(found.as_deref(), Some(root));
    }
}
