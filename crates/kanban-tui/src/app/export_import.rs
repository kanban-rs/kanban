use super::App;
use kanban_domain::export::{AllBoardsExport, BoardExporter, BoardImporter};
use std::io;

impl App {
    pub fn export_board_with_filename(&self) -> io::Result<()> {
        if let Some(board_idx) = self.selection.board.get() {
            let boards = self.model.boards();
            if let Some(board) = boards.get(board_idx) {
                let columns = self.model.columns();
                let cards = self.model.cards();
                let archived_cards = self.model.archived_cards();
                let sprints = self.model.sprints();
                let board_export =
                    BoardExporter::export_board(board, columns, cards, archived_cards, sprints);

                let export = AllBoardsExport {
                    boards: vec![board_export],
                };

                BoardExporter::export_to_file(&export, self.input.as_str())?;
            }
        }
        Ok(())
    }

    pub fn export_all_boards_with_filename(&self) -> io::Result<()> {
        let boards = self.model.boards();
        let columns = self.model.columns();
        let cards = self.model.cards();
        let archived_cards = self.model.archived_cards();
        let sprints = self.model.sprints();
        let export =
            BoardExporter::export_all_boards(boards, columns, cards, archived_cards, sprints);
        BoardExporter::export_to_file(&export, self.input.as_str())?;
        Ok(())
    }

    pub fn auto_save(&self) -> io::Result<()> {
        if let Some(ref filename) = self.persistence.save_file {
            let boards = self.model.boards();
            let columns = self.model.columns();
            let cards = self.model.cards();
            let archived_cards = self.model.archived_cards();
            let sprints = self.model.sprints();
            let export =
                BoardExporter::export_all_boards(boards, columns, cards, archived_cards, sprints);
            BoardExporter::export_to_file(&export, filename)?;
        }
        Ok(())
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
