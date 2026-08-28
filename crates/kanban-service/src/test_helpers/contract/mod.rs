pub mod archive;
pub mod board;
pub mod card;
pub mod column;
pub mod edge;
pub mod lifecycle;
pub mod movement;
pub mod prefix;
pub mod sprint;
pub mod sprint_log;

/// Assert two `Card`s are equal field-by-field (including `sprint_logs`), so a
/// round-trip mismatch names the exact field that drifted rather than dumping
/// the whole struct. Shared by the archive/edit/restore contract round-trips.
pub fn assert_card_eq(a: &kanban_domain::Card, b: &kanban_domain::Card) {
    assert_eq!(a.id, b.id, "card id");
    assert_eq!(a.column_id, b.column_id, "card column_id");
    assert_eq!(a.board_id, b.board_id, "card board_id");
    assert_eq!(a.title, b.title, "card title");
    assert_eq!(a.description, b.description, "card description");
    assert_eq!(a.priority, b.priority, "card priority");
    assert_eq!(a.status, b.status, "card status");
    assert_eq!(a.position, b.position, "card position");
    assert_eq!(a.due_date, b.due_date, "card due_date");
    assert_eq!(a.points, b.points, "card points");
    assert_eq!(a.card_number, b.card_number, "card card_number");
    assert_eq!(a.sprint_id, b.sprint_id, "card sprint_id");
    assert_eq!(a.created_at, b.created_at, "card created_at");
    assert_eq!(a.updated_at, b.updated_at, "card updated_at");
    assert_eq!(a.completed_at, b.completed_at, "card completed_at");
    assert_eq!(a.sprint_logs, b.sprint_logs, "card sprint_logs");
}
