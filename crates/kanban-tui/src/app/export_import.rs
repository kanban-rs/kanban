use super::App;
use kanban_domain::export::{AllBoardsExport, BoardExporter, BoardImporter};
use std::io;
use uuid::Uuid;

impl App {
    pub fn export_board_with_filename(&self) -> io::Result<()> {
        if let Some(board_id) = self.board_list.get_selected_board_id() {
            let export = self.build_boards_export(&[board_id])?;
            BoardExporter::export_to_file(&export, self.input.as_str())?;
        }
        Ok(())
    }

    /// Export ALL boards (live + archived) with their full subtrees. Routes
    /// through the backend snapshot so an archived board's HEAD, its columns/
    /// cards/sprints (which live in the flat collections under the archived
    /// board_id), AND its `archived_boards` marker all round-trip. The
    /// live-scoped model accessors omit archived boards and their subtrees, so
    /// they must NOT be used here (KAN-895 regression).
    pub fn export_all_boards_with_filename(&self) -> io::Result<()> {
        let export = self.build_all_boards_export()?;
        BoardExporter::export_to_file(&export, self.input.as_str())?;
        Ok(())
    }

    pub fn auto_save(&self) -> io::Result<()> {
        if let Some(ref filename) = self.persistence.save_file {
            let export = self.build_all_boards_export()?;
            BoardExporter::export_to_file(&export, filename)?;
        }
        Ok(())
    }

    /// Build a full-fidelity `AllBoardsExport` from the backend snapshot
    /// (`convert_snapshot_to_export`), which includes archived board heads and
    /// their subtrees plus `archived_boards` markers, and handles dangling-column
    /// archived cards.
    fn build_all_boards_export(&self) -> io::Result<AllBoardsExport> {
        let snapshot = self
            .ctx
            .snapshot()
            .map_err(|e| io::Error::other(e.to_string()))?;
        Ok(BoardImporter::convert_snapshot_to_export(snapshot))
    }

    /// Build an `AllBoardsExport` from the backend snapshot, keeping only the
    /// boards in `board_ids` (preserving their order). Uses the snapshot path so
    /// each board's archived-card live rows and markers round-trip correctly.
    pub(crate) fn build_boards_export(&self, board_ids: &[Uuid]) -> io::Result<AllBoardsExport> {
        let full = self.build_all_boards_export()?;
        let selected: Vec<_> = board_ids
            .iter()
            .filter_map(|id| full.boards.iter().find(|be| be.board.id == *id).cloned())
            .collect();
        Ok(AllBoardsExport::from_boards(selected))
    }

    pub fn import_board_from_file(&mut self, filename: &str) -> io::Result<()> {
        let content = std::fs::read_to_string(filename)?;

        let first_new_index = self.model.live_boards().count();
        let default_card_prefix = self.app_config.effective_default_card_prefix().to_string();
        let default_sprint_prefix = self
            .app_config
            .effective_default_sprint_prefix()
            .to_string();

        // Try V2 format first (preserves graph)
        if let Some(snapshot) = BoardImporter::try_load_snapshot(&content) {
            let cmd = kanban_domain::commands::Command::Board(
                kanban_domain::commands::BoardCommand::Import(
                    kanban_domain::commands::ImportEntities {
                        boards: snapshot.boards,
                        columns: snapshot.columns,
                        cards: snapshot.cards,
                        archived_cards: snapshot.archived_cards,
                        archived_boards: snapshot.archived_boards,
                        sprints: snapshot.sprints,
                        graph: Some(snapshot.graph),
                        prefixes: snapshot.prefixes,
                        default_card_prefix,
                        default_sprint_prefix,
                    },
                ),
            );
            if let Err(e) = self.ctx.execute_command(cmd) {
                self.set_error(e.to_string());
                tracing::error!("Failed to import V2 board: {}", e);
                return Ok(());
            }

            self.reload_model();
            self.prepare_frame();
            self.board_list
                .inner_mut()
                .set_selected_index(Some(first_new_index));
            self.switch_view_strategy(kanban_domain::TaskListView::GroupedByColumn);
            return Ok(());
        }

        // Fall back to V1 format (no graph)
        let import = BoardImporter::import_from_json(&content)?;
        let entities = BoardImporter::extract_entities(import);

        let cmd =
            kanban_domain::commands::Command::Board(kanban_domain::commands::BoardCommand::Import(
                kanban_domain::commands::ImportEntities {
                    boards: entities.boards,
                    columns: entities.columns,
                    cards: entities.cards,
                    archived_cards: entities.archived_cards,
                    archived_boards: entities.archived_boards,
                    sprints: entities.sprints,
                    // This format carries no counters; the command derives them
                    // from the imported cards.
                    prefixes: Vec::new(),
                    graph: None,
                    default_card_prefix,
                    default_sprint_prefix,
                },
            ));
        if let Err(e) = self.ctx.execute_command(cmd) {
            self.set_error(e.to_string());
            tracing::error!("Failed to import V1 board: {}", e);
            return Ok(());
        }

        self.reload_model();
        self.prepare_frame();
        self.board_list
            .inner_mut()
            .set_selected_index(Some(first_new_index));
        self.switch_view_strategy(kanban_domain::TaskListView::GroupedByColumn);

        Ok(())
    }
}

#[cfg(test)]
mod prefix_default_tests {
    use crate::App;
    use kanban_domain::KanbanOperations;

    /// The TUI import path builds `ImportEntities` itself. Letting the new
    /// default fields fall to `Default` resolves namespaces from the
    /// compile-time constants, so a prefix-less sprint's counter is restored
    /// under a namespace nothing allocates from while the one it really used
    /// stays at zero.
    #[test]
    fn test_tui_import_reconstructs_counters_from_the_configured_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("export.json");

        let mut src = App::test_default();
        src.app_config.default_sprint_prefix = Some("iteration".to_string());
        src.ctx.set_app_config(src.app_config.clone());
        let board = src.ctx.create_board("Src".into(), None).unwrap();
        for _ in 0..3 {
            let sprint = src.ctx.create_sprint(board.id, None, None).unwrap();
            // A sprint whose own prefix is cleared has to have its namespace
            // resolved through the default, which is the case the constants
            // get wrong.
            src.ctx
                .update_sprint(
                    sprint.id,
                    kanban_domain::SprintUpdate {
                        prefix: kanban_domain::FieldUpdate::Clear,
                        ..Default::default()
                    },
                )
                .unwrap();
        }
        assert_eq!(
            src.ctx
                .data_store()
                .get_prefix("iteration")
                .unwrap()
                .map(|p| p.sprint_counter),
            Some(3),
            "precondition: sprints were minted from the configured namespace"
        );
        src.input.set(path.to_str().unwrap().to_string());
        src.export_all_boards_with_filename().unwrap();

        let mut dest = App::test_default();
        dest.app_config.default_sprint_prefix = Some("iteration".to_string());
        dest.ctx.set_app_config(dest.app_config.clone());
        dest.import_board_from_file(path.to_str().unwrap()).unwrap();

        let store = dest.ctx.data_store();
        assert_eq!(
            store
                .get_prefix("iteration")
                .unwrap()
                .map_or(0, |p| p.sprint_counter),
            3,
            "the namespace the imported sprints were minted from was left at zero"
        );
        assert_eq!(
            store
                .get_prefix("sprint")
                .unwrap()
                .map_or(0, |p| p.sprint_counter),
            0,
            "a namespace nothing in the payload consumed was inflated"
        );
    }
}
