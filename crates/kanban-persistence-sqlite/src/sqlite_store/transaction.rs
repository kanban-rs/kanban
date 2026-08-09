use std::future::Future;
use std::pin::Pin;

use super::SqliteStore;
use kanban_domain::KanbanResult;

type ConnFuture<'c, T> = Pin<Box<dyn Future<Output = KanbanResult<T>> + Send + 'c>>;
type ConnFutureLocal<'c, T> = Pin<Box<dyn Future<Output = KanbanResult<T>> + 'c>>;

impl SqliteStore {
    /// Runs `f` against the ambient transaction if one is open (via
    /// `begin_ambient_transaction`), otherwise against a fresh,
    /// locally-scoped transaction that commits on `Ok` and rolls back on
    /// `Err`.
    pub(crate) async fn db_conn<F, T>(&self, f: F) -> KanbanResult<T>
    where
        F: for<'c> FnOnce(&'c mut sqlx::SqliteConnection) -> ConnFuture<'c, T>,
    {
        let _ = f;
        todo!("KAN-1067 Green step")
    }

    pub(crate) async fn begin_ambient_transaction(&self) -> KanbanResult<()> {
        todo!("KAN-1067 Green step")
    }

    pub(crate) async fn finish_ambient_transaction(&self, commit: bool) -> KanbanResult<()> {
        let _ = commit;
        todo!("KAN-1067 Green step")
    }

    pub(crate) fn begin_write_transaction(&self) -> KanbanResult<()> {
        todo!("KAN-1067 Green step")
    }
    pub(crate) fn commit_write_transaction(&self) -> KanbanResult<()> {
        todo!("KAN-1067 Green step")
    }
    pub(crate) fn rollback_write_transaction(&self) {
        todo!("KAN-1067 Green step")
    }

    /// Non-`Send` twin of `db_conn`, used only by `modify_graph_async`.
    pub(crate) async fn db_conn_local<F, T>(&self, f: F) -> KanbanResult<T>
    where
        F: for<'c> FnOnce(&'c mut sqlx::SqliteConnection) -> ConnFutureLocal<'c, T>,
    {
        let _ = f;
        todo!("KAN-1067 Green step")
    }
}
