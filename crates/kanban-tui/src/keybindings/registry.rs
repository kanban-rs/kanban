use super::{
    board_detail::BoardDetailProvider,
    card_detail::CardDetailProvider,
    card_list::CardListProvider,
    dialog_modes::{
        ConfirmSprintPrefixCollisionProvider, ConflictResolutionProvider, DeleteConfirmProvider,
        DialogInputProvider, DialogSelectionProvider, ErrorLogProvider,
        ExternalChangeDetectedProvider, FilterOptionsProvider, SearchModeProvider,
    },
    normal_mode::{
        ArchivedBoardsViewProvider, ArchivedCardsViewProvider, NormalModeBoardsProvider,
    },
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
            app.selection.active_board_id.is_some(),
        )
    }

    fn get_provider_for_mode(
        mode: &AppMode,
        focus: Focus,
        card_focus: crate::app::CardFocus,
        board_focus: crate::app::BoardFocus,
        settings_focus: SettingsFocus,
        board_activated: bool,
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
            // Once an archived board is ACTIVATED (drilled into), the tasks panel
            // is the context: advertise the card-list keys that actually work
            // there (Enter detail, e edit, p priority, H/L move) rather than the
            // board-list keys. Only while browsing the board list does the
            // archived-boards provider apply.
            AppMode::ArchivedBoardsView if board_activated => Box::new(CardListProvider),
            AppMode::ArchivedBoardsView => Box::new(ArchivedBoardsViewProvider),
            AppMode::Settings => Box::new(SettingsViewProvider::new(settings_focus)),
            AppMode::Help(previous_mode) => Self::get_provider_for_mode(
                previous_mode,
                focus,
                card_focus,
                board_focus,
                settings_focus,
                board_activated,
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
                DialogMode::OrderBoards => Box::new(DialogSelectionProvider::new("Sort Projects")),
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
                    Box::new(ConfirmSprintPrefixCollisionProvider)
                }
                DialogMode::FilterOptions => Box::new(FilterOptionsProvider),
                DialogMode::ConflictResolution => Box::new(ConflictResolutionProvider),
                DialogMode::ExternalChangeDetected => Box::new(ExternalChangeDetectedProvider),
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
                DialogMode::DeletePermanentBoardConfirm => {
                    Box::new(DeleteConfirmProvider::new("Project (Permanent)"))
                }
            },
            AppMode::ErrorLog => Box::new(ErrorLogProvider),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::App;

    #[test]
    fn test_registry_selects_bespoke_provider_for_confirm_sprint_prefix_collision() {
        let mut app = App::test_default();
        app.mode = AppMode::Dialog(DialogMode::ConfirmSprintPrefixCollision);

        let context = KeybindingRegistry::get_provider(&app).get_context();

        assert!(
            context.bindings.iter().any(|b| b.key == "y"),
            "must select the bespoke provider (advertises 'y'), not the generic list-picker provider"
        );
    }

    #[test]
    fn test_registry_selects_bespoke_provider_for_conflict_resolution() {
        let mut app = App::test_default();
        app.mode = AppMode::Dialog(DialogMode::ConflictResolution);

        let context = KeybindingRegistry::get_provider(&app).get_context();

        assert!(
            context.bindings.iter().any(|b| b.key == "o"),
            "must select the bespoke provider (advertises 'o'), not the generic list-picker provider"
        );
    }

    #[test]
    fn test_registry_selects_bespoke_provider_for_external_change_detected() {
        let mut app = App::test_default();
        app.mode = AppMode::Dialog(DialogMode::ExternalChangeDetected);

        let context = KeybindingRegistry::get_provider(&app).get_context();

        assert!(
            context.bindings.iter().any(|b| b.key == "r"),
            "must select the bespoke provider (advertises 'r'), not the generic list-picker provider"
        );
    }
}
