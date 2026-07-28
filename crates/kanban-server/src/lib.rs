//! kanban-server: HTTP API surface over the shared `kanban-service`.
//!
//! `app::router` is the single `Router` composition point; entity route
//! cards extend it rather than building their own.

pub mod app;
pub mod error;
pub mod state;
pub mod watch;

pub mod routes {
    pub mod boards;
    pub mod cards;
    pub mod columns;
}

pub mod handlers {
    pub mod boards;
    pub mod cards;
    pub mod columns;
    pub mod sprints;
}
