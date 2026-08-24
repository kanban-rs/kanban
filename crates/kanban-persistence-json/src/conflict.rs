use std::fs;
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::time::SystemTime;

/// Metadata about a file for conflict detection
///
/// Uses modification time, size, and content hash for comprehensive change detection.
/// The hash provides additional protection against edge cases where timestamp and size
/// might be identical but content differs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileMetadata {
    /// Last modified time of the file
    pub modified_time: SystemTime,
    /// File size in bytes for additional verification
    pub size: u64,
    /// Content hash for detecting changes even when timestamp/size are identical
    pub content_hash: u64,
}

impl FileMetadata {
    /// Create FileMetadata from a file on disk
    ///
    /// Computes content hash by reading and hashing the entire file.
    /// For typical kanban files (< 1MB), this adds minimal overhead (1-5ms).
    pub fn from_file(path: &Path) -> std::io::Result<Self> {
        let metadata = fs::metadata(path)?;
        let modified_time = metadata.modified()?;
        let size = metadata.len();

        // Always compute content hash for comprehensive change detection
        let content = fs::read(path)?;
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        content.hash(&mut hasher);
        let content_hash = hasher.finish();

        Ok(Self {
            modified_time,
            size,
            content_hash,
        })
    }

    /// Check if file has changed since this metadata was captured
    pub fn has_changed(&self, path: &Path) -> std::io::Result<bool> {
        let current = Self::from_file(path)?;
        Ok(current != *self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_metadata_from_file() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.json");
        let content = b"test content";
        fs::write(&file_path, content).unwrap();

        let metadata = FileMetadata::from_file(&file_path).unwrap();
        assert_eq!(metadata.size, content.len() as u64);
    }

    #[test]
    fn test_has_not_changed() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.json");
        fs::write(&file_path, b"test content").unwrap();

        let metadata = FileMetadata::from_file(&file_path).unwrap();
        assert!(!metadata.has_changed(&file_path).unwrap());
    }

    #[test]
    fn test_has_changed_on_content_modification() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.json");
        fs::write(&file_path, b"test content").unwrap();

        let metadata = FileMetadata::from_file(&file_path).unwrap();
        fs::write(&file_path, b"modified content").unwrap();

        assert!(metadata.has_changed(&file_path).unwrap());
    }

    #[test]
    fn test_size_change_detected() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.json");
        fs::write(&file_path, b"content1").unwrap();

        let metadata = FileMetadata::from_file(&file_path).unwrap();
        assert_eq!(metadata.size, 8);

        fs::write(&file_path, b"content1_longer").unwrap();
        assert!(metadata.has_changed(&file_path).unwrap());
    }

    #[test]
    fn test_file_metadata_is_defined_exactly_once_in_workspace() {
        let this_src = include_str!("conflict.rs");
        let detector_src = include_str!("../../kanban-persistence/src/conflict/detector.rs");
        let occurrences = [this_src, detector_src]
            .iter()
            .filter(|src| src.contains("pub struct FileMetadata"))
            .count();
        assert_eq!(
            occurrences, 1,
            "FileMetadata must be defined exactly once, in kanban-persistence; \
             kanban-persistence-json must import it rather than redefining it"
        );
    }
}
