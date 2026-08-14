use super::CommandContext;
use crate::data_store::DataStore;
use crate::{CardStatus, KanbanResult};
use uuid::Uuid;

fn added_and_removed(prior: &[Uuid], next: &[Uuid]) -> (Vec<Uuid>, Vec<Uuid>) {
    let added = next
        .iter()
        .copied()
        .filter(|id| !prior.contains(id))
        .collect();
    let removed = prior
        .iter()
        .copied()
        .filter(|id| !next.contains(id))
        .collect();
    (added, removed)
}

/// Keep `column.default_status` in step with a board's completion-column
/// configuration: a newly-added completion column gets `Done`; a removed
/// column is reset to `Todo` only when it was `Done` (any other value is a
/// deliberate choice and is left alone).
pub(super) fn sync_default_status(
    context: &CommandContext,
    prior: &[Uuid],
    next: &[Uuid],
) -> KanbanResult<()> {
    let (added, removed) = added_and_removed(prior, next);
    for id in added {
        if let Some(mut column) = context.store.get_column(id)? {
            column.default_status = Some(CardStatus::Done);
            context.store.upsert_column(column)?;
        }
    }
    for id in removed {
        if let Some(mut column) = context.store.get_column(id)? {
            if column.default_status == Some(CardStatus::Done) {
                column.default_status = Some(CardStatus::Todo);
                context.store.upsert_column(column)?;
            }
        }
    }
    Ok(())
}

/// Snapshot `default_status` for every column a completion-column change will
/// touch (added or removed), read BEFORE the change executes, so undo can
/// restore each column's prior value exactly.
pub(super) fn snapshot_touched_columns(
    store: &dyn DataStore,
    prior: &[Uuid],
    next: &[Uuid],
) -> KanbanResult<Vec<(Uuid, Option<CardStatus>)>> {
    let (added, removed) = added_and_removed(prior, next);
    added
        .into_iter()
        .chain(removed)
        .filter_map(|id| {
            store
                .get_column(id)
                .map(|found| found.map(|column| (column.id, column.default_status)))
                .transpose()
        })
        .collect()
}
