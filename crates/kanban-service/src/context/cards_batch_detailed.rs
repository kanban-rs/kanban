use super::{BatchOperationFailure, BatchOperationResult, KanbanContext};
use kanban_domain::commands::{CardCommand, Command};
use kanban_domain::KanbanError;
use uuid::Uuid;

impl KanbanContext {
    pub fn archive_cards_detailed(&mut self, ids: Vec<Uuid>) -> BatchOperationResult {
        use kanban_domain::commands::ArchiveCards;
        let all_cards = match self.list_live_cards_impl() {
            Ok(c) => c,
            Err(e) => {
                return BatchOperationResult {
                    succeeded: vec![],
                    failed: ids
                        .into_iter()
                        .map(|id| BatchOperationFailure {
                            id,
                            error: e.to_string(),
                        })
                        .collect(),
                };
            }
        };
        let card_ids: std::collections::HashSet<Uuid> = all_cards.iter().map(|c| c.id).collect();
        let mut to_archive = Vec::new();
        let mut failed = Vec::new();
        for id in ids {
            if card_ids.contains(&id) {
                to_archive.push(id);
            } else {
                failed.push(BatchOperationFailure {
                    id,
                    error: KanbanError::not_found("Card", id).to_string(),
                });
            }
        }
        if to_archive.is_empty() {
            return BatchOperationResult {
                succeeded: vec![],
                failed,
            };
        }
        let succeeded = to_archive.clone();
        match self.execute(vec![Command::Card(CardCommand::Archive(ArchiveCards {
            ids: to_archive,
        }))]) {
            Ok(()) => BatchOperationResult { succeeded, failed },
            Err(e) => {
                let err = e.to_string();
                let mut all_failed = failed;
                all_failed.extend(succeeded.into_iter().map(|id| BatchOperationFailure {
                    id,
                    error: err.clone(),
                }));
                BatchOperationResult {
                    succeeded: vec![],
                    failed: all_failed,
                }
            }
        }
    }

    pub fn move_cards_detailed(&mut self, ids: Vec<Uuid>, column_id: Uuid) -> BatchOperationResult {
        // Dedup at the input boundary so the per-id classification loop both
        // (a) reports each invalid id once in `failed` and (b) reports each
        // valid id once in `succeeded`, matching the one `MoveCard` per
        // unique id that `compute_move_positions` will emit. Also avoids
        // redundant get_card calls for the same id.
        let ids = kanban_domain::card_lifecycle::dedup_preserving_order(&ids);
        let mut to_move = Vec::new();
        let mut failed = Vec::new();
        for id in ids {
            match self.backend.get_card(id) {
                Ok(Some(_)) => to_move.push(id),
                Ok(None) => failed.push(BatchOperationFailure {
                    id,
                    error: KanbanError::not_found("Card", id).to_string(),
                }),
                Err(e) => failed.push(BatchOperationFailure {
                    id,
                    error: e.to_string(),
                }),
            }
        }
        if to_move.is_empty() {
            return BatchOperationResult {
                succeeded: vec![],
                failed,
            };
        }
        let succeeded = to_move.clone();

        let chained_status_updates =
            match self.chained_status_updates_for_batch_move(&to_move, column_id) {
                Ok(v) => v,
                Err(e) => {
                    let err = e.to_string();
                    let mut all_failed = failed;
                    all_failed.extend(succeeded.into_iter().map(|id| BatchOperationFailure {
                        id,
                        error: err.clone(),
                    }));
                    return BatchOperationResult {
                        succeeded: vec![],
                        failed: all_failed,
                    };
                }
            };

        let batch = match self.build_move_cards_batch(&to_move, column_id, chained_status_updates) {
            Ok(b) => b,
            Err(e) => {
                let err = e.to_string();
                let mut all_failed = failed;
                all_failed.extend(succeeded.into_iter().map(|id| BatchOperationFailure {
                    id,
                    error: err.clone(),
                }));
                return BatchOperationResult {
                    succeeded: vec![],
                    failed: all_failed,
                };
            }
        };

        match self.execute(batch) {
            Ok(()) => BatchOperationResult { succeeded, failed },
            Err(e) => {
                let err = e.to_string();
                let mut all_failed = failed;
                all_failed.extend(succeeded.into_iter().map(|id| BatchOperationFailure {
                    id,
                    error: err.clone(),
                }));
                BatchOperationResult {
                    succeeded: vec![],
                    failed: all_failed,
                }
            }
        }
    }

    pub fn assign_cards_to_sprint_detailed(
        &mut self,
        ids: Vec<Uuid>,
        sprint_id: Uuid,
    ) -> BatchOperationResult {
        use kanban_domain::commands::AssignCardsToSprint;
        let all_sprints = match self.list_live_sprints_impl() {
            Ok(s) => s,
            Err(e) => {
                return BatchOperationResult {
                    succeeded: vec![],
                    failed: ids
                        .into_iter()
                        .map(|id| BatchOperationFailure {
                            id,
                            error: e.to_string(),
                        })
                        .collect(),
                };
            }
        };
        if !all_sprints.iter().any(|s| s.id == sprint_id) {
            return BatchOperationResult {
                succeeded: vec![],
                failed: ids
                    .into_iter()
                    .map(|id| BatchOperationFailure {
                        id,
                        error: KanbanError::not_found("Sprint", sprint_id).to_string(),
                    })
                    .collect(),
            };
        }
        let all_cards = match self.list_live_cards_impl() {
            Ok(c) => c,
            Err(e) => {
                return BatchOperationResult {
                    succeeded: vec![],
                    failed: ids
                        .into_iter()
                        .map(|id| BatchOperationFailure {
                            id,
                            error: e.to_string(),
                        })
                        .collect(),
                };
            }
        };
        let card_ids: std::collections::HashSet<Uuid> = all_cards.iter().map(|c| c.id).collect();
        let mut to_assign = Vec::new();
        let mut failed = Vec::new();
        for id in ids {
            if card_ids.contains(&id) {
                to_assign.push(id);
            } else {
                failed.push(BatchOperationFailure {
                    id,
                    error: KanbanError::not_found("Card", id).to_string(),
                });
            }
        }
        if to_assign.is_empty() {
            return BatchOperationResult {
                succeeded: vec![],
                failed,
            };
        }
        let succeeded = to_assign.clone();
        match self.execute(vec![Command::Card(CardCommand::AssignToSprint(
            AssignCardsToSprint {
                ids: to_assign,
                sprint_id,
            },
        ))]) {
            Ok(()) => BatchOperationResult { succeeded, failed },
            Err(e) => {
                let err = e.to_string();
                let mut all_failed = failed;
                all_failed.extend(succeeded.into_iter().map(|id| BatchOperationFailure {
                    id,
                    error: err.clone(),
                }));
                BatchOperationResult {
                    succeeded: vec![],
                    failed: all_failed,
                }
            }
        }
    }
}
