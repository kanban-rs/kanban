mod entities;
mod graph;
mod init;
mod metadata;
mod persistence_store;

pub(crate) fn make_rt() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
}
