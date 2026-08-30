use super::Model;

#[derive(Debug)]
#[must_use = "derived projections are stale until resync consumes this"]
pub struct ModelChanged(());

impl ModelChanged {
    pub(crate) fn new() -> Self {
        Self(())
    }

    pub fn merge(self, _other: Self) -> Self {
        self
    }
}

pub trait DerivedProjections {
    fn resync(&mut self, model: &Model, changed: ModelChanged);
}

#[derive(Debug, Default)]
pub struct NoProjections;

impl DerivedProjections for NoProjections {
    fn resync(&mut self, _model: &Model, _changed: ModelChanged) {}
}

#[cfg(test)]
mod tests {
    use super::super::Model;
    use crate::{DerivedProjections, NoProjections, Snapshot};

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
            .split("#[cfg(test)]")
            .next()
            .unwrap();
        assert!(prod.contains(
            "#[must_use = \"derived projections are stale until resync consumes this\"]"
        ));
        assert!(prod.contains("pub struct ModelChanged(());"));
    }

    #[test]
    fn test_no_projections_is_a_usable_derived_projections_substitute() {
        use crate::{Board, Card, Column};

        fn drive(p: &mut impl DerivedProjections, m: &mut Model) {
            let board = Board::new("B", None::<String>);
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
        }

        let mut m = Model::default();
        let mut p = NoProjections;
        drive(&mut p, &mut m);

        assert!(m.cards_state().is_loaded());
        assert_eq!(m.cards_state().loaded_or_empty().len(), 1);
    }
}
