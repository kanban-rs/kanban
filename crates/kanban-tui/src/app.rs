use kanban_core::KanbanResult;
use kanban_domain::{Board, Column, Card, CardStatus};
use crossterm::{execute, terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen}};
use ratatui::{
    backend::CrosstermBackend,
    Terminal,
};
use std::io;
use serde::Serialize;
use crate::{events::{EventHandler, Event}, ui, selection::SelectionState, input::InputState, dialog::{handle_dialog_input, DialogAction}, editor::edit_in_external_editor};

pub struct App {
    pub should_quit: bool,
    pub mode: AppMode,
    pub input: InputState,
    pub boards: Vec<Board>,
    pub board_selection: SelectionState,
    pub active_board_index: Option<usize>,
    pub card_selection: SelectionState,
    pub active_card_index: Option<usize>,
    pub columns: Vec<Column>,
    pub cards: Vec<Card>,
    pub focus: Focus,
    pub card_focus: CardFocus,
    pub board_focus: BoardFocus,
    pub import_files: Vec<String>,
    pub import_selection: SelectionState,
    pub save_file: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Focus {
    Projects,
    Tasks,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CardFocus {
    Title,
    Metadata,
    Description,
}

#[derive(Debug, Clone, PartialEq)]
pub enum BoardFocus {
    Name,
    Description,
}

enum CardField {
    Title,
    Description,
}

enum BoardField {
    Name,
    Description,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AppMode {
    Normal,
    CreateBoard,
    CreateCard,
    CardDetail,
    RenameBoard,
    BoardDetail,
    ExportBoard,
    ExportAll,
    ImportBoard,
    SetCardPoints,
}

impl App {
    pub fn new(save_file: Option<String>) -> Self {
        let mut app = Self {
            should_quit: false,
            mode: AppMode::Normal,
            input: InputState::new(),
            boards: Vec::new(),
            board_selection: SelectionState::new(),
            active_board_index: None,
            card_selection: SelectionState::new(),
            active_card_index: None,
            columns: Vec::new(),
            cards: Vec::new(),
            focus: Focus::Projects,
            card_focus: CardFocus::Title,
            board_focus: BoardFocus::Name,
            import_files: Vec::new(),
            import_selection: SelectionState::new(),
            save_file: save_file.clone(),
        };

        if let Some(ref filename) = save_file {
            if std::path::Path::new(filename).exists() {
                if let Err(e) = app.import_board_from_file(filename) {
                    tracing::error!("Failed to load file {}: {}", filename, e);
                    app.save_file = None;
                }
            }
        }

        app
    }

    pub fn quit(&mut self) {
        self.should_quit = true;
    }


    fn handle_key_event(&mut self, key: crossterm::event::KeyEvent, terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, event_handler: &EventHandler) -> bool {
        use crossterm::event::KeyCode;
        let mut should_restart_events = false;

        if matches!(key.code, KeyCode::Char('q') | KeyCode::Char('Q')) {
            self.quit();
            return false;
        }

        match self.mode {
            AppMode::Normal => {
                match key.code {
                    KeyCode::Char('n') => {
                        match self.focus {
                            Focus::Projects => {
                                self.mode = AppMode::CreateBoard;
                                self.input.clear();
                            }
                            Focus::Tasks => {
                                if self.active_board_index.is_some() {
                                    self.mode = AppMode::CreateCard;
                                    self.input.clear();
                                }
                            }
                        }
                    }
                    KeyCode::Char('r') => {
                        if self.focus == Focus::Projects && self.board_selection.get().is_some() {
                            if let Some(board_idx) = self.board_selection.get() {
                                if let Some(board) = self.boards.get(board_idx) {
                                    self.input.set(board.name.clone());
                                    self.mode = AppMode::RenameBoard;
                                }
                            }
                        }
                    }
                    KeyCode::Char('e') => {
                        if self.focus == Focus::Projects && self.board_selection.get().is_some() {
                            self.mode = AppMode::BoardDetail;
                            self.board_focus = BoardFocus::Name;
                        }
                    }
                    KeyCode::Char('x') => {
                        if self.focus == Focus::Projects && self.board_selection.get().is_some() {
                            if let Some(board_idx) = self.board_selection.get() {
                                if let Some(board) = self.boards.get(board_idx) {
                                    let filename = format!("{}-{}.json",
                                        board.name.replace(" ", "-").to_lowercase(),
                                        chrono::Utc::now().format("%Y%m%d-%H%M%S")
                                    );
                                    self.input.set(filename);
                                    self.mode = AppMode::ExportBoard;
                                }
                            }
                        }
                    }
                    KeyCode::Char('X') => {
                        if self.focus == Focus::Projects && !self.boards.is_empty() {
                            let filename = format!("kanban-all-{}.json",
                                chrono::Utc::now().format("%Y%m%d-%H%M%S")
                            );
                            self.input.set(filename);
                            self.mode = AppMode::ExportAll;
                        }
                    }
                    KeyCode::Char('i') => {
                        if self.focus == Focus::Projects {
                            self.scan_import_files();
                            if !self.import_files.is_empty() {
                                self.import_selection.set(Some(0));
                                self.mode = AppMode::ImportBoard;
                            }
                        }
                    }
                    KeyCode::Char('c') => {
                        if self.focus == Focus::Tasks && self.card_selection.get().is_some() {
                            self.toggle_card_completion();
                        }
                    }
                    KeyCode::Char('1') => self.focus = Focus::Projects,
                    KeyCode::Char('2') => {
                        if self.active_board_index.is_some() {
                            self.focus = Focus::Tasks;
                        }
                    }
                    KeyCode::Esc => {
                        if self.active_board_index.is_some() {
                            self.active_board_index = None;
                            self.card_selection.clear();
                            self.focus = Focus::Projects;
                        }
                    }
                    KeyCode::Char('j') | KeyCode::Down => {
                        match self.focus {
                            Focus::Projects => {
                                self.board_selection.next(self.boards.len());
                            }
                            Focus::Tasks => {
                                if let Some(board_idx) = self.active_board_index {
                                    if let Some(board) = self.boards.get(board_idx) {
                                        let task_count = self.get_board_task_count(board.id);
                                        self.card_selection.next(task_count);
                                    }
                                }
                            }
                        }
                    }
                    KeyCode::Char('k') | KeyCode::Up => {
                        match self.focus {
                            Focus::Projects => {
                                self.board_selection.prev();
                            }
                            Focus::Tasks => {
                                self.card_selection.prev();
                            }
                        }
                    }
                    KeyCode::Enter | KeyCode::Char(' ') => {
                        match self.focus {
                            Focus::Projects => {
                                if self.board_selection.get().is_some() {
                                    self.active_board_index = self.board_selection.get();
                                    self.card_selection.clear();

                                    if let Some(board_idx) = self.active_board_index {
                                        if let Some(board) = self.boards.get(board_idx) {
                                            let task_count = self.get_board_task_count(board.id);
                                            if task_count > 0 {
                                                self.card_selection.set(Some(0));
                                            }
                                        }
                                    }

                                    self.focus = Focus::Tasks;
                                }
                            }
                            Focus::Tasks => {
                                if self.card_selection.get().is_some() {
                                    self.active_card_index = self.card_selection.get();
                                    self.mode = AppMode::CardDetail;
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
            AppMode::CreateBoard => {
                match handle_dialog_input(&mut self.input, key.code) {
                    DialogAction::Confirm => {
                        self.create_board();
                        self.mode = AppMode::Normal;
                        self.input.clear();
                    }
                    DialogAction::Cancel => {
                        self.mode = AppMode::Normal;
                        self.input.clear();
                    }
                    DialogAction::None => {}
                }
            }
            AppMode::CreateCard => {
                match handle_dialog_input(&mut self.input, key.code) {
                    DialogAction::Confirm => {
                        self.create_task();
                        self.mode = AppMode::Normal;
                        self.input.clear();
                    }
                    DialogAction::Cancel => {
                        self.mode = AppMode::Normal;
                        self.input.clear();
                    }
                    DialogAction::None => {}
                }
            }
            AppMode::RenameBoard => {
                match handle_dialog_input(&mut self.input, key.code) {
                    DialogAction::Confirm => {
                        self.rename_board();
                        self.mode = AppMode::Normal;
                        self.input.clear();
                    }
                    DialogAction::Cancel => {
                        self.mode = AppMode::Normal;
                        self.input.clear();
                    }
                    DialogAction::None => {}
                }
            }
            AppMode::ExportBoard => {
                match handle_dialog_input(&mut self.input, key.code) {
                    DialogAction::Confirm => {
                        if let Err(e) = self.export_board_with_filename() {
                            tracing::error!("Failed to export board: {}", e);
                        }
                        self.mode = AppMode::Normal;
                        self.input.clear();
                    }
                    DialogAction::Cancel => {
                        self.mode = AppMode::Normal;
                        self.input.clear();
                    }
                    DialogAction::None => {}
                }
            }
            AppMode::ExportAll => {
                match handle_dialog_input(&mut self.input, key.code) {
                    DialogAction::Confirm => {
                        if let Err(e) = self.export_all_boards_with_filename() {
                            tracing::error!("Failed to export all boards: {}", e);
                        }
                        self.mode = AppMode::Normal;
                        self.input.clear();
                    }
                    DialogAction::Cancel => {
                        self.mode = AppMode::Normal;
                        self.input.clear();
                    }
                    DialogAction::None => {}
                }
            }
            AppMode::ImportBoard => {
                match key.code {
                    KeyCode::Esc => {
                        self.mode = AppMode::Normal;
                        self.import_selection.clear();
                    }
                    KeyCode::Char('j') | KeyCode::Down => {
                        self.import_selection.next(self.import_files.len());
                    }
                    KeyCode::Char('k') | KeyCode::Up => {
                        self.import_selection.prev();
                    }
                    KeyCode::Enter | KeyCode::Char(' ') => {
                        if let Some(idx) = self.import_selection.get() {
                            if let Some(filename) = self.import_files.get(idx).cloned() {
                                if let Err(e) = self.import_board_from_file(&filename) {
                                    tracing::error!("Failed to import board: {}", e);
                                }
                            }
                        }
                        self.mode = AppMode::Normal;
                        self.import_selection.clear();
                    }
                    _ => {}
                }
            }
            AppMode::SetCardPoints => {
                match handle_dialog_input(&mut self.input, key.code) {
                    DialogAction::Confirm => {
                        let input_str = self.input.as_str().trim();
                        let points = if input_str.is_empty() {
                            None
                        } else if let Ok(p) = input_str.parse::<u8>() {
                            if (1..=5).contains(&p) {
                                Some(p)
                            } else {
                                tracing::error!("Points must be between 1-5");
                                self.mode = AppMode::CardDetail;
                                self.input.clear();
                                return should_restart_events;
                            }
                        } else {
                            tracing::error!("Invalid points value");
                            self.mode = AppMode::CardDetail;
                            self.input.clear();
                            return should_restart_events;
                        };

                        if let Some(task_idx) = self.active_card_index {
                            if let Some(board_idx) = self.active_board_index {
                                if let Some(board) = self.boards.get(board_idx) {
                                    let board_tasks: Vec<_> = self.cards.iter()
                                        .filter(|card| {
                                            self.columns.iter()
                                                .any(|col| col.id == card.column_id && col.board_id == board.id)
                                        })
                                        .collect();

                                    if let Some(task) = board_tasks.get(task_idx) {
                                        let task_id = task.id;
                                        if let Some(card) = self.cards.iter_mut().find(|c| c.id == task_id) {
                                            card.set_points(points);
                                            tracing::info!("Set points to: {:?}", points);
                                        }
                                    }
                                }
                            }
                        }
                        self.mode = AppMode::CardDetail;
                        self.input.clear();
                    }
                    DialogAction::Cancel => {
                        self.mode = AppMode::CardDetail;
                        self.input.clear();
                    }
                    DialogAction::None => {}
                }
            }
            AppMode::CardDetail => {
                match key.code {
                    KeyCode::Esc => {
                        self.mode = AppMode::Normal;
                        self.active_card_index = None;
                        self.card_focus = CardFocus::Title;
                    }
                    KeyCode::Char('1') => {
                        self.card_focus = CardFocus::Title;
                    }
                    KeyCode::Char('2') => {
                        self.card_focus = CardFocus::Metadata;
                    }
                    KeyCode::Char('3') => {
                        self.card_focus = CardFocus::Description;
                    }
                    KeyCode::Char('e') => {
                        match self.card_focus {
                            CardFocus::Title => {
                                if let Err(e) = self.edit_card_field(terminal, event_handler, CardField::Title) {
                                    tracing::error!("Failed to edit title: {}", e);
                                }
                                should_restart_events = true;
                            }
                            CardFocus::Description => {
                                if let Err(e) = self.edit_card_field(terminal, event_handler, CardField::Description) {
                                    tracing::error!("Failed to edit description: {}", e);
                                }
                                should_restart_events = true;
                            }
                            CardFocus::Metadata => {
                                self.input.clear();
                                self.mode = AppMode::SetCardPoints;
                            }
                        }
                    }
                    _ => {}
                }
            }
            AppMode::BoardDetail => {
                match key.code {
                    KeyCode::Esc => {
                        self.mode = AppMode::Normal;
                        self.board_focus = BoardFocus::Name;
                    }
                    KeyCode::Char('1') => {
                        self.board_focus = BoardFocus::Name;
                    }
                    KeyCode::Char('2') => {
                        self.board_focus = BoardFocus::Description;
                    }
                    KeyCode::Char('e') => {
                        match self.board_focus {
                            BoardFocus::Name => {
                                if let Err(e) = self.edit_board_field(terminal, event_handler, BoardField::Name) {
                                    tracing::error!("Failed to edit board name: {}", e);
                                }
                                should_restart_events = true;
                            }
                            BoardFocus::Description => {
                                if let Err(e) = self.edit_board_field(terminal, event_handler, BoardField::Description) {
                                    tracing::error!("Failed to edit board description: {}", e);
                                }
                                should_restart_events = true;
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        should_restart_events
    }

    fn create_board(&mut self) {
        let board = Board::new(self.input.as_str().to_string(), None);
        tracing::info!("Creating project: {} (id: {})", board.name, board.id);
        self.boards.push(board);
        let new_index = self.boards.len() - 1;
        self.board_selection.set(Some(new_index));
    }

    fn rename_board(&mut self) {
        if let Some(idx) = self.board_selection.get() {
            if let Some(board) = self.boards.get_mut(idx) {
                board.update_name(self.input.as_str().to_string());
                tracing::info!("Renamed project to: {}", board.name);
            }
        }
    }

    fn toggle_card_completion(&mut self) {
        if let Some(task_idx) = self.card_selection.get() {
            if let Some(board_idx) = self.active_board_index {
                if let Some(board) = self.boards.get(board_idx) {
                    let board_tasks: Vec<_> = self.cards.iter()
                        .filter(|card| {
                            self.columns.iter()
                                .any(|col| col.id == card.column_id && col.board_id == board.id)
                        })
                        .collect();

                    if let Some(task) = board_tasks.get(task_idx) {
                        let task_id = task.id;
                        let new_status = if task.status == CardStatus::Done {
                            CardStatus::Todo
                        } else {
                            CardStatus::Done
                        };

                        if let Some(card) = self.cards.iter_mut().find(|c| c.id == task_id) {
                            card.update_status(new_status);
                            tracing::info!("Toggled task '{}' to status: {:?}", card.title, new_status);
                        }
                    }
                }
            }
        }
    }

    fn create_task(&mut self) {
        if let Some(idx) = self.active_board_index {
            if let Some(board) = self.boards.get(idx) {
                let column = self.columns.iter()
                    .find(|col| col.board_id == board.id)
                    .cloned();

                let column = match column {
                    Some(col) => col,
                    None => {
                        let new_column = Column::new(board.id, "Todo".to_string(), 0);
                        self.columns.push(new_column.clone());
                        new_column
                    }
                };

                let position = self.cards.iter().filter(|c| c.column_id == column.id).count() as i32;
                let card = Card::new(column.id, self.input.as_str().to_string(), position);
                tracing::info!("Creating task: {} (id: {})", card.title, card.id);
                self.cards.push(card);

                let task_count = self.get_board_task_count(board.id);
                let new_task_index = task_count.saturating_sub(1);
                self.card_selection.set(Some(new_task_index));
            }
        }
    }

    fn get_board_task_count(&self, board_id: uuid::Uuid) -> usize {
        self.cards.iter()
            .filter(|card| {
                self.columns.iter()
                    .any(|col| col.id == card.column_id && col.board_id == board_id)
            })
            .count()
    }

    fn export_board_with_filename(&self) -> io::Result<()> {
        #[derive(Serialize)]
        struct BoardExport {
            board: Board,
            columns: Vec<Column>,
            tasks: Vec<Card>,
        }

        #[derive(Serialize)]
        struct AllBoardsExport {
            boards: Vec<BoardExport>,
        }

        if let Some(board_idx) = self.board_selection.get() {
            if let Some(board) = self.boards.get(board_idx) {
                let board_columns: Vec<Column> = self.columns.iter()
                    .filter(|col| col.board_id == board.id)
                    .cloned()
                    .collect();

                let column_ids: Vec<uuid::Uuid> = board_columns.iter().map(|c| c.id).collect();

                let board_cards: Vec<Card> = self.cards.iter()
                    .filter(|card| column_ids.contains(&card.column_id))
                    .cloned()
                    .collect();

                let board_export = BoardExport {
                    board: board.clone(),
                    columns: board_columns,
                    tasks: board_cards,
                };

                let export = AllBoardsExport {
                    boards: vec![board_export],
                };

                let json = serde_json::to_string_pretty(&export)
                    .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

                std::fs::write(self.input.as_str(), json)?;
                tracing::info!("Exported board to: {}", self.input.as_str());
            }
        }
        Ok(())
    }

    fn export_all_boards_with_filename(&self) -> io::Result<()> {
        #[derive(Serialize)]
        struct BoardExport {
            board: Board,
            columns: Vec<Column>,
            tasks: Vec<Card>,
        }

        #[derive(Serialize)]
        struct AllBoardsExport {
            boards: Vec<BoardExport>,
        }

        let mut board_exports = Vec::new();

        for board in &self.boards {
            let board_columns: Vec<Column> = self.columns.iter()
                .filter(|col| col.board_id == board.id)
                .cloned()
                .collect();

            let column_ids: Vec<uuid::Uuid> = board_columns.iter().map(|c| c.id).collect();

            let board_cards: Vec<Card> = self.cards.iter()
                .filter(|card| column_ids.contains(&card.column_id))
                .cloned()
                .collect();

            board_exports.push(BoardExport {
                board: board.clone(),
                columns: board_columns,
                tasks: board_cards,
            });
        }

        let export = AllBoardsExport {
            boards: board_exports,
        };

        let json = serde_json::to_string_pretty(&export)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

        std::fs::write(self.input.as_str(), json)?;
        tracing::info!("Exported all boards to: {}", self.input.as_str());

        Ok(())
    }

    fn auto_save(&self) -> io::Result<()> {
        if let Some(ref filename) = self.save_file {
            #[derive(Serialize)]
            struct BoardExport {
                board: Board,
                columns: Vec<Column>,
                tasks: Vec<Card>,
            }

            #[derive(Serialize)]
            struct AllBoardsExport {
                boards: Vec<BoardExport>,
            }

            let mut board_exports = Vec::new();

            for board in &self.boards {
                let board_columns: Vec<Column> = self.columns.iter()
                    .filter(|col| col.board_id == board.id)
                    .cloned()
                    .collect();

                let column_ids: Vec<uuid::Uuid> = board_columns.iter().map(|c| c.id).collect();

                let board_cards: Vec<Card> = self.cards.iter()
                    .filter(|card| column_ids.contains(&card.column_id))
                    .cloned()
                    .collect();

                board_exports.push(BoardExport {
                    board: board.clone(),
                    columns: board_columns,
                    tasks: board_cards,
                });
            }

            let export = AllBoardsExport {
                boards: board_exports,
            };

            let json = serde_json::to_string_pretty(&export)
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

            std::fs::write(filename, json)?;
            tracing::info!("Auto-saved {} boards to: {}", self.boards.len(), filename);
        }
        Ok(())
    }

    fn edit_board_field(&mut self, terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, event_handler: &EventHandler, field: BoardField) -> io::Result<()> {
        if let Some(board_idx) = self.board_selection.get() {
            if let Some(board) = self.boards.get(board_idx) {
                let temp_dir = std::env::temp_dir();
                let (temp_file, current_content) = match field {
                    BoardField::Name => {
                        let temp_file = temp_dir.join(format!("kanban-board-{}-name.md", board.id));
                        (temp_file, board.name.clone())
                    }
                    BoardField::Description => {
                        let temp_file = temp_dir.join(format!("kanban-board-{}-description.md", board.id));
                        let content = board.description.as_ref().map(|s| s.as_str()).unwrap_or("").to_string();
                        (temp_file, content)
                    }
                };

                if let Some(new_content) = edit_in_external_editor(terminal, event_handler, temp_file, &current_content)? {
                    let board_id = board.id;
                    if let Some(board) = self.boards.iter_mut().find(|b| b.id == board_id) {
                        match field {
                            BoardField::Name => {
                                if !new_content.trim().is_empty() {
                                    board.update_name(new_content.trim().to_string());
                                }
                            }
                            BoardField::Description => {
                                let desc = if new_content.trim().is_empty() {
                                    None
                                } else {
                                    Some(new_content)
                                };
                                board.update_description(desc);
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn edit_card_field(&mut self, terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, event_handler: &EventHandler, field: CardField) -> io::Result<()> {
        if let Some(task_idx) = self.active_card_index {
            if let Some(board_idx) = self.active_board_index {
                if let Some(board) = self.boards.get(board_idx) {
                    let board_tasks: Vec<_> = self.cards.iter()
                        .filter(|card| {
                            self.columns.iter()
                                .any(|col| col.id == card.column_id && col.board_id == board.id)
                        })
                        .collect();

                    if let Some(task) = board_tasks.get(task_idx) {
                        let temp_dir = std::env::temp_dir();
                        let (temp_file, current_content) = match field {
                            CardField::Title => {
                                let temp_file = temp_dir.join(format!("kanban-task-{}-title.md", task.id));
                                (temp_file, task.title.clone())
                            }
                            CardField::Description => {
                                let temp_file = temp_dir.join(format!("kanban-task-{}-description.md", task.id));
                                let content = task.description.as_ref().map(|s| s.as_str()).unwrap_or("").to_string();
                                (temp_file, content)
                            }
                        };

                        if let Some(new_content) = edit_in_external_editor(terminal, event_handler, temp_file, &current_content)? {
                            let task_id = task.id;
                            if let Some(card) = self.cards.iter_mut().find(|c| c.id == task_id) {
                                match field {
                                    CardField::Title => {
                                        if !new_content.trim().is_empty() {
                                            card.update_title(new_content.trim().to_string());
                                        }
                                    }
                                    CardField::Description => {
                                        let desc = if new_content.trim().is_empty() {
                                            None
                                        } else {
                                            Some(new_content)
                                        };
                                        card.update_description(desc);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    pub async fn run(&mut self) -> KanbanResult<()> {
        let mut terminal = setup_terminal()?;

        while !self.should_quit {
            let mut events = EventHandler::new();

            loop {
                terminal.draw(|frame| ui::render(self, frame))?;

                if let Some(event) = events.next().await {
                    match event {
                        Event::Key(key) => {
                            let should_restart = self.handle_key_event(key, &mut terminal, &events);
                            if should_restart {
                                break;
                            }
                        }
                        Event::Tick => {}
                    }
                }

                if self.should_quit {
                    break;
                }
            }

            if self.should_quit {
                break;
            }
        }

        if let Err(e) = self.auto_save() {
            tracing::error!("Failed to auto-save: {}", e);
        }

        restore_terminal(&mut terminal)?;
        Ok(())
    }

    fn scan_import_files(&mut self) {
        self.import_files.clear();
        if let Ok(entries) = std::fs::read_dir(".") {
            for entry in entries.flatten() {
                if let Ok(metadata) = entry.metadata() {
                    if metadata.is_file() {
                        if let Some(filename) = entry.file_name().to_str() {
                            if filename.ends_with(".json") {
                                self.import_files.push(filename.to_string());
                            }
                        }
                    }
                }
            }
        }
        self.import_files.sort();
    }

    fn import_board_from_file(&mut self, filename: &str) -> io::Result<()> {
        use serde::Deserialize;

        #[derive(Deserialize)]
        struct BoardImport {
            board: Board,
            columns: Vec<Column>,
            tasks: Vec<Card>,
        }

        #[derive(Deserialize)]
        struct AllBoardsImport {
            boards: Vec<BoardImport>,
        }

        let content = std::fs::read_to_string(filename)?;
        let first_new_index = self.boards.len();

        match serde_json::from_str::<AllBoardsImport>(&content) {
            Ok(import) => {
                let count = import.boards.len();
                for board_data in import.boards {
                    self.boards.push(board_data.board);
                    self.columns.extend(board_data.columns);
                    self.cards.extend(board_data.tasks);
                }
                tracing::info!("Imported {} boards from: {}", count, filename);
            }
            Err(err) => {
                tracing::error!("Import error: {}", err);
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("Invalid JSON format. Expected {{\"boards\": [...]}} structure. Error: {}", err)
                ));
            }
        }

        self.board_selection.set(Some(first_new_index));
        Ok(())
    }
}

fn setup_terminal() -> Result<Terminal<CrosstermBackend<io::Stdout>>, io::Error> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    Terminal::new(backend)
}

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<(), io::Error> {
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}

impl Default for App {
    fn default() -> Self {
        Self::new(None)
    }
}
