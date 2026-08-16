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
        let normalized = Prefix::normalize(&prefix.name);
        // Replace in place rather than push-and-dedup: two spellings of one
        // name are one namespace, and a second row would let two owners
        // allocate the same number.
        match state
            .prefixes
            .iter_mut()
            .find(|p| Prefix::normalize(&p.name) == normalized)
        {
            Some(existing) => *existing = prefix,
            None => state.prefixes.push(prefix),
        }
        Ok(())
    }
}
