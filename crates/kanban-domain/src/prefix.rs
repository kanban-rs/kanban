use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::board::{Board, BoardId};
use crate::sprint::{Sprint, SprintId};

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
    default_card_prefix: &str,
) -> String {
    sprint_card_prefix
        .or(board_card_prefix)
        .unwrap_or(default_card_prefix)
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
    default_card_prefix: &str,
) -> crate::KanbanResult<(String, u32)> {
    let display = effective_card_prefix(board_card_prefix, sprint_card_prefix, default_card_prefix);
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

/// Reserves the next sprint number in the namespace a new sprint belongs to,
/// and returns the `(prefix, sprint_number)` it is stamped with.
///
/// The sprint-axis twin of [`allocate_card_number`], and it exists for the
/// same reason: `board.sprint_counters` is private to one board, so two
/// boards sharing a sprint prefix each hand out number 1.
///
/// Takes the already-resolved effective prefix rather than the board/sprint
/// pair, because the caller has a `Board` in hand and `Sprint::create` needs
/// the same value for its own field. Returns it in its configured casing;
/// only the row name is normalised.
pub fn allocate_sprint_number(
    store: &dyn crate::DataStore,
    effective_sprint_prefix: &str,
) -> crate::KanbanResult<(String, u32)> {
    let name = Prefix::normalize(effective_sprint_prefix);
    let mut row = store
        .get_prefix(&name)?
        .unwrap_or_else(|| Prefix::new(&name));
    let sprint_number = row.sprint_counter + 1;
    row.sprint_counter = sprint_number;
    store.upsert_prefix(row)?;
    Ok((effective_sprint_prefix.to_string(), sprint_number))
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

/// Which board or sprint a prefix is currently allocated to.
///
/// This is an ALLOCATION record, not a resolution mechanism: it says which
/// entity a prefix currently belongs to for handing out the next number, not
/// which prefix an existing card carries. Existing cards never look this up.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PrefixOwner {
    Board(BoardId),
    Sprint(SprintId),
}

impl Prefix {
    pub fn normalize(raw: &str) -> String {
        raw.to_lowercase()
    }
}

/// A prefix that would currently be handed out to a NEW card created under
/// the given owner. Used for collision detection and for stamping a new
/// card's prefix at creation time — never for resolving an existing card's
/// already-stored prefix.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectivePrefix {
    pub name: String,
    pub owner: PrefixOwner,
}

/// Computes the set of prefixes a workspace would currently hand out to new
/// cards: every board's effective prefix (falling back to
/// `default_card_prefix` when unset), plus every sprint's override. A board
/// keeps its own entry even when one of its sprints overrides it — the two
/// are independently effective for their respective new-card allocations.
pub fn effective_prefixes(
    boards: &[Board],
    sprints: &[Sprint],
    default_card_prefix: &str,
) -> Vec<EffectivePrefix> {
    let mut result: Vec<EffectivePrefix> = boards
        .iter()
        .map(|board| EffectivePrefix {
            name: Prefix::normalize(board.card_prefix.as_deref().unwrap_or(default_card_prefix)),
            owner: PrefixOwner::Board(board.id),
        })
        .collect();

    result.extend(sprints.iter().filter_map(|sprint| {
        sprint.card_prefix.as_deref().map(|prefix| EffectivePrefix {
            name: Prefix::normalize(prefix),
            owner: PrefixOwner::Sprint(sprint.id),
        })
    }));

    result
}

/// A normalised prefix that more than one owner would currently hand out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrefixCollision {
    pub name: String,
    pub owners: Vec<PrefixOwner>,
}

/// Groups effective prefixes by their normalised name and reports every
/// group with more than one owner.
pub fn find_prefix_collisions(effective: &[EffectivePrefix]) -> Vec<PrefixCollision> {
    let mut by_name: HashMap<&str, Vec<PrefixOwner>> = HashMap::new();
    for entry in effective {
        by_name
            .entry(entry.name.as_str())
            .or_default()
            .push(entry.owner);
    }

    // Sorted so a migration dry-run reports collisions in a stable order.
    // `HashMap::into_iter` alone would reorder between runs and flake any
    // test asserting on more than one collision.
    let mut collisions: Vec<PrefixCollision> = by_name
        .into_iter()
        .filter(|(_, owners)| owners.len() > 1)
        .map(|(name, owners)| PrefixCollision {
            name: name.to_string(),
            owners,
        })
        .collect();
    collisions.sort_by(|a, b| a.name.cmp(&b.name));
    collisions
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::Board;
    use crate::sprint::Sprint;
    use uuid::Uuid;

    fn board_with_prefix(prefix: Option<&str>) -> Board {
        Board::new("board".to_string(), prefix)
    }

    fn sprint_with_prefix(board_id: Uuid, prefix: Option<&str>) -> Sprint {
        let mut sprint = Sprint::new(board_id, 1, None, None::<String>);
        sprint.card_prefix = prefix.map(str::to_string);
        sprint
    }

    #[test]
    fn test_normalize_lowercases_prefix() {
        assert_eq!(Prefix::normalize("KAN"), Prefix::normalize("kan"));
        assert_eq!(Prefix::normalize("KAN"), "kan");
    }

    #[test]
    fn test_effective_prefixes_defaults_none_board_prefix_to_config_default() {
        let board = board_with_prefix(None);
        let board_id = board.id;
        let effective = effective_prefixes(std::slice::from_ref(&board), &[], "task");

        assert_eq!(
            effective,
            vec![EffectivePrefix {
                name: "task".to_string(),
                owner: PrefixOwner::Board(board_id),
            }]
        );
    }

    #[test]
    fn test_effective_prefixes_sprint_override_yields_an_independent_entry() {
        let board = board_with_prefix(Some("KAN"));
        let board_id = board.id;
        let sprint = sprint_with_prefix(board_id, Some("AUTH"));
        let sprint_id = sprint.id;

        let mut effective = effective_prefixes(
            std::slice::from_ref(&board),
            std::slice::from_ref(&sprint),
            "task",
        );
        effective.sort_by(|a, b| a.name.cmp(&b.name));

        assert_eq!(
            effective,
            vec![
                EffectivePrefix {
                    name: "auth".to_string(),
                    owner: PrefixOwner::Sprint(sprint_id),
                },
                EffectivePrefix {
                    name: "kan".to_string(),
                    owner: PrefixOwner::Board(board_id),
                },
            ]
        );
    }

    #[test]
    fn test_find_prefix_collisions_detects_case_insensitive_duplicate() {
        let board_a = board_with_prefix(Some("KAN"));
        let board_b = board_with_prefix(Some("kan"));

        let effective = effective_prefixes(&[board_a.clone(), board_b.clone()], &[], "task");
        let collisions = find_prefix_collisions(&effective);

        assert_eq!(collisions.len(), 1);
        let collision = &collisions[0];
        assert_eq!(collision.name, "kan");
        assert_eq!(collision.owners.len(), 2);
        assert!(collision.owners.contains(&PrefixOwner::Board(board_a.id)));
        assert!(collision.owners.contains(&PrefixOwner::Board(board_b.id)));
    }

    #[test]
    fn test_find_prefix_collisions_empty_on_real_tracker_shape() {
        let prefixes = ["kan", "dev", "ops", "web", "doc", "sec"];
        let boards: Vec<Board> = prefixes
            .iter()
            .map(|p| board_with_prefix(Some(p)))
            .collect();
        let sprint = sprint_with_prefix(boards[0].id, Some("auth-sprint"));

        let effective = effective_prefixes(&boards, std::slice::from_ref(&sprint), "task");
        let collisions = find_prefix_collisions(&effective);

        assert!(collisions.is_empty());
    }

    #[test]
    fn test_sprint_without_an_override_emits_no_entry() {
        // The `None` arm is what stops every sprint echoing its board's
        // prefix into the set as a duplicate. Every other test uses `Some`.
        let board = board_with_prefix(Some("kan"));
        let sprint = sprint_with_prefix(board.id, None);
        let effective = effective_prefixes(&[board], &[sprint], "task");
        assert_eq!(
            effective.len(),
            1,
            "a sprint with no override must contribute nothing"
        );
        assert_eq!(effective[0].name, "kan");
    }
}
