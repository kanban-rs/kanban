use kanban_domain::{KanbanError, KanbanResult};
use std::path::{Path, PathBuf};

/// Validates and resolves `path` relative to the process's current working directory.
///
/// - **Absolute paths** are canonicalized if the target exists, or returned as-is if
///   the file does not yet exist (so callers can open a new file).
/// - **Relative paths** are resolved against the current directory. Any path that
///   would escape the current directory via `..` components is rejected.
///
/// When the target exists, the returned path is in canonical form: no `\\?\`
/// verbatim UNC prefix on Windows, and symlinks in the cwd (e.g. macOS
/// `/var` → `/private/var`) are resolved.
///
/// # Security contract
///
/// This function prevents callers from accidentally (or maliciously) opening files
/// outside the working directory. Path traversal attempts such as `../../secret.json`
/// return `Err` with a message containing "Path traversal not allowed".
///
/// A locator carrying a URL scheme (`kanban_core::is_remote_locator`) is returned
/// unchanged and is not validated as a path.
pub fn validate_path(path: &Path) -> KanbanResult<PathBuf> {
    let cwd = std::env::current_dir().map_err(|e| KanbanError::from(std::io::Error::other(e)))?;
    validate_path_with_cwd(path, &cwd)
}

fn validate_path_with_cwd(path: &Path, cwd: &Path) -> KanbanResult<PathBuf> {
    if path.to_str().is_some_and(kanban_core::is_remote_locator) {
        return Ok(path.to_path_buf());
    }
    if path.is_absolute() {
        Ok(dunce::canonicalize(path).unwrap_or_else(|_| path.to_path_buf()))
    } else {
        let canonical_cwd = dunce::canonicalize(cwd).unwrap_or_else(|_| cwd.to_path_buf());
        let resolved = canonical_cwd.join(path);
        let canonical =
            dunce::canonicalize(&resolved).unwrap_or_else(|_| normalize_path(&resolved));
        if !canonical.starts_with(&canonical_cwd) {
            return Err(KanbanError::validation(format!(
                "Path traversal not allowed: '{}' resolves outside current directory",
                path.display()
            )));
        }
        Ok(canonical)
    }
}

fn normalize_path(path: &Path) -> PathBuf {
    use std::path::Component;
    let mut components: Vec<Component> = Vec::new();
    for component in path.components() {
        match component {
            Component::ParentDir => match components.last() {
                Some(Component::Normal(_)) => {
                    components.pop();
                }
                _ => components.push(component),
            },
            Component::CurDir => {}
            c => components.push(c),
        }
    }
    components.iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_validate_path_relative_within_cwd_returns_resolved() -> KanbanResult<()> {
        let dir = TempDir::new().unwrap();
        let cwd = dunce::canonicalize(dir.path()).unwrap();
        let result = validate_path_with_cwd(Path::new("some/nested/file.json"), &cwd)?;
        assert!(result.starts_with(&cwd));
        assert!(result.ends_with("some/nested/file.json"));
        Ok(())
    }

    #[test]
    fn test_validate_path_absolute_passes_through() -> KanbanResult<()> {
        let dir = TempDir::new().unwrap();
        let abs = dir.path().join("file.json");
        let result = validate_path_with_cwd(&abs, dir.path())?;
        assert_eq!(result, abs);
        Ok(())
    }

    #[test]
    fn test_validate_path_traversal_is_rejected() {
        let dir = TempDir::new().unwrap();
        let deep = dir.path().join("a/b/c");
        std::fs::create_dir_all(&deep).unwrap();
        let result = validate_path_with_cwd(Path::new("../../secret.json"), &deep);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Path traversal not allowed"), "Got: {err}");
    }

    #[test]
    fn test_validate_path_relative_within_cwd_existing_file_returns_resolved() -> KanbanResult<()> {
        let dir = TempDir::new().unwrap();
        let cwd = dunce::canonicalize(dir.path()).unwrap();
        std::fs::write(cwd.join("kanban.json"), b"{}").unwrap();
        let result = validate_path_with_cwd(Path::new("kanban.json"), &cwd)?;
        assert!(
            result.starts_with(&cwd),
            "result {} should start with cwd {}",
            result.display(),
            cwd.display()
        );
        assert!(result.ends_with("kanban.json"));
        assert!(
            !result.to_string_lossy().starts_with(r"\\?\"),
            "result should not have UNC prefix, got: {}",
            result.display()
        );
        Ok(())
    }

    #[test]
    fn test_a_remote_locator_is_not_cwd_joined() {
        let dir = TempDir::new().unwrap();
        let result =
            validate_path_with_cwd(Path::new("http://127.0.0.1:3000"), dir.path()).unwrap();
        assert_eq!(result, PathBuf::from("http://127.0.0.1:3000"));
        assert!(result.to_string_lossy().contains("://"));
    }

    #[test]
    fn test_a_remote_locator_passes_through_the_public_entry_point() -> KanbanResult<()> {
        let result = validate_path(Path::new("https://example.com/boards"))?;
        assert_eq!(result, PathBuf::from("https://example.com/boards"));
        Ok(())
    }

    #[test]
    fn test_validate_path_absolute_existing_file_returns_non_unc() -> KanbanResult<()> {
        let dir = TempDir::new().unwrap();
        let cwd = dunce::canonicalize(dir.path()).unwrap();
        let abs = cwd.join("kanban.json");
        std::fs::write(&abs, b"{}").unwrap();
        let result = validate_path_with_cwd(&abs, &cwd)?;
        assert!(
            result.starts_with(&cwd),
            "result {} should start with cwd {}",
            result.display(),
            cwd.display()
        );
        assert!(
            !result.to_string_lossy().starts_with(r"\\?\"),
            "result should not have UNC prefix, got: {}",
            result.display()
        );
        assert!(result.ends_with("kanban.json"));
        Ok(())
    }
}
