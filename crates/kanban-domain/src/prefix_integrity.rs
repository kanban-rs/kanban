//! Referential-integrity rules between cards and prefix rows: is a namespace
//! a card names actually backed by a row, and is a namespace still
//! referenced by a card.

use std::collections::HashSet;

use crate::{Card, Prefix};

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
}
