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
