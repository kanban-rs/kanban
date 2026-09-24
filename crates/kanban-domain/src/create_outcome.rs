use crate::{Board, Card, Column, Sprint};

/// Result of an idempotent PUT-create for a board: the resulting board plus
/// whether this call created it (`true`, HTTP 201) or replaced an existing
/// one (`false`, HTTP 200). The HTTP binding lives in the server seam; the
/// domain and service tiers only report which arm ran.
#[derive(Debug, Clone, PartialEq)]
pub struct BoardCreateOutcome {
    pub board: Board,
    pub created: bool,
}

/// Result of an idempotent PUT-create for a card: the resulting card plus
/// whether this call created it (`true`, HTTP 201) or replaced an existing
/// one (`false`, HTTP 200). The HTTP binding lives in the server seam; the
/// domain and service tiers only report which arm ran.
#[derive(Debug, Clone, PartialEq)]
pub struct CardCreateOutcome {
    pub card: Card,
    pub created: bool,
}

/// Result of an idempotent PUT-create for a column: the resulting column plus
/// whether this call created it (`true`, HTTP 201) or replaced an existing
/// one (`false`, HTTP 200). The HTTP binding lives in the server seam; the
/// domain and service tiers only report which arm ran.
#[derive(Debug, Clone, PartialEq)]
pub struct ColumnCreateOutcome {
    pub column: Column,
    pub created: bool,
}

/// Result of an idempotent PUT-create for a sprint: the resulting sprint plus
/// whether this call created it (`true`, HTTP 201) or replaced an existing
/// one (`false`, HTTP 200). The HTTP binding lives in the server seam; the
/// domain and service tiers only report which arm ran.
#[derive(Debug, Clone, PartialEq)]
pub struct SprintCreateOutcome {
    pub sprint: Sprint,
    pub created: bool,
}
