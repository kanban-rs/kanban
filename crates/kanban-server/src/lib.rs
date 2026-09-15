//! kanban-server: HTTP API surface over the shared `kanban-service`.
//!
//! `app::router` is the single `Router` composition point; entity route
//! cards extend it rather than building their own.

pub mod app;
pub mod capabilities;
pub mod client_ident;
pub mod error;
pub mod etag;
pub mod layers;
pub mod model_read;
pub mod pagination;
pub mod scope;
mod session_ops;
pub mod state;
pub mod stores;
pub mod watch;

#[cfg(feature = "test-helpers")]
pub mod test_helpers;

pub mod routes {
    pub mod boards;
    pub mod cards;
    pub mod cards_batch;
    pub mod columns;
    pub mod events;
    pub mod graph;
    pub mod prefixes;
    pub mod sprints;
    pub mod sprints_lifecycle;
    pub mod transfer;
}

pub mod handlers {
    pub mod boards;
    pub mod cards;
    pub mod columns;
    pub mod sprints;
}
