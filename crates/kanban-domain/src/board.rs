use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use crate::field_update::FieldUpdate;
use crate::task_list_view::TaskListView;

pub type BoardId = Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SortField {
    Points,
    Priority,
    CreatedAt,
    UpdatedAt,
    DueDate,
    Status,
    Position,
    Default,
}

/// Sort dimensions for the BOARDS list (the projects panel / archived-boards
/// view). Deliberately a SEPARATE type from the card [`SortField`]: `ArchivedAt`
/// is a board-view-only dimension (the archival marker carries the timestamp;
/// the entity head does not), and it must never be representable as a board's
/// card `task_sort_field` nor round-trip through the card-sort DTO. Keeping it
/// its own enum makes that illegal state unrepresentable by construction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BoardSortField {
    /// Board order (`Board::position`).
    Position,
    /// Board name, compared case-insensitively.
    Name,
    /// Board creation time (`Board::created_at`).
    CreatedAt,
    /// Recency of archival, resolved from the archival marker's timestamp.
    ArchivedAt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SortOrder {
    Ascending,
    Descending,
}

impl SortOrder {
    /// The opposite direction. The single definition of the asc↔desc flip,
    /// reused by every sort-order toggle (card list, archived-boards list).
    #[must_use]
    pub fn toggled(self) -> Self {
        match self {
            SortOrder::Ascending => SortOrder::Descending,
            SortOrder::Descending => SortOrder::Ascending,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Board {
    pub id: BoardId,
    pub name: String,
    pub description: Option<String>,
    pub sprint_prefix: Option<String>,
    pub card_prefix: Option<String>,
    pub task_sort_field: SortField,
    pub task_sort_order: SortOrder,
    pub sprint_duration_days: Option<u32>,
    pub sprint_names: Vec<String>,
    pub sprint_name_used_count: usize,
    pub next_sprint_number: u32,
    pub active_sprint_id: Option<Uuid>,
    pub task_list_view: TaskListView,
    pub card_counter: u32,
    pub sprint_counters: HashMap<String, u32>,
    pub completion_column_id: Option<Uuid>,
    pub position: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub(crate) fn default_next_sprint_number() -> u32 {
    1
}

pub(crate) fn default_sort_field() -> SortField {
    SortField::Default
}

pub(crate) fn default_sort_order() -> SortOrder {
    SortOrder::Ascending
}

impl Board {
    pub fn new(name: impl Into<String>, card_prefix: Option<impl Into<String>>) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            description: None,
            sprint_prefix: None,
            card_prefix: card_prefix.map(Into::into),
            task_sort_field: SortField::Default,
            task_sort_order: SortOrder::Ascending,
            sprint_duration_days: None,
            sprint_names: Vec::new(),
            sprint_name_used_count: 0,
            next_sprint_number: 1,
            active_sprint_id: None,
            task_list_view: TaskListView::default(),
            card_counter: 1,
            sprint_counters: HashMap::new(),
            completion_column_id: None,
            position: 0,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn update_name(&mut self, name: impl Into<String>) {
        self.name = name.into();
        self.updated_at = Utc::now();
    }

    pub fn update_description(&mut self, description: Option<impl Into<String>>) {
        self.description = description.map(Into::into);
        self.updated_at = Utc::now();
    }

    pub fn update_sprint_prefix(&mut self, prefix: Option<impl Into<String>>) {
        self.sprint_prefix = prefix.map(Into::into);
        self.updated_at = Utc::now();
    }

    pub fn update_card_prefix(&mut self, prefix: Option<impl Into<String>>) {
        self.card_prefix = prefix.map(Into::into);
        self.updated_at = Utc::now();
    }

    pub fn effective_sprint_prefix<'a>(&'a self, default_prefix: &'a str) -> &'a str {
        self.sprint_prefix.as_deref().unwrap_or(default_prefix)
    }

    pub fn effective_card_prefix<'a>(&'a self, default_prefix: &'a str) -> &'a str {
        self.card_prefix.as_deref().unwrap_or(default_prefix)
    }

    pub fn effective_branch_prefix<'a>(&'a self, default_prefix: &'a str) -> &'a str {
        self.effective_sprint_prefix(default_prefix)
    }

    pub fn update_task_sort(&mut self, field: SortField, order: SortOrder) {
        self.task_sort_field = field;
        self.task_sort_order = order;
        self.updated_at = Utc::now();
    }

    pub fn allocate_sprint_number(&mut self) -> u32 {
        let number = self.next_sprint_number;
        self.next_sprint_number += 1;
        self.updated_at = Utc::now();
        number
    }

    pub fn consume_sprint_name(&mut self) -> Option<usize> {
        if self.sprint_name_used_count < self.sprint_names.len() {
            let index = self.sprint_name_used_count;
            self.sprint_name_used_count += 1;
            self.updated_at = Utc::now();
            Some(index)
        } else {
            None
        }
    }

    pub fn add_sprint_name_at_used_index(&mut self, name: impl Into<String>) -> usize {
        if self.sprint_name_used_count > self.sprint_names.len() {
            self.sprint_name_used_count = self.sprint_names.len();
        }
        let index = self.sprint_name_used_count;
        self.sprint_names.insert(index, name.into());
        self.sprint_name_used_count += 1;
        self.updated_at = Utc::now();
        index
    }

    pub fn update_task_list_view(&mut self, view: TaskListView) {
        self.task_list_view = view;
        self.updated_at = Utc::now();
    }

    /// Get the next card number and increment the counter.
    pub fn get_next_card_number(&mut self) -> u32 {
        let number = self.card_counter;
        self.card_counter += 1;
        self.updated_at = Utc::now();
        number
    }

    /// Set the card counter to a specific start value (used for import/migration).
    pub fn initialize_card_counter(&mut self, start: u32) {
        self.card_counter = start;
        self.updated_at = Utc::now();
    }

    /// Get the current card counter value (next number to be assigned).
    pub fn get_card_counter(&self) -> u32 {
        self.card_counter
    }

    pub fn get_next_sprint_number(&mut self, prefix: &str) -> u32 {
        let counter = self.sprint_counters.entry(prefix.to_string()).or_insert(1);
        let number = *counter;
        *counter += 1;
        self.updated_at = Utc::now();
        number
    }

    pub fn initialize_sprint_counter(&mut self, prefix: &str, start: u32) {
        self.sprint_counters.insert(prefix.to_string(), start);
        self.updated_at = Utc::now();
    }

    pub fn get_sprint_counters(&self) -> &HashMap<String, u32> {
        &self.sprint_counters
    }

    pub fn get_sprint_counter(&self, prefix: &str) -> Option<u32> {
        self.sprint_counters.get(prefix).copied()
    }

    pub fn resolve_completion_column(&self, columns: &[crate::Column]) -> Option<Uuid> {
        if let Some(id) = self.completion_column_id {
            if columns.iter().any(|c| c.id == id && c.board_id == self.id) {
                return Some(id);
            }
        }
        // Fallback: last column by position for this board
        columns
            .iter()
            .filter(|c| c.board_id == self.id)
            .max_by_key(|c| c.position)
            .map(|c| c.id)
    }

    pub fn update_completion_column_id(&mut self, column_id: Option<Uuid>) {
        self.completion_column_id = column_id;
        self.updated_at = Utc::now();
    }

    pub fn ensure_sprint_counter_initialized(
        &mut self,
        prefix: &str,
        all_sprints: &[crate::Sprint],
    ) {
        // If counter already exists for this prefix, don't reinitialize
        if self.sprint_counters.contains_key(prefix) {
            return;
        }

        // Find the highest sprint number with this prefix FOR THIS BOARD
        let max_number = all_sprints
            .iter()
            .filter(|sprint| {
                // Only consider sprints for this board
                if sprint.board_id != self.id {
                    return false;
                }

                let sprint_prefix = sprint
                    .prefix
                    .as_deref()
                    .unwrap_or_else(|| self.sprint_prefix.as_deref().unwrap_or("sprint"));
                sprint_prefix == prefix
            })
            .map(|sprint| sprint.sprint_number)
            .max()
            .unwrap_or(0);

        // Initialize counter to one more than the max, or 1 if no sprints exist
        let next_number = max_number + 1;
        self.initialize_sprint_counter(prefix, next_number);
    }

    /// Update board with partial changes
    pub fn update(&mut self, updates: BoardUpdate) {
        if let Some(name) = updates.name {
            self.name = name;
        }
        updates.description.apply_to(&mut self.description);
        updates.sprint_prefix.apply_to(&mut self.sprint_prefix);
        updates.card_prefix.apply_to(&mut self.card_prefix);
        if let Some(task_sort_field) = updates.task_sort_field {
            self.task_sort_field = task_sort_field;
        }
        if let Some(task_sort_order) = updates.task_sort_order {
            self.task_sort_order = task_sort_order;
        }
        updates
            .sprint_duration_days
            .apply_to(&mut self.sprint_duration_days);
        if let Some(task_list_view) = updates.task_list_view {
            self.task_list_view = task_list_view;
        }
        updates
            .active_sprint_id
            .apply_to(&mut self.active_sprint_id);
        updates
            .completion_column_id
            .apply_to(&mut self.completion_column_id);
        if let Some(position) = updates.position {
            self.position = position;
        }
        self.updated_at = Utc::now();
    }
}

/// Partial update struct for Board
///
/// Uses `FieldUpdate<T>` for optional fields to provide clear three-state updates.
/// See [`FieldUpdate`] documentation for usage examples.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct BoardUpdate {
    pub name: Option<String>,
    pub description: FieldUpdate<String>,
    pub sprint_prefix: FieldUpdate<String>,
    pub card_prefix: FieldUpdate<String>,
    pub task_sort_field: Option<SortField>,
    pub task_sort_order: Option<SortOrder>,
    pub sprint_duration_days: FieldUpdate<u32>,
    pub task_list_view: Option<TaskListView>,
    pub active_sprint_id: FieldUpdate<Uuid>,
    pub completion_column_id: FieldUpdate<Uuid>,
    pub position: Option<i32>,
}

/// Get the active sprint's card prefix override if one exists.
/// Returns the sprint's card_prefix if the board has an active sprint
/// that has a card_prefix override set.
pub fn get_active_sprint_card_prefix_override<'a>(
    board: &'a Board,
    sprints: &'a [crate::Sprint],
) -> Option<&'a str> {
    board.active_sprint_id.and_then(|sprint_id| {
        sprints
            .iter()
            .find(|s| s.id == sprint_id)
            .and_then(|sprint| sprint.card_prefix.as_deref())
    })
}

/// Get the active sprint's sprint prefix override if one exists.
/// Returns the sprint's prefix if the board has an active sprint
/// that has a sprint prefix override set.
pub fn get_active_sprint_prefix_override<'a>(
    board: &'a Board,
    sprints: &'a [crate::Sprint],
) -> Option<&'a str> {
    board.active_sprint_id.and_then(|sprint_id| {
        sprints
            .iter()
            .find(|s| s.id == sprint_id)
            .and_then(|sprint| sprint.prefix.as_deref())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_board_sort_field_fromstr_display_roundtrip() {
        for field in [
            BoardSortField::Position,
            BoardSortField::Name,
            BoardSortField::CreatedAt,
            BoardSortField::ArchivedAt,
        ] {
            let canonical = field.to_string();
            let parsed: BoardSortField = canonical.parse().expect("canonical form must parse");
            assert_eq!(parsed, field, "Display->FromStr must be the identity");
        }
        assert_eq!(BoardSortField::Position.to_string(), "position");
        assert_eq!(BoardSortField::Name.to_string(), "name");
        assert_eq!(BoardSortField::CreatedAt.to_string(), "created_at");
        assert_eq!(BoardSortField::ArchivedAt.to_string(), "archived_at");
    }

    #[test]
    fn test_board_sort_field_fromstr_is_tolerant() {
        for s in ["Created_At", "created-at", "CREATEDAT", "createdAt"] {
            assert_eq!(
                s.parse::<BoardSortField>().unwrap(),
                BoardSortField::CreatedAt,
                "{s} should parse to CreatedAt"
            );
        }
        for s in ["Archived_At", "archived-at", "ARCHIVEDAT"] {
            assert_eq!(
                s.parse::<BoardSortField>().unwrap(),
                BoardSortField::ArchivedAt,
                "{s} should parse to ArchivedAt"
            );
        }
        assert_eq!(
            "POSITION".parse::<BoardSortField>().unwrap(),
            BoardSortField::Position
        );
        assert_eq!(
            "Name".parse::<BoardSortField>().unwrap(),
            BoardSortField::Name
        );
    }

    #[test]
    fn test_board_sort_field_fromstr_rejects_unknown() {
        assert!("".parse::<BoardSortField>().is_err());
        assert!("bogus".parse::<BoardSortField>().is_err());
        assert!("updated_at".parse::<BoardSortField>().is_err());
    }

    #[test]
    fn test_sort_order_fromstr_display_roundtrip() {
        for order in [SortOrder::Ascending, SortOrder::Descending] {
            let canonical = order.to_string();
            let parsed: SortOrder = canonical.parse().expect("canonical form must parse");
            assert_eq!(parsed, order, "Display->FromStr must be the identity");
        }
        assert_eq!(SortOrder::Ascending.to_string(), "ascending");
        assert_eq!(SortOrder::Descending.to_string(), "descending");
    }

    #[test]
    fn test_sort_order_fromstr_is_tolerant() {
        for s in ["asc", "ASC", "Ascending", "ascending"] {
            assert_eq!(
                s.parse::<SortOrder>().unwrap(),
                SortOrder::Ascending,
                "{s} should parse to Ascending"
            );
        }
        for s in ["desc", "DESC", "Descending", "descending"] {
            assert_eq!(
                s.parse::<SortOrder>().unwrap(),
                SortOrder::Descending,
                "{s} should parse to Descending"
            );
        }
    }

    #[test]
    fn test_sort_order_fromstr_rejects_unknown() {
        assert!("".parse::<SortOrder>().is_err());
        assert!("up".parse::<SortOrder>().is_err());
        assert!("ascend".parse::<SortOrder>().is_err());
    }

    #[test]
    fn test_default_board_sort_consts() {
        assert_eq!(
            DEFAULT_BOARD_SORT_LIVE,
            (BoardSortField::Position, SortOrder::Ascending)
        );
        assert_eq!(
            DEFAULT_ARCHIVED_BOARD_SORT,
            (BoardSortField::ArchivedAt, SortOrder::Descending)
        );
    }

    #[test]
    fn test_sort_order_toggled_flips_direction_and_is_involutive() {
        assert_eq!(SortOrder::Ascending.toggled(), SortOrder::Descending);
        assert_eq!(SortOrder::Descending.toggled(), SortOrder::Ascending);
        // Two toggles return to the start (the single flip definition).
        assert_eq!(
            SortOrder::Ascending.toggled().toggled(),
            SortOrder::Ascending
        );
    }

    #[test]
    fn test_board_new_accepts_str_literal_without_to_string() {
        let board = Board::new("my-board", Some("KAN"));
        assert_eq!(board.name, "my-board");
        assert_eq!(board.card_prefix, Some("KAN".to_string()));
    }

    #[test]
    fn test_board_update_name_accepts_str_without_to_string() {
        let mut board = Board::new("initial", None::<String>);
        board.update_name("updated");
        assert_eq!(board.name, "updated");
    }

    #[test]
    fn test_board_update_description_accepts_str_without_to_string() {
        let mut board = Board::new("board", None::<String>);
        board.update_description(Some("desc"));
        assert_eq!(board.description, Some("desc".to_string()));
    }

    #[test]
    fn test_board_update_sprint_prefix_accepts_str_without_to_string() {
        let mut board = Board::new("board", None::<String>);
        board.update_sprint_prefix(Some("sprint"));
        assert_eq!(board.sprint_prefix, Some("sprint".to_string()));
    }

    #[test]
    fn test_board_update_card_prefix_accepts_str_without_to_string() {
        let mut board = Board::new("board", None::<String>);
        board.update_card_prefix(Some("KAN"));
        assert_eq!(board.card_prefix, Some("KAN".to_string()));
    }

    #[test]
    fn test_board_add_sprint_name_at_used_index_accepts_str_without_to_string() {
        let mut board = Board::new("board", None::<String>);
        let idx = board.add_sprint_name_at_used_index("Alpha");
        assert_eq!(board.sprint_names[idx], "Alpha");
    }

    #[test]
    fn test_board_new_card_counter_initialized_to_one() {
        let board = Board::new("Test", None::<String>);
        assert_eq!(board.card_counter, 1);
    }

    #[test]
    fn test_board_get_next_card_number_increments_without_prefix() {
        let mut board = Board::new("Test", None::<String>);
        assert_eq!(board.get_next_card_number(), 1);
        assert_eq!(board.get_next_card_number(), 2);
        assert_eq!(board.get_next_card_number(), 3);
        assert_eq!(board.card_counter, 4);
    }

    #[test]
    fn test_board_initialize_card_counter_sets_value_and_get_next_returns_it() {
        let mut board = Board::new("Test", None::<String>);
        board.initialize_card_counter(10);
        assert_eq!(board.get_card_counter(), 10);
        assert_eq!(board.get_next_card_number(), 10);
        assert_eq!(board.get_card_counter(), 11);
    }

    // The four card-counter migration tests moved to `board_factory.rs` as
    // `test_board_record_deserialize_*`, since the migration logic now lives on
    // `BoardRecord`'s hand-written `Deserialize` (`Board` no longer deserializes).

    #[test]
    fn test_update_sprint_prefix() {
        let mut board = Board::new("Test", None::<String>);
        assert_eq!(board.sprint_prefix, None);

        board.update_sprint_prefix(Some("feat"));
        assert_eq!(board.sprint_prefix, Some("feat".to_string()));

        board.update_sprint_prefix(None::<String>);
        assert_eq!(board.sprint_prefix, None);
    }

    #[test]
    fn test_update_card_prefix() {
        let mut board = Board::new("Test", None::<String>);
        assert_eq!(board.card_prefix, None);

        board.update_card_prefix(Some("task"));
        assert_eq!(board.card_prefix, Some("task".to_string()));

        board.update_card_prefix(None::<String>);
        assert_eq!(board.card_prefix, None);
    }

    #[test]
    fn test_effective_sprint_prefix() {
        let mut board = Board::new("Test", None::<String>);
        assert_eq!(board.effective_sprint_prefix("default"), "default");

        board.update_sprint_prefix(Some("custom"));
        assert_eq!(board.effective_sprint_prefix("default"), "custom");
    }

    #[test]
    fn test_effective_card_prefix() {
        let mut board = Board::new("Test", None::<String>);
        assert_eq!(board.effective_card_prefix("task"), "task");

        board.update_card_prefix(Some("feature"));
        assert_eq!(board.effective_card_prefix("task"), "feature");
    }

    #[test]
    fn test_effective_branch_prefix_backward_compat() {
        let mut board = Board::new("Test", None::<String>);
        board.update_sprint_prefix(Some("custom"));
        assert_eq!(board.effective_branch_prefix("default"), "custom");
    }

    #[test]
    fn test_resolve_completion_column_fallback() {
        let board = Board::new("Test", None::<String>);
        let col1 = crate::Column::new(board.id, "Todo", 0);
        let col2 = crate::Column::new(board.id, "In Progress", 1);
        let col3 = crate::Column::new(board.id, "Done", 2);
        let columns = vec![col1, col2, col3.clone()];

        assert_eq!(board.resolve_completion_column(&columns), Some(col3.id));
    }

    #[test]
    fn test_resolve_completion_column_explicit() {
        let mut board = Board::new("Test", None::<String>);
        let col1 = crate::Column::new(board.id, "Todo", 0);
        let col2 = crate::Column::new(board.id, "Done", 1);
        let col3 = crate::Column::new(board.id, "Archive", 2);
        let columns = vec![col1, col2.clone(), col3];

        board.update_completion_column_id(Some(col2.id));
        assert_eq!(board.resolve_completion_column(&columns), Some(col2.id));
    }

    #[test]
    fn test_resolve_completion_column_stale_id_falls_back() {
        let mut board = Board::new("Test", None::<String>);
        let col1 = crate::Column::new(board.id, "Todo", 0);
        let col2 = crate::Column::new(board.id, "Done", 1);
        let columns = vec![col1, col2.clone()];

        board.update_completion_column_id(Some(Uuid::new_v4()));
        assert_eq!(board.resolve_completion_column(&columns), Some(col2.id));
    }

    #[test]
    fn test_resolve_completion_column_empty_columns() {
        let board = Board::new("Test", None::<String>);
        assert_eq!(board.resolve_completion_column(&[]), None);
    }

    #[test]
    fn test_sprint_counter_initialization() {
        let mut board = Board::new("Test", None::<String>);
        assert_eq!(board.get_sprint_counter("sprint"), None);

        let num = board.get_next_sprint_number("sprint");
        assert_eq!(num, 1);
        assert_eq!(board.get_sprint_counter("sprint"), Some(2));
    }

    #[test]
    fn test_shared_sprint_sequence() {
        let mut board = Board::new("Test", None::<String>);

        let num1 = board.get_next_sprint_number("SPRINT");
        assert_eq!(num1, 1);

        let num2 = board.get_next_sprint_number("SPRINT");
        assert_eq!(num2, 2);

        let num3 = board.get_next_sprint_number("SPRINT");
        assert_eq!(num3, 3);

        assert_eq!(board.get_sprint_counter("SPRINT"), Some(4));
    }

    #[test]
    fn test_initialize_sprint_counter() {
        let mut board = Board::new("Test", None::<String>);
        board.initialize_sprint_counter("RELEASE", 5);

        let num = board.get_next_sprint_number("RELEASE");
        assert_eq!(num, 5);
        assert_eq!(board.get_sprint_counter("RELEASE"), Some(6));
    }

    #[test]
    fn test_ensure_sprint_counter_initialized_with_existing_sprints() {
        use crate::Sprint;

        let mut board = Board::new("Test", None::<String>);

        let sprint1 = Sprint::new(board.id, 1, None, None::<String>);
        let sprint2 = Sprint::new(board.id, 2, None, None::<String>);
        let sprint3 = Sprint::new(board.id, 3, None, None::<String>);

        let sprints = vec![sprint1, sprint2, sprint3];

        assert_eq!(board.get_sprint_counter("sprint"), None);
        board.ensure_sprint_counter_initialized("sprint", &sprints);
        assert_eq!(board.get_sprint_counter("sprint"), Some(4));

        let next = board.get_next_sprint_number("sprint");
        assert_eq!(next, 4);
    }

    #[test]
    fn test_ensure_sprint_counter_not_reinitialize() {
        use crate::Sprint;

        let mut board = Board::new("Test", None::<String>);
        board.initialize_sprint_counter("test", 10);

        let sprint1 = Sprint::new(board.id, 1, None, Some("test"));
        let sprint2 = Sprint::new(board.id, 2, None, Some("test"));
        let sprints = vec![sprint1, sprint2];

        board.ensure_sprint_counter_initialized("test", &sprints);
        assert_eq!(board.get_sprint_counter("test"), Some(10));
    }
}

#[cfg(test)]
mod sort_field_serialization_tests {
    use super::*;

    #[test]
    fn test_position_serializes_correctly() {
        let field = SortField::Position;
        let json = serde_json::to_string(&field).unwrap();
        assert_eq!(json, "\"Position\"");
    }

    #[test]
    fn test_position_deserializes_correctly() {
        let json = "\"Position\"";
        let field: SortField = serde_json::from_str(json).unwrap();
        assert_eq!(field, SortField::Position);
    }

    #[test]
    fn test_due_date_serializes_correctly() {
        let field = SortField::DueDate;
        let json = serde_json::to_string(&field).unwrap();
        assert_eq!(json, "\"DueDate\"");
    }

    #[test]
    fn test_due_date_deserializes_correctly() {
        let json = "\"DueDate\"";
        let field: SortField = serde_json::from_str(json).unwrap();
        assert_eq!(field, SortField::DueDate);
    }

    #[test]
    fn test_board_with_due_date_sort_serializes() {
        let mut board = Board::new("Test", None::<String>);
        board.update_task_sort(SortField::DueDate, SortOrder::Ascending);

        let json = serde_json::to_string(&board).unwrap();
        assert!(
            json.contains("\"task_sort_field\":\"DueDate\""),
            "Expected DueDate in JSON, got: {}",
            json
        );
    }

    #[test]
    fn test_board_with_position_sort_serializes() {
        let mut board = Board::new("Test", None::<String>);
        board.update_task_sort(SortField::Position, SortOrder::Descending);

        let json = serde_json::to_string(&board).unwrap();
        assert!(
            json.contains("\"task_sort_field\":\"Position\""),
            "Expected Position in JSON, got: {}",
            json
        );
        assert!(
            json.contains("\"task_sort_order\":\"Descending\""),
            "Expected Descending in JSON, got: {}",
            json
        );
    }
}
