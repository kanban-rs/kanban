use super::*;

impl Model {
    pub fn boards_state(&self) -> &LoadState<Vec<Board>> {
        &self.boards
    }

    /// The LIVE boards (unified collection minus the archived heads), in board
    /// order. The live projects panel and every live-only quantity (first-board
    /// default selection, new-board position, live counts) resolve through this,
    /// so broadening `boards_state()` to the unified collection cannot leak
    /// archived heads into live semantics.
    pub fn live_boards(&self) -> impl Iterator<Item = &Board> {
        self.boards_state()
            .loaded_or_empty()
            .iter()
            .filter(|b| !self.archived_board_ids.contains(&b.id))
    }

    pub fn archived_boards(&self) -> &[ArchivedBoard] {
        self.archived_boards.as_deref().unwrap_or(&[])
    }

    /// The whole-store archived-board-marker tier, `Loaded` exactly when a
    /// snapshot or resolve pass has supplied it, `Failed` when the last
    /// attempt errored without a subsequent successful one. Independent of
    /// `archived_boards()`, which returns `&[]` for both `NotLoaded` and a
    /// genuinely empty `Loaded`.
    pub fn archived_boards_state(&self) -> LoadState<&[ArchivedBoard]> {
        if let Some(err) = &self.archived_boards_error {
            return LoadState::Failed(std::sync::Arc::clone(err));
        }
        match &self.archived_boards {
            Some(markers) => LoadState::Loaded(markers.as_slice()),
            None => LoadState::NotLoaded,
        }
    }

    /// Distinguishes a genuinely empty archived-boards tier from one that has
    /// never been absorbed (`archived_boards()` returns `&[]` for both).
    pub fn archived_boards_absorbed(&self) -> bool {
        self.archived_boards.is_some()
    }

    /// Ids of the archived boards. The heads themselves live in the unified
    /// `boards_state()` collection; this set records which of them are archived (built
    /// from the markers). The live/archived partition is a presentation concern
    /// and lives on the view layer's `Controller`; this set is what backs that
    /// split.
    pub fn archived_board_ids(&self) -> &HashSet<Uuid> {
        &self.archived_board_ids
    }

    pub fn board_by_id_state(&self, id: Uuid) -> LoadState<&Board> {
        match self.boards.as_ref() {
            LoadState::Loaded(boards) => {
                match self.board_index.get(&id).and_then(|&idx| boards.get(idx)) {
                    Some(board) => LoadState::Loaded(board),
                    None => LoadState::Missing,
                }
            }
            LoadState::NotLoaded => LoadState::NotLoaded,
            LoadState::Missing => LoadState::Missing,
            LoadState::Failed(e) => LoadState::Failed(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resolved::Collection;
    use crate::{ArchivedBoard, Board, KanbanError, Resolved, Snapshot};
    use std::sync::Arc;

    #[test]
    fn test_default_model_returns_empty_archived_board_slices() {
        let m = Model::default();
        assert!(m.archived_boards().is_empty());
        assert!(m.archived_board_ids().is_empty());
        assert!(m
            .board_by_id_state(Uuid::new_v4())
            .loaded()
            .copied()
            .is_none());
    }

    #[test]
    fn test_archived_boards_state_is_not_loaded_by_default() {
        let m = Model::default();
        assert!(m.archived_boards_state().is_not_loaded());
    }

    #[test]
    fn test_a_failed_archived_board_read_leaves_the_marker_sets_alone() {
        let mut m = Model::default();
        let board = Board::new("Archived", None::<String>);
        let marker = ArchivedBoard::now(board.id);

        let _ = m.apply_resolved(Resolved {
            archived_boards: Collection {
                all: LoadState::Loaded(vec![marker]),
                ..Default::default()
            },
            ..Default::default()
        });
        assert!(m.archived_boards_state().is_loaded());
        assert!(m.archived_board_ids().contains(&board.id));

        let err = Arc::new(KanbanError::unsupported("boom"));
        let _ = m.apply_resolved(Resolved {
            archived_boards: Collection {
                all: LoadState::Failed(err),
                ..Default::default()
            },
            ..Default::default()
        });

        assert!(m.archived_boards_state().is_failed());
        assert!(m.archived_board_ids().contains(&board.id));
        assert_eq!(m.archived_boards().len(), 1);
    }

    #[test]
    fn test_archived_boards_absorbed_distinguishes_never_loaded_from_genuinely_empty() {
        let mut m = Model::default();
        assert!(!m.archived_boards_absorbed());

        let _ = m.load_from_snapshot(Snapshot {
            archived_boards: Vec::new(),
            ..Default::default()
        });
        assert!(m.archived_boards().is_empty());
        assert!(m.archived_boards_absorbed());
    }

    #[test]
    fn test_board_by_id_resolves_live_and_archived_from_one_collection() {
        // After unification `boards_state()` holds live AND archived heads, and
        // `board_by_id_state` resolves either from the single collection — no
        // `or_else(archived_board())` re-join.
        use crate::Archived;
        let mut m = Model::default();
        let live = Board::new("Live", None::<String>);
        let archived = Board::new("Archived", None::<String>);
        let live_id = live.id;
        let archived_id = archived.id;
        let _ = m.load_from_snapshot(Snapshot {
            // snapshot.boards carries BOTH heads; the marker names the archived one.
            boards: vec![live.clone(), archived.clone()],
            archived_boards: vec![Archived::now(archived_id)],
            ..Default::default()
        });

        // Both live and archived heads live in the single unified collection.
        assert_eq!(m.boards_state().loaded_or_empty().len(), 2);

        // The single index resolves both.
        assert_eq!(
            m.board_by_id_state(live_id).loaded().copied().map(|b| b.id),
            Some(live_id)
        );
        assert_eq!(
            m.board_by_id_state(archived_id)
                .loaded()
                .copied()
                .map(|b| b.name.clone()),
            Some("Archived".to_string())
        );

        // The archived-id set records which heads are archived.
        assert!(!m.archived_board_ids().contains(&live_id));
        assert!(m.archived_board_ids().contains(&archived_id));
    }

    #[test]
    fn test_archived_boards_view_filter_shows_archived_board_from_unified_collection() {
        // The archived-boards view filters the unified collection by
        // `archived_board_ids`. Assert an archived board is present through that
        // path, and a live board is not.
        use crate::Archived;
        let mut m = Model::default();
        let live = Board::new("Live", None::<String>);
        let archived = Board::new("Archived", None::<String>);
        let live_id = live.id;
        let archived_id = archived.id;
        let _ = m.load_from_snapshot(Snapshot {
            boards: vec![live.clone(), archived.clone()],
            archived_boards: vec![Archived::now(archived_id)],
            ..Default::default()
        });

        let displayed: Vec<Uuid> = m
            .boards_state()
            .loaded_or_empty()
            .iter()
            .filter(|b| m.archived_board_ids().contains(&b.id))
            .map(|b| b.id)
            .collect();
        assert_eq!(displayed, vec![archived_id]);
        assert!(!displayed.contains(&live_id));
    }

    #[test]
    fn test_live_board_filter_excludes_archived_board_from_unified_collection() {
        // Guard: the LIVE projects panel filters archived heads OUT of the
        // unified collection (analogue of T1a's live-branch card fix).
        use crate::Archived;
        let mut m = Model::default();
        let live = Board::new("Live", None::<String>);
        let archived = Board::new("Archived", None::<String>);
        let live_id = live.id;
        let archived_id = archived.id;
        let _ = m.load_from_snapshot(Snapshot {
            boards: vec![live.clone(), archived.clone()],
            archived_boards: vec![Archived::now(archived_id)],
            ..Default::default()
        });

        let live_only: Vec<Uuid> = m
            .boards_state()
            .loaded_or_empty()
            .iter()
            .filter(|b| !m.archived_board_ids().contains(&b.id))
            .map(|b| b.id)
            .collect();
        assert_eq!(live_only, vec![live_id]);
        assert!(!live_only.contains(&archived_id));
    }

    #[test]
    fn test_board_by_id_missing_id_returns_none() {
        let mut m = Model::default();
        let _ = m.load_from_snapshot(Snapshot::default());
        assert!(m
            .board_by_id_state(Uuid::new_v4())
            .loaded()
            .copied()
            .is_none());
    }

    #[test]
    fn test_boards_state_is_not_loaded_before_load_from_snapshot() {
        let m = Model::default();
        assert!(m.boards_state().is_not_loaded());
    }

    #[test]
    fn test_boards_state_is_loaded_and_empty_after_an_empty_snapshot() {
        let mut m = Model::default();
        let _ = m.load_from_snapshot(Snapshot::default());
        assert!(m.boards_state().is_loaded());
        assert!(m.boards_state().loaded().unwrap().is_empty());
        assert!(m.boards_state().loaded_or_empty().is_empty());
    }

    #[test]
    fn test_board_by_id_state_is_not_loaded_before_any_snapshot() {
        let m = Model::default();
        let state = m.board_by_id_state(Uuid::new_v4());
        assert!(state.is_not_loaded());
        assert!(!state.is_missing());
    }

    #[test]
    fn test_board_by_id_state_is_missing_for_an_absent_board_after_load() {
        let mut m = Model::default();
        let board = Board::new("B", None::<String>);
        let _ = m.load_from_snapshot(Snapshot {
            boards: vec![board],
            ..Default::default()
        });
        let state = m.board_by_id_state(Uuid::new_v4());
        assert!(state.is_missing());
        assert!(!state.is_not_loaded());
        assert!(state.is_terminal());
    }

    #[test]
    fn test_board_by_id_state_is_loaded_for_a_present_board() {
        let mut m = Model::default();
        let board = Board::new("B", None::<String>);
        let board_id = board.id;
        let _ = m.load_from_snapshot(Snapshot {
            boards: vec![board],
            ..Default::default()
        });
        let state = m.board_by_id_state(board_id);
        assert!(state.is_loaded());
        assert_eq!(state.loaded().map(|b| b.id), Some(board_id));
    }

    #[test]
    fn test_board_by_id_state_is_loaded_for_an_archived_board() {
        use crate::Archived;
        let mut m = Model::default();
        let live = Board::new("Live", None::<String>);
        let archived = Board::new("Archived", None::<String>);
        let archived_id = archived.id;
        let _ = m.load_from_snapshot(Snapshot {
            boards: vec![live, archived],
            archived_boards: vec![Archived::now(archived_id)],
            ..Default::default()
        });
        let state = m.board_by_id_state(archived_id);
        assert!(state.is_loaded());
        assert_eq!(
            state.loaded().map(|b| b.name.clone()),
            Some("Archived".to_string())
        );
        assert!(!state.is_missing());
    }
}
