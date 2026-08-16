use super::InMemoryStore;
use kanban_domain::{KanbanResult, Prefix};

impl InMemoryStore {
    pub(crate) fn get_prefix_impl(&self, name: &str) -> KanbanResult<Option<Prefix>> {
        let wanted = Prefix::normalize(name);
        Ok(self
            .read_state()?
            .prefixes
            .iter()
            .find(|p| Prefix::normalize(&p.name) == wanted)
            .cloned())
    }

    pub(crate) fn list_prefixes_impl(&self) -> KanbanResult<Vec<Prefix>> {
        Ok(self.read_state()?.prefixes.clone())
    }

    pub(crate) fn upsert_prefix_impl(&self, prefix: Prefix) -> KanbanResult<()> {
        let mut state = self.write_state()?;
        // Normalised on the way in rather than trusted from the caller, so the
        // stored name satisfies `Prefix`'s invariant however it was built.
        // SQLite gets this for free: its ON CONFLICT never updates the name.
        let prefix = Prefix {
            name: Prefix::normalize(&prefix.name),
            ..prefix
        };
        // Replace in place rather than push-and-dedup: two spellings of one
        // name are one namespace, and a second row would let two owners
        // allocate the same number.
        match state
            .prefixes
            .iter_mut()
            .find(|p| Prefix::normalize(&p.name) == prefix.name)
        {
            Some(existing) => *existing = prefix,
            None => state.prefixes.push(prefix),
        }
        Ok(())
    }
}
