//! The WIP-limit rule, in one place.
//!
//! Lives here rather than on `CommandContext` because two callers need it and
//! they sit on opposite sides of the command boundary: `CreateCard::execute`,
//! which must keep checking so a replayed command log cannot exceed a limit,
//! and the service's create path, which must check BEFORE it allocates a card
//! number. Allocation cannot move back inside the command -- `CreateCard`'s
//! serialized shape is frozen around `card_number` -- so a create rejected
//! inside the command would otherwise consume a number no card ever carries.
//!
//! Same shape as [`crate::prefix::allocate_card_number`], for the same reason:
//! one rule, two tiers, and a second copy is how they drift.

use uuid::Uuid;

use crate::{DataStore, DomainError, KanbanError, KanbanResult};

/// Returns `WipLimitExceeded` if adding `adding` cards to `column_id` would
/// exceed its WIP limit. Cards whose ids appear in `exclude` are not counted
/// toward the current occupancy. Returns `not_found` if the column does not
/// exist.
pub fn check_wip_limit(
    store: &dyn DataStore,
    column_id: Uuid,
    adding: usize,
    exclude: &[Uuid],
) -> KanbanResult<()> {
    let column = store
        .get_column(column_id)?
        .ok_or_else(|| KanbanError::not_found("Column", column_id))?;
    let Some(limit) = column.wip_limit else {
        return Ok(());
    };
    let current = store.count_cards_in_column_excluding(column_id, exclude)?;
    if current + adding > limit as usize {
        return Err(KanbanError::Domain(DomainError::wip_limit_exceeded(
            column_id,
            limit as u32,
        )));
    }
    Ok(())
}
