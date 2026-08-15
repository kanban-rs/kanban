//! KAN-934: the archived-CARD view is the ordinary card panel fed the archived
//! SET + archived decoration + the restore/delete extension, through the SAME
//! shared handlers a live card uses.
//!
//! These tests drive an ARCHIVED card, from the archived-cards view, through the
//! exact shared card handlers/dispatch a live card uses — detail, priority,
//! sprint-assign, move — and assert the behaviour is identical, plus the archived
//! extension (restore / permanent-delete). They also close the #414 review
//! findings: `n` (create) is not offered from the archived view, the help text
//! describes the toggle-back for `q`/`Esc`, and the tasks-panel title keys off
//! the stack-aware base mode (correct under a modal underlay).

use crossterm::event::KeyCode;
use kanban_domain::{CardPriority, CreateCardOptions, KanbanOperations};
use kanban_tui::app::focus::Focus;
use kanban_tui::app::mode::AppMode;
use kanban_tui::keybindings::card_list::CardListProvider;
use kanban_tui::keybindings::normal_mode::ArchivedCardsViewProvider;
use kanban_tui::keybindings::KeybindingProvider;
use kanban_tui::App;

/// Seed a board with two columns and a card, archive the card, then enter the
/// archived-cards view with that card selected. Returns (board_id, col1, col2,
/// card_id).
fn seed_archived_card(app: &mut App) -> (uuid::Uuid, uuid::Uuid, uuid::Uuid, uuid::Uuid) {
    let board = app.ctx.create_board("Board".to_string(), None).unwrap();
    let col1 = app
        .ctx
        .create_column(board.id, "Todo".to_string(), None)
        .unwrap();
    let col2 = app
        .ctx
        .create_column(board.id, "Doing".to_string(), None)
        .unwrap();
    let card = app
        .ctx
        .create_card(
            board.id,
            col1.id,
            "ArchiveMe".to_string(),
            CreateCardOptions::default(),
        )
        .unwrap();
    app.ctx.archive_card(card.id).unwrap();

    app.selection.active_board_id = Some(board.id);
    app.mode = AppMode::ArchivedCardsView;
    app.focus.active = Focus::Cards;
    app.reload_model();
    app.prepare_frame();
    if let Some(list) = app.view.strategy.get_active_task_list_mut() {
        list.set_selected_index(Some(0));
    }
    (board.id, col1.id, col2.id, card.id)
}

/// Enter (detail) from the archived list must open CardDetail on the archived
/// card via the SHARED activation handler — not be swallowed by the archived
/// dispatch.
#[test]
fn test_archived_card_detail_via_shared_handler_opens_detail() {
    let mut app = App::test_default();
    let (_, _, _, card_id) = seed_archived_card(&mut app);

    app.handle_archived_cards_view_mode(KeyCode::Enter);

    assert_eq!(
        app.mode,
        AppMode::CardDetail,
        "Enter from the archived list opens card detail via the shared handler"
    );
    assert_eq!(
        app.selection.active_card_id,
        Some(card_id),
        "detail resolves the archived card by id"
    );
}

/// `p` (set priority) from the archived list opens the priority dialog on the
/// archived card and, driven to Enter, changes the card's real state.
#[test]
fn test_archived_card_priority_via_shared_handler_changes_state() {
    let mut app = App::test_default();
    let (_, _, _, card_id) = seed_archived_card(&mut app);

    // Baseline priority.
    let before = app.model.card_by_id(card_id).unwrap().priority;
    assert_ne!(before, CardPriority::Critical, "precondition: not Critical");

    // `p` opens the priority dialog targeting the archived card.
    app.handle_archived_cards_view_mode(KeyCode::Char('p'));
    assert_eq!(
        app.mode,
        AppMode::Dialog(kanban_tui::app::DialogMode::SetCardPriority),
        "`p` opens the shared priority dialog from the archived view"
    );

    // Select "Critical" (index 3) and apply.
    app.dialog_input.priority_selection.set(Some(3));
    app.handle_set_card_priority_popup(KeyCode::Enter);
    app.reload_model();
    app.prepare_frame();

    assert_eq!(
        app.model.card_by_id(card_id).unwrap().priority,
        CardPriority::Critical,
        "priority change on an archived card takes real effect via the shared handler"
    );
}

/// `a` (assign-to-sprint) from the archived list opens the shared sprint-assign
/// dialog targeting the archived card.
#[test]
fn test_archived_card_sprint_assign_via_shared_handler_opens_dialog() {
    let mut app = App::test_default();
    let (board_id, _, _, card_id) = seed_archived_card(&mut app);
    // A sprint must exist for the assign dialog to open.
    app.ctx.create_sprint(board_id, None, None).unwrap();
    app.reload_model();
    app.prepare_frame();
    if let Some(list) = app.view.strategy.get_active_task_list_mut() {
        list.set_selected_index(Some(0));
    }

    app.handle_archived_cards_view_mode(KeyCode::Char('a'));

    assert_eq!(
        app.mode,
        AppMode::Dialog(kanban_tui::app::DialogMode::AssignCardToSprint),
        "`a` opens the shared sprint-assign dialog from the archived view"
    );
    assert_eq!(
        app.selection.active_card_id,
        Some(card_id),
        "sprint-assign targets the archived card"
    );
}

/// Edit/detail resolve the archived card via the shared active-card resolver —
/// proving those operations are archival-agnostic. Enter activates the archived
/// card, then the shared detail-view resolver returns it (the same resolver the
/// edit handler reads).
#[test]
fn test_archived_card_detail_resolver_finds_archived_card() {
    let mut app = App::test_default();
    let (_, _, _, card_id) = seed_archived_card(&mut app);

    app.handle_archived_cards_view_mode(KeyCode::Enter);

    assert_eq!(
        app.selection.active_card_id,
        Some(card_id),
        "the shared active-card resolver finds the archived card"
    );
    let card = app
        .get_card_for_detail_view()
        .expect("archived card resolves for detail/edit");
    assert_eq!(card.title, "ArchiveMe");
}

/// Move (`L`) from the archived list moves the archived card to the next column
/// coherently (C1's coherent service semantics), via the shared move handler.
#[test]
fn test_move_in_archived_view_matches_c1() {
    let mut app = App::test_default();
    let (_, col1, col2, card_id) = seed_archived_card(&mut app);

    assert_eq!(
        app.model.card_by_id(card_id).unwrap().column_id,
        col1,
        "precondition: archived card starts in col1"
    );

    app.handle_archived_cards_view_mode(KeyCode::Char('L'));
    app.reload_model();
    app.prepare_frame();

    assert_eq!(
        app.model.card_by_id(card_id).unwrap().column_id,
        col2,
        "moving right from the archived view lands the card in col2 (coherent), \
         not a wrong/live-only-computed column"
    );
}

/// #414 finding 1: `n` (create card) must NOT create an invisible live card
/// from the archived view. It is excluded — no dialog opens, no card is created.
#[test]
fn test_create_not_offered_in_archived_view() {
    let mut app = App::test_default();
    let (_, _, _, _) = seed_archived_card(&mut app);
    let live_before = app.model.all_cards().len();

    app.handle_archived_cards_view_mode(KeyCode::Char('n'));

    assert_eq!(
        app.mode,
        AppMode::ArchivedCardsView,
        "`n` must not open the create-card dialog from the archived view"
    );
    app.reload_model();
    app.prepare_frame();
    assert_eq!(
        app.model.all_cards().len(),
        live_before,
        "`n` must not create an invisible live card from the archived view"
    );
}

/// `r` (restore) extension: restores the archived card.
#[test]
fn test_restore_extension_in_archived_view() {
    let mut app = App::test_default();
    let (_, _, _, card_id) = seed_archived_card(&mut app);

    app.handle_archived_cards_view_mode(KeyCode::Char('r'));
    assert!(
        app.animation.animating.contains_key(&card_id),
        "`r` starts the restore animation for the archived card"
    );
}

/// `x` (permanent-delete) extension: deletes the archived card.
#[test]
fn test_delete_extension_in_archived_view() {
    let mut app = App::test_default();
    let (_, _, _, card_id) = seed_archived_card(&mut app);

    app.handle_archived_cards_view_mode(KeyCode::Char('x'));
    assert!(
        app.animation.animating.contains_key(&card_id),
        "`x` starts the permanent-delete animation for the archived card"
    );
}

/// The archived-cards provider DELEGATES to the card-list provider (inherits the
/// full live card bindings) and appends ONLY the restore/delete extension — but
/// EXCLUDES create (`n`), which makes no sense from an archived list.
#[test]
fn test_archived_provider_delegates_and_excludes_create() {
    let card_ctx = CardListProvider.get_context();
    let arch_ctx = ArchivedCardsViewProvider.get_context();

    // Inherits shared card-list bindings (e.g. edit, priority, move).
    for key in ["e", "p", "a", "L", "H"] {
        assert!(
            arch_ctx.bindings.iter().any(|b| b.key == key),
            "archived provider inherits shared card binding `{key}`"
        );
    }
    // Appends the extension keys.
    assert!(arch_ctx
        .bindings
        .iter()
        .any(|b| b.key == "r" && b.description.to_lowercase().contains("restore")));
    assert!(arch_ctx
        .bindings
        .iter()
        .any(|b| b.key == "x" && b.description.to_lowercase().contains("delete")));

    // #414 finding 1: create (`n`) is NOT offered from the archived view even
    // though the shared card provider offers it.
    assert!(
        card_ctx.bindings.iter().any(|b| b.key == "n"),
        "sanity: the live card provider does offer create"
    );
    assert!(
        !arch_ctx.bindings.iter().any(|b| b.key == "n"),
        "create `n` is excluded from the archived-cards provider"
    );
}

/// #414 finding 3: reused bindings whose behaviour differs must describe the
/// actual behaviour. `q`/`Esc` in the archived view toggle back to the live
/// set — the help must say so, NOT "Quit"/"clear selection".
#[test]
fn test_archived_help_describes_toggle_for_q_and_esc() {
    let ctx = ArchivedCardsViewProvider.get_context();

    let back = ctx
        .bindings
        .iter()
        .find(|b| b.key.contains("Esc") || b.key.contains('q'))
        .expect("archived view advertises a q/Esc binding");

    let desc = back.description.to_lowercase();
    assert!(
        desc.contains("back") || desc.contains("normal") || desc.contains("live"),
        "q/Esc must describe the toggle-back, got: {:?}",
        back.description
    );
    assert!(
        !desc.contains("quit") && !desc.contains("clear"),
        "q/Esc must not still advertise Quit/clear, got: {:?}",
        back.description
    );
}

/// #414 finding 4 / #428 review: the tasks-panel title keys off the stack-aware
/// base mode, so a confirm dialog opened OVER the archived view keeps the
/// "Archive" title rather than flipping to the live "Tasks" title.
#[test]
fn test_archived_tasks_panel_title_uses_base_mode_under_dialog() {
    let mut app = App::test_default();
    let (_, _, _, _) = seed_archived_card(&mut app);

    let title_plain = kanban_tui::ui::tasks_panel_title(&app, false);
    assert!(
        title_plain.starts_with("Archive"),
        "archived view shows the Archive title, got: {title_plain}"
    );

    // Push a dialog over the archived view: raw `mode` becomes Dialog(..) but the
    // base mode is still ArchivedCardsView.
    app.push_mode(AppMode::Dialog(
        kanban_tui::app::DialogMode::SetCardPriority,
    ));
    let title_under_dialog = kanban_tui::ui::tasks_panel_title(&app, false);
    assert!(
        title_under_dialog.starts_with("Archive"),
        "the tasks-panel title under a modal must stay Archive (base-mode-aware), \
         got: {title_under_dialog}"
    );
}

/// `V` (toggle task list view) mutates the BOARD's `task_list_view` setting
/// via a dialog and must be excluded from the archived-cards view like `n`.
#[test]
fn test_toggle_task_list_view_not_offered_in_archived_view() {
    let mut app = App::test_default();
    seed_archived_card(&mut app);

    app.handle_archived_cards_view_mode(KeyCode::Char('V'));

    assert_eq!(
        app.mode,
        AppMode::ArchivedCardsView,
        "`V` must not open the task-list-view dialog from the archived view"
    );
}

/// `t` (toggle sprint filter) mutates the shared LIVE filter state
/// (`active_sprint_filters`), which then silently affects the live cards view
/// too. It must be excluded from the archived-cards view.
#[test]
fn test_sprint_filter_toggle_not_offered_in_archived_view() {
    let mut app = App::test_default();
    let (board_id, _, _, _) = seed_archived_card(&mut app);
    let sprint = app.ctx.create_sprint(board_id, None, None).unwrap();
    app.ctx
        .update_board(
            board_id,
            kanban_domain::BoardUpdate {
                active_sprint_id: kanban_domain::FieldUpdate::Set(sprint.id),
                ..Default::default()
            },
        )
        .unwrap();
    app.reload_model();
    app.prepare_frame();

    assert!(
        app.filter.active_sprint_filters.is_empty(),
        "precondition: no sprint filter active"
    );

    app.handle_archived_cards_view_mode(KeyCode::Char('t'));

    assert!(
        app.filter.active_sprint_filters.is_empty(),
        "`t` must not toggle the shared sprint filter from the archived view"
    );
}

/// `T` (open filter options dialog) targets the shared LIVE filter state. It
/// must be excluded from the archived-cards view.
#[test]
fn test_filter_options_dialog_not_offered_in_archived_view() {
    let mut app = App::test_default();
    seed_archived_card(&mut app);

    app.handle_archived_cards_view_mode(KeyCode::Char('T'));

    assert_eq!(
        app.mode,
        AppMode::ArchivedCardsView,
        "`T` must not open the filter-options dialog from the archived view"
    );
}

/// `1` (focus projects panel) desyncs focus away from the confined
/// archived-cards navigation context on a `Flat`/`GroupedByColumn` board (the
/// default here). It must be excluded from the archived-cards view in that
/// case; under `ColumnView` it instead does column-jump navigation, which
/// stays available (see `test_column_jump_still_works_under_column_view_in_archived_view`).
#[test]
fn test_focus_panel_switch_not_offered_in_archived_view() {
    let mut app = App::test_default();
    seed_archived_card(&mut app);

    app.handle_archived_cards_view_mode(KeyCode::Char('1'));

    assert_eq!(
        app.focus.active,
        Focus::Cards,
        "`1` must not switch focus away from Cards in the archived view"
    );
    assert_eq!(
        app.mode,
        AppMode::ArchivedCardsView,
        "`1` must not change mode from the archived view"
    );
}

/// Under `ColumnView`, `1` in the archived-cards view must still perform
/// column-jump navigation (`handle_column_or_focus_switch`) rather than being
/// swallowed as a no-op — that no-op only applies to the Boards-focus-switch
/// behaviour `1` has on `Flat`/`GroupedByColumn` boards.
#[test]
fn test_column_jump_still_works_under_column_view_in_archived_view() {
    use kanban_view::card_list::CardListId;

    let mut app = App::test_default();
    let (board_id, col1, col2, _) = seed_archived_card(&mut app);
    app.ctx
        .update_board(
            board_id,
            kanban_domain::BoardUpdate {
                task_list_view: Some(kanban_domain::TaskListView::ColumnView),
                ..Default::default()
            },
        )
        .unwrap();
    app.switch_view_strategy(kanban_domain::TaskListView::ColumnView);
    app.reload_model();
    app.prepare_frame();

    // Navigate to column 2 (index 1) first so jumping to `1` (index 0) is a
    // real, observable move.
    app.view.strategy.navigate_right(false);
    assert_eq!(
        app.view
            .strategy
            .get_active_task_list()
            .map(|l| l.id.clone()),
        Some(CardListId::Column(col2)),
        "precondition: navigated to col2"
    );

    app.handle_archived_cards_view_mode(KeyCode::Char('1'));

    assert_eq!(
        app.view
            .strategy
            .get_active_task_list()
            .map(|l| l.id.clone()),
        Some(CardListId::Column(col1)),
        "`1` still jumps to column 0 under ColumnView in the archived view"
    );
    assert_eq!(
        app.mode,
        AppMode::ArchivedCardsView,
        "column-jump must not change mode"
    );
}

/// The archived-cards provider must not advertise `V`/`t`/`T`/`1` in its footer
/// — they are excluded, just like `n`.
#[test]
fn test_archived_provider_excludes_view_and_filter_and_focus_keys() {
    let card_ctx = CardListProvider.get_context();
    let arch_ctx = ArchivedCardsViewProvider.get_context();

    for key in ["V", "t", "T", "1"] {
        assert!(
            card_ctx.bindings.iter().any(|b| b.key == key),
            "sanity: the live card provider does offer `{key}`"
        );
        assert!(
            !arch_ctx.bindings.iter().any(|b| b.key == key),
            "`{key}` is excluded from the archived-cards provider"
        );
    }
}

/// Guard: the LIVE card list excludes archived cards; they appear only when the
/// panel is toggled to the archived set.
#[test]
fn test_live_card_list_excludes_archived() {
    let mut app = App::test_default();
    let board = app.ctx.create_board("Board".to_string(), None).unwrap();
    let col = app
        .ctx
        .create_column(board.id, "Todo".to_string(), None)
        .unwrap();
    let live = app
        .ctx
        .create_card(
            board.id,
            col.id,
            "Live".to_string(),
            CreateCardOptions::default(),
        )
        .unwrap();
    let arch = app
        .ctx
        .create_card(
            board.id,
            col.id,
            "Arch".to_string(),
            CreateCardOptions::default(),
        )
        .unwrap();
    app.ctx.archive_card(arch.id).unwrap();
    app.selection.active_board_id = Some(board.id);

    // Live view: only the live card.
    app.mode = AppMode::Normal;
    app.reload_model();
    app.prepare_frame();
    let live_ids: Vec<_> = app.displayed_cards().iter().map(|c| c.id).collect();
    assert!(live_ids.contains(&live.id), "live card is shown");
    assert!(
        !live_ids.contains(&arch.id),
        "archived card hidden from the live set"
    );

    // Archived view: only the archived card.
    app.mode = AppMode::ArchivedCardsView;
    app.reload_model();
    app.prepare_frame();
    let arch_ids: Vec<_> = app.displayed_cards().iter().map(|c| c.id).collect();
    assert!(
        arch_ids.contains(&arch.id),
        "archived card shown in archived set"
    );
    assert!(
        !arch_ids.contains(&live.id),
        "live card hidden from the archived set"
    );
}
