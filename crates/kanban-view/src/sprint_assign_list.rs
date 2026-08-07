use crate::list_nav;
use chrono::{DateTime, Utc};
use kanban_domain::{Sprint, SprintStatus};
use uuid::Uuid;

/// Entry in the sprint-assignment dialog list. Headers are non-selectable;
/// `None` is the unassign option (always at index 0); the three sprint
/// variants carry the section context the renderer uses for styling.
pub enum SprintAssignEntry<'a> {
    Header(&'static str),
    None,
    ActiveOrPlanned(&'a Sprint),
    Completed(&'a Sprint),
    Ended(&'a Sprint),
}

impl SprintAssignEntry<'_> {
    /// True for rows the user can land on with arrow keys (i.e. not a
    /// section header). The single source of truth for this predicate so
    /// adding a new non-selectable variant only requires updating one place.
    pub fn is_selectable(&self) -> bool {
        !matches!(self, SprintAssignEntry::Header(_))
    }
}

pub const ACTIVE_PLANNED_HEADER: &str = "Active / Planned";
pub const COMPLETED_ENDED_HEADER: &str = "Completed / Ended";

/// Build the entry list for the new-card picker: same as
/// [`build_entries`], minus the Completed/Ended section. Completed and
/// Ended sprints aren't valid targets when creating a brand-new card,
/// so they don't appear at all (no row, no section header).
pub fn build_entries_active_only<'a>(
    sprints: &'a [Sprint],
    board_id: Uuid,
    now: DateTime<Utc>,
) -> Vec<SprintAssignEntry<'a>> {
    build_entries(sprints, board_id, now)
        .into_iter()
        .filter(|entry| {
            !matches!(
                entry,
                SprintAssignEntry::Header(COMPLETED_ENDED_HEADER)
                    | SprintAssignEntry::Completed(_)
                    | SprintAssignEntry::Ended(_)
            )
        })
        .collect()
}

/// Build the entry list for the dialog. Headers are emitted only when their
/// section is non-empty. The `(None)` entry is always at index 0.
pub fn build_entries<'a>(
    sprints: &'a [Sprint],
    board_id: Uuid,
    now: DateTime<Utc>,
) -> Vec<SprintAssignEntry<'a>> {
    let (active, completed_or_ended) = Sprint::for_assignment_dialog(sprints, board_id, now);
    let active_header = if active.is_empty() { 0 } else { 1 };
    let lower_header = if completed_or_ended.is_empty() { 0 } else { 1 };
    let mut entries: Vec<SprintAssignEntry<'a>> = Vec::with_capacity(
        1 + active.len() + completed_or_ended.len() + active_header + lower_header,
    );
    entries.push(SprintAssignEntry::None);
    if !active.is_empty() {
        entries.push(SprintAssignEntry::Header(ACTIVE_PLANNED_HEADER));
        for s in active {
            entries.push(SprintAssignEntry::ActiveOrPlanned(s));
        }
    }
    if !completed_or_ended.is_empty() {
        entries.push(SprintAssignEntry::Header(COMPLETED_ENDED_HEADER));
        for s in completed_or_ended {
            if s.status == SprintStatus::Completed {
                entries.push(SprintAssignEntry::Completed(s));
            } else {
                entries.push(SprintAssignEntry::Ended(s));
            }
        }
    }
    entries
}

/// Move selection to the next selectable entry, skipping headers.
/// Clamps at the last selectable entry. Returns `None` if no selectable
/// entries exist.
pub fn next_selectable(entries: &[SprintAssignEntry], cur: Option<usize>) -> Option<usize> {
    list_nav::next_selectable_index(cur, entries.len(), |i| entries[i].is_selectable())
}

/// Move selection to the previous selectable entry, skipping headers.
/// Clamps at the first selectable entry. Returns `None` if no selectable
/// entries exist.
pub fn prev_selectable(entries: &[SprintAssignEntry], cur: Option<usize>) -> Option<usize> {
    list_nav::prev_selectable_index(cur, entries.len(), |i| entries[i].is_selectable())
}

/// Returns the `(header_index, label)` of the section header that
/// encloses `entries[idx]` — the closest `Header` walking backwards.
/// Returns `None` when `idx` is out of bounds, points at the (None)
/// entry at index 0, or precedes the first header.
pub fn section_header_for(
    entries: &[SprintAssignEntry],
    idx: usize,
) -> Option<(usize, &'static str)> {
    if idx >= entries.len() {
        return None;
    }
    for i in (0..idx).rev() {
        if let SprintAssignEntry::Header(label) = entries[i] {
            return Some((i, label));
        }
    }
    None
}

/// Returns the sprint id for a sprint-bearing entry, or `None` for
/// `Header` and `None` entries.
pub fn sprint_id_of(entry: &SprintAssignEntry) -> Option<Uuid> {
    match entry {
        SprintAssignEntry::ActiveOrPlanned(s)
        | SprintAssignEntry::Completed(s)
        | SprintAssignEntry::Ended(s) => Some(s.id),
        SprintAssignEntry::Header(_) | SprintAssignEntry::None => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ts(s: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(s).unwrap().with_timezone(&Utc)
    }

    fn make_sprint(
        sprint_number: u32,
        board_id: Uuid,
        status: SprintStatus,
        end_date: Option<DateTime<Utc>>,
    ) -> Sprint {
        Sprint {
            id: Uuid::new_v4(),
            board_id,
            sprint_number,
            name_index: None,
            prefix: None,
            card_prefix: None,
            status,
            start_date: None,
            end_date,
            created_at: ts("2026-01-01T00:00:00Z"),
            updated_at: ts("2026-01-01T00:00:00Z"),
        }
    }

    fn is_none_entry(entry: &SprintAssignEntry) -> bool {
        matches!(entry, SprintAssignEntry::None)
    }

    fn header_label<'a>(entry: &SprintAssignEntry<'a>) -> Option<&'static str> {
        match entry {
            SprintAssignEntry::Header(label) => Some(*label),
            _ => None,
        }
    }

    #[test]
    fn test_build_entries_always_includes_none_at_index_0() {
        let now = ts("2026-05-07T00:00:00Z");
        let board = Uuid::new_v4();
        let entries = build_entries(&[], board, now);
        assert!(
            !entries.is_empty(),
            "entries must contain at least the None option"
        );
        assert!(
            is_none_entry(&entries[0]),
            "index 0 must be the None unassign entry"
        );
    }

    #[test]
    fn test_build_entries_omits_active_section_header_when_no_active_or_planned() {
        let now = ts("2026-05-07T00:00:00Z");
        let board = Uuid::new_v4();
        let sprints = [make_sprint(
            1,
            board,
            SprintStatus::Completed,
            Some(ts("2026-04-01T00:00:00Z")),
        )];
        let entries = build_entries(&sprints, board, now);
        assert!(
            entries
                .iter()
                .all(|e| header_label(e) != Some(ACTIVE_PLANNED_HEADER)),
            "active/planned header must be omitted when section is empty"
        );
        assert!(
            entries
                .iter()
                .any(|e| header_label(e) == Some(COMPLETED_ENDED_HEADER)),
            "completed/ended header must appear when section is non-empty"
        );
    }

    #[test]
    fn test_build_entries_omits_completed_ended_section_header_when_section_empty() {
        let now = ts("2026-05-07T00:00:00Z");
        let board = Uuid::new_v4();
        let sprints = [make_sprint(1, board, SprintStatus::Planning, None)];
        let entries = build_entries(&sprints, board, now);
        assert!(
            entries
                .iter()
                .any(|e| header_label(e) == Some(ACTIVE_PLANNED_HEADER)),
            "active/planned header must appear when section is non-empty"
        );
        assert!(
            entries
                .iter()
                .all(|e| header_label(e) != Some(COMPLETED_ENDED_HEADER)),
            "completed/ended header must be omitted when section is empty"
        );
    }

    #[test]
    fn test_build_entries_emits_completed_and_ended_in_same_section() {
        let now = ts("2026-05-07T00:00:00Z");
        let board = Uuid::new_v4();
        let sprints = [
            make_sprint(
                5,
                board,
                SprintStatus::Completed,
                Some(ts("2026-04-01T00:00:00Z")),
            ),
            make_sprint(
                7,
                board,
                SprintStatus::Active,
                Some(ts("2026-05-01T00:00:00Z")),
            ),
        ];
        let entries = build_entries(&sprints, board, now);

        // Layout expected: [None, Header("Completed / Ended"), Ended(7), Completed(5)]
        // (sorted desc by sprint_number within section)
        let header_idx = entries
            .iter()
            .position(|e| header_label(e) == Some(COMPLETED_ENDED_HEADER))
            .expect("completed/ended header present");
        let after = &entries[header_idx + 1..];
        let mut saw_completed = false;
        let mut saw_ended = false;
        for e in after {
            match e {
                SprintAssignEntry::Completed(_) => saw_completed = true,
                SprintAssignEntry::Ended(_) => saw_ended = true,
                SprintAssignEntry::Header(_) => panic!("no second header expected"),
                _ => {}
            }
        }
        assert!(
            saw_completed && saw_ended,
            "section must contain both Completed and Ended entries"
        );
    }

    #[test]
    fn test_next_selectable_skips_headers() {
        // Build a fixture: [None, Header("A"), Sprint, Header("B"), Sprint]
        let board = Uuid::new_v4();
        let s1 = make_sprint(1, board, SprintStatus::Planning, None);
        let s2 = make_sprint(
            2,
            board,
            SprintStatus::Completed,
            Some(ts("2026-04-01T00:00:00Z")),
        );
        let entries = vec![
            SprintAssignEntry::None,
            SprintAssignEntry::Header(ACTIVE_PLANNED_HEADER),
            SprintAssignEntry::ActiveOrPlanned(&s1),
            SprintAssignEntry::Header(COMPLETED_ENDED_HEADER),
            SprintAssignEntry::Completed(&s2),
        ];
        // From None (idx 0) → next must skip the header at 1, land on 2
        assert_eq!(next_selectable(&entries, Some(0)), Some(2));
        // From idx 2 → next must skip header at 3, land on 4
        assert_eq!(next_selectable(&entries, Some(2)), Some(4));
    }

    #[test]
    fn test_prev_selectable_skips_headers() {
        let board = Uuid::new_v4();
        let s1 = make_sprint(1, board, SprintStatus::Planning, None);
        let s2 = make_sprint(
            2,
            board,
            SprintStatus::Completed,
            Some(ts("2026-04-01T00:00:00Z")),
        );
        let entries = vec![
            SprintAssignEntry::None,
            SprintAssignEntry::Header(ACTIVE_PLANNED_HEADER),
            SprintAssignEntry::ActiveOrPlanned(&s1),
            SprintAssignEntry::Header(COMPLETED_ENDED_HEADER),
            SprintAssignEntry::Completed(&s2),
        ];
        // From idx 4 → prev must skip header at 3, land on 2
        assert_eq!(prev_selectable(&entries, Some(4)), Some(2));
        // From idx 2 → prev must skip header at 1, land on 0
        assert_eq!(prev_selectable(&entries, Some(2)), Some(0));
    }

    #[test]
    fn test_next_selectable_clamps_at_last_selectable() {
        let board = Uuid::new_v4();
        let s1 = make_sprint(1, board, SprintStatus::Planning, None);
        let entries = vec![
            SprintAssignEntry::None,
            SprintAssignEntry::Header(ACTIVE_PLANNED_HEADER),
            SprintAssignEntry::ActiveOrPlanned(&s1),
        ];
        // Already at last selectable (idx 2); next should stay at 2
        assert_eq!(next_selectable(&entries, Some(2)), Some(2));
    }

    #[test]
    fn test_prev_selectable_clamps_at_first_selectable() {
        let board = Uuid::new_v4();
        let s1 = make_sprint(
            1,
            board,
            SprintStatus::Completed,
            Some(ts("2026-04-01T00:00:00Z")),
        );
        let entries = vec![
            SprintAssignEntry::None,
            SprintAssignEntry::Header(COMPLETED_ENDED_HEADER),
            SprintAssignEntry::Completed(&s1),
        ];
        // Already at first selectable (idx 0); prev should stay at 0
        assert_eq!(prev_selectable(&entries, Some(0)), Some(0));
    }

    #[test]
    fn test_sprint_id_of_returns_id_for_sprint_entries() {
        let board = Uuid::new_v4();
        let s = make_sprint(1, board, SprintStatus::Planning, None);
        let active = SprintAssignEntry::ActiveOrPlanned(&s);
        let completed = SprintAssignEntry::Completed(&s);
        let ended = SprintAssignEntry::Ended(&s);
        assert_eq!(sprint_id_of(&active), Some(s.id));
        assert_eq!(sprint_id_of(&completed), Some(s.id));
        assert_eq!(sprint_id_of(&ended), Some(s.id));
    }

    #[test]
    fn test_sprint_id_of_returns_none_for_header_and_none_entries() {
        let header = SprintAssignEntry::Header(ACTIVE_PLANNED_HEADER);
        let none = SprintAssignEntry::None;
        assert_eq!(sprint_id_of(&header), None);
        assert_eq!(sprint_id_of(&none), None);
    }

    #[test]
    fn test_section_header_for_returns_none_for_index_zero() {
        let entries = vec![SprintAssignEntry::None];
        assert_eq!(section_header_for(&entries, 0), None);
    }

    #[test]
    fn test_section_header_for_returns_none_for_out_of_bounds() {
        let entries: Vec<SprintAssignEntry> = vec![SprintAssignEntry::None];
        assert_eq!(section_header_for(&entries, 5), None);
    }

    #[test]
    fn test_section_header_for_returns_active_header_for_entry_in_active_section() {
        let board = Uuid::new_v4();
        let s = make_sprint(1, board, SprintStatus::Planning, None);
        let entries = vec![
            SprintAssignEntry::None,
            SprintAssignEntry::Header(ACTIVE_PLANNED_HEADER),
            SprintAssignEntry::ActiveOrPlanned(&s),
        ];
        assert_eq!(
            section_header_for(&entries, 2),
            Some((1, ACTIVE_PLANNED_HEADER))
        );
    }

    #[test]
    fn test_section_header_for_returns_completed_header_for_entry_in_lower_section() {
        let board = Uuid::new_v4();
        let s_active = make_sprint(1, board, SprintStatus::Planning, None);
        let s_completed = make_sprint(
            2,
            board,
            SprintStatus::Completed,
            Some(ts("2026-04-01T00:00:00Z")),
        );
        let entries = vec![
            SprintAssignEntry::None,
            SprintAssignEntry::Header(ACTIVE_PLANNED_HEADER),
            SprintAssignEntry::ActiveOrPlanned(&s_active),
            SprintAssignEntry::Header(COMPLETED_ENDED_HEADER),
            SprintAssignEntry::Completed(&s_completed),
        ];
        assert_eq!(
            section_header_for(&entries, 4),
            Some((3, COMPLETED_ENDED_HEADER))
        );
    }
}
