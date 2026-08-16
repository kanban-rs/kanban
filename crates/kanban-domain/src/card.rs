use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

use crate::{
    board::{Board, BoardId},
    column::ColumnId,
    field_update::FieldUpdate,
    sprint::Sprint,
    SprintLog,
};
use kanban_core::GraphNode;

pub type CardId = Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CardPriority {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CardStatus {
    Todo,
    InProgress,
    Blocked,
    Done,
}

impl fmt::Display for CardPriority {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Low => write!(f, "low"),
            Self::Medium => write!(f, "medium"),
            Self::High => write!(f, "high"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

impl fmt::Display for CardStatus {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Todo => write!(f, "todo"),
            Self::InProgress => write!(f, "in_progress"),
            Self::Blocked => write!(f, "blocked"),
            Self::Done => write!(f, "done"),
        }
    }
}

/// Represents card lifecycle operation types.
/// Used for visual feedback during card operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimationType {
    Archiving,
    Restoring,
    Deleting,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Card {
    pub id: CardId,
    pub column_id: ColumnId,
    /// Durable board reference, independent of `column_id`. Set at creation
    /// and kept in sync on every move (including cross-board moves). Exists
    /// so a card's board is always resolvable even if its column is later
    /// deleted (archived cards don't block column deletion) — see KAN-963.
    pub board_id: BoardId,
    pub title: String,
    pub description: Option<String>,
    pub priority: CardPriority,
    pub status: CardStatus,
    pub position: i32,
    pub due_date: Option<DateTime<Utc>>,
    pub points: Option<u8>,
    pub card_number: u32,
    /// The namespace this card's identifier belongs to, normalised and fixed
    /// at creation. Stored rather than resolved through the board, so renaming
    /// a board never renames cards it already minted.
    ///
    /// Deliberately a plain value, not a foreign key to `prefixes.name`: this
    /// is a historical fact, while a prefix row is live allocation state. An FK
    /// would either delete cards when a prefix is retired or forbid retiring
    /// one, and history must not be hostage to current allocation state.
    /// `board_id` is denormalised here for the same reason.
    #[serde(default)]
    pub prefix: String,
    pub sprint_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub sprint_logs: Vec<SprintLog>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CardSummary {
    pub id: CardId,
    pub column_id: ColumnId,
    pub title: String,
    pub priority: CardPriority,
    pub status: CardStatus,
    pub position: i32,
    pub due_date: Option<DateTime<Utc>>,
    pub points: Option<u8>,
    #[serde(default)]
    pub card_number: u32,
    #[serde(default)]
    pub sprint_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[serde(default)]
    pub completed_at: Option<DateTime<Utc>>,
    /// `Some` iff this card is archived (the marker's `archived_at`); `None` for
    /// a live card. Stamped by the service when the unified card list includes
    /// archived cards. Skipped on the wire when `None` so live summaries are
    /// byte-identical to before this field existed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub archived_at: Option<DateTime<Utc>>,
}

impl From<&Card> for CardSummary {
    fn from(card: &Card) -> Self {
        Self {
            id: card.id,
            column_id: card.column_id,
            title: card.title.clone(),
            priority: card.priority,
            status: card.status,
            position: card.position,
            due_date: card.due_date,
            points: card.points,
            card_number: card.card_number,
            sprint_id: card.sprint_id,
            created_at: card.created_at,
            updated_at: card.updated_at,
            completed_at: card.completed_at,
            archived_at: None,
        }
    }
}

impl Card {
    pub fn new(
        board: &mut Board,
        column_id: ColumnId,
        title: impl Into<String>,
        position: i32,
    ) -> Self {
        let now = Utc::now();
        let card_number = board.get_next_card_number();
        // KAN-1214 replaces this with an allocated value; until then it mirrors
        // what the identifier reader would resolve for a board-owned card.
        let prefix = crate::prefix::effective_card_prefix(
            board.card_prefix.as_deref(),
            None,
            crate::prefix_backfill::DEFAULT_CARD_PREFIX,
        );
        Self {
            id: Uuid::new_v4(),
            column_id,
            board_id: board.id,
            title: title.into(),
            description: None,
            priority: CardPriority::Medium,
            status: CardStatus::Todo,
            position,
            due_date: None,
            points: None,
            card_number,
            prefix,
            sprint_id: None,
            created_at: now,
            updated_at: now,
            completed_at: None,
            sprint_logs: Vec::new(),
        }
    }

    pub fn move_to_column(&mut self, column_id: ColumnId, position: i32) {
        self.column_id = column_id;
        self.position = position;
        self.updated_at = Utc::now();
    }

    pub fn update_status(&mut self, status: CardStatus) {
        let now = Utc::now();
        if status == CardStatus::Done && self.status != CardStatus::Done {
            self.completed_at = Some(now);
        } else if status != CardStatus::Done && self.status == CardStatus::Done {
            self.completed_at = None;
        }
        self.status = status;
        self.updated_at = now;
    }

    pub fn update_priority(&mut self, priority: CardPriority) {
        self.priority = priority;
        self.updated_at = Utc::now();
    }

    pub fn set_due_date(&mut self, due_date: Option<DateTime<Utc>>) {
        self.due_date = due_date;
        self.updated_at = Utc::now();
    }

    pub fn update_title(&mut self, title: impl Into<String>) {
        self.title = title.into();
        self.updated_at = Utc::now();
    }

    pub fn update_description(&mut self, description: Option<impl Into<String>>) {
        self.description = description.map(Into::into);
        self.updated_at = Utc::now();
    }

    pub fn set_points(&mut self, points: Option<u8>) {
        self.points = points;
        self.updated_at = Utc::now();
    }

    /// Resolve the branch name prefix using two-level hierarchy:
    /// sprint.card_prefix → board.card_prefix → default_prefix
    pub fn branch_name(&self, board: &Board, sprints: &[Sprint], default_prefix: &str) -> String {
        // The card's STORED prefix, with the casing it was minted under. A
        // branch name is the identifier a user checks out: deriving it would
        // move the branch of every existing card the moment its board's prefix
        // changed, and normalising it would move every branch to lower case.
        //
        // Derivation remains only for a card written before the prefix was
        // stored and not yet migrated.
        let derived;
        let prefix = if self.prefix.is_empty() {
            derived = if let Some(sprint_id) = self.sprint_id {
                sprints
                    .iter()
                    .find(|s| s.id == sprint_id)
                    .and_then(|sprint| sprint.card_prefix.as_deref())
                    .unwrap_or_else(|| board.effective_card_prefix(default_prefix))
            } else {
                board.effective_card_prefix(default_prefix)
            };
            derived
        } else {
            &self.prefix
        };
        let kebab_title = Self::to_kebab_case(&self.title);
        let branch = format!("{}-{}/{}", prefix, self.card_number, kebab_title);
        Self::truncate_branch_name(branch)
    }

    fn to_kebab_case(s: &str) -> String {
        s.chars()
            .map(|c| {
                if c.is_alphanumeric() {
                    c.to_ascii_lowercase()
                } else {
                    '-'
                }
            })
            .collect::<String>()
            .split('-')
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join("-")
    }

    fn truncate_branch_name(branch: String) -> String {
        const MAX_BRANCH_LENGTH: usize = 250;
        if branch.len() <= MAX_BRANCH_LENGTH {
            branch
        } else {
            branch.chars().take(MAX_BRANCH_LENGTH).collect()
        }
    }

    pub fn git_checkout_command(
        &self,
        board: &Board,
        sprints: &[Sprint],
        default_prefix: &str,
    ) -> String {
        let name = self.branch_name(board, sprints, default_prefix);
        format!("git checkout -b {}", name)
    }

    pub fn is_completed(&self) -> bool {
        self.status == CardStatus::Done
    }

    pub fn assign_to_sprint(
        &mut self,
        sprint_id: Uuid,
        sprint_number: u32,
        sprint_name: Option<impl Into<String>>,
        sprint_status: impl Into<String>,
        now: DateTime<Utc>,
    ) {
        // Only create a sprint log entry if the sprint assignment actually changes
        if self.sprint_id != Some(sprint_id) {
            self.sprint_id = Some(sprint_id);
            let sprint_log = SprintLog::new(sprint_id, sprint_number, sprint_name, sprint_status);
            self.sprint_logs.push(sprint_log);
            self.updated_at = now;
        }
    }

    pub fn end_current_sprint_log(&mut self) {
        if let Some(last_log) = self.sprint_logs.last_mut() {
            if last_log.ended_at.is_none() {
                last_log.end_sprint();
                self.updated_at = Utc::now();
            }
        }
    }

    pub fn get_sprint_history(&self) -> &[SprintLog] {
        &self.sprint_logs
    }

    /// Update card with partial changes
    pub fn update(&mut self, updates: CardUpdate, now: DateTime<Utc>) {
        if let Some(title) = updates.title {
            self.title = title;
        }
        updates.description.apply_to(&mut self.description);
        if let Some(priority) = updates.priority {
            self.priority = priority;
        }
        if let Some(status) = updates.status {
            self.update_status(status);
        }
        if let Some(position) = updates.position {
            self.position = position;
        }
        if let Some(column_id) = updates.column_id {
            self.column_id = column_id;
        }
        updates.due_date.apply_to(&mut self.due_date);
        updates.points.apply_to(&mut self.points);
        updates.sprint_id.apply_to(&mut self.sprint_id);
        self.updated_at = now;
    }
}

/// Partial update struct for Card
///
/// Uses `FieldUpdate<T>` for optional fields to provide clear three-state updates.
/// See [`FieldUpdate`] documentation for usage examples.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CardUpdate {
    pub title: Option<String>,
    pub description: FieldUpdate<String>,
    pub priority: Option<CardPriority>,
    pub status: Option<CardStatus>,
    pub position: Option<i32>,
    pub column_id: Option<ColumnId>,
    pub due_date: FieldUpdate<DateTime<Utc>>,
    pub points: FieldUpdate<u8>,
    pub sprint_id: FieldUpdate<Uuid>,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CreateCardOptions {
    pub description: Option<String>,
    pub priority: Option<CardPriority>,
    pub points: Option<u8>,
    pub due_date: Option<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sprint_id: Option<Uuid>,
}

impl GraphNode for Card {
    fn node_id(&self) -> Uuid {
        self.id
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::Board;

    #[test]
    fn test_card_new_accepts_str_title_without_to_string() {
        let column_id = uuid::Uuid::new_v4();
        let mut board = Board::new("board", None::<String>);
        let card = Card::new(&mut board, column_id, "my card", 0);
        assert_eq!(card.title, "my card");
    }

    #[test]
    fn test_card_summary_from_card_has_none_archived_at() {
        let mut board = Board::new("board", None::<String>);
        let card = Card::new(&mut board, uuid::Uuid::new_v4(), "c", 0);
        let summary = CardSummary::from(&card);
        assert_eq!(summary.archived_at, None);
    }

    #[test]
    fn test_card_summary_serializes_without_archived_at_when_none() {
        let mut board = Board::new("board", None::<String>);
        let card = Card::new(&mut board, uuid::Uuid::new_v4(), "c", 0);
        let summary = CardSummary::from(&card);
        let value = serde_json::to_value(&summary).unwrap();
        assert!(
            value.get("archived_at").is_none(),
            "a live summary must not carry an archived_at key"
        );
    }

    #[test]
    fn test_card_summary_serializes_archived_at_when_some() {
        let mut board = Board::new("board", None::<String>);
        let card = Card::new(&mut board, uuid::Uuid::new_v4(), "c", 0);
        let at = Utc::now();
        let summary = CardSummary {
            archived_at: Some(at),
            ..CardSummary::from(&card)
        };
        let value = serde_json::to_value(&summary).unwrap();
        assert!(value.get("archived_at").is_some());
    }

    #[test]
    fn test_card_update_title_accepts_str_without_to_string() {
        let column_id = uuid::Uuid::new_v4();
        let mut board = Board::new("board", None::<String>);
        let mut card = Card::new(&mut board, column_id, "initial", 0);
        card.update_title("updated");
        assert_eq!(card.title, "updated");
    }

    #[test]
    fn test_card_update_description_accepts_str_without_to_string() {
        let column_id = uuid::Uuid::new_v4();
        let mut board = Board::new("board", None::<String>);
        let mut card = Card::new(&mut board, column_id, "card", 0);
        card.update_description(Some("desc"));
        assert_eq!(card.description, Some("desc".to_string()));
    }

    #[test]
    fn test_card_assign_to_sprint_accepts_str_args_without_to_string() {
        let column_id = uuid::Uuid::new_v4();
        let mut board = Board::new("board", None::<String>);
        let mut card = Card::new(&mut board, column_id, "card", 0);
        let sprint_id = uuid::Uuid::new_v4();
        card.assign_to_sprint(sprint_id, 1, Some("Sprint 1"), "Active", Utc::now());
        assert_eq!(card.sprint_id, Some(sprint_id));
        let log = &card.sprint_logs[0];
        assert_eq!(log.sprint_name, Some("Sprint 1".to_string()));
        assert_eq!(log.status, "Active");
    }

    #[test]
    fn test_card_new_uses_board_card_counter_no_prefix_arg() {
        let column_id = uuid::Uuid::new_v4();
        let mut board = Board::new("Test Board", None::<String>);
        assert_eq!(board.card_counter, 1);

        let card1 = Card::new(&mut board, column_id, "Test Card 1", 0);
        assert_eq!(card1.card_number, 1);
        assert_eq!(board.card_counter, 2);

        let card2 = Card::new(&mut board, column_id, "Test Card 2", 1);
        assert_eq!(card2.card_number, 2);
        assert_eq!(board.card_counter, 3);
    }

    #[test]
    fn test_card_sequential_numbers_increment_board_counter() {
        let column_id = uuid::Uuid::new_v4();
        let mut board = Board::new("Test Board", None::<String>);

        let cards: Vec<Card> = (0..5)
            .map(|i| Card::new(&mut board, column_id, format!("Card {}", i), i))
            .collect();

        for (i, card) in cards.iter().enumerate() {
            assert_eq!(card.card_number, (i + 1) as u32);
        }
        assert_eq!(board.card_counter, 6);
    }

    #[test]
    fn test_branch_name_falls_back_to_board_when_no_sprint_prefix() {
        let column_id = uuid::Uuid::new_v4();
        let mut board = Board::new("Test Board", Some("KAN"));
        let card = Card::new(&mut board, column_id, "Test Card", 0);
        let sprint = crate::sprint::Sprint::new(board.id, 1, None, None::<String>);
        let mut card_with_sprint = card.clone();
        card_with_sprint.sprint_id = Some(sprint.id);
        let sprints = vec![sprint];

        // Sprint has no card_prefix, so falls back to board card_prefix
        assert_eq!(
            card_with_sprint.branch_name(&board, &sprints, "task"),
            "KAN-1/test-card"
        );
    }

    #[test]
    fn test_branch_name_two_level_hierarchy_sprint_then_board() {
        use crate::sprint::{Sprint, SprintStatus};

        let column_id = uuid::Uuid::new_v4();
        let mut board = Board::new("Test Board", Some("KAN"));
        let card = Card::new(&mut board, column_id, "Test Card", 0);

        let sprint_with_prefix = Sprint {
            id: uuid::Uuid::new_v4(),
            board_id: board.id,
            sprint_number: 1,
            name_index: None,
            prefix: None,
            card_prefix: Some("SPR".to_string()),
            status: SprintStatus::Planning,
            start_date: None,
            end_date: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        let mut card_with_sprint = card.clone();
        card_with_sprint.sprint_id = Some(sprint_with_prefix.id);
        let sprints = vec![sprint_with_prefix];

        // Attaching an EXISTING card to an overriding sprint does not rename
        // it: the prefix is stamped at creation and frozen, so a branch already
        // checked out stays valid. A card created INTO such a sprint is stamped
        // with the sprint's prefix instead -- `CreateCard::execute` resolves the
        // override before stamping.
        assert_eq!(
            card_with_sprint.branch_name(&board, &sprints, "task"),
            "KAN-1/test-card",
            "a later sprint assignment must not move an existing identifier"
        );
    }

    #[test]
    fn test_branch_name_falls_back_to_default_when_no_board_prefix() {
        let column_id = uuid::Uuid::new_v4();
        let mut board = Board::new("Test Board", None::<String>);
        let card = Card::new(&mut board, column_id, "Test Card", 0);
        let sprints = vec![];

        assert_eq!(
            card.branch_name(&board, &sprints, "task"),
            "task-1/test-card"
        );
    }

    #[test]
    fn test_branch_name() {
        let column_id = uuid::Uuid::new_v4();
        let mut board = Board::new("Test Board", None::<String>);
        let card = Card::new(&mut board, column_id, "Test Card", 0);
        let sprints = vec![];

        assert_eq!(
            card.branch_name(&board, &sprints, "task"),
            "task-1/test-card".to_string()
        );
    }

    #[test]
    fn test_to_kebab_case() {
        assert_eq!(Card::to_kebab_case("Simple Title"), "simple-title");
        assert_eq!(
            Card::to_kebab_case("Fix: Bug in Parser"),
            "fix-bug-in-parser"
        );
        assert_eq!(
            Card::to_kebab_case("Add_Feature_Support"),
            "add-feature-support"
        );
        assert_eq!(Card::to_kebab_case("Multiple   Spaces"), "multiple-spaces");
        assert_eq!(Card::to_kebab_case("Special@#$Chars"), "special-chars");
        assert_eq!(Card::to_kebab_case("CamelCaseTitle"), "camelcasetitle");
        assert_eq!(Card::to_kebab_case("Fix (Bug) [Issue]"), "fix-bug-issue");
    }

    #[test]
    fn test_branch_name_truncation() {
        let column_id = uuid::Uuid::new_v4();
        let long_title = "a".repeat(300);
        let mut board = Board::new("Test Board", None::<String>);
        let card = Card::new(&mut board, column_id, long_title, 0);
        let sprints = vec![];

        let branch = card.branch_name(&board, &sprints, "task");
        assert!(branch.len() <= 250);
        assert!(branch.starts_with("task-1/"));
    }

    #[test]
    fn test_git_checkout_command() {
        let column_id = uuid::Uuid::new_v4();
        let mut board = Board::new("Test Board", None::<String>);
        let card = Card::new(&mut board, column_id, "Test Card", 0);
        let sprints = vec![];

        assert_eq!(
            card.git_checkout_command(&board, &sprints, "task"),
            "git checkout -b task-1/test-card".to_string()
        );
    }

    #[test]
    fn test_branch_name_with_sprint_card_prefix() {
        use crate::sprint::{Sprint, SprintStatus};

        let column_id = uuid::Uuid::new_v4();
        let mut board = Board::new("Test Board", None::<String>);

        let card = Card::new(&mut board, column_id, "Test Card", 0);

        let sprint = Sprint::new(board.id, 1, Some(0), None::<String>);
        let mut card_with_sprint = card.clone();
        card_with_sprint.sprint_id = Some(sprint.id);

        let sprints = vec![sprint];

        assert_eq!(
            card_with_sprint.branch_name(&board, &sprints, "task"),
            "task-1/test-card".to_string()
        );

        let sprint_with_card_prefix = Sprint {
            id: card_with_sprint.sprint_id.unwrap(),
            board_id: board.id,
            sprint_number: 1,
            name_index: Some(0),
            prefix: Some("sprint".to_string()),
            card_prefix: Some("hotfix".to_string()),
            status: SprintStatus::Planning,
            start_date: None,
            end_date: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        let sprints_with_card_prefix = vec![sprint_with_card_prefix];

        // As above: assignment after the fact does not re-stamp the card.
        assert_eq!(
            card_with_sprint.branch_name(&board, &sprints_with_card_prefix, "task"),
            "task-1/test-card".to_string(),
            "a later sprint assignment must not move an existing identifier"
        );
    }

    #[test]
    fn test_sprint_logging() {
        let column_id = uuid::Uuid::new_v4();
        let mut board = Board::new("Test Board", None::<String>);
        let mut card = Card::new(&mut board, column_id, "Test Card", 0);

        assert_eq!(card.get_sprint_history().len(), 0);

        let sprint_id_1 = uuid::Uuid::new_v4();
        card.assign_to_sprint(sprint_id_1, 1, Some("Sprint 1"), "Active", Utc::now());

        assert_eq!(card.get_sprint_history().len(), 1);
        assert_eq!(card.sprint_id, Some(sprint_id_1));
        let first_log = &card.get_sprint_history()[0];
        assert_eq!(first_log.sprint_id, sprint_id_1);
        assert_eq!(first_log.sprint_number, 1);
        assert_eq!(first_log.sprint_name, Some("Sprint 1".to_string()));
        assert!(first_log.ended_at.is_none());
    }

    #[test]
    fn test_sprint_log_ending() {
        let column_id = uuid::Uuid::new_v4();
        let mut board = Board::new("Test Board", None::<String>);
        let mut card = Card::new(&mut board, column_id, "Test Card", 0);

        let sprint_id_1 = uuid::Uuid::new_v4();
        card.assign_to_sprint(sprint_id_1, 1, Some("Sprint 1"), "Active", Utc::now());

        card.end_current_sprint_log();

        let first_log = &card.get_sprint_history()[0];
        assert!(first_log.ended_at.is_some());
    }

    #[test]
    fn test_multiple_sprint_logs() {
        let column_id = uuid::Uuid::new_v4();
        let mut board = Board::new("Test Board", None::<String>);
        let mut card = Card::new(&mut board, column_id, "Test Card", 0);

        let sprint_id_1 = uuid::Uuid::new_v4();
        let sprint_id_2 = uuid::Uuid::new_v4();

        card.assign_to_sprint(sprint_id_1, 1, Some("Sprint 1"), "Active", Utc::now());
        assert_eq!(card.sprint_id, Some(sprint_id_1));
        assert_eq!(card.get_sprint_history().len(), 1);

        card.end_current_sprint_log();
        card.assign_to_sprint(sprint_id_2, 2, Some("Sprint 2"), "Active", Utc::now());

        assert_eq!(card.sprint_id, Some(sprint_id_2));
        assert_eq!(card.get_sprint_history().len(), 2);
        let first_log = &card.get_sprint_history()[0];
        assert!(first_log.ended_at.is_some());
        let second_log = &card.get_sprint_history()[1];
        assert!(second_log.ended_at.is_none());
    }

    #[test]
    fn test_assign_same_sprint_no_duplicate() {
        let column_id = uuid::Uuid::new_v4();
        let mut board = Board::new("Test Board", None::<String>);
        let mut card = Card::new(&mut board, column_id, "Test Card", 0);

        let sprint_id = uuid::Uuid::new_v4();

        card.assign_to_sprint(sprint_id, 1, Some("Sprint 1"), "Active", Utc::now());
        assert_eq!(card.sprint_id, Some(sprint_id));
        assert_eq!(card.get_sprint_history().len(), 1);

        card.assign_to_sprint(sprint_id, 1, Some("Sprint 1"), "Active", Utc::now());

        assert_eq!(card.sprint_id, Some(sprint_id));
        assert_eq!(card.get_sprint_history().len(), 1);
        let log = &card.get_sprint_history()[0];
        assert_eq!(log.sprint_id, sprint_id);
        assert!(log.ended_at.is_none());
    }

    #[test]
    fn test_assign_to_sprint_records_caller_provided_timestamp() {
        use chrono::TimeZone;

        let column_id = uuid::Uuid::new_v4();
        let mut board = Board::new("Test Board", None::<String>);
        let mut card = Card::new(&mut board, column_id, "Test Card", 0);
        let fixed_time = Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap();
        let sprint_id = uuid::Uuid::new_v4();

        card.assign_to_sprint(sprint_id, 1, Some("Sprint 1"), "Active", fixed_time);

        assert_eq!(card.updated_at, fixed_time);
    }

    #[test]
    fn test_update_records_caller_provided_timestamp() {
        use chrono::TimeZone;

        let column_id = uuid::Uuid::new_v4();
        let mut board = Board::new("Test Board", None::<String>);
        let mut card = Card::new(&mut board, column_id, "Test Card", 0);
        let fixed_time = Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap();

        card.update(
            CardUpdate {
                title: Some("Renamed".to_string()),
                ..Default::default()
            },
            fixed_time,
        );

        assert_eq!(card.title, "Renamed");
        assert_eq!(card.updated_at, fixed_time);
    }
}
