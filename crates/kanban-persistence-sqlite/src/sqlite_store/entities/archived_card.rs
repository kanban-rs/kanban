use kanban_domain::{ArchivedCard, KanbanResult};

use crate::sqlite_store::helpers::{db_err, fmt_dt};
use crate::sqlite_store::SqliteStore;

impl SqliteStore {
    pub(crate) async fn write_archived_card_with_conn(
        conn: &mut sqlx::SqliteConnection,
        ac: &ArchivedCard,
    ) -> KanbanResult<()> {
        Self::write_card_with_conn(conn, &ac.card).await?;
        sqlx::query(
            "INSERT INTO archived_cards (card_id, archived_at, original_column_id, original_position)
             VALUES (?, ?, ?, ?)
             ON CONFLICT(card_id) DO UPDATE SET
                archived_at=excluded.archived_at,
                original_column_id=excluded.original_column_id,
                original_position=excluded.original_position",
        )
        .bind(ac.card.id.to_string())
        .bind(fmt_dt(&ac.archived_at))
        .bind(ac.original_column_id.to_string())
        .bind(ac.original_position)
        .execute(&mut *conn)
        .await
        .map_err(db_err)?;
        Ok(())
    }

    pub(crate) async fn write_archived_card_async(&self, ac: &ArchivedCard) -> KanbanResult<()> {
        let mut tx = self.pool.begin().await.map_err(db_err)?;
        Self::write_archived_card_with_conn(&mut tx, ac).await?;
        tx.commit().await.map_err(db_err)?;
        Ok(())
    }
}
