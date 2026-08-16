use chrono::{DateTime, Utc};
use kanban_domain::KanbanResult;

/// A single git commit that references a card's identifier.
#[derive(Debug, Clone, PartialEq)]
pub struct CommitRef {
    pub short_hash: String,
    pub subject: String,
    pub author: String,
    pub committed_at: DateTime<Utc>,
}

/// Read-only access to the git commits that implement a card.
///
/// Synchronous: implementations shell out to a blocking `git log`, called
/// once when card detail opens, never per render frame.
pub trait GitProvider {
    fn commits_for_tag(&self, tag: &str) -> KanbanResult<Vec<CommitRef>>;
}
