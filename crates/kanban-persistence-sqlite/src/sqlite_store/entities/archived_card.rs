use kanban_domain::{ArchivedCard, KanbanResult};

use crate::sqlite_store::helpers::{db_err, fmt_dt};
use crate::sqlite_store::SqliteStore;

impl SqliteStore {
    pub(crate) async fn write_archived_card_with_conn(
        conn: &mut sqlx::SqliteConnection,
        ac: &ArchivedCard,
    ) -> KanbanResult<()> {
        // Reference-marker model: the card itself is NOT written here — it is a
        // live row in `cards` (the archive command upserts it). We only record the
        // marker. `original_column_id`/`original_position` are legacy NOT NULL
        // columns no longer carried by the domain marker; write inert placeholders
        // (they are never read back — reads reconstruct from the live card).
        sqlx::query(
            "INSERT INTO archived_cards (card_id, board_id, archived_at, original_column_id, original_position)
             VALUES (?, ?, ?, ?, 0)
             ON CONFLICT(card_id) DO UPDATE SET
                board_id=excluded.board_id,
                archived_at=excluded.archived_at",
        )
        .bind(ac.entity_id.to_string())
        .bind(ac.context.board_id.to_string())
        .bind(fmt_dt(&ac.metadata.archived_at))
        .bind(uuid::Uuid::nil().to_string())
        .execute(&mut *conn)
        .await
        .map_err(db_err)?;
        Ok(())
    }

    pub(crate) async fn write_archived_card_async(&self, ac: &ArchivedCard) -> KanbanResult<()> {
        let ac = *ac;
        self.db_conn(|conn| {
            Box::pin(async move { Self::write_archived_card_with_conn(conn, &ac).await })
        })
        .await
    }
}
