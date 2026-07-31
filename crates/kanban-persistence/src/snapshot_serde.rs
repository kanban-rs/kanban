use crate::{PersistenceError, PersistenceResult};
use kanban_domain::Snapshot;

pub fn snapshot_to_json_bytes(snapshot: &Snapshot) -> PersistenceResult<Vec<u8>> {
    serde_json::to_vec_pretty(snapshot).map_err(|e| PersistenceError::Serialization(e.to_string()))
}

pub fn snapshot_from_json_bytes(bytes: &[u8]) -> PersistenceResult<Snapshot> {
    serde_json::from_slice(bytes).map_err(|e| PersistenceError::Serialization(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use kanban_domain::{Board, DependencyGraph};

    #[test]
    fn test_snapshot_roundtrip() {
        let board = Board::new("Test Board", None::<String>);
        let snapshot = Snapshot::from_data(
            vec![board],
            vec![],
            vec![],
            vec![],
            vec![],
            DependencyGraph::new(),
        );

        let bytes = snapshot_to_json_bytes(&snapshot).unwrap();
        let restored = snapshot_from_json_bytes(&bytes).unwrap();

        assert_eq!(restored.boards.len(), 1);
        assert_eq!(restored.boards[0].name, "Test Board");
    }

    #[test]
    fn test_snapshot_from_invalid_json_returns_error() {
        let result = snapshot_from_json_bytes(b"not json");
        assert!(result.is_err());
        match result.unwrap_err() {
            PersistenceError::Serialization(msg) => {
                assert!(!msg.is_empty());
            }
            other => panic!("Expected Serialization error, got: {:?}", other),
        }
    }

    #[test]
    fn test_persistence_error_source_on_conflict_detected() {
        use std::error::Error;
        use std::io;

        let err = PersistenceError::ConflictDetected {
            path: "test.json".to_string(),
            source: Some(Box::new(io::Error::other("inner"))),
        };
        assert!(err.source().is_some());

        let err_none = PersistenceError::ConflictDetected {
            path: "test.json".to_string(),
            source: None,
        };
        assert!(err_none.source().is_none());
    }
}
