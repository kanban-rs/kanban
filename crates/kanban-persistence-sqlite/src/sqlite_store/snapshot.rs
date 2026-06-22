use kanban_domain::{KanbanResult, Snapshot};

use super::helpers::db_err;
use super::SqliteStore;

impl SqliteStore {
    pub(crate) async fn snapshot_async(&self) -> KanbanResult<Snapshot> {
        let boards = self.list_boards_async().await?;
        let columns = self.list_all_columns_async().await?;
        let cards = self.fetch_cards_with_filter("", &[]).await?;
        let archived_cards = self.list_archived_cards_async().await?;
        let sprints = self.list_all_sprints_async().await?;
        let graph = self.get_graph_async().await?;
        Ok(Snapshot::from_data(
            boards,
            columns,
            cards,
            archived_cards,
            sprints,
            graph,
        ))
    }

    pub(crate) async fn apply_snapshot_async(&self, snapshot: Snapshot) -> KanbanResult<()> {
        let mut tx = self.pool.begin().await.map_err(db_err)?;

        sqlx::query("PRAGMA defer_foreign_keys = ON")
            .execute(&mut *tx)
            .await
            .map_err(db_err)?;

        sqlx::query("DELETE FROM spawns_edges")
            .execute(&mut *tx)
            .await
            .map_err(db_err)?;
        sqlx::query("DELETE FROM blocks_edges")
            .execute(&mut *tx)
            .await
            .map_err(db_err)?;
        sqlx::query("DELETE FROM relates_edges")
            .execute(&mut *tx)
            .await
            .map_err(db_err)?;
        sqlx::query("DELETE FROM archived_cards")
            .execute(&mut *tx)
            .await
            .map_err(db_err)?;
        sqlx::query("DELETE FROM sprint_logs")
            .execute(&mut *tx)
            .await
            .map_err(db_err)?;
        sqlx::query("DELETE FROM cards")
            .execute(&mut *tx)
            .await
            .map_err(db_err)?;
        sqlx::query("DELETE FROM sprints")
            .execute(&mut *tx)
            .await
            .map_err(db_err)?;
        sqlx::query("DELETE FROM board_sprint_names")
            .execute(&mut *tx)
            .await
            .map_err(db_err)?;
        sqlx::query("DELETE FROM board_sprint_counters")
            .execute(&mut *tx)
            .await
            .map_err(db_err)?;
        sqlx::query("DELETE FROM columns")
            .execute(&mut *tx)
            .await
            .map_err(db_err)?;
        sqlx::query("DELETE FROM boards")
            .execute(&mut *tx)
            .await
            .map_err(db_err)?;

        for board in &snapshot.boards {
            Self::write_board_with_conn(&mut tx, board).await?;
        }
        for column in &snapshot.columns {
            Self::write_column_with_conn(&mut tx, column).await?;
        }
        for sprint in &snapshot.sprints {
            Self::write_sprint_with_conn(&mut tx, sprint).await?;
        }
        for card in &snapshot.cards {
            Self::write_card_with_conn(&mut tx, card).await?;
        }
        for ac in &snapshot.archived_cards {
            Self::write_archived_card_with_conn(&mut tx, ac).await?;
        }
        Self::write_graph_with_conn(&mut tx, &snapshot.graph).await?;

        tx.commit().await.map_err(db_err)?;
        Ok(())
    }
}
