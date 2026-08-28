//! Referential-integrity rules between cards and prefix rows: is a namespace
//! a card names actually backed by a row, and is a namespace still
//! referenced by a card.

use std::collections::HashSet;

use crate::{Card, DomainError, KanbanResult, Prefix};

/// Namespaces named by `cards` (via [`Card::prefix`]) that have no matching
/// row in `rows`. Comparison is on [`Prefix::normalize`] for both sides, so
/// the result is normalised, sorted, and deduplicated. Cards with an empty
/// prefix are not reported.
pub fn unbacked_namespaces(cards: &[Card], rows: &[Prefix]) -> Vec<String> {
    let backed: HashSet<String> = rows.iter().map(|r| Prefix::normalize(&r.name)).collect();
    let mut result: Vec<String> = cards
        .iter()
        .filter(|c| !c.prefix.is_empty())
        .map(|c| Prefix::normalize(&c.prefix))
        .filter(|name| !backed.contains(name))
        .collect();
    result.sort();
    result.dedup();
    result
}

/// Rejects a card naming a namespace with no matching row in `rows`. When
/// more than one card offends, the one with the lowest `card_number` is
/// reported (ties broken by normalised prefix name), with the prefix as
/// stored on that card.
pub fn ensure_prefix_rows_exist(cards: &[Card], rows: &[Prefix]) -> KanbanResult<()> {
    let unbacked: HashSet<String> = unbacked_namespaces(cards, rows).into_iter().collect();
    if unbacked.is_empty() {
        return Ok(());
    }
    let offender = cards
        .iter()
        .filter(|c| unbacked.contains(&Prefix::normalize(&c.prefix)))
        .min_by(|a, b| {
            a.card_number
                .cmp(&b.card_number)
                .then_with(|| Prefix::normalize(&a.prefix).cmp(&Prefix::normalize(&b.prefix)))
        });
    match offender {
        Some(card) => Err(DomainError::PrefixNotBacked {
            card_number: card.card_number,
            prefix: card.prefix.clone(),
        }
        .into()),
        None => Ok(()),
    }
}

/// Rejects removing a namespace that any card in `cards` still names.
/// Matching is on [`Prefix::normalize`] for both sides, so `KAN` on a card
/// blocks removing `kan`. The error echoes `name` as the caller spelled it.
/// Cards with an empty prefix name no namespace and are never counted.
/// `cards` is the universe the caller wants protected; `DataStore::list_all_cards`
/// excludes archived cards, which still name their namespace.
pub fn ensure_namespace_unreferenced(name: &str, cards: &[Card]) -> KanbanResult<()> {
    let target = Prefix::normalize(name);
    if target.is_empty() {
        return Ok(());
    }
    let count = cards
        .iter()
        .filter(|c| Prefix::normalize(&c.prefix) == target)
        .count();
    if count == 0 {
        return Ok(());
    }
    Err(DomainError::NamespaceStillReferenced {
        prefix: name.to_string(),
        count,
    }
    .into())
}

#[cfg(test)]
mod tests {
    use crate::card_factory::CardRecord;
    use crate::{Card, CardPriority, CardStatus, DomainError, KanbanError, Prefix};
    use chrono::Utc;
    use uuid::Uuid;

    fn card_with_prefix(prefix: &str, number: u32) -> Card {
        let now = Utc::now();
        Card::reconstitute(CardRecord {
            id: Uuid::new_v4(),
            column_id: Uuid::new_v4(),
            board_id: Uuid::new_v4(),
            title: "c".into(),
            description: None,
            priority: CardPriority::Medium,
            status: CardStatus::Todo,
            position: 0,
            due_date: None,
            points: None,
            card_number: number,
            prefix: prefix.into(),
            sprint_id: None,
            created_at: now,
            updated_at: now,
            completed_at: None,
            sprint_logs: Vec::new(),
        })
        .expect("valid record")
    }

    #[test]
    fn test_a_card_with_no_matching_row_is_reported_unbacked() {
        let cards = vec![card_with_prefix("kan", 1)];
        let result = super::unbacked_namespaces(&cards, &[]);
        assert_eq!(result, vec!["kan".to_string()]);
    }

    #[test]
    fn test_a_row_differing_only_in_case_backs_the_card() {
        let cards = vec![card_with_prefix("KAN", 1)];
        let rows = vec![Prefix::new("kan")];
        let result = super::unbacked_namespaces(&cards, &rows);
        assert!(result.is_empty());
    }

    #[test]
    fn test_a_row_whose_name_bypassed_normalisation_still_backs_the_card() {
        let cards = vec![card_with_prefix("kan", 1)];
        let rows = vec![Prefix {
            name: "KAN".into(),
            card_counter: 0,
            sprint_counter: 0,
        }];
        let result = super::unbacked_namespaces(&cards, &rows);
        assert!(result.is_empty());
    }

    #[test]
    fn test_a_card_with_an_empty_prefix_is_not_reported() {
        let cards = vec![card_with_prefix("", 1)];
        let result = super::unbacked_namespaces(&cards, &[]);
        assert!(result.is_empty());
    }

    #[test]
    fn test_several_cards_sharing_an_unbacked_namespace_report_it_once() {
        let cards = vec![card_with_prefix("KAN", 1), card_with_prefix("kan", 2)];
        let result = super::unbacked_namespaces(&cards, &[]);
        assert_eq!(result, vec!["kan".to_string()]);
    }

    #[test]
    fn test_a_card_naming_an_unbacked_namespace_is_rejected() {
        let cards = vec![card_with_prefix("KAN", 1)];
        let err = super::ensure_prefix_rows_exist(&cards, &[]).unwrap_err();
        assert!(matches!(
            err,
            KanbanError::Domain(DomainError::PrefixNotBacked { card_number: 1, ref prefix }) if prefix == "KAN"
        ));
        assert_eq!(
            err.to_string(),
            "card 1 names prefix 'KAN', which has no row"
        );
    }

    #[test]
    fn test_the_same_card_is_accepted_once_the_row_exists() {
        let cards = vec![card_with_prefix("kan", 1)];
        let rows = vec![Prefix::new("kan")];
        assert!(super::ensure_prefix_rows_exist(&cards, &rows).is_ok());
    }

    #[test]
    fn test_configured_casing_is_backed_by_the_normalised_row() {
        let cards = vec![card_with_prefix("KAN", 1)];
        let rows = vec![Prefix::new("kan")];
        assert!(super::ensure_prefix_rows_exist(&cards, &rows).is_ok());
    }

    #[test]
    fn test_the_reported_offender_is_deterministic() {
        let cards = vec![card_with_prefix("aaa", 7), card_with_prefix("zzz", 3)];
        let err = super::ensure_prefix_rows_exist(&cards, &[]).unwrap_err();
        assert!(matches!(
            err,
            KanbanError::Domain(DomainError::PrefixNotBacked { card_number: 3, ref prefix }) if prefix == "zzz"
        ));

        let cards = vec![card_with_prefix("zzz", 3), card_with_prefix("aaa", 7)];
        let err = super::ensure_prefix_rows_exist(&cards, &[]).unwrap_err();
        assert!(matches!(
            err,
            KanbanError::Domain(DomainError::PrefixNotBacked { card_number: 3, ref prefix }) if prefix == "zzz"
        ));
    }

    #[test]
    fn test_a_namespace_named_by_a_card_cannot_be_removed() {
        let cards = vec![card_with_prefix("kan", 1)];
        let err = super::ensure_namespace_unreferenced("kan", &cards).unwrap_err();
        assert!(matches!(
            err,
            KanbanError::Domain(DomainError::NamespaceStillReferenced { count: 1, ref prefix }) if prefix == "kan"
        ));
        assert_eq!(
            err.to_string(),
            "prefix 'kan' still names 1 card(s) and cannot be removed"
        );
    }

    #[test]
    fn test_the_error_reports_how_many_cards_still_name_it() {
        let cards = vec![
            card_with_prefix("kan", 1),
            card_with_prefix("kan", 2),
            card_with_prefix("kan", 3),
            card_with_prefix("ops", 4),
        ];
        let err = super::ensure_namespace_unreferenced("kan", &cards).unwrap_err();
        assert!(matches!(
            err,
            KanbanError::Domain(DomainError::NamespaceStillReferenced { count: 3, .. })
        ));
    }

    #[test]
    fn test_an_unreferenced_namespace_can_be_removed() {
        let cards = vec![card_with_prefix("kan", 1)];
        assert!(super::ensure_namespace_unreferenced("ops", &cards).is_ok());
        assert!(super::ensure_namespace_unreferenced("kan", &[]).is_ok());
    }

    #[test]
    fn test_removal_is_blocked_regardless_of_casing() {
        let cards = vec![card_with_prefix("KAN", 1)];
        let err = super::ensure_namespace_unreferenced("kan", &cards).unwrap_err();
        assert!(matches!(
            err,
            KanbanError::Domain(DomainError::NamespaceStillReferenced { count: 1, ref prefix }) if prefix == "kan"
        ));

        let cards = vec![card_with_prefix("kan", 1)];
        let err = super::ensure_namespace_unreferenced("KAN", &cards).unwrap_err();
        assert!(matches!(
            err,
            KanbanError::Domain(DomainError::NamespaceStillReferenced { count: 1, ref prefix }) if prefix == "KAN"
        ));
    }

    #[test]
    fn test_cards_with_an_empty_prefix_do_not_block_removing_the_empty_name() {
        let cards = vec![card_with_prefix("", 1)];
        assert!(super::ensure_namespace_unreferenced("", &cards).is_ok());
    }
}
