use kanban_domain::{DependencyGraph, KanbanResult};
use sqlx::Row;

use super::conversions::row_to_edge_base;
use super::helpers::{db_err, fmt_dt, opt_dt, p_enum, ser_enum};
use super::SqliteStore;

impl SqliteStore {
    pub(crate) async fn get_graph_with_conn(
        conn: &mut sqlx::SqliteConnection,
    ) -> KanbanResult<DependencyGraph> {
        use kanban_core::EdgeBase;
        use kanban_domain::{BlocksEdge, RelatesEdge, SpawnsEdge};

        let mut spawns: Vec<SpawnsEdge> = Vec::new();
        for row in
            sqlx::query("SELECT source_id, target_id, created_at, archived_at FROM spawns_edges")
                .fetch_all(&mut *conn)
                .await
                .map_err(db_err)?
        {
            spawns.push(SpawnsEdge {
                base: row_to_edge_base(&row)?,
            });
        }

        let mut blocks: Vec<BlocksEdge> = Vec::new();
        for row in sqlx::query(
            "SELECT source_id, target_id, severity, created_at, archived_at FROM blocks_edges",
        )
        .fetch_all(&mut *conn)
        .await
        .map_err(db_err)?
        {
            let severity_str: String = row.try_get("severity").map_err(db_err)?;
            blocks.push(BlocksEdge {
                base: row_to_edge_base(&row)?,
                severity: p_enum(&severity_str, "severity")?,
            });
        }

        let mut relates: Vec<RelatesEdge> = Vec::new();
        for row in sqlx::query(
            "SELECT source_id, target_id, kind, created_at, archived_at FROM relates_edges",
        )
        .fetch_all(&mut *conn)
        .await
        .map_err(db_err)?
        {
            let kind_str: String = row.try_get("kind").map_err(db_err)?;
            relates.push(RelatesEdge {
                base: row_to_edge_base(&row)?,
                kind: p_enum(&kind_str, "relates kind")?,
            });
        }

        let _ = EdgeBase::<uuid::Uuid>::new; // keep import in scope for symmetry; suppress unused
        DependencyGraph::from_validated_per_kind_edges(spawns, blocks, relates)
    }

    pub(crate) async fn modify_graph_async(
        &self,
        f: kanban_domain::GraphMutFn,
    ) -> KanbanResult<()> {
        let mut tx = self.pool.begin().await.map_err(db_err)?;
        let mut graph = Self::get_graph_with_conn(&mut tx).await?;
        f(&mut graph)?;
        Self::write_graph_with_conn(&mut tx, &graph).await?;
        tx.commit().await.map_err(db_err)?;
        Ok(())
    }

    pub(crate) async fn write_graph_with_conn(
        conn: &mut sqlx::SqliteConnection,
        graph: &DependencyGraph,
    ) -> KanbanResult<()> {
        use kanban_core::Edge as _;

        sqlx::query("DELETE FROM spawns_edges")
            .execute(&mut *conn)
            .await
            .map_err(db_err)?;
        sqlx::query("DELETE FROM blocks_edges")
            .execute(&mut *conn)
            .await
            .map_err(db_err)?;
        sqlx::query("DELETE FROM relates_edges")
            .execute(&mut *conn)
            .await
            .map_err(db_err)?;

        for e in graph.spawns_edges() {
            sqlx::query(
                "INSERT INTO spawns_edges
                    (source_id, target_id, created_at, archived_at)
                 VALUES (?, ?, ?, ?)",
            )
            .bind(e.source().to_string())
            .bind(e.target().to_string())
            .bind(fmt_dt(&e.created_at()))
            .bind(opt_dt(&e.archived_at()))
            .execute(&mut *conn)
            .await
            .map_err(db_err)?;
        }
        for e in graph.blocks_edges() {
            sqlx::query(
                "INSERT INTO blocks_edges
                    (source_id, target_id, severity, created_at, archived_at)
                 VALUES (?, ?, ?, ?, ?)",
            )
            .bind(e.source().to_string())
            .bind(e.target().to_string())
            .bind(ser_enum(&e.severity, "severity")?)
            .bind(fmt_dt(&e.created_at()))
            .bind(opt_dt(&e.archived_at()))
            .execute(&mut *conn)
            .await
            .map_err(db_err)?;
        }
        for e in graph.relates_edges() {
            sqlx::query(
                "INSERT INTO relates_edges
                    (source_id, target_id, kind, created_at, archived_at)
                 VALUES (?, ?, ?, ?, ?)",
            )
            .bind(e.source().to_string())
            .bind(e.target().to_string())
            .bind(ser_enum(&e.kind, "relates kind")?)
            .bind(fmt_dt(&e.created_at()))
            .bind(opt_dt(&e.archived_at()))
            .execute(&mut *conn)
            .await
            .map_err(db_err)?;
        }

        Ok(())
    }

    pub(crate) async fn write_graph_async(&self, graph: &DependencyGraph) -> KanbanResult<()> {
        let mut tx = self.pool.begin().await.map_err(db_err)?;
        Self::write_graph_with_conn(&mut tx, graph).await?;
        tx.commit().await.map_err(db_err)?;
        Ok(())
    }
}
