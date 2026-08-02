use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::board::Board;
use crate::field_update::FieldUpdate;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SprintStatus {
    Planning,
    Active,
    Completed,
    Cancelled,
}

/// `Sprint` is not `Deserialize`: the only door from persisted bytes to a
/// `Sprint` is [`Sprint::reconstitute`] via [`crate::SprintRecord`]
/// (`sprint_serde`/`sprint_vec_serde` for JSON, `row_to_sprint` for SQLite). The
/// legacy `prefix_override` alias and the `card_prefix` default live on
/// `SprintRecord` instead.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Sprint {
    pub id: Uuid,
    pub board_id: Uuid,
    pub sprint_number: u32,
    pub name_index: Option<usize>,
    pub prefix: Option<String>,
    pub card_prefix: Option<String>,
    pub status: SprintStatus,
    pub start_date: Option<DateTime<Utc>>,
    pub end_date: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub type SprintId = Uuid;

impl Sprint {
    pub fn new(
        board_id: Uuid,
        sprint_number: u32,
        name_index: Option<usize>,
        prefix: Option<impl Into<String>>,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            board_id,
            sprint_number,
            name_index,
            prefix: prefix.map(Into::into),
            card_prefix: None,
            status: SprintStatus::Planning,
            start_date: None,
            end_date: None,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn get_name<'a>(&self, board: &'a Board) -> Option<&'a str> {
        self.name_index
            .and_then(|idx| board.sprint_names.get(idx))
            .map(|s| s.as_str())
    }

    pub fn effective_sprint_prefix<'a>(
        &'a self,
        board: &'a Board,
        default_prefix: &'a str,
    ) -> &'a str {
        self.prefix
            .as_deref()
            .or(board.sprint_prefix.as_deref())
            .unwrap_or(default_prefix)
    }

    pub fn effective_prefix<'a>(&'a self, board: &'a Board, default_prefix: &'a str) -> &'a str {
        self.effective_sprint_prefix(board, default_prefix)
    }

    pub fn effective_card_prefix<'a>(
        &'a self,
        board: &'a Board,
        default_prefix: &'a str,
    ) -> &'a str {
        self.card_prefix
            .as_deref()
            .or(board.card_prefix.as_deref())
            .unwrap_or(default_prefix)
    }

    pub fn formatted_name(&self, board: &Board, default_prefix: &str) -> String {
        let prefix = self.effective_sprint_prefix(board, default_prefix);
        match self.get_name(board) {
            Some(name) => format!("{}-{}/{}", prefix, self.sprint_number, name),
            None => format!("{}-{}", prefix, self.sprint_number),
        }
    }

    pub fn activate(&mut self, duration_days: u32) {
        self.status = SprintStatus::Active;
        let start = Utc::now();
        self.start_date = Some(start);
        self.end_date = Some(start + chrono::Duration::days(duration_days as i64));
        self.updated_at = Utc::now();
    }

    pub fn complete(&mut self) {
        self.status = SprintStatus::Completed;
        self.updated_at = Utc::now();
    }

    pub fn cancel(&mut self) {
        self.status = SprintStatus::Cancelled;
        self.updated_at = Utc::now();
    }

    pub fn update_name_index(&mut self, name_index: Option<usize>) {
        self.name_index = name_index;
        self.updated_at = Utc::now();
    }

    pub fn update_prefix(&mut self, prefix: Option<impl Into<String>>) {
        self.prefix = prefix.map(Into::into);
        self.updated_at = Utc::now();
    }

    pub fn update_card_prefix(&mut self, card_prefix: Option<impl Into<String>>) {
        self.card_prefix = card_prefix.map(Into::into);
        self.updated_at = Utc::now();
    }

    /// Returns true when this is an Active sprint whose `end_date` is in the past.
    ///
    /// Boundary: `now == end_date` returns `false` — the sprint's clock has
    /// not strictly passed. The bucket logic in
    /// [`Sprint::for_assignment_dialog`] calls this internally and inherits the
    /// same boundary semantics.
    pub fn is_ended(&self, now: DateTime<Utc>) -> bool {
        if self.status != SprintStatus::Active {
            return false;
        }
        if let Some(end_date) = self.end_date {
            now > end_date
        } else {
            false
        }
    }

    /// Splits sprints on `board_id` into two sections for the assignment dialog:
    /// `(active_or_planned, completed_or_ended)`.
    ///
    /// - **Active / planned**: `Planning` status, plus `Active` sprints whose
    ///   `end_date` is in the future (or `None`).
    /// - **Completed / ended**: `Completed` status, plus `Active` sprints whose
    ///   `end_date` has passed (i.e. `is_ended(now)` is true).
    /// - `Cancelled` sprints and sprints from other boards are excluded.
    ///
    /// Each section is sorted by `sprint_number` descending.
    pub fn for_assignment_dialog(
        sprints: &[Sprint],
        board_id: Uuid,
        now: DateTime<Utc>,
    ) -> (Vec<&Sprint>, Vec<&Sprint>) {
        let mut active = Vec::new();
        let mut ended = Vec::new();
        for s in sprints.iter().filter(|s| s.board_id == board_id) {
            match s.status {
                SprintStatus::Cancelled => {}
                SprintStatus::Planning => active.push(s),
                SprintStatus::Active => {
                    if s.is_ended(now) {
                        ended.push(s);
                    } else {
                        active.push(s);
                    }
                }
                SprintStatus::Completed => ended.push(s),
            }
        }
        active.sort_by_key(|s| std::cmp::Reverse(s.sprint_number));
        ended.sort_by_key(|s| std::cmp::Reverse(s.sprint_number));
        (active, ended)
    }

    /// Update sprint with partial changes
    pub fn update(&mut self, updates: SprintUpdate) {
        updates.name_index.apply_to(&mut self.name_index);
        updates.prefix.apply_to(&mut self.prefix);
        updates.card_prefix.apply_to(&mut self.card_prefix);
        if let Some(status) = updates.status {
            self.status = status;
        }
        updates.start_date.apply_to(&mut self.start_date);
        updates.end_date.apply_to(&mut self.end_date);
        self.updated_at = Utc::now();
    }
}

/// Partial update struct for Sprint
///
/// Uses `FieldUpdate<T>` for optional fields to provide clear three-state updates.
/// See [`FieldUpdate`] documentation for usage examples.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SprintUpdate {
    pub name: Option<String>,
    pub name_index: FieldUpdate<usize>,
    pub prefix: FieldUpdate<String>,
    pub card_prefix: FieldUpdate<String>,
    pub status: Option<SprintStatus>,
    pub start_date: FieldUpdate<DateTime<Utc>>,
    pub end_date: FieldUpdate<DateTime<Utc>>,
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

    #[test]
    fn test_sprint_new_accepts_str_prefix_without_to_string() {
        let board_id = uuid::Uuid::new_v4();
        let sprint = Sprint::new(board_id, 1, None, Some("sprint"));
        assert_eq!(sprint.prefix, Some("sprint".to_string()));
    }

    #[test]
    fn test_sprint_update_prefix_accepts_str_without_to_string() {
        let board_id = uuid::Uuid::new_v4();
        let mut sprint = Sprint::new(board_id, 1, None, None::<String>);
        sprint.update_prefix(Some("custom"));
        assert_eq!(sprint.prefix, Some("custom".to_string()));
    }

    #[test]
    fn test_sprint_update_card_prefix_accepts_str_without_to_string() {
        let board_id = uuid::Uuid::new_v4();
        let mut sprint = Sprint::new(board_id, 1, None, None::<String>);
        sprint.update_card_prefix(Some("KAN"));
        assert_eq!(sprint.card_prefix, Some("KAN".to_string()));
    }

    #[test]
    fn test_is_ended_returns_true_for_active_with_past_end_date() {
        let now = ts("2026-05-07T00:00:00Z");
        let s = make_sprint(
            1,
            Uuid::new_v4(),
            SprintStatus::Active,
            Some(ts("2026-05-06T00:00:00Z")),
        );
        assert!(s.is_ended(now));
    }

    #[test]
    fn test_is_ended_returns_false_for_active_with_future_end_date() {
        let now = ts("2026-05-07T00:00:00Z");
        let s = make_sprint(
            1,
            Uuid::new_v4(),
            SprintStatus::Active,
            Some(ts("2026-05-08T00:00:00Z")),
        );
        assert!(!s.is_ended(now));
    }

    #[test]
    fn test_is_ended_returns_false_for_completed_status_even_with_past_end_date() {
        let now = ts("2026-05-07T00:00:00Z");
        let s = make_sprint(
            1,
            Uuid::new_v4(),
            SprintStatus::Completed,
            Some(ts("2026-05-01T00:00:00Z")),
        );
        assert!(!s.is_ended(now));
    }

    #[test]
    fn test_is_ended_returns_false_for_active_with_no_end_date() {
        let now = ts("2026-05-07T00:00:00Z");
        let s = make_sprint(1, Uuid::new_v4(), SprintStatus::Active, None);
        assert!(!s.is_ended(now));
    }

    #[test]
    fn test_for_assignment_dialog_buckets_planning_into_active_section() {
        let now = ts("2026-05-07T00:00:00Z");
        let board = Uuid::new_v4();
        let s = make_sprint(1, board, SprintStatus::Planning, None);
        let sprints = vec![s.clone()];
        let (active, ended) = Sprint::for_assignment_dialog(&sprints, board, now);
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].id, s.id);
        assert_eq!(ended.len(), 0);
    }

    #[test]
    fn test_for_assignment_dialog_buckets_active_with_future_end_date_into_active_section() {
        let now = ts("2026-05-07T00:00:00Z");
        let board = Uuid::new_v4();
        let s = make_sprint(
            2,
            board,
            SprintStatus::Active,
            Some(ts("2026-05-20T00:00:00Z")),
        );
        let sprints = vec![s.clone()];
        let (active, ended) = Sprint::for_assignment_dialog(&sprints, board, now);
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].id, s.id);
        assert_eq!(ended.len(), 0);
    }

    #[test]
    fn test_for_assignment_dialog_buckets_active_with_past_end_date_into_completed_ended_section() {
        let now = ts("2026-05-07T00:00:00Z");
        let board = Uuid::new_v4();
        let s = make_sprint(
            3,
            board,
            SprintStatus::Active,
            Some(ts("2026-05-01T00:00:00Z")),
        );
        let sprints = vec![s.clone()];
        let (active, ended) = Sprint::for_assignment_dialog(&sprints, board, now);
        assert_eq!(active.len(), 0);
        assert_eq!(ended.len(), 1);
        assert_eq!(ended[0].id, s.id);
    }

    #[test]
    fn test_for_assignment_dialog_buckets_completed_into_completed_ended_section() {
        let now = ts("2026-05-07T00:00:00Z");
        let board = Uuid::new_v4();
        let s = make_sprint(
            4,
            board,
            SprintStatus::Completed,
            Some(ts("2026-04-01T00:00:00Z")),
        );
        let sprints = vec![s.clone()];
        let (active, ended) = Sprint::for_assignment_dialog(&sprints, board, now);
        assert_eq!(active.len(), 0);
        assert_eq!(ended.len(), 1);
        assert_eq!(ended[0].id, s.id);
    }

    #[test]
    fn test_for_assignment_dialog_excludes_cancelled_from_both_sections() {
        let now = ts("2026-05-07T00:00:00Z");
        let board = Uuid::new_v4();
        let s = make_sprint(5, board, SprintStatus::Cancelled, None);
        let sprints = vec![s];
        let (active, ended) = Sprint::for_assignment_dialog(&sprints, board, now);
        assert_eq!(active.len(), 0);
        assert_eq!(ended.len(), 0);
    }

    #[test]
    fn test_for_assignment_dialog_excludes_other_boards() {
        let now = ts("2026-05-07T00:00:00Z");
        let board = Uuid::new_v4();
        let other = Uuid::new_v4();
        let mine = make_sprint(1, board, SprintStatus::Planning, None);
        let theirs_active = make_sprint(2, other, SprintStatus::Planning, None);
        let theirs_completed = make_sprint(3, other, SprintStatus::Completed, None);
        let sprints = vec![mine.clone(), theirs_active, theirs_completed];
        let (active, ended) = Sprint::for_assignment_dialog(&sprints, board, now);
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].id, mine.id);
        assert_eq!(ended.len(), 0);
    }

    #[test]
    fn test_for_assignment_dialog_orders_each_section_by_sprint_number_descending() {
        let now = ts("2026-05-07T00:00:00Z");
        let board = Uuid::new_v4();
        let p1 = make_sprint(1, board, SprintStatus::Planning, None);
        let p3 = make_sprint(3, board, SprintStatus::Planning, None);
        let a2 = make_sprint(
            2,
            board,
            SprintStatus::Active,
            Some(ts("2026-05-20T00:00:00Z")),
        );
        let c10 = make_sprint(
            10,
            board,
            SprintStatus::Completed,
            Some(ts("2026-04-01T00:00:00Z")),
        );
        let c5 = make_sprint(
            5,
            board,
            SprintStatus::Completed,
            Some(ts("2026-03-01T00:00:00Z")),
        );
        let e7 = make_sprint(
            7,
            board,
            SprintStatus::Active,
            Some(ts("2026-05-01T00:00:00Z")),
        );
        let sprints = vec![
            p1.clone(),
            p3.clone(),
            a2.clone(),
            c10.clone(),
            c5.clone(),
            e7.clone(),
        ];
        let (active, ended) = Sprint::for_assignment_dialog(&sprints, board, now);
        assert_eq!(
            active.iter().map(|s| s.sprint_number).collect::<Vec<_>>(),
            vec![3, 2, 1]
        );
        assert_eq!(
            ended.iter().map(|s| s.sprint_number).collect::<Vec<_>>(),
            vec![10, 7, 5]
        );
    }
}
