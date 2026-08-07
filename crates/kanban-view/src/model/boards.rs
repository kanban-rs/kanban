use super::*;

impl Model {
    /// The single unified board collection (live AND archived heads). Which of
    /// these are archived is recorded in `archived_board_ids`; consumers that
    /// want only one subset filter this collection by that set (the projects
    /// panel does so via `displayed_boards`). Mirrors the unified `all_cards()`.
    pub fn boards(&self) -> &[Board] {
        self.boards.as_deref().unwrap_or(&[])
    }

    /// The LIVE boards (unified collection minus the archived heads), in board
    /// order. The live projects panel and every live-only quantity (first-board
    /// default selection, new-board position, live counts) resolve through this,
    /// so broadening `boards()` to the unified collection cannot leak archived
    /// heads into live semantics.
    pub fn live_boards(&self) -> impl Iterator<Item = &Board> {
        self.boards()
            .iter()
            .filter(|b| !self.archived_board_ids.contains(&b.id))
    }

    /// The ARCHIVED heads in the CONFIGURED archived-boards order (default
    /// archived_at DESC — newest first). This is what the ArchivedBoardsView
    /// renders AND what its restore / permanent-delete affordances index into:
    /// both read this same cached, sorted partition so the rendered row and the
    /// selected id stay consistent under any sort. Independent of the transient
    /// `AppMode` (a confirm dialog opened over the archived view still resolves
    /// the archived head), because it reads the cached partition, not the mode.
    pub fn archived_boards_view(&self) -> impl Iterator<Item = &Board> {
        self.displayed_boards_archived.iter()
    }

    pub fn archived_boards(&self) -> &[ArchivedBoard] {
        self.archived_boards.as_deref().unwrap_or(&[])
    }

    /// Ids of the archived boards. The heads themselves live in the unified
    /// `boards()` collection; this set records which of them are archived (built
    /// from the markers). The live/archived partition is precomputed on load and
    /// served by [`displayed_boards`](Self::displayed_boards); this set backs that
    /// split.
    pub fn archived_board_ids(&self) -> &HashSet<Uuid> {
        &self.archived_board_ids
    }

    /// The boards the projects panel should display, selected by `want_archived`.
    /// Borrow of the partition cached on `load_from_snapshot`; the mode decision
    /// (live vs archived) lives at the `App` accessor, which passes the
    /// stack-aware base mode in.
    pub fn displayed_boards(&self, want_archived: bool) -> &[Board] {
        if want_archived {
            &self.displayed_boards_archived
        } else {
            &self.displayed_boards_live
        }
    }

    /// Resolve a board by id from the single unified collection (live AND
    /// archived heads). One index lookup — no live/archived re-join. It is
    /// deliberately archival-agnostic: a board is a board regardless of whether
    /// its head is archived.
    pub fn board_by_id(&self, id: Uuid) -> Option<&Board> {
        let &idx = self.board_index.get(&id)?;
        self.boards.as_ref()?.get(idx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kanban_domain::{Board, Snapshot};

    #[test]
    fn test_displayed_boards_partition_cached_on_load() {
        use kanban_domain::Archived;
        let mut m = Model::default();
        let live = Board::new("Live", None::<String>);
        let archived = Board::new("Archived", None::<String>);
        let live_id = live.id;
        let archived_id = archived.id;
        m.load_from_snapshot(Snapshot {
            boards: vec![live, archived],
            archived_boards: vec![Archived::now(archived_id)],
            ..Default::default()
        });

        let live_ids: Vec<Uuid> = m.displayed_boards(false).iter().map(|b| b.id).collect();
        let archived_ids: Vec<Uuid> = m.displayed_boards(true).iter().map(|b| b.id).collect();
        assert_eq!(live_ids, vec![live_id]);
        assert_eq!(archived_ids, vec![archived_id]);
    }

    #[test]
    fn test_default_model_returns_empty_archived_board_slices() {
        let m = Model::default();
        assert!(m.archived_boards().is_empty());
        assert!(m.archived_board_ids().is_empty());
        assert!(m.board_by_id(Uuid::new_v4()).is_none());
    }

    #[test]
    fn test_board_by_id_resolves_live_and_archived_from_one_collection() {
        // After unification `boards()` holds live AND archived heads, and
        // `board_by_id` resolves either from the single collection — no
        // `or_else(archived_board())` re-join.
        use kanban_domain::Archived;
        let mut m = Model::default();
        let live = Board::new("Live", None::<String>);
        let archived = Board::new("Archived", None::<String>);
        let live_id = live.id;
        let archived_id = archived.id;
        m.load_from_snapshot(Snapshot {
            // snapshot.boards carries BOTH heads; the marker names the archived one.
            boards: vec![live.clone(), archived.clone()],
            archived_boards: vec![Archived::now(archived_id)],
            ..Default::default()
        });

        // Both live and archived heads live in the single unified collection.
        assert_eq!(m.boards().len(), 2);

        // The single index resolves both.
        assert_eq!(m.board_by_id(live_id).map(|b| b.id), Some(live_id));
        assert_eq!(
            m.board_by_id(archived_id).map(|b| b.name.clone()),
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
        use kanban_domain::Archived;
        let mut m = Model::default();
        let live = Board::new("Live", None::<String>);
        let archived = Board::new("Archived", None::<String>);
        let live_id = live.id;
        let archived_id = archived.id;
        m.load_from_snapshot(Snapshot {
            boards: vec![live.clone(), archived.clone()],
            archived_boards: vec![Archived::now(archived_id)],
            ..Default::default()
        });

        let displayed: Vec<Uuid> = m
            .boards()
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
        use kanban_domain::Archived;
        let mut m = Model::default();
        let live = Board::new("Live", None::<String>);
        let archived = Board::new("Archived", None::<String>);
        let live_id = live.id;
        let archived_id = archived.id;
        m.load_from_snapshot(Snapshot {
            boards: vec![live.clone(), archived.clone()],
            archived_boards: vec![Archived::now(archived_id)],
            ..Default::default()
        });

        let live_only: Vec<Uuid> = m
            .boards()
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
        m.load_from_snapshot(Snapshot::default());
        assert!(m.board_by_id(Uuid::new_v4()).is_none());
    }
}
