use super::card_list::CardListProvider;
use super::{Keybinding, KeybindingAction, KeybindingContext, KeybindingProvider};

pub struct NormalModeBoardsProvider;

impl KeybindingProvider for NormalModeBoardsProvider {
    fn get_context(&self) -> KeybindingContext {
        KeybindingContext::new(
            "Normal Mode - Projects Panel",
            vec![
                Keybinding::new("?", "help", "Show help", KeybindingAction::ShowHelp),
                Keybinding::new("q", "quit", "Quit application", KeybindingAction::Escape),
                Keybinding::new(
                    "1",
                    "panel 1",
                    "Focus projects panel",
                    KeybindingAction::FocusPanel(0),
                ),
                Keybinding::new(
                    "2",
                    "panel 2",
                    "Focus tasks panel",
                    KeybindingAction::FocusPanel(1),
                ),
                Keybinding::new(
                    "n",
                    "new",
                    "Create new project",
                    KeybindingAction::CreateBoard,
                ),
                Keybinding::new(
                    "r",
                    "rename",
                    "Rename selected project",
                    KeybindingAction::RenameBoard,
                ),
                Keybinding::new(
                    "e",
                    "edit",
                    "Edit selected project",
                    KeybindingAction::EditBoard,
                ),
                Keybinding::new(
                    "x",
                    "export",
                    "Export selected project",
                    KeybindingAction::ExportBoard,
                ),
                Keybinding::new(
                    "X",
                    "export all",
                    "Export all projects",
                    KeybindingAction::ExportAll,
                ),
                Keybinding::new(
                    "i",
                    "import",
                    "Import project from file",
                    KeybindingAction::ImportBoard,
                ),
                Keybinding::new(
                    "d",
                    "archive",
                    "Archive selected project",
                    KeybindingAction::DeleteBoard,
                ),
                Keybinding::new(
                    "D",
                    "archived",
                    "View archived projects",
                    KeybindingAction::ToggleArchivedBoardsView,
                ),
                Keybinding::new(
                    "j/↓",
                    "down",
                    "Navigate down",
                    KeybindingAction::NavigateDown,
                ),
                Keybinding::new("k/↑", "up", "Navigate up", KeybindingAction::NavigateUp),
                Keybinding::new("gg", "top", "Jump to top", KeybindingAction::JumpToTop),
                Keybinding::new(
                    "G",
                    "bottom",
                    "Jump to bottom",
                    KeybindingAction::JumpToBottom,
                ),
                Keybinding::new(
                    "Enter/Space",
                    "detail",
                    "View project detail",
                    KeybindingAction::SelectItem,
                ),
                Keybinding::new("u", "undo", "Undo last action", KeybindingAction::Undo),
                Keybinding::new(
                    "U",
                    "redo",
                    "Redo last undone action",
                    KeybindingAction::Redo,
                ),
                Keybinding::new(
                    "S",
                    "settings",
                    "Open settings view",
                    KeybindingAction::OpenSettings,
                ),
            ],
        )
    }
}

pub struct ArchivedCardsViewProvider;

impl KeybindingProvider for ArchivedCardsViewProvider {
    /// The archived-cards view is the ordinary card panel showing a different
    /// SET, so it DELEGATES to `CardListProvider` and inherits the full card-list
    /// bindings (LSP) — an archived card is operated exactly like a live one. It
    /// then adjusts only where behaviour differs:
    /// - drops create (`n`): an archived list is not where new cards are created
    ///   (would make an invisible live card — #414 finding 1);
    /// - drops the live `q` (quit) and `Esc` (clear selection) bindings, whose
    ///   keys instead toggle back to the live set here, and re-advertises them as
    ///   the toggle (#414 finding 3);
    /// - appends the archived extension: `r` restore, `x` permanent-delete.
    fn get_context(&self) -> KeybindingContext {
        let mut bindings = CardListProvider.get_context().bindings;

        // Reused keys whose behaviour differs in the archived view: create is not
        // offered; `q`/`Esc` toggle back rather than quit/clear.
        bindings.retain(|b| b.key != "n" && b.key != "q" && b.key != "Esc");

        // Archived extension keys.
        bindings.push(Keybinding::new(
            "r",
            "restore",
            "Restore selected task(s)",
            KeybindingAction::RestoreCard,
        ));
        bindings.push(Keybinding::new(
            "x",
            "delete",
            "Permanently delete selected task(s)",
            KeybindingAction::DeleteCard,
        ));
        // Reconciled toggle-back text (not "Quit" / "clear selection").
        bindings.push(Keybinding::new(
            "q/Esc",
            "back",
            "Back to live tasks view",
            KeybindingAction::Escape,
        ));

        KeybindingContext::new("Archived Cards View", bindings)
    }
}

pub struct ArchivedBoardsViewProvider;

impl ArchivedBoardsViewProvider {
    /// The live-panel actions the archived view REUSES verbatim (same key, same
    /// handler, delegated through `handle_shared_boards_key`). The archived
    /// provider is derived from `NormalModeBoardsProvider` by keeping exactly
    /// these bindings and appending the extension/toggle keys, so navigation,
    /// drill-in, settings and undo/redo can never drift from the live panel.
    /// Live-only operations (create/rename/edit/export/import, the `d` archive,
    /// the `D` toggle) are intentionally NOT reused: they are no-ops or
    /// misbehave against the archived set, so they are curated out here.
    const REUSED_ACTIONS: &'static [KeybindingAction] = &[
        KeybindingAction::ShowHelp,
        KeybindingAction::NavigateDown,
        KeybindingAction::NavigateUp,
        KeybindingAction::JumpToTop,
        KeybindingAction::JumpToBottom,
        KeybindingAction::SelectItem,
        KeybindingAction::Undo,
        KeybindingAction::Redo,
        KeybindingAction::OpenSettings,
    ];
}

impl KeybindingProvider for ArchivedBoardsViewProvider {
    fn get_context(&self) -> KeybindingContext {
        // Delegate to the live projects provider and keep only the shared
        // bindings, then append the archived-view extension (restore /
        // permanent-delete) and the toggle-back binding. This mirrors the card
        // side: the archived view IS the ordinary panel on a different set plus a
        // small extension, not a hand-maintained parallel list.
        let live = NormalModeBoardsProvider.get_context();
        let mut bindings: Vec<Keybinding> = live
            .bindings
            .into_iter()
            .filter(|b| Self::REUSED_ACTIONS.contains(&b.action))
            .collect();

        bindings.extend([
            Keybinding::new(
                "r",
                "restore",
                "Restore selected project",
                KeybindingAction::RestoreBoard,
            ),
            Keybinding::new(
                "x",
                "delete",
                "Permanently delete selected project",
                KeybindingAction::DeleteArchivedBoard,
            ),
            // Reuse the shared SortOrder toggle to flip the projects-panel sort
            // order (the unified board sort, persisted to config).
            Keybinding::new(
                "s",
                "sort order",
                "Toggle project sort order",
                KeybindingAction::ToggleArchivedBoardsSortOrder,
            ),
            // Open the board-sort field picker (Position / Name / Date Created /
            // Recency), the board-side analogue of the card `o` picker.
            Keybinding::new(
                "o",
                "sort field",
                "Choose project sort field",
                KeybindingAction::OrderBoards,
            ),
            // Reused binding whose archived-view behavior DIFFERS from the live
            // panel's `q` (quit): here it toggles back to the live projects list.
            // The help text describes the actual behavior.
            Keybinding::new(
                "q/Esc",
                "back",
                "Back to projects view",
                KeybindingAction::Escape,
            ),
        ]);

        KeybindingContext::new("Archived Projects View", bindings)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_boards_provider_binds_d_to_remove_and_shift_d_to_archived_view() {
        // Mirrors the card removal flow where `d` is the primary removal key
        // (now archive) and `D` toggles the archived-boards view.
        let ctx = NormalModeBoardsProvider.get_context();
        let matches: Vec<_> = ctx.bindings.iter().filter(|b| b.key == "d").collect();
        assert_eq!(
            matches.len(),
            1,
            "exactly one 'd' binding on the boards panel"
        );
        assert_eq!(matches[0].action, KeybindingAction::DeleteBoard);
        let shift_d: Vec<_> = ctx.bindings.iter().filter(|b| b.key == "D").collect();
        assert_eq!(
            shift_d.len(),
            1,
            "exactly one 'D' binding on the boards panel"
        );
        assert_eq!(
            shift_d[0].action,
            KeybindingAction::ToggleArchivedBoardsView,
            "'D' toggles the archived-boards view"
        );
    }

    #[test]
    fn test_archived_boards_view_provider_binds_restore_and_delete() {
        let ctx = ArchivedBoardsViewProvider.get_context();
        assert!(ctx
            .bindings
            .iter()
            .any(|b| b.key == "r" && b.action == KeybindingAction::RestoreBoard));
        assert!(ctx
            .bindings
            .iter()
            .any(|b| b.key == "x" && b.action == KeybindingAction::DeleteArchivedBoard));
    }

    #[test]
    fn test_archived_boards_view_provider_binds_sort_order_toggle() {
        let ctx = ArchivedBoardsViewProvider.get_context();
        assert!(
            ctx.bindings.iter().any(
                |b| b.key == "s" && b.action == KeybindingAction::ToggleArchivedBoardsSortOrder
            ),
            "'s' toggles the archived-boards sort order via the shared SortOrder toggle"
        );
    }

    /// The archived-boards view is the ordinary projects panel showing a
    /// different SET: it reuses the shared navigation/activation bindings that the
    /// dispatch delegates to `handle_shared_boards_key`, rather than hand-rolling a
    /// divergent list that can drift from the live panel.
    #[test]
    fn test_archived_boards_provider_reuses_shared_navigation_bindings() {
        let ctx = ArchivedBoardsViewProvider.get_context();
        let has = |action: KeybindingAction| ctx.bindings.iter().any(|b| b.action == action);
        assert!(has(KeybindingAction::NavigateDown), "j/↓ navigate down");
        assert!(has(KeybindingAction::NavigateUp), "k/↑ navigate up");
        assert!(has(KeybindingAction::JumpToTop), "gg jump to top");
        assert!(has(KeybindingAction::JumpToBottom), "G jump to bottom");
        assert!(has(KeybindingAction::Undo), "u undo");
        assert!(has(KeybindingAction::Redo), "U redo");
    }

    /// Drilling into an archived board is the SAME activation the live panel uses
    /// (Enter/Space → `handle_selection_activate`). It must be advertised so help
    /// describes the real behavior, not a truncated one.
    #[test]
    fn test_archived_boards_provider_advertises_drill_in_activation() {
        let ctx = ArchivedBoardsViewProvider.get_context();
        assert!(
            ctx.bindings
                .iter()
                .any(|b| b.key == "Enter/Space" && b.action == KeybindingAction::SelectItem),
            "archived view must advertise Enter/Space drill-in (shared activation)"
        );
    }

    /// Live-only board operations that the archived dispatch does NOT handle must
    /// be EXCLUDED from the provider, so help never advertises a binding that is a
    /// no-op (or misbehaves) in the archived view. This is the curation half of
    /// the LSP contract: the archived view substitutes for the live panel only on
    /// the bindings that actually apply.
    #[test]
    fn test_archived_boards_provider_excludes_live_only_operations() {
        let ctx = ArchivedBoardsViewProvider.get_context();
        for action in [
            KeybindingAction::CreateBoard,
            KeybindingAction::DeleteBoard,
            KeybindingAction::EditBoard,
            KeybindingAction::ExportBoard,
            KeybindingAction::ExportAll,
            KeybindingAction::ImportBoard,
            KeybindingAction::ToggleArchivedBoardsView,
        ] {
            assert!(
                !ctx.bindings.iter().any(|b| b.action == action),
                "live-only action {action:?} must not be advertised in the archived view"
            );
        }
    }

    /// In the archived view `q`/`Esc` do NOT quit the app: they toggle back to the
    /// live projects list. The help text for the reused binding must describe that
    /// actual behavior, not the live panel's "quit".
    #[test]
    fn test_archived_board_help_describes_toggle() {
        let ctx = ArchivedBoardsViewProvider.get_context();
        let back = ctx
            .bindings
            .iter()
            .find(|b| b.key == "q/Esc")
            .expect("archived view binds q/Esc");
        assert_eq!(back.action, KeybindingAction::Escape);
        let desc = back.description.to_lowercase();
        assert!(
            desc.contains("projects") && !desc.contains("quit"),
            "q/Esc help must describe toggling back to projects, not quitting; got {:?}",
            back.description
        );
    }

    #[test]
    fn test_delete_board_action_is_distinct_from_delete_column() {
        assert_ne!(
            KeybindingAction::DeleteBoard,
            KeybindingAction::DeleteColumn
        );
    }
}
