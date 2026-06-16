use crate::command_batch::CommandBatch;
use crate::KanbanResult;

/// Append-only chronological log of executed command batches with provenance.
pub trait CommandStore: Send + Sync {
    /// Append one batch as a single entry. Returns the new entry count.
    fn append_batch(&self, batch: &CommandBatch) -> KanbanResult<u64>;

    fn batch_count(&self) -> KanbanResult<u64>;

    /// Half-open range `[from, to)` of batches in chronological order.
    fn load_batches(&self, from: u64, to: u64) -> KanbanResult<Vec<CommandBatch>>;

    /// Atomic count + load. Default is non-atomic; backends with interior
    /// locks should override.
    fn load_all_batches(&self) -> KanbanResult<(Vec<CommandBatch>, u64)> {
        let count = self.batch_count()?;
        let batches = self.load_batches(0, count)?;
        Ok((batches, count))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command_batch::CommandBatch;
    use crate::commands::{BoardCommand, Command, CreateBoard};
    use crate::InMemoryStore;
    use uuid::Uuid;

    fn make_board_batch(name: &str) -> CommandBatch {
        CommandBatch::from(vec![Command::Board(BoardCommand::Create(CreateBoard {
            id: Uuid::new_v4(),
            name: name.into(),
            card_prefix: None,
            position: 0,
        }))])
    }

    #[test]
    fn test_append_batch_returns_count() {
        let store = InMemoryStore::new();
        let count = store.append_batch(&make_board_batch("B1")).unwrap();
        assert_eq!(count, 1);

        let count = store.append_batch(&make_board_batch("B2")).unwrap();
        assert_eq!(count, 2);
    }

    #[test]
    fn test_batch_count_starts_at_zero() {
        let store = InMemoryStore::new();
        assert_eq!(store.batch_count().unwrap(), 0);
    }

    #[test]
    fn test_load_batches_returns_slice() {
        let store = InMemoryStore::new();
        store.append_batch(&make_board_batch("B1")).unwrap();
        store.append_batch(&make_board_batch("B2")).unwrap();
        store.append_batch(&make_board_batch("B3")).unwrap();

        let batches = store.load_batches(0, 3).unwrap();
        assert_eq!(batches.len(), 3);
    }

    #[test]
    fn test_load_range_is_exclusive_end() {
        let store = InMemoryStore::new();
        store.append_batch(&make_board_batch("B1")).unwrap();
        store.append_batch(&make_board_batch("B2")).unwrap();

        let batches = store.load_batches(0, 1).unwrap();
        assert_eq!(batches.len(), 1);

        let batches = store.load_batches(1, 2).unwrap();
        assert_eq!(batches.len(), 1);

        let batches = store.load_batches(0, 2).unwrap();
        assert_eq!(batches.len(), 2);
    }

    #[test]
    fn test_load_all_batches_returns_consistent_count_and_data() {
        let store = InMemoryStore::new();
        store.append_batch(&make_board_batch("B1")).unwrap();
        store.append_batch(&make_board_batch("B2")).unwrap();
        store.append_batch(&make_board_batch("B3")).unwrap();

        let (batches, count) = store.load_all_batches().unwrap();
        assert_eq!(count, 3);
        assert_eq!(batches.len(), 3);
    }

    #[test]
    fn test_batch_stores_multiple_commands() {
        let store = InMemoryStore::new();
        let batch = CommandBatch::from(vec![
            Command::Board(BoardCommand::Create(CreateBoard {
                id: Uuid::new_v4(),
                name: "B1".into(),
                card_prefix: None,
                position: 0,
            })),
            Command::Board(BoardCommand::Create(CreateBoard {
                id: Uuid::new_v4(),
                name: "B2".into(),
                card_prefix: None,
                position: 1,
            })),
        ]);
        store.append_batch(&batch).unwrap();

        let batches = store.load_batches(0, 1).unwrap();
        assert_eq!(batches.len(), 1);
        assert_eq!(batches[0].commands.len(), 2);
    }
}
