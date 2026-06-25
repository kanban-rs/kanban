//! kanban-server: HTTP API surface over the shared `kanban-service`.
//!
//! The transport (axum router, listeners) is still a stub; the typed handler
//! seams below are the create-from-spec funnel a future router binds to.

pub mod handlers {
    pub mod boards;
    pub mod cards;
    pub mod columns;
}
