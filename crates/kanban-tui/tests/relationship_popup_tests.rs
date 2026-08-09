mod helpers;

use kanban_tui::App;

#[test]
fn test_render_manage_parents_popup_renders_without_panic() {
    let app = App::test_default();
    let output = helpers::render_widget_to_string(120, 40, |frame| {
        kanban_tui::components::render_manage_parents_popup(&app, frame);
    });
    assert!(output.contains("Set Parents"));
}

#[test]
fn test_render_manage_children_popup_renders_without_panic() {
    let app = App::test_default();
    let output = helpers::render_widget_to_string(120, 40, |frame| {
        kanban_tui::components::render_manage_children_popup(&app, frame);
    });
    assert!(output.contains("Set Children"));
}

#[test]
fn test_render_manage_parents_popup_shows_search_box() {
    let app = App::test_default();
    let output = helpers::render_widget_to_string(120, 40, |frame| {
        kanban_tui::components::render_manage_parents_popup(&app, frame);
    });
    assert!(output.contains("Search"));
}

#[test]
fn test_render_manage_parents_popup_shows_no_cards_when_empty() {
    let app = App::test_default();
    let output = helpers::render_widget_to_string(120, 40, |frame| {
        kanban_tui::components::render_manage_parents_popup(&app, frame);
    });
    assert!(output.contains("No eligible"));
}

#[test]
fn test_render_manage_parents_popup_search_active_shows_query() {
    use kanban_tui::app::mode::{AppMode, DialogMode};
    let mut app = App::test_default();
    app.push_mode(AppMode::Dialog(DialogMode::ManageParents));
    app.relationship.search_active = true;
    app.relationship.search = "test".to_string();
    let output = helpers::render_widget_to_string(120, 40, |frame| {
        kanban_tui::components::render_manage_parents_popup(&app, frame);
    });
    assert!(output.contains("test"));
}

/// Cycle 5 of the KAN-504 refactor rewired the relationship popup to
/// drive edge mutations through `GraphOperations::set_parent` /
/// `remove_parent` rather than constructing `Command::Dependency`
/// directly. The existing render tests don't exercise the mutation
/// path, so without this test the new wiring is only checked at
/// compile time. Press Enter on the selected candidate and assert
/// the edge appears on the data store's graph.
#[test]
fn test_manage_parents_popup_enter_creates_parent_edge() {
    use crossterm::event::KeyCode;
    use kanban_domain::{CreateCardOptions, KanbanOperations, Snapshot};
    use kanban_tui::app::mode::{AppMode, DialogMode};

    let mut app = App::test_default();

    // Create a board with a column and two cards; the second is the
    // active card, the first is the candidate parent.
    let board = app.ctx.create_board("Board".into(), None).unwrap();
    let column = app
        .ctx
        .create_column(board.id, "TODO".into(), None)
        .unwrap();
    let parent = app
        .ctx
        .create_card(
            board.id,
            column.id,
            "Parent".into(),
            CreateCardOptions::default(),
        )
        .unwrap();
    let child = app
        .ctx
        .create_card(
            board.id,
            column.id,
            "Child".into(),
            CreateCardOptions::default(),
        )
        .unwrap();

    // Wire the model so popup_handlers' `self.model.all_cards()` reflects
    // the data store. `selection.active_card` points at child.
    let snapshot = Snapshot {
        archived_boards: Vec::new(),
        boards: app.ctx.data_store().list_boards().unwrap(),
        columns: app.ctx.data_store().list_all_columns().unwrap(),
        cards: app.ctx.data_store().list_all_cards().unwrap(),
        archived_cards: app.ctx.data_store().list_archived_cards().unwrap(),
        sprints: app.ctx.data_store().list_all_sprints().unwrap(),
        graph: app.ctx.data_store().get_graph().unwrap(),
    };
    app.model.load_from_snapshot(snapshot);
    app.selection.active_card_id = Some(child.id);

    // Enter ManageParents mode with the parent as the only candidate
    // and select it.
    app.push_mode(AppMode::Dialog(DialogMode::ManageParents));
    app.relationship.card_ids = vec![parent.id];
    app.relationship.selection.set(Some(0));

    app.handle_manage_parents_popup(KeyCode::Enter);

    let graph = app.ctx.data_store().get_graph().unwrap();
    let parents = graph.parents(child.id);
    assert!(
        parents.contains(&parent.id),
        "Enter on selected parent must add a parent->child edge; graph.parents(child) = {parents:?}, expected to contain {parent_id}",
        parent_id = parent.id
    );
    assert_eq!(
        graph.spawns_edges().len(),
        1,
        "exactly one parent-of edge should be present"
    );
}

/// When `attach_child` / `detach_child` fails in the popup (cycle,
/// self-reference, duplicate), the user must see an error banner.
/// Without this feedback the popup looks like a no-op: the selection
/// doesn't toggle, no banner appears, and the user has no way to know
/// the operation was rejected. The fix calls `set_error` on the failure
/// branch alongside the existing success-toggle path.
#[test]
fn test_manage_parents_popup_cycle_surfaces_error_banner_to_user() {
    use crossterm::event::KeyCode;
    use kanban_domain::{CreateCardOptions, GraphOperations, KanbanOperations, Snapshot};
    use kanban_tui::app::mode::{AppMode, DialogMode};

    let mut app = App::test_default();

    // Seed three cards in a chain a -> b -> c. Then in the popup we
    // try to make a a parent of c — that would close the cycle
    // a -> b -> c -> a and must fail.
    let board = app.ctx.create_board("Board".into(), None).unwrap();
    let column = app
        .ctx
        .create_column(board.id, "TODO".into(), None)
        .unwrap();
    let a = app
        .ctx
        .create_card(
            board.id,
            column.id,
            "A".into(),
            CreateCardOptions::default(),
        )
        .unwrap();
    let b = app
        .ctx
        .create_card(
            board.id,
            column.id,
            "B".into(),
            CreateCardOptions::default(),
        )
        .unwrap();
    let c = app
        .ctx
        .create_card(
            board.id,
            column.id,
            "C".into(),
            CreateCardOptions::default(),
        )
        .unwrap();
    // a -> b
    app.ctx.attach_child(a.id, b.id).unwrap();
    // b -> c
    app.ctx.attach_child(b.id, c.id).unwrap();

    let snapshot = Snapshot {
        archived_boards: Vec::new(),
        boards: app.ctx.data_store().list_boards().unwrap(),
        columns: app.ctx.data_store().list_all_columns().unwrap(),
        cards: app.ctx.data_store().list_all_cards().unwrap(),
        archived_cards: app.ctx.data_store().list_archived_cards().unwrap(),
        sprints: app.ctx.data_store().list_all_sprints().unwrap(),
        graph: app.ctx.data_store().get_graph().unwrap(),
    };
    app.model.load_from_snapshot(snapshot);
    // active card is `a`; the popup will offer `c` as a candidate parent
    // and choosing it would close the cycle c -> a -> b -> c.
    app.selection.active_card_id = Some(a.id);

    app.push_mode(AppMode::Dialog(DialogMode::ManageParents));
    app.relationship.card_ids = vec![c.id];
    app.relationship.selection.set(Some(0));

    // Sanity: no banner before the failed attempt.
    assert!(
        app.ui_state.banner.is_none(),
        "banner must start empty before the cycle attempt"
    );

    app.handle_manage_parents_popup(KeyCode::Enter);

    // The graph state must NOT have the cycle-creating edge.
    let graph = app.ctx.data_store().get_graph().unwrap();
    assert!(
        !graph.parents(a.id).contains(&c.id),
        "c->a edge would close the cycle and must be rejected"
    );

    // The user must see an error banner explaining the rejection.
    let banner = app
        .ui_state
        .banner
        .as_ref()
        .expect("failed attach_child must surface an error banner");
    let msg_lower = banner.message.to_lowercase();
    assert!(
        msg_lower.contains("cycle") || msg_lower.contains("error") || msg_lower.contains("fail"),
        "banner must explain the failure; got {:?}",
        banner.message
    );

    // The selection must not have been toggled (the relationship state
    // tracks the desired UI selection; a failed mutation must not flip it).
    assert!(
        !app.relationship.selected.contains(&c.id),
        "failed attach_child must not toggle the popup selection"
    );
}

/// End-to-end proof that the render path, not just the unit-level
/// filter helper, benefits from `CompositeSearcher`: a card whose title
/// does not contain the search query must still render because its
/// resolved card identifier does.
#[test]
fn test_relationship_popup_render_shows_card_found_only_by_identifier() {
    use kanban_domain::{CreateCardOptions, KanbanOperations, Snapshot};
    use kanban_tui::app::mode::{AppMode, DialogMode};

    let mut app = App::test_default();

    let board = app
        .ctx
        .create_board("Board".into(), Some("KAN".into()))
        .unwrap();
    let column = app
        .ctx
        .create_column(board.id, "TODO".into(), None)
        .unwrap();
    let first = app
        .ctx
        .create_card(
            board.id,
            column.id,
            "Unrelated title".into(),
            CreateCardOptions::default(),
        )
        .unwrap();
    let second = app
        .ctx
        .create_card(
            board.id,
            column.id,
            "Also unrelated".into(),
            CreateCardOptions::default(),
        )
        .unwrap();

    let snapshot = Snapshot {
        archived_boards: Vec::new(),
        boards: app.ctx.data_store().list_boards().unwrap(),
        columns: app.ctx.data_store().list_all_columns().unwrap(),
        cards: app.ctx.data_store().list_all_cards().unwrap(),
        archived_cards: app.ctx.data_store().list_archived_cards().unwrap(),
        sprints: app.ctx.data_store().list_all_sprints().unwrap(),
        graph: app.ctx.data_store().get_graph().unwrap(),
    };
    app.model.load_from_snapshot(snapshot);

    app.push_mode(AppMode::Dialog(DialogMode::ManageParents));
    app.relationship.card_ids = vec![first.id, second.id];
    app.relationship.board_id = Some(board.id);
    app.relationship.search = "kan-2".to_string();

    let output = helpers::render_widget_to_string(120, 40, |frame| {
        kanban_tui::components::render_manage_parents_popup(&app, frame);
    });

    assert!(
        output.contains("Also unrelated"),
        "card KAN-2 must render even though neither title contains 'kan-2'; got:\n{output}"
    );
    assert!(
        !output.contains("Unrelated title"),
        "card KAN-1 must be filtered out; got:\n{output}"
    );
}

/// The relationship picker's card list overflows a small popup silently
/// today (KAN-1102): every eligible card is pushed into the `Paragraph`
/// regardless of the popup's height. Once the list is migrated onto
/// `ListComponent`, a popup with more eligible cards than fit the viewport
/// must show a "below" scroll indicator, matching `render_relationship_section`'s
/// existing pattern for the read-only parents/children display.
#[test]
fn test_relationship_picker_list_shows_below_indicator_when_eligible_cards_exceed_viewport() {
    use kanban_domain::{CreateCardOptions, KanbanOperations, Snapshot};
    use kanban_tui::app::mode::{AppMode, DialogMode};

    let mut app = App::test_default();

    let board = app.ctx.create_board("Board".into(), None).unwrap();
    let column = app
        .ctx
        .create_column(board.id, "TODO".into(), None)
        .unwrap();

    let mut card_ids = Vec::new();
    for i in 0..30 {
        let card = app
            .ctx
            .create_card(
                board.id,
                column.id,
                format!("Card {i}"),
                CreateCardOptions::default(),
            )
            .unwrap();
        card_ids.push(card.id);
    }

    let snapshot = Snapshot {
        archived_boards: Vec::new(),
        boards: app.ctx.data_store().list_boards().unwrap(),
        columns: app.ctx.data_store().list_all_columns().unwrap(),
        cards: app.ctx.data_store().list_all_cards().unwrap(),
        archived_cards: app.ctx.data_store().list_archived_cards().unwrap(),
        sprints: app.ctx.data_store().list_all_sprints().unwrap(),
        graph: app.ctx.data_store().get_graph().unwrap(),
    };
    app.model.load_from_snapshot(snapshot);

    app.push_mode(AppMode::Dialog(DialogMode::ManageParents));
    app.relationship.card_ids = card_ids;
    app.relationship.board_id = Some(board.id);
    app.relationship.picker_list.update_item_count(30);

    let output = helpers::render_widget_to_string(120, 40, |frame| {
        kanban_tui::components::render_manage_parents_popup(&app, frame);
    });

    assert!(
        output.contains("below"),
        "30 eligible cards in a small popup must show a below-scroll indicator; got:\n{output}"
    );
}

/// `navigate_down` alone only moves the cursor; without an explicit
/// scroll-into-view call the scroll offset never advances, so a user
/// pressing `j` past the initial viewport would be stuck staring at cards
/// that never appear. This proves the migrated key handler actually scrolls,
/// not just that the render call renders once.
#[test]
fn test_relationship_picker_navigate_down_past_viewport_scrolls_list() {
    use crossterm::event::KeyCode;
    use kanban_domain::{CreateCardOptions, KanbanOperations, Snapshot};
    use kanban_tui::app::mode::{AppMode, DialogMode};
    use ratatui::layout::Rect;

    let mut app = App::test_default();
    app.view.last_frame_area = Rect {
        x: 0,
        y: 0,
        width: 120,
        height: 40,
    };

    let board = app.ctx.create_board("Board".into(), None).unwrap();
    let column = app
        .ctx
        .create_column(board.id, "TODO".into(), None)
        .unwrap();

    let mut card_ids = Vec::new();
    for i in 0..30 {
        let card = app
            .ctx
            .create_card(
                board.id,
                column.id,
                format!("Card {i}"),
                CreateCardOptions::default(),
            )
            .unwrap();
        card_ids.push(card.id);
    }

    let snapshot = Snapshot {
        archived_boards: Vec::new(),
        boards: app.ctx.data_store().list_boards().unwrap(),
        columns: app.ctx.data_store().list_all_columns().unwrap(),
        cards: app.ctx.data_store().list_all_cards().unwrap(),
        archived_cards: app.ctx.data_store().list_archived_cards().unwrap(),
        sprints: app.ctx.data_store().list_all_sprints().unwrap(),
        graph: app.ctx.data_store().get_graph().unwrap(),
    };
    app.model.load_from_snapshot(snapshot);

    app.push_mode(AppMode::Dialog(DialogMode::ManageParents));
    app.relationship.card_ids = card_ids;
    app.relationship.board_id = Some(board.id);
    app.relationship.picker_list.update_item_count(30);

    for _ in 0..29 {
        app.handle_manage_parents_popup(KeyCode::Char('j'));
    }

    assert!(
        app.relationship.picker_list.get_scroll_offset() > 0,
        "navigating down past the initial viewport must scroll the list"
    );
}

/// Regression for the existing cursor-reset-after-search behavior, now
/// expressed through `picker_list` instead of the old `selection` field:
/// narrowing the search must reset the cursor to the top of the new
/// (smaller) result set rather than leaving it at a stale index.
#[test]
fn test_relationship_picker_search_narrowing_resets_out_of_range_selection() {
    use crossterm::event::KeyCode;
    use kanban_domain::{CreateCardOptions, KanbanOperations, Snapshot};
    use kanban_tui::app::mode::{AppMode, DialogMode};

    let mut app = App::test_default();

    let board = app
        .ctx
        .create_board("Board".into(), Some("KAN".into()))
        .unwrap();
    let column = app
        .ctx
        .create_column(board.id, "TODO".into(), None)
        .unwrap();
    let mut card_ids = Vec::new();
    for i in 0..5 {
        let card = app
            .ctx
            .create_card(
                board.id,
                column.id,
                format!("Card {i}"),
                CreateCardOptions::default(),
            )
            .unwrap();
        card_ids.push(card.id);
    }

    let snapshot = Snapshot {
        archived_boards: Vec::new(),
        boards: app.ctx.data_store().list_boards().unwrap(),
        columns: app.ctx.data_store().list_all_columns().unwrap(),
        cards: app.ctx.data_store().list_all_cards().unwrap(),
        archived_cards: app.ctx.data_store().list_archived_cards().unwrap(),
        sprints: app.ctx.data_store().list_all_sprints().unwrap(),
        graph: app.ctx.data_store().get_graph().unwrap(),
    };
    app.model.load_from_snapshot(snapshot);

    app.push_mode(AppMode::Dialog(DialogMode::ManageParents));
    app.relationship.card_ids = card_ids;
    app.relationship.board_id = Some(board.id);
    app.relationship.picker_list.update_item_count(5);
    app.relationship.picker_list.jump_to(4);

    app.handle_manage_parents_popup(KeyCode::Char('/'));
    app.handle_manage_parents_popup(KeyCode::Char('C'));
    app.handle_manage_parents_popup(KeyCode::Char('a'));
    app.handle_manage_parents_popup(KeyCode::Char('r'));
    app.handle_manage_parents_popup(KeyCode::Char('d'));
    app.handle_manage_parents_popup(KeyCode::Char(' '));
    app.handle_manage_parents_popup(KeyCode::Char('0'));

    assert_eq!(
        app.relationship.picker_list.selection.get(),
        Some(0),
        "narrowing the search to a single match must reset the cursor to the top"
    );
}

/// The picker's checkbox membership tracking (`selected: HashSet<Uuid>`) is
/// independent of `ListComponent`'s own navigation/scroll state — this must
/// not regress when the cursor migrates onto `picker_list`.
#[test]
fn test_relationship_picker_checkbox_toggle_still_works_after_migration() {
    use crossterm::event::KeyCode;
    use kanban_domain::{CreateCardOptions, KanbanOperations, Snapshot};
    use kanban_tui::app::mode::{AppMode, DialogMode};

    let mut app = App::test_default();

    let board = app.ctx.create_board("Board".into(), None).unwrap();
    let column = app
        .ctx
        .create_column(board.id, "TODO".into(), None)
        .unwrap();
    let parent = app
        .ctx
        .create_card(
            board.id,
            column.id,
            "Parent".into(),
            CreateCardOptions::default(),
        )
        .unwrap();
    let child = app
        .ctx
        .create_card(
            board.id,
            column.id,
            "Child".into(),
            CreateCardOptions::default(),
        )
        .unwrap();

    let snapshot = Snapshot {
        archived_boards: Vec::new(),
        boards: app.ctx.data_store().list_boards().unwrap(),
        columns: app.ctx.data_store().list_all_columns().unwrap(),
        cards: app.ctx.data_store().list_all_cards().unwrap(),
        archived_cards: app.ctx.data_store().list_archived_cards().unwrap(),
        sprints: app.ctx.data_store().list_all_sprints().unwrap(),
        graph: app.ctx.data_store().get_graph().unwrap(),
    };
    app.model.load_from_snapshot(snapshot);
    app.selection.active_card_id = Some(child.id);

    app.push_mode(AppMode::Dialog(DialogMode::ManageParents));
    app.relationship.card_ids = vec![parent.id];
    app.relationship.picker_list.update_item_count(1);

    app.handle_manage_parents_popup(KeyCode::Char(' '));

    assert!(
        app.relationship.selected.contains(&parent.id),
        "Space on the selected candidate must toggle its checkbox membership"
    );

    app.handle_manage_parents_popup(KeyCode::Char(' '));

    assert!(
        !app.relationship.selected.contains(&parent.id),
        "Space again must untoggle the checkbox membership"
    );
}
