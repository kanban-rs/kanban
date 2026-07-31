//! Shared fixture for the command integration tests. Each test binary uses a
//! subset, so unused items would otherwise warn as dead code per-binary.
#![allow(dead_code)]

use kanban_backend_memory::InMemoryStore;
use kanban_domain::commands::CommandContext;

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

impl Default for TestContext {
    fn default() -> Self {
        Self::new()
    }
}
