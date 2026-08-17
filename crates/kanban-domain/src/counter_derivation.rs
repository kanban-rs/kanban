//! Reconstructing prefix counters from the entities that consumed them.
//!
//! A payload that carries cards and sprints but no counters still records what
//! was handed out: every card holds its number, every sprint holds its own. The
//! highest of each per namespace is the floor a destination must clear to avoid
//! re-minting one.
//!
//! Namespaces are resolved the way the allocator resolved them when it issued
//! the number — a sprint's own prefix, else its board's, else the default —
//! because a sprint with no prefix of its own still consumed a real namespace.
//! Resolving differently here would restore the wrong row and leave the one
//! that was actually used sitting at zero.

use crate::{Board, Card, Prefix, Sprint, DEFAULT_SPRINT_PREFIX};
use std::collections::HashMap;
use uuid::Uuid;

/// The counters implied by these entities, one row per namespace they address.
pub fn counters_implied_by(cards: &[Card], sprints: &[Sprint], boards: &[Board]) -> Vec<Prefix> {
    let mut cards_high: HashMap<String, u32> = HashMap::new();
    for card in cards {
        let slot = cards_high
            .entry(Prefix::normalize(&card.prefix))
            .or_insert(0);
        *slot = (*slot).max(card.card_number);
    }

    let board_sprint_prefix: HashMap<Uuid, &str> = boards
        .iter()
        .filter_map(|b| b.sprint_prefix.as_deref().map(|p| (b.id, p)))
        .collect();

    let mut sprints_high: HashMap<String, u32> = HashMap::new();
    for sprint in sprints {
        let prefix = sprint
            .prefix
            .as_deref()
            .or_else(|| board_sprint_prefix.get(&sprint.board_id).copied())
            .unwrap_or(DEFAULT_SPRINT_PREFIX);
        let slot = sprints_high.entry(Prefix::normalize(prefix)).or_insert(0);
        *slot = (*slot).max(sprint.sprint_number);
    }

    let mut names: Vec<String> = cards_high.keys().cloned().collect();
    names.extend(sprints_high.keys().cloned());
    names.sort();
    names.dedup();

    names
        .into_iter()
        .map(|name| Prefix {
            card_counter: cards_high.get(&name).copied().unwrap_or(0),
            sprint_counter: sprints_high.get(&name).copied().unwrap_or(0),
            name,
        })
        .collect()
}
