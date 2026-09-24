use crate::{Invalidation, KanbanResult};

/// The undo/redo capability surface every application answers.
///
/// `undo`/`redo` return `None` when there is nothing to undo/redo, and
/// `Some(invalidation)` naming what the inverse execution touched;
/// implementers must not discard it. `can_undo`/`can_redo` are cheap
/// capability probes.
///
/// Object-safe by construction: no default bodies, no generic methods.
pub trait UndoOperations {
    fn undo(&mut self) -> KanbanResult<Option<Invalidation>>;
    fn redo(&mut self) -> KanbanResult<Option<Invalidation>>;
    fn can_undo(&self) -> bool;
    fn can_redo(&self) -> bool;
}

#[cfg(test)]
mod tests {
    #[test]
    fn trait_is_object_safe() {
        fn _accepts_dyn(_: &mut dyn super::UndoOperations) {}
    }

    #[test]
    fn test_undo_operations_declares_every_method() {
        let src = include_str!("undo_operations.rs");
        let decl = &src[..src.find("#[cfg(test)]").unwrap()];
        assert_eq!(
            decl.matches("\n    fn ").count(),
            4,
            "UndoOperations must declare exactly 4 methods"
        );
        assert!(
            !decl.contains("\n    }"),
            "UndoOperations methods must have no default bodies"
        );
    }
}
