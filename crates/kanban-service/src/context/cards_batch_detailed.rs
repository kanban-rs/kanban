use super::{BatchOperationFailure, BatchOperationResult, KanbanContext};
use kanban_domain::commands::{CardCommand, Command};
use kanban_domain::{Card, EntityIds, Invalidation, KanbanError};
use uuid::Uuid;

impl KanbanContext {
    pub fn archive_cards_detailed(
        &mut self,
        ids: Vec<Uuid>,
    ) -> (BatchOperationResult, Invalidation) {
        use kanban_domain::commands::ArchiveCards;
        if let Some(rw) = self.backend.remote_batch_writes() {
            return match rw.archive_cards(&ids) {
                Ok((outcome, invalidation)) => (outcome.into(), invalidation),
                Err(e) => (BatchOperationResult::all_failed(ids, &e), Invalidation::All),
            };
        }
        if self.backend.remote_writes().is_some() {
            let e = KanbanError::unsupported("archive_cards");
            return (
                BatchOperationResult::all_failed(ids, &e),
                Invalidation::Entities(EntityIds::default()),
            );
        }
        let all_cards = match self.list_live_cards_impl() {
            Ok(c) => c,
            Err(e) => {
                return (
                    BatchOperationResult::all_failed(ids, &e),
                    Invalidation::Entities(EntityIds::default()),
                );
            }
        };
        let (to_archive, succeeded, failed) = self.classify_archive_ids(ids, &all_cards);
        if to_archive.is_empty() {
            return (
                BatchOperationResult { succeeded, failed },
                Invalidation::Entities(EntityIds::default()),
            );
        }
        let to_archive_set: std::collections::HashSet<Uuid> = to_archive.iter().copied().collect();
        match self.execute(vec![Command::Card(CardCommand::Archive(ArchiveCards {
            ids: to_archive,
        }))]) {
            Ok(invalidation) => (BatchOperationResult { succeeded, failed }, invalidation),
            Err(e) => {
                let (lost, kept): (Vec<Uuid>, Vec<Uuid>) = succeeded
                    .into_iter()
                    .partition(|id| to_archive_set.contains(id));
                let mut all_failed = failed;
                all_failed.extend(
                    lost.into_iter()
                        .map(|id| BatchOperationFailure::new(id, &e)),
                );
                (
                    BatchOperationResult {
                        succeeded: kept,
                        failed: all_failed,
                    },
                    Invalidation::Entities(EntityIds::default()),
                )
            }
        }
    }

    fn classify_archive_ids(
        &self,
        ids: Vec<Uuid>,
        live_cards: &[Card],
    ) -> (Vec<Uuid>, Vec<Uuid>, Vec<BatchOperationFailure>) {
        let live_ids: std::collections::HashSet<Uuid> = live_cards.iter().map(|c| c.id).collect();
        let mut to_archive = Vec::new();
        let mut succeeded = Vec::new();
        let mut failed = Vec::new();
        for id in ids {
            if live_ids.contains(&id) {
                to_archive.push(id);
                succeeded.push(id);
                continue;
            }
            match self.backend.get_archived_card(id) {
                Ok(Some(_)) => succeeded.push(id),
                Ok(None) => failed.push(BatchOperationFailure::new(
                    id,
                    &KanbanError::not_found("Card", id),
                )),
                Err(e) => failed.push(BatchOperationFailure::new(id, &e)),
            }
        }
        (to_archive, succeeded, failed)
    }

    pub fn move_cards_detailed(
        &mut self,
        ids: Vec<Uuid>,
        column_id: Uuid,
    ) -> (BatchOperationResult, Invalidation) {
        // Dedup at the input boundary so the per-id classification loop both
        // (a) reports each invalid id once in `failed` and (b) reports each
        // valid id once in `succeeded`, matching the one `MoveCard` per
        // unique id that `compute_move_positions` will emit. Also avoids
        // redundant get_card calls for the same id.
        if let Some(rw) = self.backend.remote_batch_writes() {
            return match rw.move_cards(&ids, column_id) {
                Ok((outcome, invalidation)) => (outcome.into(), invalidation),
                Err(e) => (BatchOperationResult::all_failed(ids, &e), Invalidation::All),
            };
        }
        if self.backend.remote_writes().is_some() {
            let e = KanbanError::unsupported("move_cards");
            return (
                BatchOperationResult::all_failed(ids, &e),
                Invalidation::Entities(EntityIds::default()),
            );
        }
        let ids = kanban_domain::card_lifecycle::dedup_preserving_order(&ids);
        let mut to_move = Vec::new();
        let mut failed = Vec::new();
        for id in ids {
            match self.backend.get_card(id) {
                Ok(Some(_)) => to_move.push(id),
                Ok(None) => failed.push(BatchOperationFailure::new(
                    id,
                    &KanbanError::not_found("Card", id),
                )),
                Err(e) => failed.push(BatchOperationFailure::new(id, &e)),
            }
        }
        if to_move.is_empty() {
            return (
                BatchOperationResult {
                    succeeded: vec![],
                    failed,
                },
                Invalidation::Entities(EntityIds::default()),
            );
        }
        let succeeded = to_move.clone();

        let chained_status_updates =
            match self.chained_status_updates_for_batch_move(&to_move, column_id) {
                Ok(v) => v,
                Err(e) => {
                    let mut all_failed = failed;
                    all_failed.extend(
                        succeeded
                            .into_iter()
                            .map(|id| BatchOperationFailure::new(id, &e)),
                    );
                    return (
                        BatchOperationResult {
                            succeeded: vec![],
                            failed: all_failed,
                        },
                        Invalidation::Entities(EntityIds::default()),
                    );
                }
            };

        let batch = match self.build_move_cards_batch(&to_move, column_id, chained_status_updates) {
            Ok(b) => b,
            Err(e) => {
                let mut all_failed = failed;
                all_failed.extend(
                    succeeded
                        .into_iter()
                        .map(|id| BatchOperationFailure::new(id, &e)),
                );
                return (
                    BatchOperationResult {
                        succeeded: vec![],
                        failed: all_failed,
                    },
                    Invalidation::Entities(EntityIds::default()),
                );
            }
        };

        match self.execute(batch) {
            Ok(invalidation) => (BatchOperationResult { succeeded, failed }, invalidation),
            Err(e) => {
                let mut all_failed = failed;
                all_failed.extend(
                    succeeded
                        .into_iter()
                        .map(|id| BatchOperationFailure::new(id, &e)),
                );
                (
                    BatchOperationResult {
                        succeeded: vec![],
                        failed: all_failed,
                    },
                    Invalidation::Entities(EntityIds::default()),
                )
            }
        }
    }

    pub fn assign_cards_to_sprint_detailed(
        &mut self,
        ids: Vec<Uuid>,
        sprint_id: Uuid,
    ) -> (BatchOperationResult, Invalidation) {
        use kanban_domain::commands::AssignCardsToSprint;
        if let Some(rw) = self.backend.remote_batch_writes() {
            return match rw.assign_cards_to_sprint(&ids, sprint_id) {
                Ok((outcome, invalidation)) => (outcome.into(), invalidation),
                Err(e) => (BatchOperationResult::all_failed(ids, &e), Invalidation::All),
            };
        }
        if self.backend.remote_writes().is_some() {
            let e = KanbanError::unsupported("assign_cards_to_sprint");
            return (
                BatchOperationResult::all_failed(ids, &e),
                Invalidation::Entities(EntityIds::default()),
            );
        }
        let all_sprints = match self.list_live_sprints_impl() {
            Ok(s) => s,
            Err(e) => {
                return (
                    BatchOperationResult::all_failed(ids, &e),
                    Invalidation::Entities(EntityIds::default()),
                );
            }
        };
        let Some(sprint) = all_sprints.iter().find(|s| s.id == sprint_id) else {
            let e = KanbanError::not_found("Sprint", sprint_id);
            return (
                BatchOperationResult::all_failed(ids, &e),
                Invalidation::Entities(EntityIds::default()),
            );
        };
        let all_cards = match self.list_live_cards_impl() {
            Ok(c) => c,
            Err(e) => {
                return (
                    BatchOperationResult::all_failed(ids, &e),
                    Invalidation::Entities(EntityIds::default()),
                );
            }
        };
        let cards_by_id: std::collections::HashMap<Uuid, &Card> =
            all_cards.iter().map(|c| (c.id, c)).collect();
        let mut to_assign = Vec::new();
        let mut failed = Vec::new();
        for id in ids {
            match cards_by_id.get(&id) {
                None => failed.push(BatchOperationFailure::new(
                    id,
                    &KanbanError::not_found("Card", id),
                )),
                Some(card) if card.sprint_id != Some(sprint_id) => {
                    match kanban_domain::sprint_membership::require_sprint_on_board(
                        sprint,
                        card.board_id,
                    ) {
                        Ok(()) => to_assign.push(id),
                        Err(e) => failed.push(BatchOperationFailure::new(id, &e)),
                    }
                }
                Some(_) => to_assign.push(id),
            }
        }
        if to_assign.is_empty() {
            return (
                BatchOperationResult {
                    succeeded: vec![],
                    failed,
                },
                Invalidation::Entities(EntityIds::default()),
            );
        }
        let succeeded = to_assign.clone();
        match self.execute(vec![Command::Card(CardCommand::AssignToSprint(
            AssignCardsToSprint {
                ids: to_assign,
                sprint_id,
            },
        ))]) {
            Ok(invalidation) => (BatchOperationResult { succeeded, failed }, invalidation),
            Err(e) => {
                let mut all_failed = failed;
                all_failed.extend(
                    succeeded
                        .into_iter()
                        .map(|id| BatchOperationFailure::new(id, &e)),
                );
                (
                    BatchOperationResult {
                        succeeded: vec![],
                        failed: all_failed,
                    },
                    Invalidation::Entities(EntityIds::default()),
                )
            }
        }
    }
}
