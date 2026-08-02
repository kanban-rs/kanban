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
