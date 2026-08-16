use std::collections::HashMap;

use crate::board::{Board, BoardId};
use crate::sprint::{Sprint, SprintId};

/// A prefix that has been (or will be) allocated to a board or sprint.
///
/// A card's prefix is fixed at creation and stored on the card itself — see
/// [`Card`](crate::Card). `Prefix` is not consulted to resolve an EXISTING
/// card's identifier; it exists to allocate NEW cards and to detect
/// collisions among the effective prefixes a workspace could hand out next.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Prefix {
    pub name: String,
    pub owner: PrefixOwner,
    pub card_counter: u32,
    pub sprint_counter: u32,
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

    by_name
        .into_iter()
        .filter(|(_, owners)| owners.len() > 1)
        .map(|(name, owners)| PrefixCollision {
            name: name.to_string(),
            owners,
        })
        .collect()
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
    fn test_effective_prefixes_sprint_override_wins_over_board() {
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
}
