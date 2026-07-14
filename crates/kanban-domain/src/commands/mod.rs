use crate::data_store::DataStore;
use crate::{DomainError, KanbanError, KanbanResult};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub mod board_commands;
pub mod card;
pub mod cascade_commands;
pub mod column_commands;
pub mod dependency_commands;
pub mod sprint_commands;

pub use board_commands::*;
pub use card::*;
pub use cascade_commands::{CascadeCommand, SetArchivedCardsSprint};
pub use column_commands::*;
pub use dependency_commands::*;
pub use sprint_commands::*;

/// Every domain mutation flows through this enum.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "domain", rename_all = "snake_case")]
pub enum Command {
    Board(BoardCommand),
    Column(ColumnCommand),
    Card(CardCommand),
    Sprint(SprintCommand),
    Dependency(DependencyCommand),
    Cascade(CascadeCommand),
}

impl Command {
    pub fn execute(&self, context: &CommandContext) -> KanbanResult<()> {
        match self {
            Command::Board(cmd) => cmd.execute(context),
            Command::Column(cmd) => cmd.execute(context),
            Command::Card(cmd) => cmd.execute(context),
            Command::Sprint(cmd) => cmd.execute(context),
            Command::Dependency(cmd) => cmd.execute(context),
            Command::Cascade(cmd) => cmd.execute(context),
        }
    }

    pub fn description(&self) -> String {
        match self {
            Command::Board(cmd) => cmd.description(),
            Command::Column(cmd) => cmd.description(),
            Command::Card(cmd) => cmd.description(),
            Command::Sprint(cmd) => cmd.description(),
            Command::Dependency(cmd) => cmd.description(),
            Command::Cascade(cmd) => cmd.description(),
        }
    }

    /// Build the inverse batch by reading pre-state from `store`.
    /// Called before the forward `execute` runs.
    ///
    /// An empty `Vec` is "this forward is a no-op; nothing to undo."
    /// `Err` means the inverse cannot be captured (entity missing,
    /// store error, or the command is synthetic-only).
    pub fn capture_inverse(&self, store: &dyn DataStore) -> KanbanResult<Vec<Command>> {
        match self {
            Command::Board(cmd) => cmd.capture_inverse(store),
            Command::Column(cmd) => cmd.capture_inverse(store),
            Command::Card(cmd) => cmd.capture_inverse(store),
            Command::Sprint(cmd) => cmd.capture_inverse(store),
            Command::Dependency(cmd) => cmd.capture_inverse(store),
            Command::Cascade(cmd) => cmd.capture_inverse(store),
        }
    }
}

/// Context passed to commands for mutation.
/// Holds a reference to the DataStore which uses interior mutability.
pub struct CommandContext<'a> {
    pub store: &'a dyn DataStore,
}

impl<'a> CommandContext<'a> {
    pub fn get_board(&self, id: Uuid) -> KanbanResult<crate::Board> {
        self.store
            .get_board(id)?
            .ok_or_else(|| KanbanError::not_found("Board", id))
    }

    pub fn get_card(&self, id: Uuid) -> KanbanResult<crate::Card> {
        self.store
            .get_card(id)?
            .ok_or_else(|| KanbanError::not_found("Card", id))
    }

    pub fn get_column(&self, id: Uuid) -> KanbanResult<crate::Column> {
        self.store
            .get_column(id)?
            .ok_or_else(|| KanbanError::not_found("Column", id))
    }

    /// Canonical column-membership check for the command tier: returns the
    /// column or `NotFound`. Mirror of the service-tier
    /// `KanbanContext::require_column` so the two layers share name + behavior
    /// (KAN-248). Commands that need to validate a target column FK before
    /// mutating route through this rather than an inline `iter().any`.
    pub fn require_column(&self, id: Uuid) -> KanbanResult<crate::Column> {
        self.get_column(id)
    }

    pub fn get_sprint(&self, id: Uuid) -> KanbanResult<crate::Sprint> {
        self.store
            .get_sprint(id)?
            .ok_or_else(|| KanbanError::not_found("Sprint", id))
    }

    pub fn filter_valid_card_ids(&self, ids: &[Uuid], command_name: &str) -> Vec<Uuid> {
        let (valid, rejected): (Vec<_>, Vec<_>) = ids
            .iter()
            .copied()
            .partition(|&id| self.store.get_card(id).ok().flatten().is_some());
        for id in &rejected {
            tracing::warn!("{}: card {} not found, skipping", command_name, id);
        }
        valid
    }

    /// Returns `WipLimitExceeded` if adding `adding` cards to `column_id` would exceed its WIP
    /// limit. Cards whose IDs appear in `exclude` are not counted toward the current occupancy.
    /// Returns `not_found` if the column does not exist.
    pub fn check_wip_limit(
        &self,
        column_id: Uuid,
        adding: usize,
        exclude: &[Uuid],
    ) -> KanbanResult<()> {
        let column = self.get_column(column_id)?;
        if let Some(limit) = column.wip_limit {
            let current = self
                .store
                .count_cards_in_column_excluding(column_id, exclude)?;
            if current + adding > limit as usize {
                return Err(KanbanError::Domain(DomainError::wip_limit_exceeded(
                    column_id,
                    limit as u32,
                )));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::test_helpers::TestContext;
    use super::*;
    use crate::DataStore;

    #[test]
    fn test_check_wip_limit_column_not_found_returns_error() {
        let tc = TestContext::new();
        let ctx = tc.as_command_context();
        let result = ctx.check_wip_limit(Uuid::new_v4(), 1, &[]);
        assert!(result.unwrap_err().is_not_found());
    }

    #[test]
    fn test_require_column_missing_returns_not_found() {
        let tc = TestContext::new();
        let ctx = tc.as_command_context();
        let err = ctx.require_column(Uuid::new_v4()).unwrap_err();
        assert!(err.is_not_found());
    }

    #[test]
    fn test_require_column_present_returns_column() {
        let tc = TestContext::new();
        let col = crate::Column::new(Uuid::new_v4(), "Col", 0);
        let col_id = col.id;
        tc.store.upsert_column(col).unwrap();
        let ctx = tc.as_command_context();
        assert_eq!(ctx.require_column(col_id).unwrap().id, col_id);
    }

    #[test]
    fn test_check_wip_limit_no_limit_always_ok() {
        let tc = TestContext::new();
        let mut board = crate::Board::new("B", None::<String>);
        let col = crate::Column::new(board.id, "Col", 0);
        let col_id = col.id;
        let card = crate::Card::new(&mut board, col_id, "C", 0);
        tc.store.upsert_column(col).unwrap();
        tc.store.upsert_card(card).unwrap();
        let ctx = tc.as_command_context();
        assert!(ctx.check_wip_limit(col_id, 1, &[]).is_ok());
    }

    #[test]
    fn test_check_wip_limit_below_limit_ok() {
        let tc = TestContext::new();
        let mut board = crate::Board::new("B", None::<String>);
        let mut col = crate::Column::new(board.id, "Col", 0);
        col.wip_limit = Some(2);
        let col_id = col.id;
        let card = crate::Card::new(&mut board, col_id, "C", 0);
        tc.store.upsert_column(col).unwrap();
        tc.store.upsert_card(card).unwrap();
        let ctx = tc.as_command_context();
        assert!(ctx.check_wip_limit(col_id, 1, &[]).is_ok());
    }

    #[test]
    fn test_check_wip_limit_at_limit_returns_error() {
        let tc = TestContext::new();
        let mut board = crate::Board::new("B", None::<String>);
        let mut col = crate::Column::new(board.id, "Col", 0);
        col.wip_limit = Some(1);
        let col_id = col.id;
        let card = crate::Card::new(&mut board, col_id, "C", 0);
        tc.store.upsert_column(col).unwrap();
        tc.store.upsert_card(card).unwrap();
        let ctx = tc.as_command_context();
        let result = ctx.check_wip_limit(col_id, 1, &[]);
        assert!(result.unwrap_err().is_wip_limit_exceeded());
    }

    #[test]
    fn test_check_wip_limit_exclude_reduces_count() {
        let tc = TestContext::new();
        let mut board = crate::Board::new("B", None::<String>);
        let mut col = crate::Column::new(board.id, "Col", 0);
        col.wip_limit = Some(1);
        let col_id = col.id;
        let card = crate::Card::new(&mut board, col_id, "C", 0);
        let card_id = card.id;
        tc.store.upsert_column(col).unwrap();
        tc.store.upsert_card(card).unwrap();
        let ctx = tc.as_command_context();
        assert!(ctx.check_wip_limit(col_id, 1, &[card_id]).is_ok());
    }

    #[test]
    fn test_check_wip_limit_batch_exceeds_limit_returns_error() {
        let tc = TestContext::new();
        let board = crate::Board::new("B", None::<String>);
        let mut col = crate::Column::new(board.id, "Col", 0);
        col.wip_limit = Some(1);
        let col_id = col.id;
        tc.store.upsert_board(board).unwrap();
        tc.store.upsert_column(col).unwrap();
        let ctx = tc.as_command_context();
        let result = ctx.check_wip_limit(col_id, 2, &[]);
        assert!(result.unwrap_err().is_wip_limit_exceeded());
    }

    #[test]
    fn test_command_serde_roundtrip_create_board() {
        let cmd = Command::Board(BoardCommand::Create(CreateBoard {
            id: Uuid::new_v4(),
            name: "B".into(),
            card_prefix: None,
            position: 0,
        }));
        let json = serde_json::to_string(&cmd).unwrap();
        let back: Command = serde_json::from_str(&json).unwrap();
        assert!(matches!(back, Command::Board(BoardCommand::Create(_))));
    }

    #[test]
    fn test_command_serde_roundtrip_archive_restore_board() {
        let id = Uuid::new_v4();
        let archive = Command::Board(BoardCommand::Archive(ArchiveBoards { ids: vec![id] }));
        let value: serde_json::Value =
            serde_json::from_str(&serde_json::to_string(&archive).unwrap()).unwrap();
        assert_eq!(value["domain"], "board");
        assert_eq!(value["action"], "archive");
        let back: Command = serde_json::from_value(value).unwrap();
        assert!(matches!(back, Command::Board(BoardCommand::Archive(_))));

        let restore = Command::Board(BoardCommand::Restore(RestoreBoard { board_id: id }));
        let value: serde_json::Value =
            serde_json::from_str(&serde_json::to_string(&restore).unwrap()).unwrap();
        assert_eq!(value["action"], "restore");
        let back: Command = serde_json::from_value(value).unwrap();
        assert!(matches!(back, Command::Board(BoardCommand::Restore(_))));
    }

    #[test]
    fn test_command_serde_tagged_format() {
        let cmd = Command::Card(CardCommand::Move(MoveCard {
            card_id: Uuid::new_v4(),
            new_column_id: Uuid::new_v4(),
            new_position: 0,
        }));
        let json = serde_json::to_string(&cmd).unwrap();
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(value["domain"], "card");
        assert_eq!(value["action"], "move");
    }

    #[test]
    fn test_command_execute_delegates_to_struct() {
        let tc = TestContext::new();
        let ctx = tc.as_command_context();
        let cmd = Command::Board(BoardCommand::Create(CreateBoard {
            id: Uuid::new_v4(),
            name: "B".into(),
            card_prefix: None,
            position: 0,
        }));
        cmd.execute(&ctx).unwrap();
        assert_eq!(tc.store.list_boards().unwrap().len(), 1);
    }

    #[test]
    fn test_command_description_delegates() {
        let cmd = Command::Board(BoardCommand::Create(CreateBoard {
            id: Uuid::new_v4(),
            name: "My Board".into(),
            card_prefix: None,
            position: 0,
        }));
        assert!(cmd.description().contains("My Board"));
    }

    #[test]
    fn test_command_serde_roundtrip_all_domains() {
        let commands = vec![
            Command::Board(BoardCommand::Delete(DeleteBoard {
                board_id: Uuid::new_v4(),
            })),
            Command::Column(ColumnCommand::Create(CreateColumn {
                id: Uuid::new_v4(),
                board_id: Uuid::new_v4(),
                name: "Col".into(),
                position: 0,
            })),
            Command::Card(CardCommand::Delete(DeleteCard {
                card_id: Uuid::new_v4(),
            })),
            Command::Sprint(SprintCommand::Delete(DeleteSprint {
                sprint_id: Uuid::new_v4(),
                timestamp: chrono::Utc::now(),
            })),
            Command::Dependency(DependencyCommand::RemoveSpawns(RemoveSpawns {
                source: Uuid::new_v4(),
                target: Uuid::new_v4(),
                tolerate_missing: false,
            })),
        ];
        for cmd in commands {
            let json = serde_json::to_string(&cmd).unwrap();
            let _back: Command = serde_json::from_str(&json).unwrap();
        }
    }

    #[test]
    fn test_command_serde_roundtrip_import_entities() {
        let board = crate::Board::new("Imported", Some("IMP"));
        let col = crate::Column::new(board.id, "Col", 0);
        let cmd = Command::Board(BoardCommand::Import(ImportEntities {
            boards: vec![board],
            columns: vec![col],
            cards: vec![],
            archived_cards: vec![],
            sprints: vec![],
            graph: Some(crate::DependencyGraph::new()),
        }));
        let json = serde_json::to_string(&cmd).unwrap();
        let back: Command = serde_json::from_str(&json).unwrap();
        match back {
            Command::Board(BoardCommand::Import(ie)) => {
                assert_eq!(ie.boards.len(), 1);
                assert_eq!(ie.columns.len(), 1);
                assert!(ie.graph.is_some());
            }
            _ => panic!("expected ImportEntities"),
        }
    }

    #[test]
    fn test_command_serde_roundtrip_complex_card_commands() {
        let commands = vec![
            Command::Card(CardCommand::Archive(ArchiveCards {
                ids: vec![Uuid::new_v4(), Uuid::new_v4()],
            })),
            Command::Card(CardCommand::AssignToSprint(AssignCardsToSprint {
                ids: vec![Uuid::new_v4()],
                sprint_id: Uuid::new_v4(),
            })),
            Command::Card(CardCommand::Restore(RestoreCard {
                card_id: Uuid::new_v4(),
                column_id: Uuid::new_v4(),
                position: 3,
                timestamp: chrono::Utc::now(),
            })),
            Command::Card(CardCommand::CompactPositions(CompactColumnPositions {
                column_id: Uuid::new_v4(),
            })),
        ];
        for cmd in commands {
            let json = serde_json::to_string(&cmd).unwrap();
            let back: Command = serde_json::from_str(&json).unwrap();
            assert_eq!(std::mem::discriminant(&cmd), std::mem::discriminant(&back));
        }
    }
}

#[cfg(test)]
pub(crate) mod test_helpers {
    use super::*;
    use crate::InMemoryStore;

    pub struct TestContext {
        pub store: InMemoryStore,
    }

    impl TestContext {
        pub fn new() -> Self {
            Self {
                store: InMemoryStore::new(),
            }
        }

        pub fn as_command_context(&self) -> CommandContext<'_> {
            CommandContext { store: &self.store }
        }
    }
}
