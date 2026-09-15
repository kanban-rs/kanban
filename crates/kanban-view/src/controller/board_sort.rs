use super::Controller;
use kanban_core::AppConfig;
use kanban_domain::{BoardSortField, SortOrder, DEFAULT_BOARD_SORT_LIVE};
use std::str::FromStr;

impl Controller {
    /// The current board-list sort dimension (`BoardSortField`/`SortOrder`) for
    /// the requested partition. Live and archived each carry their own
    /// independent pair — see the field docs on `Controller`.
    pub fn board_sort(&self, archived: bool) -> (BoardSortField, SortOrder) {
        if archived {
            (
                self.archived_board_sort_field,
                self.archived_board_sort_order,
            )
        } else {
            (self.live_board_sort_field, self.live_board_sort_order)
        }
    }

    /// Set the requested partition's sort field/order and re-sort both cached
    /// partitions in place (each against its own, independent pair).
    pub fn set_board_sort(&mut self, archived: bool, field: BoardSortField, order: SortOrder) {
        if archived {
            self.archived_board_sort_field = field;
            self.archived_board_sort_order = order;
        } else {
            self.live_board_sort_field = field;
            self.live_board_sort_order = order;
        }
        self.sort_partitions();
    }

    /// Flip the requested partition's sort ORDER via the shared
    /// `SortOrder::toggled` (the same asc↔desc flip the card list uses),
    /// keeping its current field.
    pub fn toggle_board_sort_order(&mut self, archived: bool) {
        let (field, order) = self.board_sort(archived);
        self.set_board_sort(archived, field, order.toggled());
    }

    /// Seed the LIVE partition's sort field/order from
    /// `AppConfig.board_sort_field`/`board_sort_order`, and re-sort the cached
    /// partitions. The archived partition is never config-seeded — it stays on
    /// its own default (recency) unless changed in-session. Called once on
    /// start so the live projects-panel sort survives a restart.
    pub fn set_board_sort_from_config(&mut self, config: &AppConfig) {
        let field = config
            .board_sort_field
            .as_deref()
            .and_then(|s| BoardSortField::from_str(s).ok());
        let order = config
            .board_sort_order
            .as_deref()
            .and_then(|s| SortOrder::from_str(s).ok());
        match (field, order) {
            // A field with an optional order is an explicit choice; a bare order
            // with no field is ignored (there is no field to apply it to).
            (Some(field), order) => {
                self.set_board_sort(false, field, order.unwrap_or(DEFAULT_BOARD_SORT_LIVE.1));
            }
            _ => {
                self.live_board_sort_field = DEFAULT_BOARD_SORT_LIVE.0;
                self.live_board_sort_order = DEFAULT_BOARD_SORT_LIVE.1;
                self.sort_partitions();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kanban_domain::{Archived, Board, DerivedProjections, Model, Snapshot};
    use uuid::Uuid;

    fn seed_two_archived_boards(m: &mut Model, c: &mut Controller) -> (Uuid, Uuid) {
        // `first` sits at position 0 but was archived EARLIER; `second` sits at
        // position 1 but was archived LATER. Position order and recency order
        // therefore disagree, so the two orderings are distinguishable.
        let mut first = Board::new("First", None::<String>);
        first.position = 0;
        let mut second = Board::new("Second", None::<String>);
        second.position = 1;
        let first_id = first.id;
        let second_id = second.id;
        let t_old = chrono::DateTime::parse_from_rfc3339("2026-01-01T00:00:00Z")
            .unwrap()
            .with_timezone(&chrono::Utc);
        let t_new = chrono::DateTime::parse_from_rfc3339("2026-06-01T00:00:00Z")
            .unwrap()
            .with_timezone(&chrono::Utc);
        let changed = m.load_from_snapshot(Snapshot {
            boards: vec![first, second],
            archived_boards: vec![
                Archived::at(first_id, t_old),
                Archived::at(second_id, t_new),
            ],
            ..Default::default()
        });
        c.resync(m, changed);
        (first_id, second_id)
    }

    #[test]
    fn test_board_sort_field_config_string_is_canonical() {
        // The on-disk board_sort_field string is the domain `Display` spelling,
        // and it round-trips back through the domain `FromStr` — one canonical
        // spelling, no TUI-local PascalCase divergence.
        for field in [
            BoardSortField::Position,
            BoardSortField::Name,
            BoardSortField::CreatedAt,
            BoardSortField::ArchivedAt,
        ] {
            let s = field.to_string();
            assert_eq!(
                BoardSortField::from_str(&s),
                Ok(field),
                "config string {s:?} must round-trip through the domain FromStr"
            );
        }
        assert_eq!(BoardSortField::ArchivedAt.to_string(), "archived_at");
    }

    #[test]
    fn test_archived_board_view_defaults_to_recency() {
        // With no explicit user sort, the ARCHIVED partition defaults to recency
        // (ArchivedAt DESC) — newest-archived first — while the LIVE partition
        // keeps Position ASC. `second` was archived later, so it leads the
        // archived list; `first` (pos 0) still leads the live list.
        let mut m = Model::default();
        let mut c = Controller::default();
        let (first_id, second_id) = seed_two_archived_boards(&mut m, &mut c);
        let archived: Vec<Uuid> = c
            .displayed_boards(true)
            .loaded()
            .copied()
            .unwrap_or(&[])
            .iter()
            .map(|b| b.id)
            .collect();
        assert_eq!(
            archived,
            vec![second_id, first_id],
            "archived board view defaults to recency DESC (newest archived first)"
        );
    }

    #[test]
    fn test_live_board_view_defaults_to_position() {
        // The LIVE partition default is unchanged: Position ASC. `first` at
        // position 0 precedes `second` at position 1.
        let mut m = Model::default();
        let mut c = Controller::default();
        let mut first = Board::new("First", None::<String>);
        first.position = 0;
        let mut second = Board::new("Second", None::<String>);
        second.position = 1;
        let first_id = first.id;
        let second_id = second.id;
        let changed = m.load_from_snapshot(Snapshot {
            boards: vec![second, first],
            archived_boards: vec![],
            ..Default::default()
        });
        c.resync(&m, changed);
        let live: Vec<Uuid> = c
            .displayed_boards(false)
            .loaded()
            .copied()
            .unwrap_or(&[])
            .iter()
            .map(|b| b.id)
            .collect();
        assert_eq!(
            live,
            vec![first_id, second_id],
            "live board view defaults to Position ASC"
        );
    }

    #[test]
    fn test_archived_boards_sort_by_recency_orders_newest_first() {
        // Recency DESC (archived_at) puts the newest-archived board first.
        let mut m = Model::default();
        let mut c = Controller::default();
        let (first_id, second_id) = seed_two_archived_boards(&mut m, &mut c);
        c.set_board_sort(true, BoardSortField::ArchivedAt, SortOrder::Descending);
        let order: Vec<Uuid> = c
            .displayed_boards(true)
            .loaded()
            .copied()
            .unwrap_or(&[])
            .iter()
            .map(|b| b.id)
            .collect();
        assert_eq!(
            order,
            vec![second_id, first_id],
            "newest-archived (second) must come first under recency DESC"
        );
    }

    #[test]
    fn test_archived_boards_sort_by_position_matches_board_order() {
        let mut m = Model::default();
        let mut c = Controller::default();
        let (first_id, second_id) = seed_two_archived_boards(&mut m, &mut c);
        c.set_board_sort(true, BoardSortField::Position, SortOrder::Ascending);
        let order: Vec<Uuid> = c
            .displayed_boards(true)
            .loaded()
            .copied()
            .unwrap_or(&[])
            .iter()
            .map(|b| b.id)
            .collect();
        assert_eq!(
            order,
            vec![first_id, second_id],
            "position order restores board order (first at pos 0)"
        );
    }

    #[test]
    fn test_toggle_reverses_board_sort_order() {
        // The shared SortOrder toggle flips the board-list order for the shown
        // partition. From recency DESC a toggle yields recency ASC (oldest first).
        let mut m = Model::default();
        let mut c = Controller::default();
        let (first_id, second_id) = seed_two_archived_boards(&mut m, &mut c);
        c.set_board_sort(true, BoardSortField::ArchivedAt, SortOrder::Descending);
        let before: Vec<Uuid> = c
            .displayed_boards(true)
            .loaded()
            .copied()
            .unwrap_or(&[])
            .iter()
            .map(|b| b.id)
            .collect();
        assert_eq!(before, vec![second_id, first_id]);

        c.toggle_board_sort_order(true);
        let after: Vec<Uuid> = c
            .displayed_boards(true)
            .loaded()
            .copied()
            .unwrap_or(&[])
            .iter()
            .map(|b| b.id)
            .collect();
        assert_eq!(
            after,
            vec![first_id, second_id],
            "toggle reverses to oldest-archived first"
        );
    }

    #[test]
    fn test_board_sort_applies_to_live_projects_panel() {
        // Picking Name sorts the LIVE projects panel alphabetically.
        let mut m = Model::default();
        let mut c = Controller::default();
        let mut zed = Board::new("Zed", None::<String>);
        zed.position = 0;
        let mut alpha = Board::new("Alpha", None::<String>);
        alpha.position = 1;
        let zed_id = zed.id;
        let alpha_id = alpha.id;
        let changed = m.load_from_snapshot(Snapshot {
            boards: vec![zed, alpha],
            archived_boards: vec![],
            ..Default::default()
        });
        c.resync(&m, changed);

        c.set_board_sort(false, BoardSortField::Name, SortOrder::Ascending);
        let live: Vec<Uuid> = c
            .displayed_boards(false)
            .loaded()
            .copied()
            .unwrap_or(&[])
            .iter()
            .map(|b| b.id)
            .collect();
        assert_eq!(
            live,
            vec![alpha_id, zed_id],
            "live panel sorts alphabetically by Name"
        );
    }

    #[test]
    fn test_board_sort_from_config_seeds_field_and_order() {
        // The board sort field/order is restored from AppConfig on start.
        let mut c = Controller::default();
        let config = AppConfig {
            board_sort_field: Some("Name".into()),
            board_sort_order: Some("Ascending".into()),
            ..Default::default()
        };
        c.set_board_sort_from_config(&config);
        assert_eq!(
            c.board_sort(false),
            (BoardSortField::Name, SortOrder::Ascending),
            "config field/order restored into the controller state"
        );
    }

    #[test]
    fn test_board_sort_from_config_unknown_falls_back_to_live_default() {
        // Unrecognised / missing config values fall back to the LIVE built-in
        // default (Position ASC); the archived partition keeps its recency default.
        let mut c = Controller::default();
        c.set_board_sort_from_config(&AppConfig {
            board_sort_field: Some("nonsense".into()),
            board_sort_order: None,
            ..Default::default()
        });
        assert_eq!(c.board_sort(false), DEFAULT_BOARD_SORT_LIVE);
    }

    #[test]
    fn test_board_sort_persists_to_appconfig_and_restores() {
        // Change the sort, write the canonical domain strings to AppConfig (what
        // the TUI saves), then seed a FRESH controller from that config: the
        // choice survives a "restart".
        let mut c = Controller::default();
        c.set_board_sort(false, BoardSortField::Name, SortOrder::Descending);
        let (field, order) = c.board_sort(false);

        let config = AppConfig {
            board_sort_field: Some(field.to_string()),
            board_sort_order: Some(order.to_string()),
            ..Default::default()
        };

        let mut restored = Controller::default();
        restored.set_board_sort_from_config(&config);
        assert_eq!(
            restored.board_sort(false),
            (BoardSortField::Name, SortOrder::Descending),
            "board sort choice survives a config round-trip"
        );
    }
}
