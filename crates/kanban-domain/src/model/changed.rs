use super::Model;

/// Proof that a `Model` mutation ran and that whatever derives from it may be
/// stale. Minted only by the `Model` mutators, since the field is private, and
/// consumed by a `DerivedProjections` implementor. `any()` reports whether the
/// mutator actually wrote anything, so a mutator that ran but touched nothing
/// (an untouched `apply_resolved`, an `invalidate` of an empty `EntityIds`)
/// can tell its consumer there is nothing to recompute.
#[derive(Debug)]
#[must_use = "derived projections are stale until resync consumes this"]
pub struct ModelChanged {
    dirty: bool,
}

impl ModelChanged {
    pub(crate) fn new() -> Self {
        Self { dirty: true }
    }

    pub(crate) fn unchanged() -> Self {
        Self { dirty: false }
    }

    /// Whether the mutator that minted this receipt actually wrote.
    pub fn any(&self) -> bool {
        self.dirty
    }

    /// Folds two receipts into one, as a disjunction, so a caller performing
    /// several mutations resyncs once instead of discarding the extras.
    pub fn merge(self, other: Self) -> Self {
        Self {
            dirty: self.dirty || other.dirty,
        }
    }
}

/// State an application derives from a `Model` and must recompute whenever the
/// `Model` changes. Implemented where the derived state actually lives, so no
/// layer below it has to name the implementor.
pub trait DerivedProjections {
    /// Recompute everything derived from `model`. Consumes the receipt, so this
    /// cannot be called without a mutation having produced one. An implementor
    /// may skip its recompute entirely when `changed.any()` is false.
    fn resync(&mut self, model: &Model, changed: ModelChanged);
}

/// For a path that derives nothing from the `Model`, so nothing can go stale:
/// a one-shot command that renders straight from the `Model` and keeps nothing,
/// or a test fixture. Recomputing nothing is the complete behaviour there, not
/// a stub. Most applications hold real derived state and implement the trait
/// over their own projection type instead.
#[derive(Debug, Default)]
pub struct NoProjections;

impl DerivedProjections for NoProjections {
    fn resync(&mut self, _model: &Model, _changed: ModelChanged) {}
}

#[cfg(test)]
mod tests {
    use super::super::Model;
    use crate::{DerivedProjections, NoProjections, Snapshot};
    use uuid::Uuid;

    #[test]
    fn test_merge_folds_two_receipts_into_one() {
        let mut m = Model::default();
        let a = m.load_from_snapshot(Snapshot::default());
        let b = m.load_from_snapshot(Snapshot::default());
        let merged = a.merge(b);
        NoProjections.resync(&m, merged);
    }

    #[test]
    fn test_model_changed_keeps_its_must_use_attribute_and_private_field() {
        let prod = include_str!("changed.rs")
            .replace("\r\n", "\n")
            .split("#[cfg(test)]")
            .next()
            .map(str::to_owned)
            .unwrap();
        assert!(prod.contains(
            "#[must_use = \"derived projections are stale until resync consumes this\"]"
        ));
        assert!(prod.contains("pub struct ModelChanged {"));
        assert!(prod.contains("\n    dirty: bool,\n"));
        assert!(!prod.contains("pub dirty"));
    }

    #[test]
    fn test_merge_of_an_unchanged_and_a_changed_receipt_reports_changed() {
        let mut m = Model::default();

        let unchanged_a = m.apply_resolved(crate::Resolved::default());
        let unchanged_b = m.apply_resolved(crate::Resolved::default());
        assert!(!unchanged_a.merge(unchanged_b).any());

        let unchanged = m.apply_resolved(crate::Resolved::default());
        let changed = m.load_from_snapshot(Snapshot::default());
        assert!(unchanged.merge(changed).any());

        let changed = m.load_from_snapshot(Snapshot::default());
        let unchanged = m.apply_resolved(crate::Resolved::default());
        assert!(changed.merge(unchanged).any());

        let changed_a = m.load_from_snapshot(Snapshot::default());
        let changed_b = m.load_from_snapshot(Snapshot::default());
        assert!(changed_a.merge(changed_b).any());
    }

    #[test]
    fn test_no_projections_is_a_usable_derived_projections_substitute() {
        use crate::{Board, Card, Column};

        fn drive(p: &mut impl DerivedProjections, m: &mut Model) -> Uuid {
            let board = Board::new("B", None::<String>);
            let board_id = board.id;
            let col = Column::new(board.id, "Col", 0);
            let card = Card::new(board.id, col.id, "task", 0);
            let changed = m.load_from_snapshot(Snapshot {
                boards: vec![board],
                columns: vec![col],
                cards: vec![card],
                archived_boards: Vec::new(),
                ..Default::default()
            });
            p.resync(m, changed);
            board_id
        }

        let mut m = Model::default();
        let mut p = NoProjections;
        let board_id = drive(&mut p, &mut m);

        let state = m.board_cards_state(board_id);
        assert!(state.is_loaded());
        assert_eq!(state.loaded().unwrap().len(), 1);
    }
}
