use chrono::{DateTime, Utc};
use kanban_domain::Snapshot;
use kanban_persistence::{
    PersistenceError, PersistenceMetadata, PersistenceResult, PersistenceStore, StoreSnapshot,
};
use uuid::Uuid;

use super::{SqliteStore, SUPPORTED_SCHEMA_VERSION};

#[async_trait::async_trait]
impl PersistenceStore for SqliteStore {
    async fn save(&self, snapshot: StoreSnapshot) -> PersistenceResult<PersistenceMetadata> {
        let domain_snapshot: Snapshot = serde_json::from_slice(&snapshot.data)
            .map_err(|e| PersistenceError::Serialization(e.to_string()))?;
        self.apply_snapshot_async(domain_snapshot)
            .await
            .map_err(|e| PersistenceError::Database(e.to_string()))?;

        let saved_at = self
            .stamp_writer()
            .await
            .map_err(|e| PersistenceError::Database(e.to_string()))?;
        self.checkpoint()
            .await
            .map_err(|e| PersistenceError::Database(e.to_string()))?;

        // Values are all knowable inline — stamp_writer just wrote them and
        // returned the timestamp, the writer/commit are compile-time consts.
        // format_version is SUPPORTED because migrate() on open() normalised
        // schema_version to SUPPORTED and nothing in the save path touches
        // it. The read paths (load, read_metadata_sync) re-read from the row
        // to honour the "DB is the source of truth" contract.
        Ok(PersistenceMetadata {
            instance_id: self.instance_id,
            saved_at,
            writer_version: Some(kanban_core::KANBAN_VERSION.to_string()),
            writer_commit: Some(kanban_core::KANBAN_COMMIT.to_string()),
            format_version: Some(SUPPORTED_SCHEMA_VERSION),
        })
    }

    async fn load(&self) -> PersistenceResult<(StoreSnapshot, PersistenceMetadata)> {
        let domain_snapshot = self
            .snapshot_async()
            .await
            .map_err(|e| PersistenceError::Database(e.to_string()))?;
        let data = serde_json::to_vec(&domain_snapshot)
            .map_err(|e| PersistenceError::Serialization(e.to_string()))?;

        let row: (String, Option<String>, Option<String>, u32) = sqlx::query_as(
            "SELECT saved_at, writer_version, writer_commit, schema_version \
             FROM metadata WHERE id = 1",
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| PersistenceError::Database(e.to_string()))?;
        let saved_at = DateTime::parse_from_rfc3339(&row.0)
            .map_err(|e| PersistenceError::Serialization(e.to_string()))?
            .with_timezone(&Utc);
        let meta = PersistenceMetadata {
            instance_id: self.instance_id,
            saved_at,
            writer_version: row.1,
            writer_commit: row.2,
            format_version: Some(row.3),
        };
        Ok((
            StoreSnapshot {
                data,
                metadata: meta.clone(),
            },
            meta,
        ))
    }

    async fn exists(&self) -> bool {
        self.path.exists()
    }

    fn path(&self) -> &std::path::Path {
        &self.path
    }

    fn instance_id(&self) -> Uuid {
        self.instance_id
    }

    async fn close(&self) {
        self.pool.close().await;
    }
}
