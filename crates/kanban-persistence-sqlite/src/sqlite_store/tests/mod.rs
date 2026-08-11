mod archived_cards;
mod board_archival;
mod boards;
mod cards;
mod columns;
mod command_log;
mod completion_columns;
mod entities;
mod filtered_reads;
mod graph;
mod init;
mod metadata;
mod migration_coverage;
mod migration_v2_to_v3;
mod migration_v4_to_v5;
mod migration_v5_to_v6;
mod persistence_store;
mod pre_migration_backup;
mod snapshot_atomicity;
mod transaction;

/// Ordered `board_completion_columns.column_id`s for a board, straight from
/// the join table. Shared by the migration and round-trip test modules.
pub(crate) async fn completion_rows(
    store: &super::SqliteStore,
    board_id: uuid::Uuid,
) -> Vec<String> {
    sqlx::query_scalar(
        "SELECT column_id FROM board_completion_columns WHERE board_id = ? ORDER BY position",
    )
    .bind(board_id.to_string())
    .fetch_all(store.pool())
    .await
    .unwrap()
}

pub(crate) fn make_rt() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
}
