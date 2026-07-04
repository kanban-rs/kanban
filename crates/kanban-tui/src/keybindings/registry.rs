use super::{
    board_detail::BoardDetailProvider,
    card_detail::CardDetailProvider,
    card_list::CardListProvider,
    dialog_modes::{
        DeleteConfirmProvider, DialogInputProvider, DialogSelectionProvider, ErrorLogProvider,
        FilterOptionsProvider, SearchModeProvider,
    },
    normal_mode::{ArchivedCardsViewProvider, NormalModeBoardsProvider},
    settings::SettingsViewProvider,
    sprint_detail::SprintDetailProvider,
    KeybindingProvider,
};
use crate::app::{App, AppMode, DialogMode, Focus, SettingsFocus};

pub struct KeybindingRegistry;

impl KeybindingRegistry {
    pub fn get_provider(app: &App) -> Box<dyn KeybindingProvider> {
        Self::get_provider_for_mode(
            &app.mode,
            app.focus.active.clone(),
            app.focus.card_focus,
            app.focus.board_focus,
            app.focus.settings_focus,
        )
    }

    fn get_provider_for_mode(
        mode: &AppMode,
        focus: Focus,
        card_focus: crate::app::CardFocus,
        board_focus: crate::app::BoardFocus,
        settings_focus: SettingsFocus,
    ) -> Box<dyn KeybindingProvider> {
        match mode {
            AppMode::Normal => match focus {
                Focus::Cards => Box::new(CardListProvider),
                Focus::Boards => Box::new(NormalModeBoardsProvider),
            },
            AppMode::CardDetail => Box::new(CardDetailProvider::new(card_focus)),
            AppMode::BoardDetail => Box::new(BoardDetailProvider::new(board_focus)),
            AppMode::SprintDetail => Box::new(SprintDetailProvider),
            AppMode::Search => Box::new(SearchModeProvider),
            AppMode::ArchivedCardsView => Box::new(ArchivedCardsViewProvider),
            AppMode::Settings => Box::new(SettingsViewProvider::new(settings_focus)),
            AppMode::Help(previous_mode) => Self::get_provider_for_mode(
                previous_mode,
                focus,
                card_focus,
                board_focus,
                settings_focus,
            ),
            AppMode::Dialog(dialog) => match dialog {
                DialogMode::CreateBoard => Box::new(DialogInputProvider::new("Create Project")),
                DialogMode::CreateCard => Box::new(DialogInputProvider::new("Create Task")),
                DialogMode::CreateSprint => Box::new(DialogInputProvider::new("Create Sprint")),
                DialogMode::RenameBoard => Box::new(DialogInputProvider::new("Rename Project")),
                DialogMode::RenameColumn => Box::new(DialogInputProvider::new("Rename Column")),
                DialogMode::CreateColumn => Box::new(DialogInputProvider::new("Create Column")),
                DialogMode::ExportBoard => Box::new(DialogInputProvider::new("Export Project")),
                DialogMode::ExportAll => Box::new(DialogInputProvider::new("Export All Projects")),
                DialogMode::SetCardPoints => Box::new(DialogInputProvider::new("Set Points")),
                DialogMode::SetBranchPrefix => {
                    Box::new(DialogInputProvider::new("Set Branch Prefix"))
                }
                DialogMode::SetSprintPrefix => {
                    Box::new(DialogInputProvider::new("Set Sprint Prefix"))
                }
                DialogMode::SetSprintCardPrefix => {
                    Box::new(DialogInputProvider::new("Set Card Prefix"))
                }
                DialogMode::ImportBoard => Box::new(DialogSelectionProvider::new("Import Project")),
                DialogMode::SetCardPriority => {
                    Box::new(DialogSelectionProvider::new("Set Priority"))
                }
                DialogMode::SetMultipleCardsPriority => {
                    Box::new(DialogSelectionProvider::new("Set Priority (Bulk)"))
                }
                DialogMode::OrderCards => Box::new(DialogSelectionProvider::new("Sort Tasks")),
                DialogMode::AssignCardToSprint => {
                    Box::new(DialogSelectionProvider::new("Assign to Sprint"))
                }
                DialogMode::AssignMultipleCardsToSprint => {
                    Box::new(DialogSelectionProvider::new("Assign Cards to Sprint"))
                }
                DialogMode::SelectTaskListView => {
                    Box::new(DialogSelectionProvider::new("Select Task View"))
                }
                DialogMode::DeleteColumnConfirm => Box::new(DeleteConfirmProvider::new("Column")),
                DialogMode::DeleteBoardConfirm => Box::new(DeleteConfirmProvider::new("Project")),
                DialogMode::ConfirmSprintPrefixCollision => {
                    Box::new(DialogSelectionProvider::new("Confirm Action"))
                }
                DialogMode::FilterOptions => Box::new(FilterOptionsProvider),
                DialogMode::ConflictResolution => {
                    Box::new(DialogSelectionProvider::new("Resolve Conflict"))
                }
                DialogMode::ExternalChangeDetected => {
                    Box::new(DialogSelectionProvider::new("External Change"))
                }
                DialogMode::ManageParents => Box::new(DialogSelectionProvider::new("Set Parents")),
                DialogMode::ManageChildren => {
                    Box::new(DialogSelectionProvider::new("Set Children"))
                }
                DialogMode::CarryOverSprint => {
                    Box::new(DialogSelectionProvider::new("Carry Over to Sprint"))
                }
                DialogMode::ExportBoards => Box::new(DialogSelectionProvider::new("Export Boards")),
                DialogMode::ChooseStorageFile => {
                    Box::new(DialogInputProvider::new("Choose Storage File"))
                }
            },
            AppMode::ErrorLog => Box::new(ErrorLogProvider),
        }
    }
}
