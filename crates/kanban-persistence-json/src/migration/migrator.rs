use crate::json_file_store::JsonEnvelope;
use kanban_persistence::{FormatVersion, PersistenceError, PersistenceResult};
use serde_json::Value;
use std::path::Path;

/// Orchestrates migrations between format versions
pub struct Migrator;

impl Migrator {
    /// Detect the format version from an already-parsed JSON value.
    /// Pure: no I/O. Returns `UnsupportedFutureVersion` for versions newer
    /// than this binary's max — silently coercing them would risk dropping
    /// fields the old reader does not understand.
    pub fn detect_version_from_value(value: &Value) -> PersistenceResult<FormatVersion> {
        // V2+ files have a "version" field at root level
        if let Some(version) = value.get("version").and_then(|v| v.as_u64()) {
            // Saturate-on-overflow rather than truncate: a `version` field
            // that exceeds u32::MAX is still a "future version" the binary
            // doesn't understand, not the wrapped-around small number a
            // naive `as u32` would yield.
            let v: u32 = u32::try_from(version).unwrap_or(u32::MAX);
            return FormatVersion::from_u32(v).ok_or(PersistenceError::UnsupportedFutureVersion {
                file_version: v,
                binary_max: FormatVersion::MAX.as_u32(),
            });
        }
        // V1 files have "boards" at root level but no version field
        if value.get("boards").is_some() {
            return Ok(FormatVersion::V1);
        }
        // Unknown shape with no version field, treat as V2
        Ok(FormatVersion::V2)
    }

    /// Detect the version of a persisted file
    pub async fn detect_version(path: &Path) -> PersistenceResult<FormatVersion> {
        if !path.exists() {
            return Ok(FormatVersion::MAX); // Default to current for new files
        }

        let content = tokio::fs::read_to_string(path).await?;
        let value: Value = serde_json::from_str(&content)
            .map_err(|e| PersistenceError::Serialization(e.to_string()))?;

        Self::detect_version_from_value(&value)
    }

    /// Migrate a file from one version to another, stepping through intermediate versions
    pub async fn migrate(
        from: FormatVersion,
        to: FormatVersion,
        path: &Path,
    ) -> PersistenceResult<()> {
        if from == to {
            return Ok(());
        }

        match (from, to) {
            (FormatVersion::V1, FormatVersion::V2) => Self::migrate_v1_to_v2(path).await,
            (FormatVersion::V1, FormatVersion::V3) => {
                Self::migrate_v1_to_v2(path).await?;
                super::v2_to_v3::migrate_v2_to_v3(path).await
            }
            (FormatVersion::V2, FormatVersion::V3) => super::v2_to_v3::migrate_v2_to_v3(path).await,
            (_, FormatVersion::V16) if from < FormatVersion::V16 => {
                // See `migration::backup` for the source-version → backup-path
                // policy shared with the sync orchestrator. A `.v{N}.backup`
                // is the user's escape hatch if the upgrade has to be rolled
                // back, since a migrated file cannot be opened by an older
                // binary. The backup is taken BEFORE any per-step migration runs
                // so it covers the entire chain (V1→V2, V2→V3, split_graph,
                // v6→v7, v7→v8, v8→v9, v9→v10, v10→v11, v11→v12, v12→v13,
                // v13→v14), not just the destructive tail.
                let backup_path = super::pre_latest_backup_path_for(from, path);
                if let Some(backup) = &backup_path {
                    tokio::fs::copy(path, backup).await?;
                    tracing::info!("Created pre-V14 backup at {}", backup.display());
                }

                let result: PersistenceResult<()> = async {
                    if from == FormatVersion::V1 {
                        Self::migrate_v1_to_v2(path).await?;
                    }
                    if from <= FormatVersion::V2 {
                        super::v2_to_v3::migrate_v2_to_v3(path).await?;
                    }
                    // V3, V4, and V5 all share the pre-split graph schema; the
                    // split-graph transform is the only step that distinguishes
                    // them. V6 files skip the split-graph step entirely; V7
                    // files skip straight to the v7→v8 archived-cards backfill.
                    Self::run_split_and_upgrade_chain(from, path).await
                }
                .await;

                match (result, backup_path) {
                    (Ok(()), Some(backup)) => {
                        if let Err(e) = tokio::fs::remove_file(&backup).await {
                            tracing::warn!(
                                "Migration successful but failed to remove backup at {}: {}",
                                backup.display(),
                                e
                            );
                        } else {
                            tracing::info!("Migration to V16 verified, backup removed");
                        }
                        Ok(())
                    }
                    (Ok(()), None) => Ok(()),
                    (Err(e), Some(backup)) => {
                        tracing::error!(
                            "Migration to V16 failed: {}. Backup preserved at {}",
                            e,
                            backup.display()
                        );
                        Err(e)
                    }
                    (Err(e), None) => Err(e),
                }
            }
            _ => Err(PersistenceError::Serialization(format!(
                "Unsupported migration: {:?} -> {:?}",
                from, to
            ))),
        }
    }

    /// Run the V6 split-graph transform (only if the file is pre-V6), the
    /// v6→v7 spawns-bucket rename, the v7→v8 archived-cards backfill, the
    /// v8→v9 archived-boards bump, the v9→v10 archival reference-marker
    /// collapse, the v10→v11 cards.board_id backfill, the v11→v12
    /// completion_column_ids backfill, the v12→v13 default_status backfill,
    /// then the v13→v14 default_status derivation. Every step is a no-op
    /// when it doesn't apply (each transform short-circuits on a file
    /// already at or beyond its target version), so this is safe to call
    /// for any `from < V14` and always leaves the file at V16.
    async fn run_split_and_upgrade_chain(
        from: FormatVersion,
        path: &Path,
    ) -> PersistenceResult<()> {
        if from < FormatVersion::V6 {
            super::split_graph::migrate_to_v6_split_graph(path).await?;
        }
        super::v6_to_v7_rename::migrate_v6_to_v7(path).await?;
        super::v7_to_v8_archived_cards::migrate_v7_to_v8(path).await?;
        super::v8_to_v9_archived_boards::migrate_v8_to_v9(path).await?;
        super::v9_to_v10_archival_refs::migrate_v9_to_v10(path).await?;
        super::v10_to_v11_card_board_id::migrate_v10_to_v11(path).await?;
        super::v11_to_v12_completion_columns::migrate_v11_to_v12(path).await?;
        super::v13_column_default_status::migrate_v12_to_v13(path).await?;
        super::v14_default_status_derivation::migrate_v13_to_v14(path).await?;
        super::v15_prefixes::migrate_v14_to_v15(path).await?;
        super::v16_card_prefix::migrate_v15_to_v16(path).await
    }

    /// Migrate from V1 format to V2 format. Per-step backup removed: the
    /// outer `Migrator::migrate` wrap owns the `.v1.backup` now and keeps
    /// it for the entire V1→V8 chain, not just this step.
    async fn migrate_v1_to_v2(path: &Path) -> PersistenceResult<()> {
        let content = tokio::fs::read_to_string(path).await?;
        let v1_data: Value = serde_json::from_str(&content)
            .map_err(|e| PersistenceError::Serialization(e.to_string()))?;

        let v2_envelope = JsonEnvelope::new(v1_data.clone());
        let json_str = v2_envelope
            .to_json_string()
            .map_err(|e| PersistenceError::Serialization(e.to_string()))?;
        crate::atomic_writer::AtomicWriter::write_atomic(path, json_str.as_bytes()).await?;

        tracing::info!("Migrated {} from V1 to V2 format", path.display());

        // Verify the transform produced a parseable V2 envelope with the
        // original data preserved. Verification failure now propagates as
        // a plain Err; the outer wrap is responsible for backup retention.
        Self::verify_migration(path, &v1_data).await
    }

    /// Verify that a migration was successful
    async fn verify_migration(path: &Path, original_data: &Value) -> PersistenceResult<()> {
        // Read the migrated file
        let migrated_content = tokio::fs::read_to_string(path).await?;
        let migrated_value: Value = serde_json::from_str(&migrated_content).map_err(|e| {
            PersistenceError::Serialization(format!("Failed to parse migrated file: {}", e))
        })?;

        // Verify V2 structure
        migrated_value
            .get("version")
            .and_then(|v| v.as_u64())
            .filter(|&v| v == 2)
            .ok_or_else(|| {
                PersistenceError::Serialization(
                    "Migrated file missing or invalid version field".to_string(),
                )
            })?;

        migrated_value
            .get("metadata")
            .filter(|m| m.is_object())
            .ok_or_else(|| {
                PersistenceError::Serialization(
                    "Migrated file missing or invalid metadata field".to_string(),
                )
            })?;

        // Verify data was preserved
        let migrated_data = migrated_value.get("data").ok_or_else(|| {
            PersistenceError::Serialization("Migrated file missing data field".to_string())
        })?;

        if migrated_data != original_data {
            return Err(PersistenceError::Serialization(
                "Migrated data does not match original data".to_string(),
            ));
        }

        tracing::debug!("Migration verification passed");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_detect_v1_format() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.json");

        let v1_data = json!({
            "boards": [],
            "columns": [],
            "cards": []
        });

        tokio::fs::write(&file_path, v1_data.to_string())
            .await
            .unwrap();

        let version = Migrator::detect_version(&file_path).await.unwrap();
        assert_eq!(version, FormatVersion::V1);
    }

    #[tokio::test]
    async fn test_detect_v2_format() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.json");

        let v2_data = json!({
            "version": 2,
            "metadata": {},
            "data": {}
        });

        tokio::fs::write(&file_path, v2_data.to_string())
            .await
            .unwrap();

        let version = Migrator::detect_version(&file_path).await.unwrap();
        assert_eq!(version, FormatVersion::V2);
    }

    #[tokio::test]
    async fn test_migrate_v1_to_v2() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.json");

        let v1_data = json!({
            "boards": [{ "id": "1", "name": "Test Board" }],
            "columns": [],
            "cards": []
        });

        tokio::fs::write(&file_path, v1_data.to_string())
            .await
            .unwrap();

        Migrator::migrate(FormatVersion::V1, FormatVersion::V2, &file_path)
            .await
            .unwrap();

        let migrated = tokio::fs::read_to_string(&file_path).await.unwrap();
        let v2_data: Value = serde_json::from_str(&migrated).unwrap();

        assert_eq!(v2_data["version"], 2);
        assert!(v2_data["metadata"].is_object());
        assert!(v2_data["data"]["boards"].is_array());

        assert!(
            !file_path.with_extension("v1.backup").exists(),
            "Backup should be removed after successful migration"
        );
    }

    // Inode identity is a POSIX-only observable: an atomic write replaces the
    // directory entry via rename (new inode), while an in-place overwrite
    // reuses the same inode. Windows has no equivalent notion, so this guard
    // against a crash-window regression is Unix-only.
    #[cfg(unix)]
    #[tokio::test]
    async fn test_migrate_v1_to_v2_writes_via_atomic_rename_not_in_place() {
        use std::os::unix::fs::MetadataExt;

        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.json");
        let v1_data = json!({ "boards": [], "columns": [], "cards": [] });
        tokio::fs::write(&file_path, v1_data.to_string())
            .await
            .unwrap();

        let inode_before = tokio::fs::metadata(&file_path).await.unwrap().ino();
        Migrator::migrate_v1_to_v2(&file_path).await.unwrap();
        let inode_after = tokio::fs::metadata(&file_path).await.unwrap().ino();

        assert_ne!(
            inode_before, inode_after,
            "migrate_v1_to_v2 must write via a temp file + atomic rename, not an in-place overwrite"
        );
    }

    #[tokio::test]
    async fn test_migrate_v7_to_v7_is_a_noop_byte_for_byte() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("v7.json");
        let v7 = json!({
            "version": 7,
            "metadata": {
                "instance_id": "550e8400-e29b-41d4-a716-446655440000",
                "saved_at": "2024-01-01T00:00:00Z"
            },
            "data": {
                "boards": [], "columns": [], "cards": [], "archived_cards": [], "sprints": [],
                "graph": {
                    "spawns": { "edges": [] },
                    "blocks": { "edges": [] },
                    "relates": { "edges": [] }
                }
            }
        });
        let original = serde_json::to_string_pretty(&v7).unwrap();
        tokio::fs::write(&path, &original).await.unwrap();

        Migrator::migrate(FormatVersion::V7, FormatVersion::V7, &path)
            .await
            .unwrap();

        let after = tokio::fs::read_to_string(&path).await.unwrap();
        assert_eq!(
            after, original,
            "V7 -> V7 migration must be a byte-for-byte noop"
        );
    }

    #[tokio::test]
    async fn test_migrate_v4_to_v7_skips_legacy_steps() {
        // A V4 file goes straight through split_graph then the v6→v7
        // spawns-rename: no v1→v2 (no .v1.backup file should appear)
        // and no v2→v3 (the pre-V3 'prefix_counters' shaping doesn't
        // apply because V4 already shipped past that point).
        let dir = tempdir().unwrap();
        let path = dir.path().join("v4.json");
        let v4 = json!({
            "version": 4,
            "metadata": {
                "instance_id": "550e8400-e29b-41d4-a716-446655440000",
                "saved_at": "2024-01-01T00:00:00Z"
            },
            "data": {
                "boards": [],
                "columns": [],
                "cards": [],
                "archived_cards": [],
                "sprints": [],
                "graph": { "cards": { "edges": [] } }
            }
        });
        tokio::fs::write(&path, serde_json::to_string_pretty(&v4).unwrap())
            .await
            .unwrap();

        Migrator::migrate(FormatVersion::V4, FormatVersion::MAX, &path)
            .await
            .unwrap();

        let after: Value =
            serde_json::from_str(&tokio::fs::read_to_string(&path).await.unwrap()).unwrap();
        assert_eq!(after["version"], 16);
        assert!(after["data"]["graph"]["spawns"].is_object());
        assert!(
            after["data"]["graph"]
                .as_object()
                .unwrap()
                .get("parent_child")
                .is_none(),
            "post-v7 graph must not retain the legacy parent_child key"
        );
        assert!(
            !path.with_extension("v1.backup").exists(),
            "V4 -> V7 must not trigger the v1→v2 step that creates .v1.backup"
        );
    }

    #[tokio::test]
    async fn test_migrate_v5_to_v7_skips_legacy_steps() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("v5.json");
        let v5 = json!({
            "version": 5,
            "metadata": {
                "instance_id": "550e8400-e29b-41d4-a716-446655440000",
                "saved_at": "2024-01-01T00:00:00Z"
            },
            "data": {
                "boards": [],
                "columns": [],
                "cards": [],
                "archived_cards": [],
                "sprints": [],
                "graph": { "cards": { "edges": [] } }
            }
        });
        tokio::fs::write(&path, serde_json::to_string_pretty(&v5).unwrap())
            .await
            .unwrap();

        Migrator::migrate(FormatVersion::V5, FormatVersion::MAX, &path)
            .await
            .unwrap();

        let after: Value =
            serde_json::from_str(&tokio::fs::read_to_string(&path).await.unwrap()).unwrap();
        assert_eq!(after["version"], 16);
        assert!(after["data"]["graph"]["spawns"].is_object());
        assert!(after["data"]["graph"]["relates"].is_object());
        assert!(
            !path.with_extension("v1.backup").exists(),
            "V5 -> V7 must not trigger the v1→v2 step that creates .v1.backup"
        );
    }

    #[tokio::test]
    async fn test_migrate_v6_to_v7_renames_parent_child_and_writes_backup() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("v6.json");
        let v6 = json!({
            "version": 6,
            "metadata": {
                "instance_id": "550e8400-e29b-41d4-a716-446655440000",
                "saved_at": "2024-01-01T00:00:00Z"
            },
            "data": {
                "boards": [], "columns": [], "cards": [], "archived_cards": [], "sprints": [],
                "graph": {
                    "parent_child": { "edges": [{
                        "source": "11111111-1111-1111-1111-111111111111",
                        "target": "22222222-2222-2222-2222-222222222222",
                        "created_at": "2024-01-01T00:00:00Z",
                        "archived_at": null
                    }]},
                    "blocks": { "edges": [] },
                    "relates": { "edges": [] }
                }
            }
        });
        tokio::fs::write(&path, serde_json::to_string_pretty(&v6).unwrap())
            .await
            .unwrap();

        Migrator::migrate(FormatVersion::V6, FormatVersion::MAX, &path)
            .await
            .unwrap();

        let after: Value =
            serde_json::from_str(&tokio::fs::read_to_string(&path).await.unwrap()).unwrap();
        assert_eq!(after["version"], 16);
        assert!(after["data"]["graph"]["spawns"].is_object());
        assert!(after["data"]["graph"]
            .as_object()
            .unwrap()
            .get("parent_child")
            .is_none());
        assert_eq!(
            after["data"]["graph"]["spawns"]["edges"]
                .as_array()
                .unwrap()
                .len(),
            1,
            "the original parent_child edge must survive"
        );
        // Backup must be removed after a successful migration.
        assert!(
            !path.with_extension("v6.backup").exists(),
            "v6.backup should be removed after successful migration to V7"
        );
    }

    #[tokio::test]
    async fn test_migrate_v6_to_v7_preserves_v6_backup_on_failure() {
        // The orchestrator must hold onto the .v6.backup if the V6→V7
        // step fails: otherwise a corrupt V6 file becomes unrecoverable
        // after a failed upgrade. The both-keys ambiguity is the
        // canonical failure mode — the transform refuses and the user
        // needs the original file to investigate. Pinning this prevents
        // a future orchestrator refactor from silently dropping the
        // backup-preservation behaviour.
        let dir = tempdir().unwrap();
        let path = dir.path().join("v6_ambiguous.json");
        let v6 = json!({
            "version": 6,
            "metadata": {
                "instance_id": "550e8400-e29b-41d4-a716-446655440000",
                "saved_at": "2024-01-01T00:00:00Z"
            },
            "data": {
                "boards": [], "columns": [], "cards": [], "archived_cards": [], "sprints": [],
                "graph": {
                    "parent_child": { "edges": [{
                        "source": "11111111-1111-1111-1111-111111111111",
                        "target": "22222222-2222-2222-2222-222222222222",
                        "created_at": "2024-01-01T00:00:00Z",
                        "archived_at": null
                    }]},
                    "spawns": { "edges": [{
                        "source": "33333333-3333-3333-3333-333333333333",
                        "target": "44444444-4444-4444-4444-444444444444",
                        "created_at": "2024-01-01T00:00:00Z",
                        "archived_at": null
                    }]},
                    "blocks": { "edges": [] },
                    "relates": { "edges": [] }
                }
            }
        });
        tokio::fs::write(&path, serde_json::to_string_pretty(&v6).unwrap())
            .await
            .unwrap();

        let err = Migrator::migrate(FormatVersion::V6, FormatVersion::MAX, &path)
            .await
            .expect_err("migration must refuse a V6 envelope carrying both bucket keys");
        let msg = err.to_string();
        assert!(
            msg.contains("parent_child") && msg.contains("spawns"),
            "diagnostic should name both colliding keys; got: {msg}"
        );

        assert!(
            path.with_extension("v6.backup").exists(),
            ".v6.backup must be preserved when the V6→V7 step fails so the user can recover"
        );
    }

    #[tokio::test]
    async fn test_detect_version_from_value_rejects_future_version() {
        let v99 = json!({
            "version": 99,
            "metadata": {},
            "data": {}
        });
        let err = Migrator::detect_version_from_value(&v99).expect_err("v99 must be refused");
        assert!(
            matches!(
                err,
                PersistenceError::UnsupportedFutureVersion {
                    file_version: 99,
                    binary_max: 16
                }
            ),
            "expected UnsupportedFutureVersion, got: {err:?}"
        );
    }

    #[test]
    fn test_detect_version_from_value_rejects_u32_overflow_that_truncates_to_valid_version() {
        // 2^32 + 7 truncates to 7 under a naive `as u32` cast — which would
        // be silently accepted as V7. The guard must refuse it as a future
        // version instead.
        let truncates_to_v7 = json!({
            "version": (1u64 << 32) + 7,
            "metadata": {},
            "data": {}
        });
        let err = Migrator::detect_version_from_value(&truncates_to_v7)
            .expect_err("version > u32::MAX must be refused, not truncated to V7");
        assert!(
            matches!(
                err,
                PersistenceError::UnsupportedFutureVersion { binary_max: 16, .. }
            ),
            "expected UnsupportedFutureVersion, got: {err:?}"
        );
    }

    #[tokio::test]
    async fn test_detect_version_async_rejects_future_version() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("v99.json");
        let v99 = json!({
            "version": 99,
            "metadata": {},
            "data": {}
        });
        tokio::fs::write(&path, v99.to_string()).await.unwrap();

        let err = Migrator::detect_version(&path)
            .await
            .expect_err("v99 must be refused");
        assert!(
            matches!(
                err,
                PersistenceError::UnsupportedFutureVersion {
                    file_version: 99,
                    binary_max: 16
                }
            ),
            "expected UnsupportedFutureVersion, got: {err:?}"
        );
    }

    #[tokio::test]
    async fn test_migrate_v1_to_v3_via_chain() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("board.json");

        let v1_content = json!({
            "boards": [{
                "id": "550e8400-e29b-41d4-a716-446655440001",
                "name": "Test",
                "card_prefix": "KAN",
                "prefix_counters": { "KAN": 3 },
                "sprint_counters": {},
                "created_at": "2024-01-01T00:00:00Z",
                "updated_at": "2024-01-01T00:00:00Z"
            }],
            "columns": [{ "id": "550e8400-e29b-41d4-a716-446655440002",
                          "board_id": "550e8400-e29b-41d4-a716-446655440001" }],
            "cards": [{
                "id": "550e8400-e29b-41d4-a716-446655440003",
                "column_id": "550e8400-e29b-41d4-a716-446655440002",
                "card_number": 1,
                "assigned_prefix": "KAN",
                "card_prefix": "KAN"
            }],
            "archived_cards": [],
            "sprints": []
        });
        tokio::fs::write(&path, v1_content.to_string())
            .await
            .unwrap();

        Migrator::migrate(FormatVersion::V1, FormatVersion::V3, &path)
            .await
            .expect("V1→V3 migration must succeed");

        let result: serde_json::Value =
            serde_json::from_str(&tokio::fs::read_to_string(&path).await.unwrap()).unwrap();

        assert_eq!(result["version"], 3, "output version must be 3");
        assert!(result["metadata"].is_object());

        let board = &result["data"]["boards"][0];
        assert!(
            board.get("prefix_counters").is_none(),
            "prefix_counters must be stripped"
        );
        assert_eq!(
            board["card_counter"], 3,
            "card_counter derived from prefix_counters[KAN]"
        );

        let card = &result["data"]["cards"][0];
        assert!(
            card.get("assigned_prefix").is_none(),
            "assigned_prefix must be stripped"
        );
        assert!(
            card.get("card_prefix").is_none(),
            "card_prefix must be stripped"
        );
        assert_eq!(card["card_number"], 1);
    }

    /// KAN-660: V2 source migrating to V8 via Migrator::migrate is now
    /// covered by the outer backup wrap. .v2.backup is created before
    /// migrate_v2_to_v3 runs and cleaned up after the full chain succeeds.
    /// (No paired failure-preservation test: the V2 envelope shape can't
    /// cleanly inject the V6 both-keys ambiguity that drives the existing
    /// `test_migrate_v6_to_v7_preserves_v6_backup_on_failure`, and the
    /// outer-wrap failure-handling code path is shared.)
    #[tokio::test]
    async fn test_migrate_v2_to_v7_cleans_up_v2_backup_on_success() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("v2.json");
        let v2 = json!({
            "version": 2,
            "metadata": {
                "instance_id": "550e8400-e29b-41d4-a716-446655440000",
                "saved_at": "2024-01-01T00:00:00Z"
            },
            "data": {
                "boards": [], "columns": [], "cards": [], "archived_cards": [], "sprints": []
            }
        });
        tokio::fs::write(&path, serde_json::to_string_pretty(&v2).unwrap())
            .await
            .unwrap();

        Migrator::migrate(FormatVersion::V2, FormatVersion::MAX, &path)
            .await
            .expect("V2→V8 must succeed");

        let after: Value =
            serde_json::from_str(&tokio::fs::read_to_string(&path).await.unwrap()).unwrap();
        assert_eq!(after["version"], 16);

        assert!(
            !path.with_extension("v2.backup").exists(),
            ".v2.backup must be removed after successful V2→V8 migration"
        );
    }

    /// KAN-660: V1 source migrating to V8 via Migrator::migrate is now
    /// covered by the outer backup wrap. The .v1.backup persists across
    /// the entire V1→V8 chain rather than being removed after V1→V2.
    #[tokio::test]
    async fn test_migrate_v1_to_v7_cleans_up_v1_backup_on_success() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("v1.json");
        let v1 = json!({
            "boards": [],
            "columns": [],
            "cards": []
        });
        tokio::fs::write(&path, v1.to_string()).await.unwrap();

        Migrator::migrate(FormatVersion::V1, FormatVersion::MAX, &path)
            .await
            .expect("V1→V8 must succeed");

        let after: Value =
            serde_json::from_str(&tokio::fs::read_to_string(&path).await.unwrap()).unwrap();
        assert_eq!(after["version"], 16);

        assert!(
            !path.with_extension("v1.backup").exists(),
            ".v1.backup must be removed after successful V1→V8 migration"
        );
    }

    #[tokio::test]
    async fn test_migrate_v7_to_v8_backfills_and_writes_backup() {
        // The new V7→V8 step: backfill board_id from the column graph, take a
        // .v7.backup before the destructive write, and clean it up on success.
        let dir = tempdir().unwrap();
        let path = dir.path().join("v7.json");
        let board = "11111111-1111-1111-1111-111111111111";
        let column = "22222222-2222-2222-2222-222222222222";
        let v7 = json!({
            "version": 7,
            "metadata": {
                "instance_id": "550e8400-e29b-41d4-a716-446655440000",
                "saved_at": "2024-01-01T00:00:00Z"
            },
            "data": {
                "boards": [{ "id": board, "name": "B" }],
                "columns": [{ "id": column, "board_id": board }],
                "cards": [],
                "archived_cards": [{
                    "card": { "id": "33333333-3333-3333-3333-333333333333", "title": "T" },
                    "archived_at": "2024-01-01T00:00:00Z",
                    "original_column_id": column,
                    "original_position": 0
                }],
                "sprints": [],
                "graph": {
                    "spawns": { "edges": [] },
                    "blocks": { "edges": [] },
                    "relates": { "edges": [] }
                }
            }
        });
        tokio::fs::write(&path, serde_json::to_string_pretty(&v7).unwrap())
            .await
            .unwrap();

        Migrator::migrate(FormatVersion::V7, FormatVersion::MAX, &path)
            .await
            .expect("V7→V8 must succeed");

        let after: Value =
            serde_json::from_str(&tokio::fs::read_to_string(&path).await.unwrap()).unwrap();
        assert_eq!(after["version"], 16);
        assert_eq!(
            after["data"]["archived_cards"][0]["board_id"]
                .as_str()
                .unwrap(),
            board,
            "board_id must be backfilled from original_column_id"
        );
        assert!(
            !path.with_extension("v7.backup").exists(),
            ".v7.backup must be removed after successful V7→V8 migration"
        );
    }
}
