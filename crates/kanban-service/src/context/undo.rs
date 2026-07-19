use super::KanbanContext;
use kanban_core::{ClientId, KANBAN_VERSION};
use kanban_domain::commands::{Command, CommandContext};
use kanban_domain::{DataStore, KanbanResult};
use std::sync::Arc;
use uuid::Uuid;

impl KanbanContext {
    /// Execute a batch as one undo unit. Entity mutations, inverse
    /// capture, and audit-log append run inside one transaction —
    /// either all commit or all roll back.
    ///
    /// Each command's inverse is captured against the state the
    /// previous command left behind. The composed inverse is the
    /// per-command inverses in reverse order, so undoing each `Fk_inv`
    /// runs against the state `Fk` itself saw at capture time.
    pub fn execute(&mut self, commands: Vec<Command>) -> KanbanResult<()> {
        self.reject_content_mutation_on_archived_board(&commands)?;
        let backend = Arc::clone(&self.backend);
        let cmds = &commands;
        let mut per_cmd_inverses: Vec<Vec<Command>> = Vec::new();
        self.backend.with_transaction(&mut || {
            let store: &dyn DataStore = backend.as_data_store();
            let ctx = CommandContext { store };
            for cmd in cmds.iter() {
                per_cmd_inverses.push(cmd.capture_inverse(store)?);
                cmd.execute(&ctx)?;
            }
            let batch = kanban_domain::CommandBatch {
                commands: cmds.clone(),
                correlation_id: Uuid::new_v4(),
                // nil locally; the HTTP layer assigns the real client identity (KAN-751)
                issued_by: ClientId::nil(),
                timestamp: chrono::Utc::now(),
                app_type: self.app_type,
                app_version: KANBAN_VERSION.to_string(),
                session_id: self.session_id,
            };
            backend.append_batch(&batch)?;
            Ok(())
        })?;
        let inverses: Vec<Command> = per_cmd_inverses.into_iter().rev().flatten().collect();

        self.undo_stack.push(crate::undo_stack::UndoEntry {
            forward: commands,
            inverse: inverses,
        });

        self.dirty = true;
        Ok(())
    }

    /// Undo the most recent batch via inverse-command execution.
    /// The cursor advances only if the inverse commits successfully —
    /// a failed undo leaves the stack ready to retry the same entry.
    pub fn undo(&mut self) -> KanbanResult<bool> {
        let inverse = match self.undo_stack.peek_undo() {
            Some(entry) => entry.inverse.clone(),
            None => return Ok(false),
        };
        let backend = Arc::clone(&self.backend);
        let inv = &inverse;
        self.backend.with_transaction(&mut || {
            let store: &dyn DataStore = backend.as_data_store();
            let ctx = CommandContext { store };
            inv.iter().try_for_each(|cmd| cmd.execute(&ctx))
        })?;
        self.undo_stack.commit_undo();
        self.dirty = true;
        Ok(true)
    }

    /// Redo the next undone batch via forward-command execution.
    /// The cursor advances only if the forward batch commits — a failed
    /// redo leaves the stack ready to retry the same entry.
    pub fn redo(&mut self) -> KanbanResult<bool> {
        let forward = match self.undo_stack.peek_redo() {
            Some(entry) => entry.forward.clone(),
            None => return Ok(false),
        };
        let backend = Arc::clone(&self.backend);
        let fwd = &forward;
        self.backend.with_transaction(&mut || {
            let store: &dyn DataStore = backend.as_data_store();
            let ctx = CommandContext { store };
            fwd.iter().try_for_each(|cmd| cmd.execute(&ctx))
        })?;
        self.undo_stack.commit_redo();
        self.dirty = true;
        Ok(true)
    }

    pub fn can_undo(&self) -> bool {
        self.undo_stack.can_undo()
    }

    pub fn can_redo(&self) -> bool {
        self.undo_stack.can_redo()
    }

    /// Drop the per-session undo/redo history. The audit log is
    /// append-only and is not touched.
    pub fn clear_history(&mut self) -> KanbanResult<()> {
        self.undo_stack.clear();
        Ok(())
    }

    pub fn undo_depth(&self) -> usize {
        self.undo_stack.undo_depth()
    }

    pub fn redo_depth(&self) -> usize {
        self.undo_stack.redo_depth()
    }
}
