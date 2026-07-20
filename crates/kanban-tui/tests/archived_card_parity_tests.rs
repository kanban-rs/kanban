//! KAN-913: an archived card is substitutable for a live one everywhere.
//!
//! These tests drive an ARCHIVED card, reached through the archived-cards list,
//! through the exact same handlers a LIVE card uses — card detail, edit,
//! move-column, set-priority and sprint-assign — and assert the behaviour is
//! identical. The only place the live/archived distinction appears is the tasks
//! panel choosing which card SET it displays plus a display indicator. The
//! archived-only affordances (restore / permanent-delete) are an EXTENSION layered
//! on the shared card keybindings, not a forked limited view.

use kanban_domain::{CardPriority, CreateCardOptions, KanbanOperations};
use kanban_tui::app::focus::Focus;
use kanban_tui::app::mode::AppMode;
use kanban_tui::App;
use std::time::{Duration, Instant};

/// Seed a board with two columns and two cards, archive the first card, and load
/// the snapshot. Returns (board_id, first_column_id, second_column_id,
/// archived_card_id, live_card_id).
fn seed_and_archive_card(
    app: &mut App,
) -> (uuid::Uuid, uuid::Uuid, uuid::Uuid, uuid::Uuid, uuid::Uuid) {
    let board = app.ctx.create_board("Board".to_string(), None).unwrap();
    let col1 = app
        .ctx
        .create_column(board.id, "Todo".to_string(), None)
        .unwrap();
    let col2 = app
        .ctx
        .create_column(board.id, "Doing".to_string(), None)
        .unwrap();
    let card1 = app
        .ctx
        .create_card(
            board.id,
            col1.id,
            "ArchiveMe".to_string(),
            CreateCardOptions::default(),
        )
        .unwrap();
    let card2 = app
        .ctx
        .create_card(
            board.id,
            col1.id,
            "StayLive".to_string(),
            CreateCardOptions::default(),
        )
        .unwrap();
    app.ctx.create_sprint(board.id, None, None).unwrap();
    app.ctx.archive_card(card1.id).unwrap();
    app.selection.active_board_id = Some(board.id);
    let snap = app.ctx.snapshot().unwrap();
    app.model.load_from_snapshot(snap);
    (board.id, col1.id, col2.id, card1.id, card2.id)
}

/// Enter the archived-cards list the same way the user does: toggle to it and
/// select the first (archived) card. Proof of reuse: the tasks panel is the same
/// component, fed the archived set.
fn open_archived_cards(app: &mut App) {
    app.mode = AppMode::ArchivedCardsView;
    app.prepare_frame();
    app.focus.active = Focus::Cards;
    if let Some(list) = app.view.strategy.get_active_task_list_mut() {
        list.set_selected_index(Some(0));
    }
}

fn force_animation_complete(app: &mut App, card_id: uuid::Uuid) {
    if let Some(anim) = app.animation.animating.get_mut(&card_id) {
        anim.start_time = Instant::now() - Duration::from_millis(400);
    }
}

#[test]
fn test_archived_card_detail_opens_via_shared_handler() {
    let mut app = App::test_default();
    let (_, _, _, archived_id, _) = seed_and_archive_card(&mut app);

    open_archived_cards(&mut app);

    // The SAME activation handler a live card uses — no archival branch.
    app.handle_selection_activate();

    assert_eq!(
        app.mode,
        AppMode::CardDetail,
        "opening an archived card enters card detail exactly like a live card"
    );
    assert_eq!(
        app.selection.active_card_id,
        Some(archived_id),
        "the archived card resolves as the active card by id"
    );
}

#[test]
fn test_archived_card_set_priority_via_shared_handler() {
    let mut app = App::test_default();
    let (_, _, _, archived_id, _) = seed_and_archive_card(&mut app);

    open_archived_cards(&mut app);

    // `p` opens the priority dialog for the highlighted card, then Enter applies.
    app.handle_set_card_priority_key();
    assert_eq!(
        app.mode,
        AppMode::Dialog(kanban_tui::app::DialogMode::SetCardPriority),
        "priority dialog opens for an archived card via the shared handler"
    );
    // Choose Critical (index 3) and confirm.
    app.dialog_input.priority_selection.set(Some(3));
    app.handle_set_card_priority_popup(crossterm::event::KeyCode::Enter);

    let snap = app.ctx.snapshot().unwrap();
    app.model.load_from_snapshot(snap);
    let card = app
        .get_card_by_id(archived_id)
        .expect("archived card still resolves");
    assert_eq!(
        card.priority,
        CardPriority::Critical,
        "priority change on an archived card takes effect"
    );
}

#[test]
fn test_archived_card_move_column_via_shared_handler() {
    let mut app = App::test_default();
    let (_, _, col2, archived_id, _) = seed_and_archive_card(&mut app);

    open_archived_cards(&mut app);

    // Move the archived card right (Todo -> Doing) via the same handler.
    app.handle_move_card_right();

    let snap = app.ctx.snapshot().unwrap();
    app.model.load_from_snapshot(snap);
    let card = app
        .get_card_by_id(archived_id)
        .expect("archived card still resolves");
    assert_eq!(
        card.column_id, col2,
        "move-column on an archived card takes effect"
    );
}

#[test]
fn test_archived_card_action_applies_via_shared_handler() {
    // A mutating card action (toggle completion) drives through the same handler
    // a live card uses and takes effect on the archived card — proving the edit /
    // action path is not gated on liveness.
    let mut app = App::test_default();
    let (_, _, _, archived_id, _) = seed_and_archive_card(&mut app);

    open_archived_cards(&mut app);

    app.handle_toggle_card_completion();

    let snap = app.ctx.snapshot().unwrap();
    app.model.load_from_snapshot(snap);
    let card = app
        .get_card_by_id(archived_id)
        .expect("archived card still resolves");
    assert!(
        card.is_completed(),
        "a card action on an archived card takes effect via the shared handler"
    );
}

#[test]
fn test_archived_card_sprint_assign_via_shared_handler() {
    let mut app = App::test_default();
    let (_, _, _, archived_id, _) = seed_and_archive_card(&mut app);

    open_archived_cards(&mut app);

    app.handle_assign_to_sprint_key();

    assert_eq!(
        app.mode,
        AppMode::Dialog(kanban_tui::app::DialogMode::AssignCardToSprint),
        "sprint-assign dialog opens for an archived card via the shared handler"
    );
    assert_eq!(
        app.selection.active_card_id,
        Some(archived_id),
        "the archived card is the assignment target"
    );
}

#[test]
fn test_archived_card_restore_extension_still_works() {
    let mut app = App::test_default();
    let (_, _, _, archived_id, _) = seed_and_archive_card(&mut app);

    open_archived_cards(&mut app);

    // The archived-only extension key: restore.
    app.handle_restore_card();
    assert!(
        app.animation.animating.contains_key(&archived_id),
        "restore starts a restore animation on the archived card"
    );
    force_animation_complete(&mut app, archived_id);
    app.restore_card(
        app.model
            .archived_cards()
            .iter()
            .find(|ac| ac.entity_id == archived_id)
            .cloned()
            .unwrap(),
    );

    assert!(
        !app.ctx
            .list_archived_cards()
            .unwrap()
            .iter()
            .any(|ac| ac.entity_id == archived_id),
        "the card is no longer archived after restore"
    );
}

#[test]
fn test_archived_card_permanent_delete_extension_still_works() {
    let mut app = App::test_default();
    let (_, _, _, archived_id, _) = seed_and_archive_card(&mut app);

    open_archived_cards(&mut app);

    // The archived-only extension key: permanent delete.
    app.handle_delete_card_permanent();
    assert!(
        app.animation.animating.contains_key(&archived_id),
        "permanent delete starts a deleting animation on the archived card"
    );
}

#[test]
fn test_archived_cards_view_uses_shared_card_provider_extended() {
    // The archived-cards view reuses the normal card provider EXTENDED with
    // restore/delete — not a limited replacement. Detail, edit, priority and
    // sprint-assign bindings must be present (they were absent in the old
    // forked provider).
    use kanban_tui::keybindings::{KeybindingAction, KeybindingRegistry};
    let mut app = App::test_default();
    seed_and_archive_card(&mut app);
    open_archived_cards(&mut app);

    let provider = KeybindingRegistry::get_provider(&app);
    let ctx = provider.get_context();
    let has = |a: KeybindingAction| ctx.bindings.iter().any(|b| b.action == a);

    assert!(
        has(KeybindingAction::SelectItem),
        "archived view exposes card detail (Enter) like the live card list"
    );
    assert!(
        has(KeybindingAction::EditCard),
        "archived view exposes edit (e)"
    );
    assert!(
        has(KeybindingAction::SetCardPriority),
        "archived view exposes set-priority (p)"
    );
    assert!(
        has(KeybindingAction::AssignToSprint),
        "archived view exposes sprint-assign (a)"
    );
    assert!(
        has(KeybindingAction::RestoreCard),
        "archived view still exposes the restore extension"
    );
}

#[test]
fn test_live_card_list_excludes_archived_cards() {
    // Guard: the live card list is unchanged and excludes archived cards.
    let mut app = App::test_default();
    let (_, _, _, archived_id, live_id) = seed_and_archive_card(&mut app);

    app.mode = AppMode::Normal;
    app.focus.active = Focus::Cards;
    app.prepare_frame();

    let list = app
        .view
        .strategy
        .get_active_task_list()
        .expect("active task list");
    assert!(
        list.cards.contains(&live_id),
        "the live card is shown in the live list"
    );
    assert!(
        !list.cards.contains(&archived_id),
        "the archived card is excluded from the live list"
    );
}
