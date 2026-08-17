//! Reconstructing prefix rows from the entities that consumed them.
//!
//! A payload that carries cards and sprints but no counters still records what
//! was handed out: every card holds its number, every sprint holds its own. The
//! highest of each per namespace is the floor a destination must clear.
//!
//! A card resolves to the namespace its stored prefix names. One written before
//! cards stored a prefix falls back to the live board/sprint chain and then to
//! [`DEFAULT_CARD_PREFIX`], which is what the storage backfills write for that
//! same population -- see [`stamp_card_prefix`]. A sprint resolves through its
//! own prefix, then its board's, then the supplied default.
//!
//! An entity whose namespace cannot be resolved at all is skipped rather than
//! attributed to a default: raising a namespace the entity never consumed
//! corrupts numbering for whoever does own it. A caller with no workspace
//! default to offer passes `None` and gets that skip rather than a guess.

use crate::search::resolve_card_prefix;
use crate::{Board, Card, Column, Prefix, Sprint};
use std::collections::HashMap;

fn card_namespace(
    card: &Card,
    columns: &[Column],
    boards: &[Board],
    sprints: &[Sprint],
) -> Option<String> {
    if !card.prefix.is_empty() {
        return Some(Prefix::normalize(&card.prefix));
    }
    // A card written before cards stored their prefix. It is still addressed
    // through the live chain, so resolve it the way the identifier reader does
    // -- but only when that chain actually reaches a board present here.
    let reaches_a_board = columns
        .iter()
        .find(|col| col.id == card.column_id)
        .is_some_and(|col| boards.iter().any(|b| b.id == col.board_id));

    reaches_a_board.then(|| {
        Prefix::normalize(&resolve_card_prefix(
            card,
            columns,
            boards,
            sprints,
            crate::DEFAULT_CARD_PREFIX,
        ))
    })
}

fn sprint_namespace(
    sprint: &Sprint,
    boards: &[Board],
    default_sprint_prefix: Option<&str>,
) -> Option<String> {
    if let Some(own) = sprint.prefix.as_deref() {
        return Some(Prefix::normalize(own));
    }
    let board = boards.iter().find(|b| b.id == sprint.board_id)?;
    board
        .sprint_prefix
        .as_deref()
        .or(default_sprint_prefix)
        .map(Prefix::normalize)
}

/// The prefix a card carries, or the one the storage backfills would write for
/// a card written before cards stored one.
///
/// `columns`, `boards` and `sprints` must span everything the card can reach,
/// caller-supplied so an importer can union its payload with the destination.
pub fn stamp_card_prefix(
    card: &Card,
    columns: &[Column],
    boards: &[Board],
    sprints: &[Sprint],
) -> Card {
    if !card.prefix.is_empty() {
        return card.clone();
    }
    let mut card = card.clone();
    card.prefix = resolve_card_prefix(&card, columns, boards, sprints, crate::DEFAULT_CARD_PREFIX);
    card
}

/// Raises `derived` to cover `carried`, per namespace and per axis.
///
/// A carried row can lag the entities it accompanies, and entities can address
/// a namespace no row was carried for, so neither side alone is the floor.
pub fn merge_counter_rows(derived: &mut Vec<Prefix>, carried: &[Prefix]) {
    for row in carried {
        let name = Prefix::normalize(&row.name);
        match derived.iter_mut().find(|d| d.name == name) {
            Some(existing) => {
                existing.card_counter = existing.card_counter.max(row.card_counter);
                existing.sprint_counter = existing.sprint_counter.max(row.sprint_counter);
            }
            None => derived.push(Prefix {
                name,
                card_counter: row.card_counter,
                sprint_counter: row.sprint_counter,
            }),
        }
    }
    derived.sort_by(|a, b| a.name.cmp(&b.name));
}

/// Every namespace these entities are addressed by, normalised and deduped.
pub fn namespaces_addressed_by(
    cards: &[Card],
    columns: &[Column],
    sprints: &[Sprint],
    boards: &[Board],
    default_sprint_prefix: Option<&str>,
) -> Vec<String> {
    let mut names: Vec<String> = cards
        .iter()
        .filter_map(|c| card_namespace(c, columns, boards, sprints))
        .chain(
            sprints
                .iter()
                .filter_map(|s| sprint_namespace(s, boards, default_sprint_prefix)),
        )
        .collect();
    names.sort();
    names.dedup();
    names
}

/// The counters implied by these entities, one row per namespace they address.
pub fn counters_implied_by(
    cards: &[Card],
    columns: &[Column],
    sprints: &[Sprint],
    boards: &[Board],
    default_sprint_prefix: Option<&str>,
) -> Vec<Prefix> {
    let mut cards_high: HashMap<String, u32> = HashMap::new();
    for card in cards {
        let Some(name) = card_namespace(card, columns, boards, sprints) else {
            continue;
        };
        let slot = cards_high.entry(name).or_insert(0);
        *slot = (*slot).max(card.card_number);
    }

    let mut sprints_high: HashMap<String, u32> = HashMap::new();
    for sprint in sprints {
        let Some(name) = sprint_namespace(sprint, boards, default_sprint_prefix) else {
            continue;
        };
        let slot = sprints_high.entry(name).or_insert(0);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::card_factory::CardRecord;
    use crate::{CardPriority, CardStatus, Column, DEFAULT_SPRINT_PREFIX};
    use chrono::Utc;
    use uuid::Uuid;

    fn board(card_prefix: Option<&str>, sprint_prefix: Option<&str>) -> Board {
        let mut b = Board::new("B", card_prefix);
        b.sprint_prefix = sprint_prefix.map(Into::into);
        b
    }

    fn card_without_stored_prefix(column_id: Uuid, board_id: Uuid, number: u32) -> Card {
        let now = Utc::now();
        Card::reconstitute(CardRecord {
            id: Uuid::new_v4(),
            column_id,
            board_id,
            title: "c".into(),
            description: None,
            priority: CardPriority::Medium,
            status: CardStatus::Todo,
            position: 0,
            due_date: None,
            points: None,
            card_number: number,
            prefix: String::new(),
            sprint_id: None,
            created_at: now,
            updated_at: now,
            completed_at: None,
            sprint_logs: Vec::new(),
        })
        .expect("valid record")
    }

    fn card_counter_of(rows: &[Prefix], name: &str) -> Option<u32> {
        rows.iter().find(|p| p.name == name).map(|p| p.card_counter)
    }

    fn sprint_counter_of(rows: &[Prefix], name: &str) -> Option<u32> {
        rows.iter()
            .find(|p| p.name == name)
            .map(|p| p.sprint_counter)
    }

    #[test]
    fn test_a_card_with_no_stored_prefix_derives_the_namespace_it_is_addressed_by() {
        let b = board(None, None);
        let col = Column::new(b.id, "Todo", 0);
        let cards = vec![card_without_stored_prefix(col.id, b.id, 7)];

        let rows = counters_implied_by(
            &cards,
            std::slice::from_ref(&col),
            &[],
            std::slice::from_ref(&b),
            Some(DEFAULT_SPRINT_PREFIX),
        );

        assert_eq!(card_counter_of(&rows, "task"), Some(7));
        assert_eq!(card_counter_of(&rows, ""), None);
    }

    #[test]
    fn test_a_card_with_no_stored_prefix_prefers_the_boards_card_prefix_over_the_default() {
        let b = board(Some("KAN"), None);
        let col = Column::new(b.id, "Todo", 0);
        let cards = vec![card_without_stored_prefix(col.id, b.id, 3)];

        let rows = counters_implied_by(
            &cards,
            std::slice::from_ref(&col),
            &[],
            std::slice::from_ref(&b),
            Some(DEFAULT_SPRINT_PREFIX),
        );

        assert_eq!(card_counter_of(&rows, "kan"), Some(3));
    }

    #[test]
    fn test_a_card_with_no_stored_prefix_and_an_unresolvable_board_is_skipped() {
        let b = board(None, None);
        let col = Column::new(b.id, "Todo", 0);
        let cards = vec![card_without_stored_prefix(col.id, b.id, 9)];

        let rows = counters_implied_by(&cards, &[], &[], &[], Some(DEFAULT_SPRINT_PREFIX));

        assert!(rows.is_empty(), "guessed a namespace: {rows:?}");
    }

    #[test]
    fn test_a_stored_prefix_wins_over_derivation() {
        let b = board(Some("KAN"), None);
        let col = Column::new(b.id, "Todo", 0);
        let mut c = card_without_stored_prefix(col.id, b.id, 2);
        c.prefix = "OLD".into();

        let rows = counters_implied_by(
            &[c],
            std::slice::from_ref(&col),
            &[],
            std::slice::from_ref(&b),
            Some(DEFAULT_SPRINT_PREFIX),
        );

        assert_eq!(card_counter_of(&rows, "old"), Some(2));
        assert_eq!(card_counter_of(&rows, "kan"), None);
    }

    #[test]
    fn test_cards_sharing_a_namespace_under_different_casing_fold_to_one_row() {
        let b = board(None, None);
        let col = Column::new(b.id, "Todo", 0);
        let mut low = card_without_stored_prefix(col.id, b.id, 2);
        low.prefix = "kan".into();
        let mut high = card_without_stored_prefix(col.id, b.id, 5);
        high.prefix = "KAN".into();

        let rows = counters_implied_by(
            &[low, high],
            std::slice::from_ref(&col),
            &[],
            std::slice::from_ref(&b),
            Some(DEFAULT_SPRINT_PREFIX),
        );

        assert_eq!(rows.len(), 1);
        assert_eq!(card_counter_of(&rows, "kan"), Some(5));
    }

    #[test]
    fn test_a_prefixless_sprint_uses_the_configured_default_sprint_prefix() {
        let b = board(None, None);
        let sprints = vec![Sprint::new(b.id, 3, None, None::<String>)];

        let rows = counters_implied_by(
            &[],
            &[],
            &sprints,
            std::slice::from_ref(&b),
            Some("iteration"),
        );

        assert_eq!(sprint_counter_of(&rows, "iteration"), Some(3));
        assert_eq!(sprint_counter_of(&rows, "sprint"), None);
    }

    #[test]
    fn test_a_prefixless_sprint_with_no_default_offered_is_skipped() {
        let b = board(None, None);
        let sprints = vec![Sprint::new(b.id, 9, None, None::<String>)];

        let rows = counters_implied_by(&[], &[], &sprints, std::slice::from_ref(&b), None);

        assert!(rows.is_empty(), "guessed a namespace: {rows:?}");
    }

    #[test]
    fn test_a_prefixless_sprint_whose_board_is_absent_is_skipped() {
        let b = board(None, Some("REL"));
        let sprints = vec![Sprint::new(b.id, 50, None, None::<String>)];

        let rows = counters_implied_by(&[], &[], &sprints, &[], Some(DEFAULT_SPRINT_PREFIX));

        assert!(rows.is_empty(), "guessed a namespace: {rows:?}");
    }

    #[test]
    fn test_a_sprint_with_its_own_prefix_resolves_without_its_board() {
        let sprints = vec![Sprint::new(Uuid::new_v4(), 4, None, Some("REL"))];

        let rows = counters_implied_by(&[], &[], &sprints, &[], Some(DEFAULT_SPRINT_PREFIX));

        assert_eq!(sprint_counter_of(&rows, "rel"), Some(4));
    }

    #[test]
    fn test_empty_inputs_imply_no_counters() {
        let rows = counters_implied_by(&[], &[], &[], &[], Some(DEFAULT_SPRINT_PREFIX));

        assert!(rows.is_empty());
    }
}
