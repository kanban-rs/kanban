use crate::{
    card_list::{CardList, CardListId},
    card_list_component::{CardListComponent, CardListComponentConfig},
    clipboard,
    editor::edit_in_external_editor,
    events::{Event, EventHandler},
    export::{BoardExporter, BoardImporter},
    filters::FilterDialogState,
    input::InputState,
    search::SearchState,
    selection::SelectionState,
    services::{filter::CardFilter, get_sorter_for_field, BoardFilter, OrderedSorter},
    state::StateManager,
    ui,
    view_strategy::{UnifiedViewStrategy, ViewRefreshContext, ViewStrategy},
};
use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use kanban_core::{AppConfig, Editable, KanbanResult};
use kanban_domain::{ArchivedCard, Board, Card, Column, SortField, SortOrder, Sprint};
use kanban_persistence::{PersistenceMetadata, PersistenceStore, StoreSnapshot};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::collections::HashMap;
use std::io;
use std::time::Instant;

const ANIMATION_DURATION_MS: u128 = 150;

#[derive(Debug, Clone, Copy)]
pub enum AnimationType {
    Archiving,
    Restoring,
    Deleting,
}

pub struct CardAnimation {
    pub animation_type: AnimationType,
    pub start_time: Instant,
}

pub struct App {
    pub should_quit: bool,
    pub quit_with_pending: bool, // Force quit even if saves are pending (second 'q' press)
    pub mode: AppMode,
    pub input: InputState,
    pub boards: Vec<Board>,
    pub board_selection: SelectionState,
    pub active_board_index: Option<usize>,
    pub active_card_index: Option<usize>,
    pub columns: Vec<Column>,
    pub cards: Vec<Card>,
    pub archived_cards: Vec<ArchivedCard>,
    pub animating_cards: HashMap<uuid::Uuid, CardAnimation>,
    pub sprints: Vec<Sprint>,
    pub sprint_selection: SelectionState,
    pub active_sprint_index: Option<usize>,
    pub active_sprint_filters: std::collections::HashSet<uuid::Uuid>,
    pub hide_assigned_cards: bool,
    pub sprint_assign_selection: SelectionState,
    pub focus: Focus,
    pub card_focus: CardFocus,
    pub board_focus: BoardFocus,
    pub import_files: Vec<String>,
    pub import_selection: SelectionState,
    pub save_file: Option<String>,
    pub state_manager: StateManager,
    pub app_config: AppConfig,
    pub sort_field_selection: SelectionState,
    pub current_sort_field: Option<SortField>,
    pub current_sort_order: Option<SortOrder>,
    pub selected_cards: std::collections::HashSet<uuid::Uuid>,
    pub priority_selection: SelectionState,
    pub column_selection: SelectionState,
    pub task_list_view_selection: SelectionState,
    pub help_selection: SelectionState,
    pub help_pending_action: Option<(Instant, crate::keybindings::KeybindingAction)>,
    pub sprint_task_panel: SprintTaskPanel,
    pub sprint_uncompleted_cards: CardList,
    pub sprint_completed_cards: CardList,
    pub sprint_uncompleted_component: CardListComponent,
    pub sprint_completed_component: CardListComponent,
    pub view_strategy: Box<dyn ViewStrategy>,
    pub card_list_component: CardListComponent,
    pub search: SearchState,
    pub filter_dialog_state: Option<FilterDialogState>,
    pub viewport_height: usize,
    pub pending_key: Option<char>,
    pub file_change_rx: Option<tokio::sync::broadcast::Receiver<kanban_persistence::ChangeEvent>>,
    pub file_watcher: Option<kanban_persistence::FileWatcher>,
    pub save_worker_handle: Option<tokio::task::JoinHandle<()>>,
    pub save_completion_rx: Option<tokio::sync::mpsc::UnboundedReceiver<()>>,
    pub last_error: Option<(String, Instant)>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Focus {
    Boards,
    Cards,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CardFocus {
    Title,
    Metadata,
    Description,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BoardFocus {
    Name,
    Description,
    Settings,
    Sprints,
    Columns,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SprintTaskPanel {
    Uncompleted,
    Completed,
}

pub enum CardField {
    Title,
    Description,
}

pub enum BoardField {
    Name,
    Description,
}

#[derive(Debug, Clone, PartialEq, Eq)]
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
    SetCardPriority,
    SetBranchPrefix,
    OrderCards,
    SprintDetail,
    CreateSprint,
    AssignCardToSprint,
    AssignMultipleCardsToSprint,
    CreateColumn,
    RenameColumn,
    DeleteColumnConfirm,
    SelectTaskListView,
    Search,
    SetSprintPrefix,
    SetSprintCardPrefix,
    ConfirmSprintPrefixCollision,
    FilterOptions,
    ArchivedCardsView,
    ConflictResolution,
    ExternalChangeDetected,
    Help(Box<AppMode>),
}

impl App {
    pub fn new(
        save_file: Option<String>,
    ) -> (
        Self,
        Option<tokio::sync::mpsc::Receiver<crate::state::DataSnapshot>>,
    ) {
        let app_config = AppConfig::load();
        let (state_manager, save_rx, save_completion_rx) = StateManager::new(save_file.clone());
        let mut app = Self {
            should_quit: false,
            quit_with_pending: false,
            mode: AppMode::Normal,
            input: InputState::new(),
            boards: Vec::new(),
            board_selection: SelectionState::new(),
            active_board_index: None,
            active_card_index: None,
            columns: Vec::new(),
            cards: Vec::new(),
            archived_cards: Vec::new(),
            animating_cards: HashMap::new(),
            sprints: Vec::new(),
            sprint_selection: SelectionState::new(),
            active_sprint_index: None,
            active_sprint_filters: std::collections::HashSet::new(),
            hide_assigned_cards: false,
            sprint_assign_selection: SelectionState::new(),
            focus: Focus::Boards,
            card_focus: CardFocus::Title,
            board_focus: BoardFocus::Name,
            import_files: Vec::new(),
            import_selection: SelectionState::new(),
            save_file: save_file.clone(),
            state_manager,
            app_config,
            sort_field_selection: SelectionState::new(),
            current_sort_field: None,
            current_sort_order: None,
            selected_cards: std::collections::HashSet::new(),
            priority_selection: SelectionState::new(),
            column_selection: SelectionState::new(),
            task_list_view_selection: SelectionState::new(),
            help_selection: SelectionState::new(),
            help_pending_action: None,
            sprint_task_panel: SprintTaskPanel::Uncompleted,
            sprint_uncompleted_cards: CardList::new(CardListId::All),
            sprint_completed_cards: CardList::new(CardListId::All),
            sprint_uncompleted_component: CardListComponent::new(
                CardListId::All,
                CardListComponentConfig::new()
                    .with_actions(vec![
                        crate::card_list_component::CardListActionType::Navigation,
                        crate::card_list_component::CardListActionType::Selection,
                        crate::card_list_component::CardListActionType::Editing,
                        crate::card_list_component::CardListActionType::Completion,
                        crate::card_list_component::CardListActionType::Priority,
                        crate::card_list_component::CardListActionType::Sorting,
                    ])
                    .with_movement(false),
            ),
            sprint_completed_component: CardListComponent::new(
                CardListId::All,
                CardListComponentConfig::new()
                    .with_actions(vec![
                        crate::card_list_component::CardListActionType::Navigation,
                        crate::card_list_component::CardListActionType::Selection,
                        crate::card_list_component::CardListActionType::Sorting,
                    ])
                    .with_multi_select(false),
            ),
            view_strategy: Box::new(UnifiedViewStrategy::grouped()),
            card_list_component: CardListComponent::new(
                CardListId::All,
                CardListComponentConfig::new(),
            ),
            search: SearchState::new(),
            filter_dialog_state: None,
            viewport_height: 20,
            pending_key: None,
            file_change_rx: None,
            file_watcher: None,
            save_worker_handle: None,
            save_completion_rx,
            last_error: None,
        };

        if let Some(ref filename) = save_file {
            if std::path::Path::new(filename).exists() {
                if let Err(e) = app.import_board_from_file(filename) {
                    tracing::error!("Failed to load file {}: {}", filename, e);
                    app.save_file = None;
                    app.state_manager.clear_store();
                }
            }
        }

        app.migrate_sprint_logs();
        app.check_ended_sprints();

        (app, save_rx)
    }

    pub fn quit(&mut self) {
        self.should_quit = true;
    }

    pub fn set_error(&mut self, message: String) {
        self.last_error = Some((message, Instant::now()));
    }

    pub fn clear_error(&mut self) {
        self.last_error = None;
    }

    pub fn should_clear_error(&self) -> bool {
        if let Some((_, instant)) = self.last_error {
            instant.elapsed().as_secs() >= 5
        } else {
            false
        }
    }

    fn keycode_matches_binding_key(
        key_code: &crossterm::event::KeyCode,
        binding_key: &str,
    ) -> bool {
        use crossterm::event::KeyCode;

        match key_code {
            KeyCode::Char(c) => {
                // Check if the entire binding_key is a single char match (handles "/" correctly)
                if binding_key.len() == 1 && binding_key.starts_with(*c) {
                    return true;
                }
                // Check if any part after splitting on '/' matches
                binding_key
                    .split('/')
                    .any(|k| k.trim().len() == 1 && k.trim().starts_with(*c))
            }
            KeyCode::Enter => binding_key.split('/').any(|k| {
                let trimmed = k.trim();
                trimmed == "Enter" || trimmed == "ENTER"
            }),
            KeyCode::Esc => binding_key.split('/').any(|k| {
                let trimmed = k.trim();
                trimmed == "Esc" || trimmed == "ESC"
            }),
            KeyCode::Backspace => binding_key.split('/').any(|k| {
                let trimmed = k.trim();
                trimmed == "Backspace" || trimmed == "BACKSPACE"
            }),
            KeyCode::Home => binding_key.split('/').any(|k| {
                let trimmed = k.trim();
                trimmed == "Home" || trimmed == "HOME"
            }),
            KeyCode::End => binding_key.split('/').any(|k| {
                let trimmed = k.trim();
                trimmed == "End" || trimmed == "END"
            }),
            KeyCode::Down => binding_key.split('/').any(|k| {
                let trimmed = k.trim();
                trimmed == "↓" || trimmed == "Down" || trimmed == "DOWN"
            }),
            KeyCode::Up => binding_key.split('/').any(|k| {
                let trimmed = k.trim();
                trimmed == "↑" || trimmed == "Up" || trimmed == "UP"
            }),
            KeyCode::Left => binding_key.split('/').any(|k| {
                let trimmed = k.trim();
                trimmed == "←" || trimmed == "Left" || trimmed == "LEFT"
            }),
            KeyCode::Right => binding_key.split('/').any(|k| {
                let trimmed = k.trim();
                trimmed == "→" || trimmed == "Right" || trimmed == "RIGHT"
            }),
            _ => false,
        }
    }

    fn execute_action(&mut self, action: &crate::keybindings::KeybindingAction) {
        use crate::keybindings::KeybindingAction;

        match action {
            KeybindingAction::NavigateDown => self.handle_navigation_down(),
            KeybindingAction::NavigateUp => self.handle_navigation_up(),
            KeybindingAction::NavigateLeft => self.handle_kanban_column_left(),
            KeybindingAction::NavigateRight => self.handle_kanban_column_right(),
            KeybindingAction::SelectItem => self.handle_selection_activate(),
            KeybindingAction::CreateCard => self.handle_create_card_key(),
            KeybindingAction::CreateBoard => self.handle_create_board_key(),
            KeybindingAction::CreateSprint => self.handle_create_sprint_key(),
            KeybindingAction::CreateColumn => self.handle_create_column_key(),
            KeybindingAction::RenameBoard => self.handle_rename_board_key(),
            KeybindingAction::RenameColumn => self.handle_rename_column_key(),
            KeybindingAction::EditCard => {}
            KeybindingAction::EditBoard => self.handle_edit_board_key(),
            KeybindingAction::ToggleCompletion => self.handle_toggle_card_completion(),
            KeybindingAction::AssignToSprint => self.handle_assign_to_sprint_key(),
            KeybindingAction::ArchiveCard => self.handle_archive_card(),
            KeybindingAction::RestoreCard => self.handle_restore_card(),
            KeybindingAction::DeleteCard => self.handle_delete_card_permanent(),
            KeybindingAction::MoveCardLeft => self.handle_move_card_left(),
            KeybindingAction::MoveCardRight => self.handle_move_card_right(),
            KeybindingAction::MoveColumnUp => self.handle_move_column_up(),
            KeybindingAction::MoveColumnDown => self.handle_move_column_down(),
            KeybindingAction::DeleteColumn => self.handle_delete_column_key(),
            KeybindingAction::ExportBoard => self.handle_export_board_key(),
            KeybindingAction::ExportAll => self.handle_export_all_key(),
            KeybindingAction::ImportBoard => self.handle_import_board_key(),
            KeybindingAction::OrderCards => self.handle_order_cards_key(),
            KeybindingAction::ToggleSortOrder => self.handle_toggle_sort_order_key(),
            KeybindingAction::ToggleFilter => self.handle_toggle_sprint_filter(),
            KeybindingAction::ToggleHideAssigned => self.handle_open_filter_dialog(),
            KeybindingAction::ToggleArchivedView => self.handle_toggle_archived_cards_view(),
            KeybindingAction::ToggleTaskListView => self.handle_toggle_task_list_view(),
            KeybindingAction::ToggleCardSelection => self.handle_card_selection_toggle(),
            KeybindingAction::Search => {
                if self.focus == Focus::Cards {
                    self.search.activate();
                    self.mode = AppMode::Search;
                }
            }
            KeybindingAction::ShowHelp => {}
            KeybindingAction::Escape => self.handle_escape_key(),
            KeybindingAction::FocusPanel(panel) => self.handle_column_or_focus_switch(*panel),
            KeybindingAction::JumpToTop => self.handle_jump_to_top(),
            KeybindingAction::JumpToBottom => self.handle_jump_to_bottom(),
            KeybindingAction::JumpHalfViewportUp => self.handle_jump_half_viewport_up(),
            KeybindingAction::JumpHalfViewportDown => self.handle_jump_half_viewport_down(),
        }
    }

    fn handle_key_event(
        &mut self,
        key: crossterm::event::KeyEvent,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
        event_handler: &EventHandler,
    ) -> bool {
        use crossterm::event::KeyCode;
        let mut should_restart_events = false;

        let is_input_mode = matches!(
            self.mode,
            AppMode::CreateBoard
                | AppMode::CreateCard
                | AppMode::CreateSprint
                | AppMode::RenameBoard
                | AppMode::ExportBoard
                | AppMode::ExportAll
                | AppMode::SetCardPoints
                | AppMode::SetBranchPrefix
                | AppMode::CreateColumn
                | AppMode::RenameColumn
                | AppMode::Search
                | AppMode::SetSprintPrefix
                | AppMode::SetSprintCardPrefix
        );

        if matches!(key.code, KeyCode::Char('q') | KeyCode::Char('Q')) && !is_input_mode {
            // Check for pending saves before quitting
            if self.state_manager.has_pending_saves() && !self.quit_with_pending {
                // First quit attempt with pending saves - show warning
                self.set_error(
                    "⏳ Saves pending... press 'q' again to force quit, or wait for completion"
                        .to_string(),
                );
                self.quit_with_pending = true;
                tracing::warn!("Quit attempted with pending saves, requiring confirmation");
                return false;
            }

            // Either no pending saves, or user confirmed force quit
            self.quit();
            return false;
        }

        if matches!(key.code, KeyCode::Char('?'))
            && !is_input_mode
            && !matches!(self.mode, AppMode::Help(_))
        {
            let previous_mode = self.mode.clone();
            self.help_selection.set(Some(0));
            self.mode = AppMode::Help(Box::new(previous_mode));
            return false;
        }

        match self.mode {
            AppMode::Normal => match key.code {
                KeyCode::Char('/') => {
                    self.pending_key = None;
                    if self.focus == Focus::Cards {
                        self.search.activate();
                        self.mode = AppMode::Search;
                    }
                }
                KeyCode::Char('g') => {
                    if self.pending_key == Some('g') {
                        self.pending_key = None;
                        self.handle_jump_to_top();
                    } else {
                        self.pending_key = Some('g');
                    }
                }
                KeyCode::Char('G') => {
                    self.pending_key = None;
                    self.handle_jump_to_bottom();
                }
                KeyCode::Char('{') => {
                    self.pending_key = None;
                    self.handle_jump_half_viewport_up();
                }
                KeyCode::Char('}') => {
                    self.pending_key = None;
                    self.handle_jump_half_viewport_down();
                }
                KeyCode::Char('n') => {
                    self.pending_key = None;
                    match self.focus {
                        Focus::Boards => self.handle_create_board_key(),
                        Focus::Cards => self.handle_create_card_key(),
                    }
                }
                KeyCode::Char('r') => {
                    self.pending_key = None;
                    self.handle_rename_board_key();
                }
                KeyCode::Char('e') => {
                    self.pending_key = None;
                    match self.focus {
                        Focus::Boards => self.handle_edit_board_key(),
                        Focus::Cards => {
                            should_restart_events =
                                self.handle_edit_card_key(terminal, event_handler);
                        }
                    }
                }
                KeyCode::Char('x') => {
                    self.pending_key = None;
                    self.handle_export_board_key();
                }
                KeyCode::Char('X') => {
                    self.pending_key = None;
                    self.handle_export_all_key();
                }
                KeyCode::Char('d') => {
                    self.pending_key = None;
                    self.handle_archive_card();
                }
                KeyCode::Char('D') => {
                    self.pending_key = None;
                    self.handle_toggle_archived_cards_view();
                }
                KeyCode::Char('i') => {
                    self.pending_key = None;
                    self.handle_import_board_key();
                }
                KeyCode::Char('a') => {
                    self.pending_key = None;
                    self.handle_assign_to_sprint_key();
                }
                KeyCode::Char('c') => {
                    self.pending_key = None;
                    self.handle_toggle_card_completion();
                }
                KeyCode::Char('o') => {
                    self.pending_key = None;
                    self.handle_order_cards_key();
                }
                KeyCode::Char('O') => {
                    self.pending_key = None;
                    self.handle_toggle_sort_order_key();
                }
                KeyCode::Char('T') => {
                    self.pending_key = None;
                    self.handle_open_filter_dialog();
                }
                KeyCode::Char('t') => {
                    self.pending_key = None;
                    self.handle_toggle_sprint_filter();
                }
                KeyCode::Char('v') => {
                    self.pending_key = None;
                    self.handle_card_selection_toggle();
                }
                KeyCode::Char('V') => {
                    self.pending_key = None;
                    self.handle_toggle_task_list_view();
                }
                KeyCode::Char('H') => {
                    self.pending_key = None;
                    self.handle_move_card_left();
                }
                KeyCode::Char('L') => {
                    self.pending_key = None;
                    self.handle_move_card_right();
                }
                KeyCode::Char('h') => {
                    self.pending_key = None;
                    self.handle_kanban_column_left();
                }
                KeyCode::Char('l') => {
                    self.pending_key = None;
                    self.handle_kanban_column_right();
                }
                KeyCode::Char('1') => {
                    self.pending_key = None;
                    self.handle_column_or_focus_switch(0);
                }
                KeyCode::Char('2') => {
                    self.pending_key = None;
                    self.handle_column_or_focus_switch(1);
                }
                KeyCode::Char('3') => {
                    self.pending_key = None;
                    self.handle_column_or_focus_switch(2);
                }
                KeyCode::Char('4') => {
                    self.pending_key = None;
                    self.handle_column_or_focus_switch(3);
                }
                KeyCode::Char('5') => {
                    self.pending_key = None;
                    self.handle_column_or_focus_switch(4);
                }
                KeyCode::Char('6') => {
                    self.pending_key = None;
                    self.handle_column_or_focus_switch(5);
                }
                KeyCode::Char('7') => {
                    self.pending_key = None;
                    self.handle_column_or_focus_switch(6);
                }
                KeyCode::Char('8') => {
                    self.pending_key = None;
                    self.handle_column_or_focus_switch(7);
                }
                KeyCode::Char('9') => {
                    self.pending_key = None;
                    self.handle_column_or_focus_switch(8);
                }
                KeyCode::Esc => {
                    self.pending_key = None;
                    self.handle_escape_key();
                }
                KeyCode::Char('j') | KeyCode::Down => {
                    self.pending_key = None;
                    self.handle_navigation_down();
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    self.pending_key = None;
                    self.handle_navigation_up();
                }
                KeyCode::Enter | KeyCode::Char(' ') => {
                    self.pending_key = None;
                    self.handle_selection_activate();
                }
                _ => {
                    self.pending_key = None;
                }
            },
            AppMode::CreateBoard => self.handle_create_board_dialog(key.code),
            AppMode::CreateCard => self.handle_create_card_dialog(key.code),
            AppMode::CreateSprint => self.handle_create_sprint_dialog(key.code),
            AppMode::RenameBoard => self.handle_rename_board_dialog(key.code),
            AppMode::ExportBoard => self.handle_export_board_dialog(key.code),
            AppMode::ExportAll => self.handle_export_all_dialog(key.code),
            AppMode::ImportBoard => self.handle_import_board_popup(key.code),
            AppMode::SetCardPoints => {
                should_restart_events = self.handle_set_card_points_dialog(key.code);
            }
            AppMode::SetCardPriority => self.handle_set_card_priority_popup(key.code),
            AppMode::CardDetail => {
                should_restart_events =
                    self.handle_card_detail_key(key.code, terminal, event_handler);
            }
            AppMode::BoardDetail => {
                should_restart_events =
                    self.handle_board_detail_key(key.code, terminal, event_handler);
            }
            AppMode::SetBranchPrefix => self.handle_set_branch_prefix_dialog(key.code),
            AppMode::SetSprintPrefix => self.handle_set_sprint_prefix_dialog(key.code),
            AppMode::SetSprintCardPrefix => self.handle_set_sprint_card_prefix_dialog(key.code),
            AppMode::OrderCards => {
                should_restart_events = self.handle_order_cards_popup(key.code);
            }
            AppMode::SprintDetail => self.handle_sprint_detail_key(key.code),
            AppMode::AssignCardToSprint => self.handle_assign_card_to_sprint_popup(key.code),
            AppMode::AssignMultipleCardsToSprint => {
                self.handle_assign_multiple_cards_to_sprint_popup(key.code)
            }
            AppMode::CreateColumn => self.handle_create_column_dialog(key.code),
            AppMode::RenameColumn => self.handle_rename_column_dialog(key.code),
            AppMode::DeleteColumnConfirm => self.handle_delete_column_confirm_popup(key.code),
            AppMode::SelectTaskListView => self.handle_select_task_list_view_popup(key.code),
            AppMode::Search => self.handle_search_mode(key.code),
            AppMode::ConfirmSprintPrefixCollision => {
                self.handle_confirm_sprint_prefix_collision_popup(key.code)
            }
            AppMode::FilterOptions => self.handle_filter_options_popup(key.code),
            AppMode::ArchivedCardsView => self.handle_archived_cards_view_mode(key.code),
            AppMode::ConflictResolution => self.handle_conflict_resolution_popup(key.code),
            AppMode::ExternalChangeDetected => self.handle_external_change_detected_popup(key.code),
            AppMode::Help(_) => self.handle_help_mode(key.code),
        }
        should_restart_events
    }

    fn handle_search_mode(&mut self, key_code: crossterm::event::KeyCode) {
        use crossterm::event::KeyCode;
        match key_code {
            KeyCode::Char(c) => {
                self.search.input.insert_char(c);
                self.refresh_view();
            }
            KeyCode::Backspace => {
                self.search.input.backspace();
                self.refresh_view();
            }
            KeyCode::Esc | KeyCode::Enter => {
                self.search.deactivate();
                self.mode = AppMode::Normal;
            }
            _ => {}
        }
    }

    fn handle_archived_cards_view_mode(&mut self, key_code: crossterm::event::KeyCode) {
        use crossterm::event::KeyCode;
        if self.focus != Focus::Cards {
            self.focus = Focus::Cards;
        }

        match key_code {
            KeyCode::Char('r') => self.handle_restore_card(),
            KeyCode::Char('x') => self.handle_delete_card_permanent(),
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('Q') => {
                self.handle_toggle_archived_cards_view();
            }
            KeyCode::Char('v') => self.handle_card_selection_toggle(),
            KeyCode::Char('V') => self.handle_toggle_task_list_view(),
            KeyCode::Char('h') => self.handle_kanban_column_left(),
            KeyCode::Char('l') => self.handle_kanban_column_right(),
            KeyCode::Char('j') | KeyCode::Down => self.handle_navigation_down(),
            KeyCode::Char('k') | KeyCode::Up => self.handle_navigation_up(),
            _ => {}
        }
    }

    fn handle_help_mode(&mut self, key_code: crossterm::event::KeyCode) {
        use crate::keybindings::KeybindingRegistry;
        use crossterm::event::KeyCode;

        match key_code {
            KeyCode::Char('j') | KeyCode::Down => {
                self.help_pending_action = None;
                let provider = KeybindingRegistry::get_provider(self);
                let context = provider.get_context();
                self.help_selection.next(context.bindings.len());
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.help_pending_action = None;
                self.help_selection.prev();
            }
            KeyCode::Char('h') | KeyCode::Char('l') => {
                self.help_pending_action = None;
            }
            KeyCode::Enter => {
                self.help_pending_action = None;
                if let Some(index) = self.help_selection.get() {
                    let provider = KeybindingRegistry::get_provider(self);
                    let context = provider.get_context();

                    if let Some(binding) = context.bindings.get(index) {
                        if let AppMode::Help(previous_mode) = &self.mode {
                            self.mode = (**previous_mode).clone();
                        } else {
                            self.mode = AppMode::Normal;
                        }
                        self.help_selection.clear();

                        self.execute_action(&binding.action);
                    }
                }
            }
            KeyCode::Esc | KeyCode::Char('?') => {
                self.help_pending_action = None;
                if let AppMode::Help(previous_mode) = &self.mode {
                    self.mode = (**previous_mode).clone();
                } else {
                    self.mode = AppMode::Normal;
                }
                self.help_selection.clear();
            }
            _ => {
                let provider = KeybindingRegistry::get_provider(self);
                let context = provider.get_context();

                if let Some((index, binding)) = context
                    .bindings
                    .iter()
                    .enumerate()
                    .find(|(_, b)| Self::keycode_matches_binding_key(&key_code, &b.key))
                {
                    self.help_selection.set(Some(index));
                    self.help_pending_action = Some((Instant::now(), binding.action));
                }
            }
        }
    }

    fn handle_animation_tick(&mut self) {
        let now = Instant::now();
        let mut completed_animations = Vec::new();

        for (&card_id, animation) in &self.animating_cards {
            let elapsed = now.duration_since(animation.start_time).as_millis();
            if elapsed >= ANIMATION_DURATION_MS {
                completed_animations.push((card_id, animation.animation_type));
            }
        }

        // Group animations by type for batch processing
        let mut archive_cards = Vec::new();
        let mut restore_cards = Vec::new();
        let mut delete_cards = Vec::new();
        let mut last_archive_column = None;
        let mut last_archive_position = None;

        for (card_id, animation_type) in completed_animations {
            self.animating_cards.remove(&card_id);
            match animation_type {
                AnimationType::Archiving => {
                    if let Some(card_pos) = self.cards.iter().position(|c| c.id == card_id) {
                        let card = self.cards[card_pos].clone();
                        last_archive_column = Some(card.column_id);
                        last_archive_position = Some(card.position);
                        archive_cards.push(card_id);
                    }
                }
                AnimationType::Restoring => {
                    restore_cards.push(card_id);
                }
                AnimationType::Deleting => {
                    delete_cards.push(card_id);
                }
            }
        }

        let had_archives = !archive_cards.is_empty();
        let had_deletes = !delete_cards.is_empty();
        let had_restores = !restore_cards.is_empty();

        // Execute batch archive commands
        if had_archives {
            let mut archive_commands: Vec<Box<dyn crate::state::commands::Command>> = Vec::new();
            for card_id in archive_cards {
                let cmd = Box::new(kanban_domain::commands::ArchiveCard { card_id })
                    as Box<dyn crate::state::commands::Command>;
                archive_commands.push(cmd);
            }
            if let Err(e) = self.execute_commands_batch(archive_commands) {
                tracing::error!("Failed to archive cards: {}", e);
            } else if let (Some(column_id), Some(position)) =
                (last_archive_column, last_archive_position)
            {
                self.compact_column_positions(column_id);
                self.select_card_after_deletion(column_id, position);
            }
        }

        // Execute batch delete commands
        if had_deletes {
            let mut delete_commands: Vec<Box<dyn crate::state::commands::Command>> = Vec::new();
            for card_id in delete_cards {
                let cmd = Box::new(kanban_domain::commands::DeleteCard { card_id })
                    as Box<dyn crate::state::commands::Command>;
                delete_commands.push(cmd);
            }
            if let Err(e) = self.execute_commands_batch(delete_commands) {
                tracing::error!("Failed to delete cards: {}", e);
            }
        }

        // Handle restore animations individually (less common)
        for card_id in restore_cards {
            self.complete_restore_animation(card_id);
        }

        // Refresh view once at the end
        if had_archives || had_deletes || had_restores {
            self.refresh_view();
        }
    }

    fn complete_restore_animation(&mut self, card_id: uuid::Uuid) {
        if let Some(pos) = self
            .archived_cards
            .iter()
            .position(|dc| dc.card.id == card_id)
        {
            let deleted_card = self.archived_cards.remove(pos);
            self.restore_card(deleted_card);
        }
    }

    pub fn get_board_card_count(&self, board_id: uuid::Uuid) -> usize {
        let board_filter = BoardFilter::new(board_id, &self.columns);

        let cards: Vec<_> = self
            .cards
            .iter()
            .filter(|c| {
                if !board_filter.matches(c) {
                    return false;
                }
                true
            })
            .collect();

        if self.active_sprint_filters.is_empty() && !self.hide_assigned_cards {
            return cards.len();
        }

        cards
            .iter()
            .filter(|c| {
                if !self.active_sprint_filters.is_empty() {
                    if let Some(sprint_id) = c.sprint_id {
                        if !self.active_sprint_filters.contains(&sprint_id) {
                            return false;
                        }
                    } else {
                        return false;
                    }
                }
                if self.hide_assigned_cards && c.sprint_id.is_some() {
                    return false;
                }
                true
            })
            .count()
    }

    pub fn get_sorted_board_cards(&self, board_id: uuid::Uuid) -> Vec<&Card> {
        let board = self.boards.iter().find(|b| b.id == board_id).unwrap();
        let board_filter = BoardFilter::new(board_id, &self.columns);

        let mut cards: Vec<&Card> = self
            .cards
            .iter()
            .filter(|c| {
                if !board_filter.matches(c) {
                    return false;
                }
                if !self.active_sprint_filters.is_empty() {
                    if let Some(sprint_id) = c.sprint_id {
                        if !self.active_sprint_filters.contains(&sprint_id) {
                            return false;
                        }
                    } else {
                        return false;
                    }
                }
                if self.hide_assigned_cards && c.sprint_id.is_some() {
                    return false;
                }
                true
            })
            .collect();

        let sorter = get_sorter_for_field(board.task_sort_field);
        let ordered_sorter = OrderedSorter::new(sorter, board.task_sort_order);
        ordered_sorter.sort(&mut cards);

        cards
    }

    pub fn get_selected_card_in_context(&self) -> Option<&Card> {
        if let Some(task_list) = self.view_strategy.get_active_task_list() {
            if let Some(card_id) = task_list.get_selected_card_id() {
                if self.mode == AppMode::ArchivedCardsView {
                    return self
                        .archived_cards
                        .iter()
                        .find(|dc| dc.card.id == card_id)
                        .map(|dc| &dc.card);
                } else {
                    return self.cards.iter().find(|c| c.id == card_id);
                }
            }
        }
        None
    }

    pub fn get_selected_card_id(&self) -> Option<uuid::Uuid> {
        self.view_strategy
            .get_active_task_list()
            .and_then(|list| list.get_selected_card_id())
    }

    pub fn select_card_by_id(&mut self, card_id: uuid::Uuid) {
        if let Some(task_list) = self.view_strategy.get_active_task_list_mut() {
            task_list.select_card(card_id);
        }
    }

    pub fn get_card_by_id(&self, card_id: uuid::Uuid) -> Option<&Card> {
        if self.mode == AppMode::ArchivedCardsView {
            self.archived_cards
                .iter()
                .find(|dc| dc.card.id == card_id)
                .map(|dc| &dc.card)
        } else {
            self.cards.iter().find(|c| c.id == card_id)
        }
    }

    pub fn get_sprint_cards(&self, sprint_id: uuid::Uuid) -> Vec<&Card> {
        self.cards
            .iter()
            .filter(|card| card.sprint_id == Some(sprint_id))
            .collect()
    }

    pub fn get_sprint_completed_cards(&self, sprint_id: uuid::Uuid) -> Vec<&Card> {
        let cards: Vec<&Card> = self
            .cards
            .iter()
            .filter(|card| card.sprint_id == Some(sprint_id) && card.is_completed())
            .collect();
        tracing::debug!(
            "get_sprint_completed_cards({}): found {} cards",
            sprint_id,
            cards.len()
        );
        cards
    }

    pub fn get_sprint_uncompleted_cards(&self, sprint_id: uuid::Uuid) -> Vec<&Card> {
        let cards: Vec<&Card> = self
            .cards
            .iter()
            .filter(|card| card.sprint_id == Some(sprint_id) && !card.is_completed())
            .collect();
        tracing::debug!(
            "get_sprint_uncompleted_cards({}): found {} cards",
            sprint_id,
            cards.len()
        );
        cards
    }

    pub fn populate_sprint_task_lists(&mut self, sprint_id: uuid::Uuid) {
        let uncompleted_ids: Vec<uuid::Uuid> = self
            .cards
            .iter()
            .filter(|card| card.sprint_id == Some(sprint_id) && !card.is_completed())
            .map(|card| card.id)
            .collect();

        let completed_ids: Vec<uuid::Uuid> = self
            .cards
            .iter()
            .filter(|card| card.sprint_id == Some(sprint_id) && card.is_completed())
            .map(|card| card.id)
            .collect();

        self.sprint_uncompleted_cards.update_cards(uncompleted_ids);
        self.sprint_completed_cards.update_cards(completed_ids);

        self.sprint_uncompleted_component
            .update_cards(self.sprint_uncompleted_cards.cards.clone());
        self.sprint_completed_component
            .update_cards(self.sprint_completed_cards.cards.clone());

        // Default to uncompleted panel
        self.sprint_task_panel = SprintTaskPanel::Uncompleted;
    }

    pub fn apply_sort_to_sprint_lists(&mut self, sort_field: SortField, sort_order: SortOrder) {
        let uncompleted_card_ids: Vec<uuid::Uuid> = self.sprint_uncompleted_cards.cards.clone();
        let completed_card_ids: Vec<uuid::Uuid> = self.sprint_completed_cards.cards.clone();

        let mut uncompleted_cards: Vec<&Card> = uncompleted_card_ids
            .iter()
            .filter_map(|id| self.cards.iter().find(|c| c.id == *id))
            .collect();

        let mut completed_cards: Vec<&Card> = completed_card_ids
            .iter()
            .filter_map(|id| self.cards.iter().find(|c| c.id == *id))
            .collect();

        let sorter = get_sorter_for_field(sort_field);
        let ordered_sorter = OrderedSorter::new(sorter, sort_order);

        ordered_sorter.sort(&mut uncompleted_cards);
        ordered_sorter.sort(&mut completed_cards);

        let sorted_uncompleted_ids: Vec<uuid::Uuid> =
            uncompleted_cards.iter().map(|c| c.id).collect();
        let sorted_completed_ids: Vec<uuid::Uuid> = completed_cards.iter().map(|c| c.id).collect();

        self.sprint_uncompleted_cards
            .update_cards(sorted_uncompleted_ids);
        self.sprint_completed_cards
            .update_cards(sorted_completed_ids);

        self.sprint_uncompleted_component
            .update_cards(self.sprint_uncompleted_cards.cards.clone());
        self.sprint_completed_component
            .update_cards(self.sprint_completed_cards.cards.clone());
    }

    pub fn calculate_points(cards: &[&Card]) -> u32 {
        cards
            .iter()
            .filter_map(|card| card.points.map(|p| p as u32))
            .sum()
    }

    /// Execute multiple commands as a batch with a single pause/resume cycle
    /// This is the preferred method as it prevents race conditions where rapid successive saves
    /// detect previous writes as external. For single commands, this still works efficiently.
    pub fn execute_command(
        &mut self,
        command: Box<dyn crate::state::commands::Command>,
    ) -> KanbanResult<()> {
        self.execute_commands_batch(vec![command])
    }

    /// Execute multiple commands as a batch with a single pause/resume cycle
    /// This prevents race conditions where rapid successive saves detect previous writes as external
    pub fn execute_commands_batch(
        &mut self,
        commands: Vec<Box<dyn crate::state::commands::Command>>,
    ) -> KanbanResult<()> {
        // Execute all commands first
        {
            let Self {
                state_manager,
                boards,
                columns,
                cards,
                sprints,
                archived_cards,
                ..
            } = self;

            for command in commands {
                state_manager.execute_with_context(
                    boards,
                    columns,
                    cards,
                    sprints,
                    archived_cards,
                    command,
                )?;
            }
        }

        // Queue one snapshot for async save after all commands have executed
        let snapshot = crate::state::DataSnapshot::from_app(self);
        self.state_manager.queue_snapshot(snapshot);

        Ok(())
    }

    pub fn refresh_view(&mut self) {
        let board_idx = self.active_board_index.or(self.board_selection.get());
        if let Some(idx) = board_idx {
            if let Some(board) = self.boards.get(idx) {
                let search_query = if self.search.is_active {
                    Some(self.search.query())
                } else {
                    None
                };

                // When in DeletedCardsView, convert deleted cards to Card objects for display
                let cards_for_display: Vec<Card> = if self.mode == AppMode::ArchivedCardsView {
                    self.archived_cards
                        .iter()
                        .map(|dc| dc.card.clone())
                        .collect()
                } else {
                    self.cards.clone()
                };

                let ctx = ViewRefreshContext {
                    board,
                    all_cards: &cards_for_display,
                    all_columns: &self.columns,
                    all_sprints: &self.sprints,
                    active_sprint_filters: self.active_sprint_filters.clone(),
                    hide_assigned_cards: self.hide_assigned_cards,
                    search_query,
                };
                self.view_strategy.refresh_task_lists(&ctx);
            }
        }
        self.sync_card_list_component();
    }

    pub fn sync_card_list_component(&mut self) {
        if let Some(active_list) = self.view_strategy.get_active_task_list() {
            self.card_list_component
                .update_cards(active_list.cards.clone());
        }
    }

    pub fn switch_view_strategy(&mut self, task_list_view: kanban_domain::TaskListView) {
        let new_strategy: Box<dyn ViewStrategy> = match task_list_view {
            kanban_domain::TaskListView::Flat => Box::new(UnifiedViewStrategy::flat()),
            kanban_domain::TaskListView::GroupedByColumn => {
                Box::new(UnifiedViewStrategy::grouped())
            }
            kanban_domain::TaskListView::ColumnView => Box::new(UnifiedViewStrategy::kanban()),
        };

        self.view_strategy = new_strategy;
        self.refresh_view();
    }

    pub fn export_board_with_filename(&self) -> io::Result<()> {
        if let Some(board_idx) = self.board_selection.get() {
            if let Some(board) = self.boards.get(board_idx) {
                let board_export = BoardExporter::export_board(
                    board,
                    &self.columns,
                    &self.cards,
                    &self.archived_cards,
                    &self.sprints,
                );

                let export = crate::export::AllBoardsExport {
                    boards: vec![board_export],
                };

                BoardExporter::export_to_file(&export, self.input.as_str())?;
            }
        }
        Ok(())
    }

    pub fn export_all_boards_with_filename(&self) -> io::Result<()> {
        let export = BoardExporter::export_all_boards(
            &self.boards,
            &self.columns,
            &self.cards,
            &self.archived_cards,
            &self.sprints,
        );
        BoardExporter::export_to_file(&export, self.input.as_str())?;
        Ok(())
    }

    pub fn auto_save(&self) -> io::Result<()> {
        if let Some(ref filename) = self.save_file {
            let export = BoardExporter::export_all_boards(
                &self.boards,
                &self.columns,
                &self.cards,
                &self.archived_cards,
                &self.sprints,
            );
            BoardExporter::export_to_file(&export, filename)?;
        }
        Ok(())
    }

    fn check_ended_sprints(&self) {
        let ended_sprints: Vec<_> = self.sprints.iter().filter(|s| s.is_ended()).collect();

        if !ended_sprints.is_empty() {
            tracing::warn!(
                "Found {} ended sprint(s) that need attention:",
                ended_sprints.len()
            );
            for sprint in &ended_sprints {
                if let Some(board) = self.boards.iter().find(|b| b.id == sprint.board_id) {
                    tracing::warn!(
                        "  - {} (ended: {})",
                        sprint.formatted_name(board, "sprint"),
                        sprint
                            .end_date
                            .map(|d| d.format("%Y-%m-%d %H:%M UTC").to_string())
                            .unwrap_or_else(|| "unknown".to_string())
                    );
                }
            }
        }
    }

    pub fn edit_board_field(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
        event_handler: &EventHandler,
        field: BoardField,
    ) -> io::Result<()> {
        if let Some(board_idx) = self.board_selection.get() {
            if let Some(board) = self.boards.get(board_idx) {
                let temp_dir = std::env::temp_dir();
                let (temp_file, current_content) = match field {
                    BoardField::Name => {
                        let temp_file = temp_dir.join(format!("kanban-board-{}-name.md", board.id));
                        (temp_file, board.name.clone())
                    }
                    BoardField::Description => {
                        let temp_file =
                            temp_dir.join(format!("kanban-board-{}-description.md", board.id));
                        let content = board.description.as_deref().unwrap_or("").to_string();
                        (temp_file, content)
                    }
                };

                if let Some(new_content) =
                    edit_in_external_editor(terminal, event_handler, temp_file, &current_content)?
                {
                    let board_id = board.id;
                    if let Some(board) = self.boards.iter_mut().find(|b| b.id == board_id) {
                        match field {
                            BoardField::Name => {
                                if !new_content.trim().is_empty() {
                                    board.update_name(new_content.trim().to_string());
                                    self.state_manager.mark_dirty();
                                    let snapshot = crate::state::DataSnapshot::from_app(self);
                                    self.state_manager.queue_snapshot(snapshot);
                                }
                            }
                            BoardField::Description => {
                                let desc = if new_content.trim().is_empty() {
                                    None
                                } else {
                                    Some(new_content)
                                };
                                board.update_description(desc);
                                self.state_manager.mark_dirty();
                                let snapshot = crate::state::DataSnapshot::from_app(self);
                                self.state_manager.queue_snapshot(snapshot);
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    pub fn edit_card_field(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
        event_handler: &EventHandler,
        field: CardField,
    ) -> io::Result<()> {
        if let Some(card_idx) = self.active_card_index {
            if let Some(card) = self.cards.get(card_idx) {
                let temp_dir = std::env::temp_dir();
                let (temp_file, current_content) = match field {
                    CardField::Title => {
                        let temp_file = temp_dir.join(format!("kanban-card-{}-title.md", card.id));
                        (temp_file, card.title.clone())
                    }
                    CardField::Description => {
                        let temp_file =
                            temp_dir.join(format!("kanban-card-{}-description.md", card.id));
                        let content = card.description.as_deref().unwrap_or("").to_string();
                        (temp_file, content)
                    }
                };

                if let Some(new_content) =
                    edit_in_external_editor(terminal, event_handler, temp_file, &current_content)?
                {
                    let card_id = card.id;
                    if let Some(card) = self.cards.iter_mut().find(|c| c.id == card_id) {
                        match field {
                            CardField::Title => {
                                if !new_content.trim().is_empty() {
                                    card.update_title(new_content.trim().to_string());
                                    self.state_manager.mark_dirty();
                                    let snapshot = crate::state::DataSnapshot::from_app(self);
                                    self.state_manager.queue_snapshot(snapshot);
                                }
                            }
                            CardField::Description => {
                                let desc = if new_content.trim().is_empty() {
                                    None
                                } else {
                                    Some(new_content)
                                };
                                card.update_description(desc);
                                self.state_manager.mark_dirty();
                                let snapshot = crate::state::DataSnapshot::from_app(self);
                                self.state_manager.queue_snapshot(snapshot);
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    pub fn edit_entity_json_impl<T: Editable<E>, E>(
        entity: &mut E,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
        event_handler: &EventHandler,
        temp_file: std::path::PathBuf,
    ) -> io::Result<()> {
        let dto = T::from_entity(entity);
        let current_content =
            serde_json::to_string_pretty(&dto).unwrap_or_else(|_| "{}".to_string());

        if let Some(new_content) =
            edit_in_external_editor(terminal, event_handler, temp_file, &current_content)?
        {
            match serde_json::from_str::<T>(&new_content) {
                Ok(updated_dto) => {
                    updated_dto.apply_to(entity);
                    tracing::info!("Updated entity via JSON editor");
                }
                Err(e) => {
                    tracing::error!("Failed to parse JSON: {}", e);
                }
            }
        }

        Ok(())
    }

    pub async fn run(
        &mut self,
        save_rx: Option<tokio::sync::mpsc::Receiver<crate::state::DataSnapshot>>,
    ) -> KanbanResult<()> {
        let mut terminal = setup_terminal()?;

        // Initialize file watching if a save file is configured
        // (Done before spawning save worker so worker can pause/resume it)
        if let Some(ref save_file) = self.save_file {
            use kanban_persistence::ChangeDetector;
            tracing::info!("Initializing file watcher for: {}", save_file);
            let watcher = kanban_persistence::FileWatcher::new();
            let rx = watcher.subscribe();
            self.file_change_rx = Some(rx);
            tracing::debug!("File change broadcast receiver subscribed");

            let path = std::path::PathBuf::from(save_file);
            if let Err(e) = watcher.start_watching(path.clone()).await {
                tracing::warn!(
                    "Failed to start file watching for {}: {}",
                    path.display(),
                    e
                );
            } else {
                tracing::info!("File watcher started for: {}", path.display());
            }

            // Store the watcher to keep the background task alive
            self.file_watcher = Some(watcher.clone());
            // Also set it on the state manager (wrapped in Arc) so queue_snapshot can pause it
            let watcher_arc = std::sync::Arc::new(watcher);
            self.state_manager.set_file_watcher(watcher_arc);
        }

        // Spawn async save worker if save channel is configured
        if let Some(mut rx) = save_rx {
            tracing::debug!("Save channel receiver available");
            if let Some(store) = self.state_manager.store().cloned() {
                let instance_id = self.state_manager.instance_id();
                let file_watcher = self.file_watcher.clone();
                let save_completion_tx = self.state_manager.save_completion_tx().cloned();

                tracing::info!("Spawning save worker to process snapshots");
                let handle = tokio::spawn(async move {
                    tracing::info!("Save worker task started, waiting for snapshots");
                    while let Some(snapshot) = rx.recv().await {
                        tracing::debug!("Save worker received snapshot, starting save operation");

                        let data = match snapshot.to_json_bytes() {
                            Ok(d) => d,
                            Err(e) => {
                                tracing::error!("Failed to serialize snapshot: {}", e);
                                // Resume file watching since save is not happening
                                if let Some(ref watcher) = file_watcher {
                                    watcher.resume();
                                }
                                continue;
                            }
                        };

                        let persistence_snapshot = StoreSnapshot {
                            data,
                            metadata: PersistenceMetadata::new(2, instance_id),
                        };

                        match store.save(persistence_snapshot).await {
                            Ok(_) => {
                                tracing::debug!("Save worker completed save");
                                // Signal that save is complete
                                if let Some(ref tx) = save_completion_tx {
                                    if let Err(e) = tx.send(()) {
                                        tracing::error!(
                                            "Failed to send save completion signal: {}",
                                            e
                                        );
                                    }
                                }
                            }
                            Err(kanban_core::KanbanError::ConflictDetected { path, .. }) => {
                                tracing::warn!("Save worker detected conflict at {}", path);
                                // Signal completion even on conflict
                                if let Some(ref tx) = save_completion_tx {
                                    if let Err(e) = tx.send(()) {
                                        tracing::error!(
                                            "Failed to send save completion signal: {}",
                                            e
                                        );
                                    }
                                }
                            }
                            Err(e) => {
                                tracing::error!("Save worker failed: {}", e);
                                // Signal completion even on failure
                                if let Some(ref tx) = save_completion_tx {
                                    if let Err(e) = tx.send(()) {
                                        tracing::error!(
                                            "Failed to send save completion signal: {}",
                                            e
                                        );
                                    }
                                }
                            }
                        }

                        // Resume file watching after save completes
                        // Metadata-based own-write detection filters out our own writes
                        if let Some(ref watcher) = file_watcher {
                            watcher.resume();
                        }
                    }
                    tracing::info!("Save worker exited recv loop (channel closed)");
                });
                self.save_worker_handle = Some(handle);
            } else {
                tracing::warn!("Could not spawn save worker: no store available");
            }
        } else {
            tracing::debug!("No save channel receiver - no saves will be processed");
        }

        while !self.should_quit {
            let mut events = EventHandler::new();

            loop {
                terminal.draw(|frame| ui::render(self, frame))?;

                tokio::select! {
                    Some(event) = events.next() => {
                        match event {
                            Event::Key(key) => {
                                let should_restart = self.handle_key_event(key, &mut terminal, &events);
                                if should_restart {
                                    break;
                                }
                            }
                            Event::Tick => {
                                self.handle_animation_tick();

                                // Auto-clear errors after 5 seconds
                                if self.should_clear_error() {
                                    self.clear_error();
                                }

                                // Handle pending conflict resolution actions
                                if let Some(action) = self.pending_key.take() {
                                    match action {
                                        'o' => {
                                            // Pause file watcher to avoid conflict detection for our own save
                                            if let Some(ref watcher) = self.file_watcher {
                                                watcher.pause();
                                            }
                                            let snapshot = crate::state::DataSnapshot::from_app(self);
                                            if let Err(e) = self.state_manager.force_overwrite(&snapshot).await {
                                                tracing::error!("Failed to force overwrite: {}", e);
                                            }
                                            // Resume file watcher after save completes
                                            if let Some(ref watcher) = self.file_watcher {
                                                watcher.resume();
                                            }
                                        }
                                        't' => {
                                            // Reload from disk
                                            if let Some(store) = self.state_manager.store() {
                                                match store.load().await {
                                                    Ok((snapshot, _metadata)) => {
                                                        match serde_json::from_slice::<crate::state::DataSnapshot>(&snapshot.data) {
                                                            Ok(data) => {
                                                                data.apply_to_app(self);
                                                                self.state_manager.clear_conflict();
                                                                self.refresh_view();
                                                                tracing::info!("Reloaded state from disk");
                                                            }
                                                            Err(e) => {
                                                                tracing::error!("Failed to deserialize reloaded state: {}", e);
                                                            }
                                                        }
                                                    }
                                                    Err(e) => {
                                                        tracing::error!("Failed to reload from disk: {}", e);
                                                    }
                                                }
                                            }
                                        }
                                        _ => {}
                                    }
                                }

                                // Handle pending external change reload
                                if let Some(action) = self.pending_key.take() {
                                    if action == 'r' {
                                        self.auto_reload_from_external_change().await;
                                    }
                                }

                                // Check if help menu pending action should execute
                                if let Some((start_time, action)) = &self.help_pending_action {
                                    if start_time.elapsed().as_millis() >= 100 {
                                        if let AppMode::Help(previous_mode) = &self.mode {
                                            self.mode = (**previous_mode).clone();
                                        } else {
                                            self.mode = AppMode::Normal;
                                        }
                                        self.help_selection.clear();

                                        let action = *action;
                                        self.help_pending_action = None;
                                        self.execute_action(&action);
                                    }
                                }

                                // Auto-refresh view if state manager indicates it's needed
                                if self.state_manager.needs_refresh() {
                                    self.refresh_view();
                                    self.state_manager.clear_refresh();
                                }
                            }
                        }
                    }
                    _ = async {
                        if let Some(ref mut rx) = &mut self.save_completion_rx {
                            rx.recv().await
                        } else {
                            std::future::pending().await
                        }
                    } => {
                        // Save operation completed - update dirty flag
                        tracing::debug!("Save completion signal received");
                        self.state_manager.save_completed();
                        // Reset force quit flag if all saves are now complete
                        if !self.state_manager.has_pending_saves() {
                            self.quit_with_pending = false;
                        }
                    }
                    Some(_change_event) = async {
                        if let Some(ref mut rx) = &mut self.file_change_rx {
                            match rx.recv().await {
                                Ok(event) => {
                                    tracing::debug!(
                                        "File change event received at {}",
                                        event.detected_at
                                    );
                                    Some(event)
                                }
                                Err(tokio::sync::broadcast::error::RecvError::Lagged(count)) => {
                                    tracing::warn!(
                                        "File watcher events lagged: {} events dropped",
                                        count
                                    );
                                    None
                                }
                                Err(e) => {
                                    tracing::error!("File change receiver error: {}", e);
                                    None
                                }
                            }
                        } else {
                            std::future::pending().await
                        }
                    } => {
                        // External file change detected - handle smart reload
                        // (File watcher is paused during our own saves, so this is always external)
                        if !self.state_manager.is_dirty() {
                            // No local changes, auto-reload silently
                            tracing::info!("External change detected, auto-reloading");
                            self.auto_reload_from_external_change().await;
                            tracing::info!("Auto-reloaded due to external file change");
                        } else if self.mode != AppMode::ConflictResolution
                            && self.mode != AppMode::ExternalChangeDetected
                        {
                            // Local changes exist, prompt user
                            tracing::warn!("External file change detected with local changes");
                            self.mode = AppMode::ExternalChangeDetected;
                        }
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

        // Graceful shutdown: ensure all queued saves complete before exit
        self.state_manager.close_save_channel(); // Close save_tx channel to signal worker to finish

        // Wait for save worker to finish processing all queued saves
        if let Some(handle) = self.save_worker_handle.take() {
            handle.await.ok();
            tracing::info!("Save worker finished, all saves complete");
        }

        restore_terminal(&mut terminal)?;
        Ok(())
    }

    pub fn import_board_from_file(&mut self, filename: &str) -> io::Result<()> {
        let first_new_index = self.boards.len();
        let import = BoardImporter::import_from_file(filename)?;
        let (boards, columns, cards, deleted_cards, sprints) =
            BoardImporter::extract_entities(import);

        self.boards.extend(boards);
        self.columns.extend(columns);
        self.cards.extend(cards);
        self.archived_cards.extend(deleted_cards);
        self.sprints.extend(sprints);

        self.board_selection.set(Some(first_new_index));

        self.switch_view_strategy(kanban_domain::TaskListView::GroupedByColumn);

        Ok(())
    }

    async fn auto_reload_from_external_change(&mut self) {
        if let Some(store) = self.state_manager.store() {
            match store.load().await {
                Ok((snapshot, _metadata)) => {
                    match serde_json::from_slice::<crate::state::DataSnapshot>(&snapshot.data) {
                        Ok(data) => {
                            data.apply_to_app(self);
                            self.state_manager.mark_clean();
                            self.state_manager.clear_conflict();
                            self.mode = AppMode::Normal;
                            self.refresh_view();
                            tracing::info!("Auto-reloaded state from external file change");
                        }
                        Err(e) => {
                            tracing::error!("Failed to deserialize reloaded state: {}", e);
                        }
                    }
                }
                Err(e) => {
                    tracing::error!("Failed to reload from disk: {}", e);
                }
            }
        }
    }

    fn migrate_sprint_logs(&mut self) {
        let mut migrated_count = 0;

        for card in &mut self.cards {
            if let Some(sprint_id) = card.sprint_id {
                if card.sprint_logs.is_empty() {
                    if let Some(sprint) = self.sprints.iter().find(|s| s.id == sprint_id) {
                        let sprint_log = kanban_domain::SprintLog::new(
                            sprint_id,
                            sprint.sprint_number,
                            sprint.name_index.and_then(|idx| {
                                self.boards
                                    .iter()
                                    .find(|b| b.id == sprint.board_id)
                                    .and_then(|board| board.sprint_names.get(idx).cloned())
                            }),
                            format!("{:?}", sprint.status),
                        );
                        card.sprint_logs.push(sprint_log);
                        migrated_count += 1;
                    }
                }
            }
        }

        if migrated_count > 0 {
            tracing::info!("Migrated sprint logs for {} cards", migrated_count);
        }
    }

    /// Generic handler for copying card outputs to clipboard
    fn copy_card_output<F>(&mut self, output_type: &str, get_output: F)
    where
        F: Fn(&Card, &Board, &[Sprint], &str) -> String,
    {
        if let Some(card_idx) = self.active_card_index {
            if let Some(board_idx) = self.active_board_index {
                if let Some(board) = self.boards.get(board_idx) {
                    if let Some(card) = self.cards.get(card_idx) {
                        let output = get_output(
                            card,
                            board,
                            &self.sprints,
                            self.app_config.effective_default_card_prefix(),
                        );
                        if let Err(e) = clipboard::copy_to_clipboard(&output) {
                            tracing::error!("Failed to copy to clipboard: {}", e);
                        } else {
                            tracing::info!("Copied {}: {}", output_type, output);
                        }
                    }
                }
            }
        }
    }

    pub fn copy_branch_name(&mut self) {
        self.copy_card_output("branch name", |card, board, sprints, prefix| {
            card.branch_name(board, sprints, prefix)
        });
    }

    pub fn copy_git_checkout_command(&mut self) {
        self.copy_card_output("command", |card, board, sprints, prefix| {
            card.git_checkout_command(board, sprints, prefix)
        });
    }

    pub fn get_current_priority_selection_index(&self) -> usize {
        if let Some(card_idx) = self.active_card_index {
            if let Some(card) = self.cards.get(card_idx) {
                use kanban_domain::CardPriority;
                return match card.priority {
                    CardPriority::Low => 0,
                    CardPriority::Medium => 1,
                    CardPriority::High => 2,
                    CardPriority::Critical => 3,
                };
            }
        }
        0
    }

    pub fn get_current_sprint_selection_index(&self) -> usize {
        if let Some(card_idx) = self.active_card_index {
            if let Some(card) = self.cards.get(card_idx) {
                if let Some(card_sprint_id) = card.sprint_id {
                    if let Some(board_idx) = self.active_board_index {
                        if let Some(board) = self.boards.get(board_idx) {
                            let board_sprints: Vec<_> = self
                                .sprints
                                .iter()
                                .filter(|s| s.board_id == board.id)
                                .collect();
                            for (idx, sprint) in board_sprints.iter().enumerate() {
                                if sprint.id == card_sprint_id {
                                    return idx + 1;
                                }
                            }
                        }
                    }
                }
            }
        }
        0
    }

    pub fn get_current_sort_field_selection_index(&self) -> usize {
        if let Some(sort_field) = self.current_sort_field {
            return match sort_field {
                SortField::Points => 0,
                SortField::Priority => 1,
                SortField::CreatedAt => 2,
                SortField::UpdatedAt => 3,
                SortField::Status => 4,
                SortField::Default => 5,
            };
        }
        0
    }
}

fn setup_terminal() -> Result<Terminal<CrosstermBackend<io::Stdout>>, io::Error> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    Terminal::new(backend)
}

fn restore_terminal(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
) -> Result<(), io::Error> {
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}

impl Default for App {
    fn default() -> Self {
        let (app, _rx) = Self::new(None);
        app
    }
}
