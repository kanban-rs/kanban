//! The single definition of how an existing workspace's boards and sprints
//! become `prefixes` rows.
//!
//! A prefix row is a NAMESPACE, not a possession: it owns the counters that
//! hand out the next card and sprint number for one name. Several boards may
//! point at the same row, and boards that never chose a prefix all point at
//! the default one — which is exactly what they already do today, since they
//! all resolve to the same effective prefix.
//!
//! That many-to-one shape is what makes this migration safe. A card's prefix
//! is stored at creation, so renaming a board's prefix cannot retroactively
//! repair identifiers its existing cards already carry; it would only change
//! what the board hands out next, splitting one board's cards across two
//! namespaces. Sharing the row changes nobody's prefix and still removes the
//! defect, because a single counter per namespace can no longer mint a
//! number twice.
//!
//! Backends previously derived rows independently and drifted, so the policy
//! lives here and each backend projects its own on-disk shape into
//! [`BackfillBoard`] / [`BackfillSprint`].

use std::collections::HashMap;

use uuid::Uuid;

use crate::prefix::Prefix;

/// The prefix a board falls back to when it never set one. Hardcoded
/// because a migration has no path to the CLI/service-level configured
/// default: it runs against files written by earlier versions, long before
/// any config is loaded.
pub const DEFAULT_CARD_PREFIX: &str = "task";

/// [`DEFAULT_CARD_PREFIX`]'s counterpart on the sprint-naming axis.
pub const DEFAULT_SPRINT_PREFIX: &str = "sprint";

/// A board's pre-migration prefix state, projected from whatever shape the
/// calling backend stores.
pub struct BackfillBoard {
    pub id: Uuid,
    pub card_prefix: Option<String>,
    pub sprint_prefix: Option<String>,
    /// The NEXT card number to hand out, seeded to 1 on a new board. The row
    /// it feeds is a high-water mark, so [`plan_prefix_backfill`] converts.
    pub card_counter: i64,
    /// Sprint counters keyed by the prefix they were recorded under, in
    /// whatever casing was current at the time. Matched normalised.
    ///
    /// These hold the NEXT number to hand out, not the last used. The row
    /// they feed is a high-water mark like [`BackfillBoard::card_counter`],
    /// so [`plan_prefix_backfill`] converts.
    pub sprint_counters: Vec<(String, i64)>,
}

/// A sprint that overrides its board's card prefix, allocating card numbers
/// from a namespace of its own.
pub struct BackfillSprint {
    pub card_prefix: String,
}

/// One namespace's row: the name, and the counters that allocate from it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackfillRow {
    pub name: String,
    pub card_counter: i64,
    pub sprint_counter: i64,
}

/// Computes one row per distinct normalised prefix in the workspace.
///
/// Where several boards share a name, each counter is the MAXIMUM across
/// them, never the sum: the counter is a high-water mark, and starting below
/// one would hand out a number some existing card already carries.
///
/// Rows come back sorted by name so every backend writes them in the same
/// order and two stores migrated from equivalent data compare equal.
pub fn plan_prefix_backfill(
    boards: &[BackfillBoard],
    sprints: &[BackfillSprint],
    default_card_prefix: &str,
    default_sprint_prefix: &str,
) -> Vec<BackfillRow> {
    let mut by_name: HashMap<String, BackfillRow> = HashMap::new();

    let mut raise = |name: String, card_counter: i64, sprint_counter: i64| {
        let row = by_name.entry(name.clone()).or_insert(BackfillRow {
            name,
            card_counter: 0,
            sprint_counter: 0,
        });
        row.card_counter = row.card_counter.max(card_counter);
        row.sprint_counter = row.sprint_counter.max(sprint_counter);
    };

    for b in boards {
        let card_name = Prefix::normalize(b.card_prefix.as_deref().unwrap_or(default_card_prefix));
        let sprint_name =
            Prefix::normalize(b.sprint_prefix.as_deref().unwrap_or(default_sprint_prefix));

        // Legacy counters are next-to-hand-out; rows are last-used. A board
        // with no entry, and one initialized to 1 without ever allocating,
        // both mean zero.
        let sprint_counter = b
            .sprint_counters
            .iter()
            .find(|(prefix, _)| Prefix::normalize(prefix) == sprint_name)
            .map(|(_, counter)| (*counter - 1).max(0))
            .unwrap_or(0);

        let card_counter = (b.card_counter - 1).max(0);

        if card_name == sprint_name {
            raise(card_name, card_counter, sprint_counter);
        } else {
            raise(card_name, card_counter, 0);
            raise(sprint_name, 0, sprint_counter);
        }
    }

    // A sprint override allocates from its own namespace, so the row must
    // exist even when no board points at it. Pre-V15 stores kept no
    // per-sprint card counter, so it contributes none.
    for s in sprints {
        raise(Prefix::normalize(&s.card_prefix), 0, 0);
    }

    let mut rows: Vec<BackfillRow> = by_name.into_values().collect();
    rows.sort_by(|a, b| a.name.cmp(&b.name));
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    fn board(
        card_prefix: Option<&str>,
        sprint_prefix: Option<&str>,
        card_counter: i64,
    ) -> BackfillBoard {
        BackfillBoard {
            id: Uuid::new_v4(),
            card_prefix: card_prefix.map(str::to_string),
            sprint_prefix: sprint_prefix.map(str::to_string),
            card_counter,
            sprint_counters: vec![],
        }
    }

    fn plan(boards: &[BackfillBoard], sprints: &[BackfillSprint]) -> Vec<BackfillRow> {
        plan_prefix_backfill(boards, sprints, DEFAULT_CARD_PREFIX, DEFAULT_SPRINT_PREFIX)
    }

    fn names(rows: &[BackfillRow]) -> Vec<&str> {
        rows.iter().map(|r| r.name.as_str()).collect()
    }

    #[test]
    fn test_plan_emits_one_row_per_distinct_name() {
        let rows = plan(&[board(Some("KAN"), Some("KAN"), 3)], &[]);
        assert_eq!(names(&rows), vec!["kan"]);
    }

    #[test]
    fn test_plan_separates_card_and_sprint_namespaces_when_they_differ() {
        let rows = plan(&[board(Some("DEV"), Some("REL"), 3)], &[]);
        assert_eq!(names(&rows), vec!["dev", "rel"]);
    }

    #[test]
    fn test_plan_shares_one_row_between_boards_that_never_chose_a_prefix() {
        let rows = plan(&[board(None, None, 4), board(None, None, 9)], &[]);

        assert_eq!(
            names(&rows),
            vec!["sprint", "task"],
            "unprefixed boards already resolve to the same effective prefix; \
             renaming one would change what it hands out next"
        );
    }

    #[test]
    fn test_plan_takes_the_maximum_counter_among_boards_sharing_a_name() {
        let rows = plan(&[board(None, None, 4), board(None, None, 9)], &[]);
        let task = rows.iter().find(|r| r.name == "task").unwrap();

        assert_eq!(
            task.card_counter, 8,
            "the counter is a high-water mark; starting below the highest would \
             re-mint a number an existing card already carries. Legacy 9 was \
             the next to hand out, so 8 was the last used"
        );
    }

    /// The sprint counter needs the same high-water rule as the card counter,
    /// and needs it MORE: sprints allocate from this row, so a shared row that
    /// started below the highest contributor would hand out a sprint number a
    /// board has already used.
    #[test]
    fn test_plan_takes_the_maximum_sprint_counter_among_boards_sharing_a_name() {
        let mut low = board(None, None, 0);
        low.sprint_counters = vec![("sprint".to_string(), 3)];
        let mut high = board(None, None, 0);
        high.sprint_counters = vec![("sprint".to_string(), 9)];

        let rows = plan(&[low, high], &[]);
        let sprint = rows.iter().find(|r| r.name == "sprint").unwrap();

        assert_eq!(
            sprint.sprint_counter, 8,
            "the shared sprint namespace must start at the highest number any \
             contributing board has USED (9 was next, so 8 was last), or the \
             next sprint re-uses a number"
        );
    }

    /// Order must not matter: a fold that overwrites rather than maximises
    /// passes when the highest happens to come last.
    #[test]
    fn test_plan_maximum_counters_are_independent_of_board_order() {
        let mut a = board(None, None, 9);
        a.sprint_counters = vec![("sprint".to_string(), 2)];
        let mut b = board(None, None, 4);
        b.sprint_counters = vec![("sprint".to_string(), 7)];

        let forward = plan(&[a, b], &[]);
        let mut a2 = board(None, None, 9);
        a2.sprint_counters = vec![("sprint".to_string(), 2)];
        let mut b2 = board(None, None, 4);
        b2.sprint_counters = vec![("sprint".to_string(), 7)];
        let reverse = plan(&[b2, a2], &[]);

        let counters = |rows: &[BackfillRow]| {
            rows.iter()
                .map(|r| (r.name.clone(), r.card_counter, r.sprint_counter))
                .collect::<Vec<_>>()
        };
        assert_eq!(counters(&forward), counters(&reverse));
        let task = forward.iter().find(|r| r.name == "task").unwrap();
        let sprint = forward.iter().find(|r| r.name == "sprint").unwrap();
        assert_eq!(
            (task.card_counter, sprint.sprint_counter),
            (8, 6),
            "each counter takes its own maximum, from whichever board held it"
        );
    }

    #[test]
    fn test_plan_shares_one_row_between_boards_explicitly_given_the_same_prefix() {
        let rows = plan(
            &[board(Some("alpha"), None, 2), board(Some("ALPHA"), None, 7)],
            &[],
        );
        let alpha = rows.iter().find(|r| r.name == "alpha").unwrap();

        assert_eq!(
            alpha.card_counter, 6,
            "legacy 7 next-to-hand-out is 6 last-used"
        );
        assert_eq!(
            names(&rows),
            vec!["alpha", "sprint"],
            "two boards deliberately set to one prefix are asking for one \
             namespace, not an error and not a rename"
        );
    }

    #[test]
    fn test_plan_never_invents_a_suffixed_name() {
        let rows = plan(
            &[
                board(None, None, 0),
                board(Some("task2"), None, 0),
                board(None, None, 0),
            ],
            &[],
        );

        assert_eq!(
            names(&rows),
            vec!["sprint", "task", "task2"],
            "colliding boards share a row, so no suffixed name is ever generated \
             and none can collide with a name a board explicitly holds"
        );
    }

    /// `board.card_counter` has the same next-to-hand-out meaning as
    /// `sprint_counters`, and starts at 1 on a brand-new board rather than 0.
    /// Copying it across verbatim makes the first card allocated after
    /// migrating skip a number: a board with cards 1..3 records a legacy 4,
    /// and a row reading 4 as last-used issues 5.
    #[test]
    fn test_plan_records_the_card_counter_as_the_last_number_used() {
        let rows = plan(&[board(Some("kan"), Some("kan"), 4)], &[]);

        assert_eq!(
            rows[0].card_counter, 3,
            "legacy 4 means 'next is 4', so 3 was the last used and 4 must \
             still be issued"
        );
    }

    #[test]
    fn test_plan_records_zero_for_a_board_that_has_allocated_no_card() {
        // `Board::new` seeds card_counter to 1, not 0.
        let rows = plan(&[board(Some("kan"), Some("kan"), 1)], &[]);

        assert_eq!(
            rows[0].card_counter, 0,
            "nothing allocated yet, so the first card must be number 1"
        );
    }

    /// `board.sprint_counters` stores the NEXT number to hand out, while
    /// `Prefix.card_counter` -- the field sitting beside `sprint_counter` in
    /// the same struct -- stores the LAST one used. Copying the legacy value
    /// across verbatim gives one struct two opposite meanings, and the first
    /// sprint allocated after migrating would skip a number forever.
    ///
    /// One struct, one meaning: both counters are high-water marks.
    #[test]
    fn test_plan_records_the_sprint_counter_as_the_last_number_used() {
        let mut b = board(None, None, 0);
        // Sprints 1 and 2 exist, so the legacy counter says "3 is next".
        b.sprint_counters = vec![("sprint".to_string(), 3)];

        let rows = plan(&[b], &[]);
        let sprint = rows.iter().find(|r| r.name == "sprint").unwrap();

        assert_eq!(
            sprint.sprint_counter, 2,
            "the row records the highest number USED, matching card_counter; \
             storing the legacy next-to-hand-out here makes the first sprint \
             after migration number 4 and 3 is never issued"
        );
    }

    /// A board that has never allocated a sprint has no legacy entry at all,
    /// and `initialize_sprint_counter` writes 1 for its first. Both mean
    /// "nothing used yet", which is 0 -- and 0 must not underflow.
    #[test]
    fn test_plan_records_zero_for_a_board_that_has_allocated_no_sprint() {
        let mut never = board(None, None, 0);
        never.sprint_counters = vec![];
        let mut initialized = board(Some("dev"), Some("dev"), 0);
        initialized.sprint_counters = vec![("dev".to_string(), 1)];

        let rows = plan(&[never, initialized], &[]);

        for name in ["sprint", "dev"] {
            let row = rows.iter().find(|r| r.name == name).unwrap();
            assert_eq!(
                row.sprint_counter, 0,
                "{name}: nothing allocated yet, so the next sprint must be 1"
            );
        }
    }

    #[test]
    fn test_plan_matches_sprint_counter_key_case_insensitively() {
        let mut b = board(Some("KAN"), Some("KAN"), 12);
        b.sprint_counters = vec![("kan".to_string(), 7)];

        let rows = plan(&[b], &[]);

        assert_eq!(
            rows[0].sprint_counter, 6,
            "the recorded key's casing need not match the board's current prefix"
        );
        assert_eq!(rows[0].card_counter, 11);
    }

    #[test]
    fn test_plan_emits_a_row_for_a_sprint_override_no_board_points_at() {
        let rows = plan(
            &[board(Some("KAN"), Some("KAN"), 0)],
            &[BackfillSprint {
                card_prefix: "AUTH".to_string(),
            }],
        );

        assert_eq!(names(&rows), vec!["auth", "kan"]);
    }
}
