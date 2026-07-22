//! V9 marks the format as archived-board-capable. `archived_boards` is an
//! additive, `#[serde(default)]` snapshot field (C1/C4), so no data transform
//! is required; the bump exists so a pre-archived-board binary (max V8) REJECTS
//! a V9 file instead of loading it and dropping the `archived_boards` array on
//! its next save. The transform normalizes the key to `[]` when absent so a V9
//! file is self-describing.

use std::path::Path;

use kanban_persistence::{PersistenceError, PersistenceResult};
use serde_json::Value;

pub(crate) async fn migrate_v8_to_v9(path: &Path) -> PersistenceResult<()> {
    let content = tokio::fs::read_to_string(path).await?;
    let mut envelope: Value = serde_json::from_str(&content)
        .map_err(|e| PersistenceError::Serialization(e.to_string()))?;
    if !transform_v8_to_v9_value(&mut envelope)? {
        return Ok(());
    }
    let out = serde_json::to_string_pretty(&envelope)
        .map_err(|e| PersistenceError::Serialization(e.to_string()))?;
    // Atomic write (temp file + rename) like every other migration step and the
    // sync V8->V9 twin: a crash mid-write must not truncate the live file.
    crate::atomic_writer::AtomicWriter::write_atomic(path, out.as_bytes()).await?;
    Ok(())
}

/// Returns `true` if the envelope was changed (needs writing back). Idempotent:
/// a file already at version >= 9 is left untouched.
pub(crate) fn transform_v8_to_v9_value(envelope: &mut Value) -> PersistenceResult<bool> {
    let version = envelope.get("version").and_then(Value::as_u64).unwrap_or(0);
    if version >= 9 {
        return Ok(false);
    }
    if let Some(data) = envelope.get_mut("data").and_then(Value::as_object_mut) {
        data.entry("archived_boards")
            .or_insert_with(|| Value::Array(vec![]));
    }
    envelope["version"] = Value::Number(9.into());
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_v8_to_v9_bumps_version_and_defaults_archived_boards() {
        let mut env = json!({ "version": 8, "data": { "boards": [] } });
        assert!(transform_v8_to_v9_value(&mut env).unwrap());
        assert_eq!(env["version"], 9);
        assert_eq!(env["data"]["archived_boards"], json!([]));
    }

    #[test]
    fn test_v8_to_v9_preserves_existing_archived_boards() {
        let mut env =
            json!({ "version": 8, "data": { "archived_boards": [{"entity":{"id":"x"}}] } });
        transform_v8_to_v9_value(&mut env).unwrap();
        assert_eq!(env["data"]["archived_boards"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn test_v8_to_v9_is_idempotent_on_v9() {
        let mut env = json!({ "version": 9, "data": {} });
        assert!(!transform_v8_to_v9_value(&mut env).unwrap());
    }

    // KAN-966: the async on-disk step must write the upgraded envelope through
    // the atomic writer (temp file + rename) and leave a valid V9 file, matching
    // every sibling step and the sync twin. Round-trips the on-disk write path.
    #[tokio::test]
    async fn test_migrate_v8_to_v9_writes_valid_v9_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("v8.json");
        let env = json!({ "version": 8, "data": { "boards": [] } });
        tokio::fs::write(&path, serde_json::to_string_pretty(&env).unwrap())
            .await
            .unwrap();

        migrate_v8_to_v9(&path).await.unwrap();

        let after: Value =
            serde_json::from_str(&tokio::fs::read_to_string(&path).await.unwrap()).unwrap();
        assert_eq!(after["version"], 9);
        assert_eq!(after["data"]["archived_boards"], json!([]));
    }
}
