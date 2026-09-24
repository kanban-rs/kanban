use kanban_domain::{Board, BoardCreateOutcome, Card, CardCreateOutcome};
use kanban_domain::{Column, ColumnCreateOutcome, Sprint, SprintCreateOutcome};

fn _accepts_dyn(_: &mut dyn kanban_domain::MutationOperations) {}

#[test]
fn test_mutation_operations_is_object_safe() {
    fn _use(_: &mut dyn kanban_domain::MutationOperations) {}
}

#[test]
fn test_create_outcome_types_live_in_domain() {
    let board = Board::new("B", None::<String>);
    let outcome = BoardCreateOutcome {
        board: board.clone(),
        created: true,
    };
    assert!(outcome.created);
    assert_eq!(outcome.board.name, "B");
    assert_eq!(outcome.clone(), outcome);
    assert!(format!("{outcome:?}").contains("BoardCreateOutcome"));

    let column = Column::new(board.id, "Todo", 0);
    let column_outcome = ColumnCreateOutcome {
        column: column.clone(),
        created: false,
    };
    assert!(!column_outcome.created);
    assert_eq!(column_outcome.column.name, "Todo");
    assert_eq!(column_outcome.clone(), column_outcome);
    assert!(format!("{column_outcome:?}").contains("ColumnCreateOutcome"));

    let card = Card::new(board.id, column.id, "Task", 0);
    let card_outcome = CardCreateOutcome {
        card: card.clone(),
        created: true,
    };
    assert!(card_outcome.created);
    assert_eq!(card_outcome.card.title, "Task");
    assert_eq!(card_outcome.clone(), card_outcome);
    assert!(format!("{card_outcome:?}").contains("CardCreateOutcome"));

    let sprint = Sprint::new(board.id, 1, None, None::<String>);
    let sprint_outcome = SprintCreateOutcome {
        sprint: sprint.clone(),
        created: false,
    };
    assert!(!sprint_outcome.created);
    assert_eq!(sprint_outcome.sprint.board_id, board.id);
    assert_eq!(sprint_outcome.clone(), sprint_outcome);
    assert!(format!("{sprint_outcome:?}").contains("SprintCreateOutcome"));
}
