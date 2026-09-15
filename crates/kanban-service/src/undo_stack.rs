//! Per-session undo/redo state for `KanbanContext`. In-memory; never
//! persisted; lifetime tied to the running session.

use kanban_domain::commands::Command;

/// A successfully-executed batch paired with its inverse. Both are
/// `Vec<Command>` so they round-trip through `KanbanContext::execute`.
#[derive(Debug, Clone)]
pub struct UndoEntry {
    pub forward: Vec<Command>,
    pub inverse: Vec<Command>,
}

/// Linear history with a cursor: `entries[0..cursor]` are applied,
/// `entries[cursor..]` are the redo tail. The cursor only moves on a
/// successful commit; failed undos / redos leave it pinned so a retry
/// sees the same entry.
#[derive(Debug, Default, Clone)]
pub struct UndoStack {
    entries: Vec<UndoEntry>,
    cursor: usize,
}

impl UndoStack {
    /// Upper bound on retained entries. Pushing past it evicts the oldest,
    /// so a long-running process retains a constant amount of undo state.
    pub const MAX_ENTRIES: usize = 100;

    pub fn new() -> Self {
        Self::default()
    }

    /// Append a new entry. Truncates the redo tail first; at capacity the
    /// oldest entry is dropped so the stack holds the newest
    /// [`MAX_ENTRIES`](Self::MAX_ENTRIES) batches.
    pub fn push(&mut self, entry: UndoEntry) {
        self.entries.truncate(self.cursor);
        if self.entries.len() >= Self::MAX_ENTRIES {
            self.entries.remove(0);
        }
        self.entries.push(entry);
        self.cursor = self.entries.len();
    }

    /// Read-only view of the entry [`commit_undo`][Self::commit_undo]
    /// would apply.
    pub fn peek_undo(&self) -> Option<&UndoEntry> {
        if self.cursor == 0 {
            return None;
        }
        self.entries.get(self.cursor - 1)
    }

    /// Read-only view of the entry [`commit_redo`][Self::commit_redo]
    /// would apply.
    pub fn peek_redo(&self) -> Option<&UndoEntry> {
        if self.cursor >= self.entries.len() {
            return None;
        }
        self.entries.get(self.cursor)
    }

    /// Step the cursor back. Caller must have already applied the
    /// `peek_undo` entry's inverse. Returns `false` at the bottom.
    pub fn commit_undo(&mut self) -> bool {
        if self.cursor == 0 {
            return false;
        }
        self.cursor -= 1;
        true
    }

    /// Step the cursor forward. Caller must have already re-applied
    /// the `peek_redo` entry's forward batch. Returns `false` at the
    /// top.
    pub fn commit_redo(&mut self) -> bool {
        if self.cursor >= self.entries.len() {
            return false;
        }
        self.cursor += 1;
        true
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.cursor = 0;
    }

    pub fn undo_depth(&self) -> usize {
        self.cursor
    }

    pub fn redo_depth(&self) -> usize {
        self.entries.len().saturating_sub(self.cursor)
    }

    pub fn can_undo(&self) -> bool {
        self.cursor > 0
    }

    pub fn can_redo(&self) -> bool {
        self.cursor < self.entries.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kanban_domain::commands::{BoardCommand, CreateBoard, DeleteBoard};
    use uuid::Uuid;

    fn make_pair(name: &str) -> UndoEntry {
        let id = Uuid::new_v4();
        let forward = vec![Command::Board(BoardCommand::Create(CreateBoard {
            id,
            name: name.into(),
            card_prefix: None,
            position: 0,
        }))];
        let inverse = vec![Command::Board(BoardCommand::Delete(DeleteBoard {
            board_id: id,
        }))];
        UndoEntry { forward, inverse }
    }

    #[test]
    fn test_new_stack_is_empty() {
        let stack = UndoStack::new();
        assert_eq!(stack.undo_depth(), 0);
        assert_eq!(stack.redo_depth(), 0);
        assert!(!stack.can_undo());
        assert!(!stack.can_redo());
    }

    #[test]
    fn test_push_advances_cursor() {
        let mut stack = UndoStack::new();
        stack.push(make_pair("A"));
        assert_eq!(stack.undo_depth(), 1);
        assert!(stack.can_undo());
        assert!(!stack.can_redo());
    }

    #[test]
    fn test_peek_then_commit_walks_back_in_reverse_order() {
        let mut stack = UndoStack::new();
        stack.push(make_pair("A"));
        stack.push(make_pair("B"));

        let e = stack.peek_undo().expect("peek B");
        assert!(format!("{e:?}").contains("B"));
        assert!(stack.commit_undo());

        let e = stack.peek_undo().expect("peek A");
        assert!(format!("{e:?}").contains("A"));
        assert!(stack.commit_undo());

        assert!(stack.peek_undo().is_none());
        assert!(!stack.commit_undo());
    }

    #[test]
    fn test_peek_redo_walks_forward_after_undo() {
        let mut stack = UndoStack::new();
        stack.push(make_pair("A"));
        stack.push(make_pair("B"));
        assert!(stack.commit_undo());
        assert!(stack.commit_undo());

        let e = stack.peek_redo().expect("redo A");
        assert!(format!("{e:?}").contains("A"));
        assert!(stack.commit_redo());

        let e = stack.peek_redo().expect("redo B");
        assert!(format!("{e:?}").contains("B"));
        assert!(stack.commit_redo());

        assert!(stack.peek_redo().is_none());
        assert!(!stack.commit_redo());
    }

    #[test]
    fn test_peek_undo_does_not_mutate_cursor() {
        let mut stack = UndoStack::new();
        stack.push(make_pair("A"));
        stack.push(make_pair("B"));
        let depth = stack.undo_depth();
        let _ = stack.peek_undo();
        let _ = stack.peek_undo();
        assert_eq!(stack.undo_depth(), depth);
    }

    #[test]
    fn test_peek_undo_without_commit_lets_retry_see_same_entry() {
        let mut stack = UndoStack::new();
        stack.push(make_pair("A"));
        stack.push(make_pair("B"));
        let first = stack.peek_undo().unwrap();
        let first_dbg = format!("{first:?}");
        // Imagine the inverse fails; commit is not called. A retry must
        // see the same entry on top.
        let retry = stack.peek_undo().unwrap();
        assert_eq!(format!("{retry:?}"), first_dbg);
        assert_eq!(stack.undo_depth(), 2);
    }

    #[test]
    fn test_push_truncates_redo_tail() {
        let mut stack = UndoStack::new();
        stack.push(make_pair("A"));
        stack.push(make_pair("B"));
        assert!(stack.commit_undo());
        assert_eq!(stack.redo_depth(), 1);

        stack.push(make_pair("C"));
        assert_eq!(stack.undo_depth(), 2);
        assert_eq!(stack.redo_depth(), 0);
    }

    #[test]
    fn test_push_past_cap_drops_oldest_and_keeps_newest_in_order() {
        let mut stack = UndoStack::new();
        let total = UndoStack::MAX_ENTRIES + 3;
        for i in 0..total {
            stack.push(make_pair(&format!("E{i}")));
        }
        assert_eq!(stack.undo_depth(), UndoStack::MAX_ENTRIES);
        assert_eq!(stack.redo_depth(), 0);

        let dbg = format!("{stack:?}");
        for name in ["E0", "E1", "E2"] {
            assert!(!dbg.contains(&format!("\"{name}\"")));
        }

        for i in (3..total).rev() {
            let e = stack.peek_undo().expect("entry present");
            assert!(
                format!("{e:?}").contains(&format!("\"E{i}\"")),
                "expected E{i} at this depth, got {e:?}"
            );
            assert!(stack.commit_undo());
        }
        assert!(stack.peek_undo().is_none());
    }

    #[test]
    fn test_undo_after_overflow_replays_newest_entry() {
        let mut stack = UndoStack::new();
        let total = UndoStack::MAX_ENTRIES + 1;
        for i in 0..total {
            stack.push(make_pair(&format!("E{i}")));
        }
        assert_eq!(stack.undo_depth(), UndoStack::MAX_ENTRIES);

        let e = stack.peek_undo().expect("entry present");
        assert!(format!("{e:?}").contains(&format!("\"E{}\"", total - 1)));
        assert!(stack.commit_undo());
        assert_eq!(stack.redo_depth(), 1);
    }

    #[test]
    fn test_push_at_cap_after_undo_truncates_redo_without_evicting() {
        let mut stack = UndoStack::new();
        for i in 0..UndoStack::MAX_ENTRIES {
            stack.push(make_pair(&format!("E{i}")));
        }
        assert!(stack.commit_undo());
        assert_eq!(stack.redo_depth(), 1);

        stack.push(make_pair("X"));
        assert_eq!(stack.undo_depth(), UndoStack::MAX_ENTRIES);
        assert_eq!(stack.redo_depth(), 0);

        for _ in 1..UndoStack::MAX_ENTRIES {
            assert!(stack.commit_undo());
        }
        let bottom = stack.peek_undo().expect("bottom entry present");
        assert!(format!("{bottom:?}").contains("\"E0\""));
        assert!(stack.commit_undo());
        assert!(!stack.commit_undo());
    }

    #[test]
    fn test_clear_resets_state() {
        let mut stack = UndoStack::new();
        stack.push(make_pair("A"));
        stack.push(make_pair("B"));
        stack.clear();
        assert!(!stack.can_undo());
        assert!(!stack.can_redo());
        assert_eq!(stack.undo_depth(), 0);
    }
}
