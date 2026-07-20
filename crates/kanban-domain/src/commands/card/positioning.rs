use super::super::{Command, CommandContext};
use super::CardCommand;
use crate::data_store::DataStore;
use crate::{KanbanError, KanbanResult};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Move card to a different column
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MoveCard {
    pub card_id: Uuid,
    pub new_column_id: Uuid,
    pub new_position: i32,
}

impl MoveCard {
    pub fn execute(&self, context: &CommandContext) -> KanbanResult<()> {
        // Canonical target-column membership check (KAN-248): reject a move to a
        // non-existent column up front via the shared helper.
        context.require_column(self.new_column_id)?;
        context.check_wip_limit(self.new_column_id, 1, &[self.card_id])?;
        let mut card = context.get_card(self.card_id)?;
        card.move_to_column(self.new_column_id, self.new_position);
        context.store.upsert_card(card)?;
        Ok(())
    }

    pub fn description(&self) -> String {
        format!(
            "Move card {} to column {}",
            self.card_id, self.new_column_id
        )
    }

    /// Inverse: another MoveCard pointing back to the card's current
    /// (column_id, position).
    pub fn capture_inverse(&self, store: &dyn DataStore) -> KanbanResult<Vec<Command>> {
        let card = match store.get_card(self.card_id)? {
            Some(c) => c,
            None => return Err(KanbanError::not_found("Card", self.card_id)),
        };
        Ok(vec![Command::Card(CardCommand::Move(MoveCard {
            card_id: self.card_id,
            new_column_id: card.column_id,
            new_position: card.position,
        }))])
    }
}

/// Compact card positions in a column to be sequential (0, 1, 2, ...).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompactColumnPositions {
    pub column_id: Uuid,
}

impl CompactColumnPositions {
    pub fn execute(&self, context: &CommandContext) -> KanbanResult<()> {
        // KAN-936 (ARCH-DECOR C2): compact over the Include set (live +
        // archived). Archived cards stay live in `cards` behind a marker and
        // keep their coherently-placed ordinal (C1); renumbering the live-only
        // set would re-collide a live card onto an archived ordinal.
        let cards = context
            .store
            .list_cards_by_column_filtered(self.column_id, crate::ArchivedFilter::Include)?;
        for (i, mut card) in cards.into_iter().enumerate() {
            if card.position != i as i32 {
                card.position = i as i32;
                context.store.upsert_card(card)?;
            }
        }
        Ok(())
    }

    pub fn description(&self) -> String {
        format!("Compact positions in column {}", self.column_id)
    }

    /// Inverse: for each card in the column, emit a MoveCard back to its
    /// original position. Compaction is lossy without pre-state capture
    /// (multiple gappy arrangements compact to the same result), so this
    /// is the only way to reverse it. Captured over the Include set (KAN-936)
    /// to match `execute`, which now renumbers archived cards too.
    pub fn capture_inverse(&self, store: &dyn DataStore) -> KanbanResult<Vec<Command>> {
        let cards =
            store.list_cards_by_column_filtered(self.column_id, crate::ArchivedFilter::Include)?;
        let mut commands: Vec<Command> = Vec::new();
        for card in cards {
            commands.push(Command::Card(CardCommand::Move(MoveCard {
                card_id: card.id,
                new_column_id: card.column_id,
                new_position: card.position,
            })));
        }
        Ok(commands)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::test_helpers::TestContext;

    #[test]
    fn test_move_card_not_found_returns_error() {
        let tc = TestContext::new();
        let column = crate::Column::new(Uuid::new_v4(), "Col", 0);
        let column_id = column.id;
        tc.store.upsert_column(column).unwrap();
        let context = tc.as_command_context();
        let cmd = MoveCard {
            card_id: Uuid::new_v4(),
            new_column_id: column_id,
            new_position: 0,
        };
        let result = cmd.execute(&context);
        assert!(result.unwrap_err().is_not_found());
    }

    #[test]
    fn test_move_card_column_not_found_returns_error() {
        let tc = TestContext::new();
        let mut board = crate::Board::new("Test", Some("TST"));
        let card = crate::Card::new(&mut board, Uuid::new_v4(), "Card", 0);
        let card_id = card.id;
        tc.store.upsert_card(card).unwrap();
        let context = tc.as_command_context();
        let cmd = MoveCard {
            card_id,
            new_column_id: Uuid::new_v4(),
            new_position: 0,
        };
        let result = cmd.execute(&context);
        assert!(result.unwrap_err().is_not_found());
    }

    #[test]
    fn test_move_card_exceeding_wip_limit_returns_error() {
        let tc = TestContext::new();
        let mut board = crate::Board::new("Test", Some("TST"));
        let src_col = crate::Column::new(board.id, "Source", 0);
        let mut dst_col = crate::Column::new(board.id, "Dest", 1);
        dst_col.wip_limit = Some(1);
        let dst_id = dst_col.id;
        let existing = crate::Card::new(&mut board, dst_id, "Existing", 0);
        let mover = crate::Card::new(&mut board, src_col.id, "Mover", 0);
        let mover_id = mover.id;
        tc.store.upsert_board(board).unwrap();
        tc.store.upsert_column(src_col).unwrap();
        tc.store.upsert_column(dst_col).unwrap();
        tc.store.upsert_card(existing).unwrap();
        tc.store.upsert_card(mover).unwrap();

        let context = tc.as_command_context();
        let cmd = MoveCard {
            card_id: mover_id,
            new_column_id: dst_id,
            new_position: 1,
        };
        let result = cmd.execute(&context);
        assert!(result.unwrap_err().is_wip_limit_exceeded());
    }

    #[test]
    fn test_compact_column_positions_makes_sequential() {
        let tc = TestContext::new();
        let mut board = crate::Board::new("B", Some("TST"));
        let col = crate::Column::new(board.id, "Col", 0);
        let column_id = col.id;
        let mut card1 = crate::Card::new(&mut board, column_id, "C1", 0);
        card1.position = 0;
        let mut card2 = crate::Card::new(&mut board, column_id, "C2", 5);
        card2.position = 5;
        tc.store.upsert_board(board).unwrap();
        tc.store.upsert_column(col).unwrap();
        tc.store.upsert_card(card1).unwrap();
        tc.store.upsert_card(card2).unwrap();

        let context = tc.as_command_context();
        let cmd = CompactColumnPositions { column_id };
        cmd.execute(&context).unwrap();

        let cards = tc.store.list_cards_by_column(column_id).unwrap();
        assert_eq!(cards[0].position, 0);
        assert_eq!(cards[1].position, 1);
    }

    // KAN-936 (ARCH-DECOR C2): compaction must operate over the Include set
    // (live + archived) so no live card is renumbered onto an archived card's
    // coherently-placed ordinal. The live-only compaction (reading
    // `list_cards_by_column`) re-collides an archived ordinal.
    #[test]
    fn test_compact_preserves_distinct_ordinals_across_live_and_archived() {
        let tc = TestContext::new();
        let mut board = crate::Board::new("B", Some("TST"));
        let col = crate::Column::new(board.id, "Col", 0);
        let column_id = col.id;
        // Interleaved: live @0, archived @1, live @3 (gappy). After compaction
        // over the Include set the three cards should hold {0,1,2} distinctly.
        let mut live_a = crate::Card::new(&mut board, column_id, "LiveA", 0);
        live_a.position = 0;
        let live_a_id = live_a.id;
        let mut archived = crate::Card::new(&mut board, column_id, "Archived", 1);
        archived.position = 1;
        let archived_id = archived.id;
        let mut live_b = crate::Card::new(&mut board, column_id, "LiveB", 3);
        live_b.position = 3;
        let live_b_id = live_b.id;
        tc.store.upsert_board(board).unwrap();
        tc.store.upsert_column(col).unwrap();
        tc.store.upsert_card(live_a).unwrap();
        tc.store.upsert_card(archived.clone()).unwrap();
        tc.store.upsert_card(live_b).unwrap();
        tc.store
            .insert_archived_card(crate::ArchivedCard::new(archived_id, Uuid::nil()))
            .unwrap();

        let context = tc.as_command_context();
        CompactColumnPositions { column_id }
            .execute(&context)
            .unwrap();

        let positions = |id: Uuid| tc.store.get_card(id).unwrap().unwrap().position;
        let pa = positions(live_a_id);
        let par = positions(archived_id);
        let pb = positions(live_b_id);

        // No two cards in the column share a position.
        let mut all = [pa, par, pb];
        all.sort_unstable();
        assert_eq!(
            all,
            [0, 1, 2],
            "column should be dense over the Include set"
        );

        // Relative order preserved for each subset (live: A before B; archived
        // sits between them as seeded).
        assert!(pa < pb, "live relative order preserved");
        assert!(
            pa < par && par < pb,
            "archived ordinal stays coherent between live cards"
        );
    }

    #[test]
    fn test_archive_then_compact_keeps_archived_card_coherent() {
        use crate::commands::card::ArchiveCards;

        let tc = TestContext::new();
        let mut board = crate::Board::new("B", Some("TST"));
        let col = crate::Column::new(board.id, "Col", 0);
        let column_id = col.id;
        // Three live cards at 0,1,2. Archive the middle one, then run the TUI's
        // compact path. The archived card keeps position 1; live cards must not
        // be renumbered onto it.
        let mut c0 = crate::Card::new(&mut board, column_id, "C0", 0);
        c0.position = 0;
        let c0_id = c0.id;
        let mut c1 = crate::Card::new(&mut board, column_id, "C1", 1);
        c1.position = 1;
        let c1_id = c1.id;
        let mut c2 = crate::Card::new(&mut board, column_id, "C2", 2);
        c2.position = 2;
        let c2_id = c2.id;
        tc.store.upsert_board(board).unwrap();
        tc.store.upsert_column(col).unwrap();
        tc.store.upsert_card(c0).unwrap();
        tc.store.upsert_card(c1).unwrap();
        tc.store.upsert_card(c2).unwrap();

        let context = tc.as_command_context();
        ArchiveCards { ids: vec![c1_id] }.execute(&context).unwrap();
        CompactColumnPositions { column_id }
            .execute(&context)
            .unwrap();

        let get = |id: Uuid| tc.store.get_card(id).unwrap().unwrap().position;
        let archived_pos = get(c1_id);
        let live0 = get(c0_id);
        let live2 = get(c2_id);

        assert_ne!(
            live0, archived_pos,
            "live C0 must not collide with archived ordinal"
        );
        assert_ne!(
            live2, archived_pos,
            "live C2 must not collide with archived ordinal"
        );
        // Whole column dense and distinct over the Include set.
        let mut all = [live0, archived_pos, live2];
        all.sort_unstable();
        assert_eq!(all, [0, 1, 2]);
    }
}
