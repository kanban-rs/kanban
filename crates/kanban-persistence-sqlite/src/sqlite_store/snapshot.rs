use sqlx::Row;

use kanban_domain::{Archived, ArchivedBoard, KanbanResult, Snapshot};

use super::helpers::{db_err, fmt_dt, p_dt, p_uuid};
use super::SqliteStore;

impl SqliteStore {
    pub(crate) async fn list_archived_boards_async(&self) -> KanbanResult<Vec<ArchivedBoard>> {
        // Reference-marker model: markers only. The board heads live in `boards`.
        let rows = sqlx::query(
            "SELECT board_id, archived_at FROM board_archival ORDER BY archived_at ASC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(db_err)?;
        let mut out = Vec::with_capacity(rows.len());
        for row in &rows {
            let id_str: String = row.try_get("board_id").map_err(db_err)?;
            let at: String = row.try_get("archived_at").map_err(db_err)?;
            out.push(Archived::at(p_uuid(&id_str)?, p_dt(&at)?));
        }
        Ok(out)
    }

    pub(crate) async fn snapshot_async(&self) -> KanbanResult<Snapshot> {
        // Reference-marker model: carry ALL board heads (live + archived) so an
        // archived board's row survives the round-trip; archived-ness rides on the
        // separate `archived_boards` markers.
        let boards = self.all_boards_async().await?;
        let columns = self.list_all_columns_async().await?;
        let cards = self.fetch_cards_with_filter("", &[]).await?;
        let archived_cards = self.list_archived_cards_async().await?;
        let sprints = self.list_all_sprints_async().await?;
        let graph = self.get_graph_async().await?;
        // C3b/KAN-860 fidelity: carry archived boards (their subtree is already
        // in the flat columns/cards/sprints reads above, which are unfiltered).
        let archived_boards = self.list_archived_boards_async().await?;
        let mut snap = Snapshot::from_data(boards, columns, cards, archived_cards, sprints, graph);
        snap.archived_boards = archived_boards;
        Ok(snap)
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
        sqlx::query("DELETE FROM board_archival")
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
        // Reference-marker model: the board head is already written from
        // `snapshot.boards` above (which now carries archived heads too). Here we
        // only record the archival marker.
        for ab in &snapshot.archived_boards {
            sqlx::query(
                "INSERT INTO board_archival (board_id, archived_at) VALUES (?, ?)
                 ON CONFLICT(board_id) DO UPDATE SET archived_at = excluded.archived_at",
            )
            .bind(ab.entity_id.to_string())
            .bind(fmt_dt(&ab.metadata.archived_at))
            .execute(&mut *tx)
            .await
            .map_err(db_err)?;
        }
        Self::write_graph_with_conn(&mut tx, &snapshot.graph).await?;

        tx.commit().await.map_err(db_err)?;
        Ok(())
    }
}
