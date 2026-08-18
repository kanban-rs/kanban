mod helpers;

use helpers::{assert_ops, CountingBackend};
use kanban_domain::KanbanOperations;
use kanban_tui::app::mode::AppMode;
use kanban_tui::keybindings::{KeybindingAction, KeybindingRegistry};
use kanban_tui::App;
use std::collections::HashSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Purity {
    Pure,
    Mutating,
}

fn classify(action: &KeybindingAction) -> Purity {
    use KeybindingAction::*;
    match action {
        NavigateDown | NavigateUp | NavigateLeft | NavigateRight | SelectItem | Escape
        | FocusPanel(_) | JumpToTop | JumpToBottom | JumpHalfViewportUp | JumpHalfViewportDown
        | ToggleArchivedView | ToggleArchivedBoardsView | ToggleCardSelection
        | ClearCardSelection | SelectAllCards | ShowHelp | EditCard | Search => Purity::Pure,

        CreateCard | CreateBoard | CreateSprint | CreateColumn | RenameBoard | RenameColumn
        | SetColumnDefaultStatus | EditBoard | ToggleCompletion | AssignToSprint | ArchiveCard
        | RestoreCard | DeleteCard | MoveCardLeft | MoveCardRight | MoveColumnUp
        | MoveColumnDown | DeleteColumn | DeleteBoard | ExportBoard | ExportAll | ImportBoard
        | OrderCards | OrderBoards | ToggleSortOrder | ToggleFilter | ToggleHideAssigned
        | RestoreBoard | DeleteArchivedBoard | ToggleBoardsSortOrder | ToggleTaskListView
        | SetCardPriority | SetSelectedCardsPriority | ManageParents | ManageChildren
        | CarryOver | Undo | Redo | OpenSettings | ExportBoards | CopyBranchName
        | CopyGitCheckoutCommand | ConfirmPrefixCollision | RejectPrefixCollision
        | CancelPrefixCollision | ForceOverwriteConflict | TakeTheirsConflict
        | CancelConflictResolution | ReloadDiscardLocal | KeepLocalChanges
        | DismissExternalChange => Purity::Mutating,
    }
}

fn seeded_app() -> App {
    let mut app = App::test_default();
    let board = app.ctx.create_board("Board".to_string(), None).unwrap();
    let column = app
        .ctx
        .create_column(board.id, "Todo".to_string(), Some(0))
        .unwrap();
    for i in 0..3 {
        app.ctx
            .create_card(
                board.id,
                column.id,
                format!("Card {i}"),
                kanban_domain::CreateCardOptions::default(),
            )
            .unwrap();
    }
    app.reload_model();
    app.prepare_frame();
    app.selection.active_board_id = Some(board.id);
    app
}

fn advertised_actions_for(app: &App) -> Vec<KeybindingAction> {
    KeybindingRegistry::get_provider(app)
        .get_context()
        .bindings
        .into_iter()
        .map(|b| b.action)
        .collect()
}

fn advertised_actions() -> Vec<(&'static str, Vec<KeybindingAction>)> {
    let mut normal_boards = seeded_app();
    normal_boards.mode = AppMode::Normal;
    normal_boards.focus.active = kanban_tui::app::focus::Focus::Boards;

    let mut normal_cards = seeded_app();
    normal_cards.mode = AppMode::Normal;
    normal_cards.focus.active = kanban_tui::app::focus::Focus::Cards;

    let mut archived_boards = seeded_app();
    archived_boards.mode = AppMode::ArchivedBoardsView;
    archived_boards.focus.active = kanban_tui::app::focus::Focus::Boards;

    let mut archived_cards = seeded_app();
    archived_cards.mode = AppMode::ArchivedCardsView;

    vec![
        ("Normal/Boards", advertised_actions_for(&normal_boards)),
        ("Normal/Cards", advertised_actions_for(&normal_cards)),
        (
            "ArchivedBoardsView",
            advertised_actions_for(&archived_boards),
        ),
        ("ArchivedCardsView", advertised_actions_for(&archived_cards)),
    ]
}

#[test]
fn test_every_advertised_keybinding_action_is_classified() {
    let modes = advertised_actions();
    for (mode, actions) in &modes {
        assert!(
            !actions.is_empty(),
            "mode {mode} advertised zero keybindings; a provider regression can silently hide this"
        );
        for action in actions {
            let _ = classify(action);
        }
    }
}

#[test]
fn test_every_advertised_navigation_keybinding_performs_no_store_reads() {
    let modes = advertised_actions();
    let mut seen: HashSet<&'static str> = HashSet::new();
    for (mode, actions) in &modes {
        for action in actions {
            if classify(action) != Purity::Pure {
                continue;
            }
            let key = match action {
                KeybindingAction::FocusPanel(n) => {
                    Box::leak(format!("FocusPanel({n})").into_boxed_str()) as &str
                }
                other => Box::leak(format!("{other:?}").into_boxed_str()) as &str,
            };
            if !seen.insert(key) {
                continue;
            }

            let mut app = seeded_app();
            match *mode {
                "Normal/Cards" => app.focus.active = kanban_tui::app::focus::Focus::Cards,
                "ArchivedBoardsView" => app.mode = AppMode::ArchivedBoardsView,
                "ArchivedCardsView" => app.mode = AppMode::ArchivedCardsView,
                _ => {}
            }

            let (backend, _reads, ops) = CountingBackend::wrap(app.ctx.backend());
            app.ctx.replace_backend(backend);

            app.execute_action(action);

            assert_ops(
                &ops,
                &[],
            );
        }
    }
}
