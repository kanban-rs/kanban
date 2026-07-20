use super::App;
use kanban_domain::export::{AllBoardsExport, BoardExporter, BoardImporter};
use std::io;
use uuid::Uuid;

impl App {
    pub fn export_board_with_filename(&self) -> io::Result<()> {
        if let Some(board_idx) = self.selection.board.get() {
            let board_id = self.model.boards().get(board_idx).map(|b| b.id);
            if let Some(board_id) = board_id {
                let export = self.build_boards_export(&[board_id])?;
                BoardExporter::export_to_file(&export, self.input.as_str())?;
            }
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

        let first_new_index = self.model.boards().len();

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
                    },
                ),
            );
            if let Err(e) = self.ctx.execute_command(cmd) {
                self.set_error(e.to_string());
                tracing::error!("Failed to import V2 board: {}", e);
                return Ok(());
            }

            self.selection.board.set(Some(first_new_index));
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
                    graph: None,
                },
            ));
        if let Err(e) = self.ctx.execute_command(cmd) {
            self.set_error(e.to_string());
            tracing::error!("Failed to import V1 board: {}", e);
            return Ok(());
        }

        self.selection.board.set(Some(first_new_index));
        self.switch_view_strategy(kanban_domain::TaskListView::GroupedByColumn);

        Ok(())
    }
}
