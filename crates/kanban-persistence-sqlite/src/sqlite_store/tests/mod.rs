mod archived_cards;
mod board_archival;
mod boards;
mod cards;
mod columns;
mod entities;
mod filtered_reads;
mod graph;
mod init;
mod metadata;
mod migration_coverage;
mod migration_v2_to_v3;
mod migration_v4_to_v5;
mod persistence_store;
mod pre_migration_backup;

pub(crate) fn make_rt() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
}
