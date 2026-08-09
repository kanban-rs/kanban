use std::future::Future;
use std::pin::Pin;

use sqlx::Sqlite;

use super::helpers::db_err;
use super::SqliteStore;
use kanban_domain::{KanbanError, KanbanResult};

type ConnFuture<'c, T> = Pin<Box<dyn Future<Output = KanbanResult<T>> + Send + 'c>>;
type ConnFutureLocal<'c, T> = Pin<Box<dyn Future<Output = KanbanResult<T>> + 'c>>;

impl SqliteStore {
    /// Runs `f` against the ambient transaction if one is open (via
    /// `begin_ambient_transaction`), otherwise against a fresh,
    /// locally-scoped transaction that commits on `Ok` and rolls back on
    /// `Err` — preserving today's per-call atomicity when no ambient
    /// transaction is present.
    pub(crate) async fn db_conn<F, T>(&self, f: F) -> KanbanResult<T>
    where
        F: for<'c> FnOnce(&'c mut sqlx::SqliteConnection) -> ConnFuture<'c, T>,
    {
        let mut guard = self.active_tx.lock().await;
        if let Some(tx) = guard.as_mut() {
            return f(tx).await;
        }
        drop(guard);

        let mut tx = self.pool.begin().await.map_err(db_err)?;
        match f(&mut tx).await {
            Ok(value) => {
                tx.commit().await.map_err(db_err)?;
                Ok(value)
            }
            Err(e) => {
                let _ = tx.rollback().await;
                Err(e)
            }
        }
    }

    pub(crate) async fn begin_ambient_transaction(&self) -> KanbanResult<()> {
        let mut guard = self.active_tx.lock().await;
        if guard.is_some() {
            return Err(KanbanError::Internal(
                "SqliteStore ambient transaction is not re-entrant; begin_ambient_transaction \
                 called while one is already open"
                    .into(),
            ));
        }
        let tx: sqlx::Transaction<'static, Sqlite> = self.pool.begin().await.map_err(db_err)?;
        *guard = Some(tx);
        Ok(())
    }

    pub(crate) async fn finish_ambient_transaction(&self, commit: bool) -> KanbanResult<()> {
        let tx = self
            .active_tx
            .lock()
            .await
            .take()
            .expect("finish_ambient_transaction called without begin_ambient_transaction");
        if commit {
            tx.commit().await.map_err(db_err)
        } else {
            tx.rollback().await.map_err(db_err)
        }
    }

    pub(crate) fn begin_write_transaction(&self) -> KanbanResult<()> {
        super::helpers::run(self.begin_ambient_transaction())
    }
    pub(crate) fn commit_write_transaction(&self) -> KanbanResult<()> {
        super::helpers::run(self.finish_ambient_transaction(true))
    }
    pub(crate) fn rollback_write_transaction(&self) {
        if let Err(e) = super::helpers::run(self.finish_ambient_transaction(false)) {
            tracing::error!("SqliteStore: failed to roll back ambient transaction: {e}");
        }
    }

    /// Non-`Send` twin of `db_conn`, used only by `modify_graph_async`: its
    /// `f` closure closes over a `kanban_domain::GraphMutFn`
    /// (`Box<dyn FnOnce(&mut DependencyGraph) -> KanbanResult<()>>`, no
    /// `Send` bound), so the resulting future cannot satisfy `db_conn`'s
    /// `+ Send` requirement. Safe because `modify_graph_async`'s only
    /// caller, `SqliteStore::modify_graph`, drives it through `run`
    /// (a bare `block_on` with no `Send` bound on `F`) — never through
    /// `#[async_trait]` or any other `Send`-requiring path.
    pub(crate) async fn db_conn_local<F, T>(&self, f: F) -> KanbanResult<T>
    where
        F: for<'c> FnOnce(&'c mut sqlx::SqliteConnection) -> ConnFutureLocal<'c, T>,
    {
        let mut guard = self.active_tx.lock().await;
        if let Some(tx) = guard.as_mut() {
            return f(tx).await;
        }
        drop(guard);

        let mut tx = self.pool.begin().await.map_err(db_err)?;
        match f(&mut tx).await {
            Ok(value) => {
                tx.commit().await.map_err(db_err)?;
                Ok(value)
            }
            Err(e) => {
                let _ = tx.rollback().await;
                Err(e)
            }
        }
    }
}
