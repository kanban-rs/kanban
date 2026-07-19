use super::KanbanContext;
use kanban_domain::commands::{
    BoardCommand, CardCommand, ColumnCommand, Command, DependencyCommand, SprintCommand,
};
use kanban_domain::{KanbanError, KanbanResult};
use std::collections::HashSet;
use uuid::Uuid;

impl KanbanContext {
    /// Read-only-shelf guard (KAN-862): reject a batch that mutates content on an
    /// ARCHIVED board. Runs BEFORE the transaction so nothing is written.
    /// Lifecycle (archive/restore/delete-board) and internal/synthetic commands
    /// resolve to no gated board and pass through.
    pub(super) fn reject_content_mutation_on_archived_board(
        &self,
        commands: &[Command],
    ) -> KanbanResult<()> {
        let archived: HashSet<Uuid> = self
            .backend
            .list_archived_boards()?
            .iter()
            .map(|ab| ab.entity.id)
            .collect();
        if archived.is_empty() {
            return Ok(());
        }
        for cmd in commands {
            for board_id in self.content_target_boards(cmd)? {
                if archived.contains(&board_id) {
                    return Err(KanbanError::board_archived(board_id));
                }
            }
        }
        Ok(())
    }

    /// A `Card` is column-scoped, not board-scoped: hop card -> column -> board.
    fn board_of_card(&self, id: Uuid) -> KanbanResult<Option<Uuid>> {
        match self.backend.get_card(id)? {
            Some(card) => self.board_of_column(card.column_id),
            None => Ok(None),
        }
    }
    fn board_of_column(&self, id: Uuid) -> KanbanResult<Option<Uuid>> {
        Ok(self.backend.get_column(id)?.map(|c| c.board_id))
    }
    fn board_of_sprint(&self, id: Uuid) -> KanbanResult<Option<Uuid>> {
        Ok(self.backend.get_sprint(id)?.map(|s| s.board_id))
    }

    fn push_board_of_card(&self, id: Uuid, out: &mut Vec<Uuid>) -> KanbanResult<()> {
        if let Some(b) = self.board_of_card(id)? {
            out.push(b);
        }
        Ok(())
    }
    fn push_board_of_column(&self, id: Uuid, out: &mut Vec<Uuid>) -> KanbanResult<()> {
        if let Some(b) = self.board_of_column(id)? {
            out.push(b);
        }
        Ok(())
    }
    fn push_board_of_sprint(&self, id: Uuid, out: &mut Vec<Uuid>) -> KanbanResult<()> {
        if let Some(b) = self.board_of_sprint(id)? {
            out.push(b);
        }
        Ok(())
    }

    /// The board(s) a CONTENT mutation writes to. Empty for lifecycle/internal
    /// commands and for creating a brand-new board. Exhaustive match — a new
    /// command variant must declare its class here (fail-closed).
    fn content_target_boards(&self, cmd: &Command) -> KanbanResult<Vec<Uuid>> {
        let mut boards: Vec<Uuid> = Vec::new();
        match cmd {
            Command::Board(b) => match b {
                BoardCommand::Update(c) => boards.push(c.board_id),
                BoardCommand::SetTaskSort(c) => boards.push(c.board_id),
                BoardCommand::SetTaskListView(c) => boards.push(c.board_id),
                BoardCommand::ApplySettings(c) => boards.push(c.board_id),
                // lifecycle + internal + create-of-new-board (not yet archived):
                BoardCommand::Create(_)
                | BoardCommand::Delete(_)
                | BoardCommand::Archive(_)
                | BoardCommand::Restore(_)
                | BoardCommand::Import(_)
                | BoardCommand::RestoreSprintPool(_) => {}
            },
            Command::Column(c) => match c {
                ColumnCommand::Create(cc) => boards.push(cc.board_id),
                ColumnCommand::Update(cc) => {
                    self.push_board_of_column(cc.column_id, &mut boards)?
                }
                ColumnCommand::Delete(cc) => {
                    self.push_board_of_column(cc.column_id, &mut boards)?
                }
            },
            Command::Card(c) => match c {
                CardCommand::Create(cc) => boards.push(cc.board_id),
                CardCommand::Update(cc) => self.push_board_of_card(cc.card_id, &mut boards)?,
                CardCommand::Move(cc) => {
                    self.push_board_of_card(cc.card_id, &mut boards)?;
                    self.push_board_of_column(cc.new_column_id, &mut boards)?;
                }
                CardCommand::Delete(cc) => self.push_board_of_card(cc.card_id, &mut boards)?,
                CardCommand::Archive(cc) => {
                    for id in &cc.ids {
                        self.push_board_of_card(*id, &mut boards)?;
                    }
                }
                CardCommand::AssignToSprint(cc) => {
                    for id in &cc.ids {
                        self.push_board_of_card(*id, &mut boards)?;
                    }
                }
                CardCommand::UnassignFromSprint(cc) => {
                    self.push_board_of_card(cc.card_id, &mut boards)?
                }
                CardCommand::ApplyMetadata(cc) => {
                    self.push_board_of_card(cc.card_id, &mut boards)?
                }
                CardCommand::CompactPositions(cc) => {
                    self.push_board_of_column(cc.column_id, &mut boards)?
                }
                CardCommand::Restore(cc) => self.push_board_of_card(cc.card_id, &mut boards)?,
                // internal/synthetic:
                CardCommand::RestoreSprintAttachment(_) => {}
            },
            Command::Sprint(s) => match s {
                SprintCommand::Create(sc) => boards.push(sc.board_id),
                SprintCommand::Update(sc) => {
                    self.push_board_of_sprint(sc.sprint_id, &mut boards)?
                }
                SprintCommand::Activate(sc) => {
                    self.push_board_of_sprint(sc.sprint_id, &mut boards)?
                }
                SprintCommand::Complete(sc) => {
                    self.push_board_of_sprint(sc.sprint_id, &mut boards)?
                }
                SprintCommand::Cancel(sc) => {
                    self.push_board_of_sprint(sc.sprint_id, &mut boards)?
                }
                SprintCommand::Delete(sc) => {
                    self.push_board_of_sprint(sc.sprint_id, &mut boards)?
                }
            },
            Command::Dependency(d) => {
                let (s, t) = match d {
                    DependencyCommand::AddSpawns(e) => (e.source, e.target),
                    DependencyCommand::RemoveSpawns(e) => (e.source, e.target),
                    DependencyCommand::AddBlocks(e) => (e.source, e.target),
                    DependencyCommand::RemoveBlocks(e) => (e.source, e.target),
                    DependencyCommand::AddRelates(e) => (e.source, e.target),
                    DependencyCommand::RemoveRelates(e) => (e.source, e.target),
                    // Creating a subcard writes to its board directly.
                    DependencyCommand::CreateSubcard(sc) => {
                        boards.push(sc.board_id);
                        return Ok(boards);
                    }
                };
                self.push_board_of_card(s, &mut boards)?;
                self.push_board_of_card(t, &mut boards)?;
            }
            // Cascade commands are internal sub-steps of delete/undo flows.
            Command::Cascade(_) => {}
        }
        Ok(boards)
    }
}
