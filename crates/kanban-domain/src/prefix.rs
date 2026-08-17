use serde::{Deserialize, Serialize};

use crate::board::Board;
use crate::sprint::Sprint;

/// The namespace a NEW card belongs to, given its board's prefix and its
/// sprint's override. Normalised.
///
/// A sprint override beats its board's prefix, and the configured default
/// applies when neither is set. Shared so the service's allocator and
/// `CreateCard::execute` cannot derive different prefixes for one card -- they
/// run at different layers and the command's serialized shape is frozen, so
/// the number is passed to it but the prefix is re-derived.
///
/// Returned with its CONFIGURED CASING, not normalised. This is the value
/// stamped onto a card and rendered in identifiers and branch names, and a git
/// branch name is case-sensitive: normalising here would turn `KAN-668/...`
/// into `kan-668/...` for every user. Uniqueness is enforced separately, by
/// normalising the prefix ROW's name -- see [`allocate_card_number`].
pub fn effective_card_prefix(
    board_card_prefix: Option<&str>,
    sprint_card_prefix: Option<&str>,
    configured: Option<&str>,
) -> String {
    crate::prefix_resolution::resolve(
        crate::PrefixAxis::Card,
        [sprint_card_prefix, board_card_prefix],
        configured,
    )
    .to_string()
}

/// Reserves the next card number in the namespace a new card belongs to, and
/// returns the `(prefix, card_number)` it is stamped with.
///
/// Numbering belongs to the prefix row, not to the card and not to the board.
/// One counter per namespace is what makes `(prefix, card_number)` unique: with
/// per-board counters, two boards sharing a prefix each mint number 1 and the
/// same identifier names two cards.
///
/// Lives here rather than in the service tier because two callers need it and
/// they sit on opposite sides of that boundary -- the service's create path and
/// `CreateSubcardCommand`, which runs inside the domain. A subcard allocating
/// from a different counter than a card is the collision this prevents.
///
/// Creates the row on demand: a board can predate the prefixes table, and an
/// absent row means nothing has been allocated from that namespace yet.
pub fn allocate_card_number(
    store: &dyn crate::DataStore,
    board_card_prefix: Option<&str>,
    sprint_card_prefix: Option<&str>,
    configured: Option<&str>,
) -> crate::KanbanResult<(String, u32)> {
    let display = effective_card_prefix(board_card_prefix, sprint_card_prefix, configured);
    // The ROW is keyed on the normalised name, so `KAN` and `kan` are one
    // namespace with one counter. The card keeps the configured casing.
    let name = Prefix::normalize(&display);
    let mut row = store
        .get_prefix(&name)?
        .unwrap_or_else(|| Prefix::new(&name));
    let card_number = row.card_counter + 1;
    row.card_counter = card_number;
    store.upsert_prefix(row)?;
    Ok((display, card_number))
}

/// Reserves and returns the next sprint number in the namespace a new sprint
/// belongs to.
///
/// The sprint-axis twin of [`allocate_card_number`], and it exists for the
/// same reason: `board.sprint_counters` is private to one board, so two
/// boards sharing a sprint prefix each hand out number 1.
///
/// Takes the already-resolved effective prefix rather than the board/sprint
/// pair the card version takes, because the caller has a `Board` in hand and
/// `Sprint::create` needs the same value for its own field. It returns only
/// the number for that reason: unlike [`allocate_card_number`], which derives
/// the prefix it returns, this one would only hand back its own argument.
/// Only the row name is normalised; the caller keeps its configured casing.
pub fn allocate_sprint_number(
    store: &dyn crate::DataStore,
    effective_sprint_prefix: &str,
) -> crate::KanbanResult<u32> {
    let name = Prefix::normalize(effective_sprint_prefix);
    let mut row = store
        .get_prefix(&name)?
        .unwrap_or_else(|| Prefix::new(&name));
    let sprint_number = row.sprint_counter + 1;
    row.sprint_counter = sprint_number;
    store.upsert_prefix(row)?;
    Ok(sprint_number)
}

/// A namespace that allocates card and sprint numbers for one name.
///
/// Both counters are HIGH-WATER MARKS: the last number handed out, so the
/// next is `counter + 1`. This differs from the legacy `board.sprint_counters`
/// map, which stores the next number instead; anything projecting that map
/// into this field must convert.
///
/// Several boards may share a prefix, so this deliberately records no owner:
/// the reference runs board -> prefix, not the other way round.
///
/// A card's prefix is fixed at creation and WILL be stored on the card itself.
/// `Card` does not carry that field yet; it arrives with the allocation card.
/// `Prefix` is not consulted to resolve an EXISTING
/// card's identifier; it exists to allocate NEW cards and to detect
/// collisions among the effective prefixes a workspace could hand out next.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Prefix {
    /// Always normalised. Construct through [`Prefix::new`] rather than a
    /// struct literal, or the normalisation contract is silently violated.
    pub name: String,
    #[serde(default)]
    pub card_counter: u32,
    #[serde(default)]
    pub sprint_counter: u32,
}

impl Prefix {
    /// Normalises `raw` so the stored name always satisfies the type's
    /// contract. A struct literal bypasses this; prefer this constructor.
    pub fn new(raw: &str) -> Self {
        Self {
            name: Self::normalize(raw),
            card_counter: 0,
            sprint_counter: 0,
        }
    }
}

impl Prefix {
    pub fn normalize(raw: &str) -> String {
        raw.to_lowercase()
    }
}

/// Computes the set of normalised prefix names a workspace would currently
/// hand out to new cards: every board's effective prefix (falling back to
/// `default_card_prefix` when unset), plus every sprint's override. A board
/// keeps its own entry even when one of its sprints overrides it — the two
/// are independently effective for their respective new-card allocations.
pub fn effective_prefixes(
    boards: &[Board],
    sprints: &[Sprint],
    default_card_prefix: &str,
) -> Vec<String> {
    let mut result: Vec<String> = boards
        .iter()
        .map(|board| Prefix::normalize(board.card_prefix.as_deref().unwrap_or(default_card_prefix)))
        .collect();

    result.extend(
        sprints
            .iter()
            .filter_map(|sprint| sprint.card_prefix.as_deref().map(Prefix::normalize)),
    );

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_lowercases_prefix() {
        assert_eq!(Prefix::normalize("KAN"), Prefix::normalize("kan"));
        assert_eq!(Prefix::normalize("KAN"), "kan");
    }
}
