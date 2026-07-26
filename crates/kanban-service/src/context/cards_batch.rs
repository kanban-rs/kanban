use super::KanbanContext;
use kanban_domain::commands::{CardCommand, Command};
use kanban_domain::{CardStatus, CardUpdate, KanbanError, KanbanResult};
use uuid::Uuid;

impl KanbanContext {
    /// KAN-394: given a status that's about to be applied to a card, compute the
    /// target column the card should live in (and the position to use in that
    /// column) to maintain the status ↔ completion column invariant. Returns
    /// None when no chained move is needed.
    ///
    /// The position is computed via a column-scoped `list_cards_by_column`
    /// query — same convention as `KanbanContext::move_card(_, _, None)` — so
    /// we only ever read the target column, never the full cards table.
    pub(super) fn compute_target_column_for_status(
        &self,
        card_id: Uuid,
        new_status: CardStatus,
    ) -> KanbanResult<Option<(Uuid, i32)>> {
        let Some(card) = self.backend.get_card(card_id)? else {
            return Ok(None);
        };
        let Some(column) = self.backend.get_column(card.column_id)? else {
            return Ok(None);
        };
        let Some(board) = self.backend.get_board(column.board_id)? else {
            return Ok(None);
        };
        let columns = self.backend.list_columns_by_board(board.id)?;
        let Some(target_col) = kanban_domain::card_lifecycle::target_column_for_status(
            &card, new_status, &board, &columns,
        ) else {
            return Ok(None);
        };
        let pos = self.backend.list_cards_by_column(target_col)?.len() as i32;
        Ok(Some((target_col, pos)))
    }

    /// KAN-394: per-card chained status updates for a batch move. For each id
    /// in `ids`, asks the domain whether moving to `new_column_id` requires a
    /// status flip. Returns the cards that need a status update along with
    /// their target status. Cards that aren't found are silently skipped —
    /// individual `MoveCard` commands will surface the not-found error.
    pub(super) fn chained_status_updates_for_batch_move(
        &self,
        ids: &[Uuid],
        new_column_id: Uuid,
    ) -> KanbanResult<Vec<(Uuid, CardStatus)>> {
        let mut updates = Vec::new();
        for &card_id in ids {
            if let Some(new_status) = self.compute_target_status_for_move(card_id, new_column_id)? {
                updates.push((card_id, new_status));
            }
        }
        Ok(updates)
    }

    /// KAN-428: build the command batch for a multi-card move into one column.
    ///
    /// Validates that every input id is a known card up front so that an
    /// unknown id surfaces as `not_found` rather than being miscounted by
    /// the batch WIP pre-check. When the target column has a WIP limit,
    /// performs a single batch-level pre-check that returns one clean
    /// `WipLimitExceeded` before any per-card command runs. The per-card
    /// `MoveCard::execute` WIP check still runs as belt-and-suspenders, but
    /// since `count_cards_in_column_excluding` is now O(column_size +
    /// exclude.len()), the redundant per-card checks are cheap.
    pub(super) fn build_move_cards_batch(
        &self,
        ids: &[Uuid],
        column_id: Uuid,
        chained_status_updates: Vec<(Uuid, CardStatus)>,
    ) -> KanbanResult<Vec<Command>> {
        use kanban_domain::commands::{MoveCard, UpdateCard};
        use kanban_domain::DomainError;
        use std::collections::HashSet;

        for &id in ids {
            if self.backend.get_card(id)?.is_none() {
                return Err(KanbanError::not_found("Card", id));
            }
        }

        let existing = self.backend.list_cards_by_column(column_id)?;
        let column = self
            .backend
            .get_column(column_id)?
            .ok_or_else(|| KanbanError::not_found("Column", column_id))?;

        if let Some(limit) = column.wip_limit {
            // `moving_set.len()` is the post-dedup mover count — `compute_move_positions`
            // emits one `MoveCard` per unique id, so the pre-check must use the same
            // count to avoid a false `WipLimitExceeded` when the caller passes
            // duplicates that would actually fit under the limit.
            let moving_set: HashSet<Uuid> = ids.iter().copied().collect();
            let non_moving = existing
                .iter()
                .filter(|c| !moving_set.contains(&c.id))
                .count();
            if non_moving + moving_set.len() > limit as usize {
                return Err(KanbanError::Domain(DomainError::wip_limit_exceeded(
                    column_id,
                    limit as u32,
                )));
            }
        }

        let positions = kanban_domain::card_lifecycle::compute_move_positions(&existing, ids);

        let mut batch: Vec<Command> =
            Vec::with_capacity(positions.len() + chained_status_updates.len());
        for (card_id, new_position) in positions {
            batch.push(Command::Card(CardCommand::Move(MoveCard {
                card_id,
                new_column_id: column_id,
                new_position,
            })));
        }
        for (card_id, new_status) in chained_status_updates {
            batch.push(Command::Card(CardCommand::Update(UpdateCard {
                card_id,
                updates: CardUpdate {
                    status: Some(new_status),
                    ..Default::default()
                },
            })));
        }
        Ok(batch)
    }

    /// KAN-394: given a column the card is about to move to, compute the status
    /// the card should have to maintain the status ↔ completion column invariant.
    /// Returns None when no chained status update is needed.
    pub(super) fn compute_target_status_for_move(
        &self,
        card_id: Uuid,
        new_column_id: Uuid,
    ) -> KanbanResult<Option<CardStatus>> {
        let Some(card) = self.backend.get_card(card_id)? else {
            return Ok(None);
        };
        let Some(column) = self.backend.get_column(new_column_id)? else {
            return Ok(None);
        };
        let Some(board) = self.backend.get_board(column.board_id)? else {
            return Ok(None);
        };
        let columns = self.backend.list_columns_by_board(board.id)?;
        Ok(
            kanban_domain::card_lifecycle::target_status_for_column_move(
                &card,
                new_column_id,
                &board,
                &columns,
            ),
        )
    }

    pub(super) fn archive_cards_impl(&mut self, ids: Vec<Uuid>) -> KanbanResult<usize> {
        use kanban_domain::commands::ArchiveCards;
        let before = self.backend.list_archived_cards()?.len();
        self.execute(vec![Command::Card(CardCommand::Archive(ArchiveCards {
            ids,
        }))])?;
        Ok(self.backend.list_archived_cards()?.len() - before)
    }

    pub(super) fn move_cards_impl(
        &mut self,
        ids: Vec<Uuid>,
        column_id: Uuid,
    ) -> KanbanResult<usize> {
        let before = self.backend.list_cards_by_column(column_id)?.len();

        let chained_status_updates = self.chained_status_updates_for_batch_move(&ids, column_id)?;
        let batch = self.build_move_cards_batch(&ids, column_id, chained_status_updates)?;

        self.execute(batch)?;
        let after = self.backend.list_cards_by_column(column_id)?.len();
        Ok(after - before)
    }

    pub(super) fn update_cards_impl(
        &mut self,
        updates: Vec<(Uuid, CardUpdate)>,
    ) -> KanbanResult<usize> {
        use kanban_domain::commands::{MoveCard, UpdateCard};
        use kanban_domain::ArchivedFilter;
        use std::collections::HashMap;

        let count = updates.len();
        let mut batch: Vec<Command> = Vec::with_capacity(count * 2);
        // Track per-column position offsets within this batch so chained moves
        // into the same target column don't all collapse onto the same
        // position. `compute_target_column_for_status` reads `list_cards_by_column`
        // once per call against the pre-batch state.
        let mut position_offsets: HashMap<Uuid, i32> = HashMap::new();

        enum Chained {
            Move(Uuid, i32),
            Status(CardStatus),
        }

        for (card_id, mut card_updates) in updates {
            let chained = match (card_updates.status, card_updates.column_id) {
                (Some(new_status), None) => self
                    .compute_target_column_for_status(card_id, new_status)?
                    .map(|(col, base_pos)| {
                        let offset = position_offsets.entry(col).or_insert(0);
                        let pos = base_pos + *offset;
                        *offset += 1;
                        Chained::Move(col, pos)
                    }),
                (None, Some(new_col)) => {
                    // A genuine column change needs its position recomputed the
                    // same way move_card/move_cards do (KAN-987) — otherwise the
                    // card keeps its old position and collides with whatever
                    // already sits there in the new column. Re-affirming the
                    // card's current column (e.g. a PUT-replace that resubmits
                    // every field) is not a move, so it must not touch position.
                    let already_in_column = self
                        .backend
                        .get_card(card_id)?
                        .is_some_and(|c| c.column_id == new_col);
                    if !already_in_column {
                        let base_pos = self
                            .backend
                            .count_cards_in_column_filtered(new_col, ArchivedFilter::Include)?
                            as i32;
                        let offset = position_offsets.entry(new_col).or_insert(0);
                        card_updates.position = Some(base_pos + *offset);
                        *offset += 1;
                    }
                    self.compute_target_status_for_move(card_id, new_col)?
                        .map(Chained::Status)
                }
                _ => None,
            };

            batch.push(Command::Card(CardCommand::Update(UpdateCard {
                card_id,
                updates: card_updates,
            })));

            match chained {
                Some(Chained::Move(col, pos)) => {
                    batch.push(Command::Card(CardCommand::Move(MoveCard {
                        card_id,
                        new_column_id: col,
                        new_position: pos,
                    })));
                }
                Some(Chained::Status(status)) => {
                    batch.push(Command::Card(CardCommand::Update(UpdateCard {
                        card_id,
                        updates: CardUpdate {
                            status: Some(status),
                            ..Default::default()
                        },
                    })));
                }
                None => {}
            }
        }

        self.execute(batch)?;
        Ok(count)
    }

    pub(super) fn assign_cards_to_sprint_impl(
        &mut self,
        ids: Vec<Uuid>,
        sprint_id: Uuid,
    ) -> KanbanResult<usize> {
        use kanban_domain::commands::AssignCardsToSprint;
        let before = self.backend.list_cards_by_sprint(sprint_id)?.len();
        self.execute(vec![Command::Card(CardCommand::AssignToSprint(
            AssignCardsToSprint { ids, sprint_id },
        ))])?;
        let after = self.backend.list_cards_by_sprint(sprint_id)?.len();
        Ok(after - before)
    }
}
