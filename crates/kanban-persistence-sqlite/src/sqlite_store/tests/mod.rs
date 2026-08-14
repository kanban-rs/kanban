mod archived_cards;
mod board_archival;
mod boards;
mod cards;
mod columns;
mod command_log;
mod entities;
mod filtered_reads;
mod graph;
mod init;
mod metadata;
mod migration_coverage;
mod migration_v2_to_v3;
mod migration_v4_to_v5;
mod migration_v5_to_v6;
mod migration_v6_to_v7;
mod migration_v7_to_v8;
mod migration_v8_to_v9;
mod persistence_store;
mod pre_migration_backup;
mod snapshot_atomicity;
mod transaction;

/// Ordered `board_completion_columns.column_id`s for a board, straight from
/// the join table. Takes a raw pool rather than a `SqliteStore` because by
/// the time `SqliteStore::open` returns, the full migrate chain (through
/// `migrate_v8_to_v9_drop_completion_columns`) has already dropped the
/// table; callers that need to observe it mid-chain migrate a pool directly.
pub(crate) async fn completion_rows(
    pool: &sqlx::Pool<sqlx::Sqlite>,
    board_id: uuid::Uuid,
) -> Vec<String> {
    sqlx::query_scalar(
        "SELECT column_id FROM board_completion_columns WHERE board_id = ? ORDER BY position",
    )
    .bind(board_id.to_string())
    .fetch_all(pool)
    .await
    .unwrap()
}

pub(crate) fn make_rt() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
}
