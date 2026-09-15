mod helpers;

use kanban_domain::Model;
use kanban_domain::{
    CreateCardOptions, EntityIds, GraphOperations, Invalidation, KanbanOperations,
};
use kanban_tui::app::mode::{AppMode, DialogMode};
use kanban_tui::components::resolve_relationship_cards;
use kanban_tui::App;
use uuid::Uuid;

fn invalidate_graph(app: &mut App) {
    let _ = app
        .model
        .invalidate(Invalidation::Entities(EntityIds::default().with_graph()));
}

/// Seeds one card's per-id tier directly. Required for any card outside the
/// active board, which `ViewScope`'s board-scoped card tier never reaches.
fn warm_card_body(app: &mut App, card_id: Uuid) {
    let card = app.ctx.get_card(card_id).unwrap().unwrap();
    let _ = app.model.apply_resolved(kanban_domain::Resolved {
        cards: kanban_domain::resolved::Collection {
            by_id: [(card_id, kanban_domain::LoadState::Loaded(card))].into(),
            ..Default::default()
        },
        ..Default::default()
    });
}

/// The dependency graph is only requested by `ViewScope` for `CardDetail`
/// and the manage-parents/children dialogs. Tests that inspect
/// `model.graph_state()` directly, without driving one of those handlers,
/// must warm it themselves.
fn warm_graph(app: &mut App) {
    let prior = app.mode.clone();
    app.mode = AppMode::CardDetail;
    app.resolve_for_view();
    app.mode = prior;
}

fn create_board_and_column(app: &mut App, board_title: &str) -> (Uuid, Uuid) {
    let board = app.ctx.create_board(board_title.into(), None).unwrap();
    let column = app
        .ctx
        .create_column(board.id, "TODO".into(), None)
        .unwrap();
    (board.id, column.id)
}

fn create_card(app: &mut App, board_id: Uuid, column_id: Uuid, title: &str) -> Uuid {
    app.ctx
        .create_card(
            board_id,
            column_id,
            title.into(),
            CreateCardOptions::default(),
        )
        .unwrap()
        .id
}

#[test]
fn test_resolve_relationship_cards_returns_only_the_related_cards() {
    let mut app = App::test_default();
    let (board_id, column_id) = create_board_and_column(&mut app, "Board");

    let mut card_ids = Vec::new();
    for i in 0..50 {
        card_ids.push(create_card(
            &mut app,
            board_id,
            column_id,
            &format!("Card {i}"),
        ));
    }
    let subject = card_ids[0];
    let parent = card_ids[1];
    let child = card_ids[2];

    app.ctx.attach_child(parent, subject).unwrap();
    app.ctx.attach_child(subject, child).unwrap();
    app.selection.active_board_id = Some(board_id);
    app.reload_model();
    warm_graph(&mut app);

    assert_eq!(
        app.model
            .board_cards_state(board_id)
            .loaded()
            .map(|v| v.len())
            .unwrap_or(0),
        50
    );

    let ids = [
        app.model
            .graph_state()
            .loaded()
            .unwrap_or_else(|| Model::empty_graph())
            .parents(subject),
        app.model
            .graph_state()
            .loaded()
            .unwrap_or_else(|| Model::empty_graph())
            .children(subject),
    ]
    .concat();

    let resolved = resolve_relationship_cards(&app.model, &ids);
    let resolved_ids: std::collections::HashSet<Uuid> = resolved.iter().map(|c| c.id).collect();

    assert_eq!(resolved.len(), 2);
    assert_eq!(
        resolved_ids,
        std::collections::HashSet::from([parent, child])
    );
}

#[test]
fn test_resolve_relationship_cards_resolves_archived_related_card() {
    let mut app = App::test_default();
    let (board_id, column_id) = create_board_and_column(&mut app, "Board");

    let subject = create_card(&mut app, board_id, column_id, "Subject");
    let parent = create_card(&mut app, board_id, column_id, "ArchivedParentXYZ");

    app.ctx.attach_child(parent, subject).unwrap();
    app.ctx.archive_card(parent).unwrap();
    app.selection.active_board_id = Some(board_id);
    app.reload_model();
    helpers::warm_archived_card_markers(&mut app);

    assert!(app.model.archived_card_ids().contains(&parent));
    assert!(!app
        .controller
        .live_cards()
        .loaded()
        .copied()
        .unwrap_or(&[])
        .iter()
        .any(|c| c.id == parent));

    let resolved = resolve_relationship_cards(&app.model, &[parent]);

    assert_eq!(resolved.len(), 1);
    assert_eq!(resolved[0].id, parent);
    assert_eq!(resolved[0].title, "ArchivedParentXYZ");
}

#[test]
fn test_resolve_relationship_cards_resolves_a_cross_board_card_whose_body_was_fetched() {
    let mut app = App::test_default();
    let (board_b_id, column_b_id) = create_board_and_column(&mut app, "Board B");
    let (board_c_id, column_c_id) = create_board_and_column(&mut app, "Board C");

    let subject = create_card(&mut app, board_b_id, column_b_id, "Subject");
    let cross = create_card(&mut app, board_c_id, column_c_id, "CrossBoardChildXYZ");

    app.ctx.attach_child(subject, cross).unwrap();
    app.selection.active_board_id = Some(board_b_id);
    app.reload_model();
    warm_graph(&mut app);
    warm_card_body(&mut app, cross);

    let children = app
        .model
        .graph_state()
        .loaded()
        .unwrap_or_else(|| Model::empty_graph())
        .children(subject);
    let resolved = resolve_relationship_cards(&app.model, &children);

    assert_eq!(resolved.len(), 1);
    assert_eq!(resolved[0].id, cross);
    assert_eq!(resolved[0].title, "CrossBoardChildXYZ");
}

#[test]
fn test_resolve_relationship_cards_omits_id_with_no_card() {
    let mut app = App::test_default();
    let (board_id, column_id) = create_board_and_column(&mut app, "Board");

    let known = create_card(&mut app, board_id, column_id, "Known");
    create_card(&mut app, board_id, column_id, "Other1");
    create_card(&mut app, board_id, column_id, "Other2");
    app.selection.active_board_id = Some(board_id);
    app.reload_model();

    let unknown = Uuid::new_v4();
    let resolved = resolve_relationship_cards(&app.model, &[known, unknown]);

    let resolved_ids: Vec<Uuid> = resolved.iter().map(|c| c.id).collect();
    assert_eq!(resolved_ids, vec![known]);
}

#[test]
fn test_resolve_relationship_cards_returns_live_archived_and_cross_board_bodies_in_input_order() {
    let mut app = App::test_default();
    let (board_b_id, column_b_id) = create_board_and_column(&mut app, "Board B");
    let (board_c_id, column_c_id) = create_board_and_column(&mut app, "Board C");

    let subject = create_card(&mut app, board_b_id, column_b_id, "Subject");
    let live_parent = create_card(&mut app, board_b_id, column_b_id, "LiveParent");
    let archived_parent = create_card(&mut app, board_b_id, column_b_id, "ArchivedParent");
    let cross_child = create_card(&mut app, board_c_id, column_c_id, "CrossBoardChild");

    app.ctx.attach_child(live_parent, subject).unwrap();
    app.ctx.attach_child(archived_parent, subject).unwrap();
    app.ctx.attach_child(subject, cross_child).unwrap();
    app.ctx.archive_card(archived_parent).unwrap();
    app.selection.active_board_id = Some(board_b_id);
    app.reload_model();
    helpers::warm_archived_card_markers(&mut app);
    warm_graph(&mut app);
    warm_card_body(&mut app, cross_child);

    let mut ids = [
        app.model
            .graph_state()
            .loaded()
            .unwrap_or_else(|| Model::empty_graph())
            .parents(subject),
        app.model
            .graph_state()
            .loaded()
            .unwrap_or_else(|| Model::empty_graph())
            .children(subject),
    ]
    .concat();
    ids.push(archived_parent);
    ids.push(Uuid::new_v4());

    assert!(
        !app.model
            .graph_state()
            .loaded()
            .unwrap_or_else(|| Model::empty_graph())
            .parents(subject)
            .contains(&archived_parent),
        "archiving a card archives its spawns edge, so it must be reintroduced by id"
    );

    let actual: Vec<Uuid> = resolve_relationship_cards(&app.model, &ids)
        .iter()
        .map(|c| c.id)
        .collect();

    assert_eq!(
        actual,
        vec![live_parent, cross_child, archived_parent],
        "input order is preserved; the archived parent resolves from the per-id tier even though its spawns edge is archived, and the unknown id drops"
    );
}

#[test]
fn test_resolve_relationship_cards_with_no_ids_returns_empty() {
    let mut app = App::test_default();
    let (board_id, column_id) = create_board_and_column(&mut app, "Board");
    for i in 0..50 {
        create_card(&mut app, board_id, column_id, &format!("Card {i}"));
    }
    app.selection.active_board_id = Some(board_id);
    app.reload_model();

    assert_eq!(
        app.model
            .board_cards_state(board_id)
            .loaded()
            .map(|v| v.len())
            .unwrap_or(0),
        50
    );

    let resolved = resolve_relationship_cards(&app.model, &[]);
    assert!(resolved.is_empty());
}

#[test]
fn test_card_detail_children_box_shows_a_cross_board_child_whose_body_was_fetched() {
    use kanban_tui::app::mode::AppMode;

    let mut app = App::test_default();
    let (board_b_id, column_b_id) = create_board_and_column(&mut app, "Board B");
    let (board_c_id, column_c_id) = create_board_and_column(&mut app, "Board C");

    let subject = create_card(&mut app, board_b_id, column_b_id, "Subject");
    let cross = create_card(&mut app, board_c_id, column_c_id, "CrossBoardChildXYZ");
    let decoy = create_card(&mut app, board_b_id, column_b_id, "UnrelatedDecoyABC");
    let _ = decoy;

    app.ctx.attach_child(subject, cross).unwrap();
    app.reload_model();
    app.selection.active_board_id = Some(board_b_id);
    app.selection.active_card_id = Some(subject);
    app.push_mode(AppMode::CardDetail);
    warm_graph(&mut app);
    warm_card_body(&mut app, cross);
    app.relationship.children_list.update_item_count(1);

    let output =
        helpers::render_widget_to_string(120, 40, |frame| kanban_tui::ui::render(&mut app, frame));

    assert!(output.contains("CrossBoardChildXYZ"));
    assert!(!output.contains("UnrelatedDecoyABC"));
}

#[test]
fn test_manage_parents_refuses_to_open_when_the_graph_is_not_loaded() {
    let mut app = App::test_default();
    let (board_id, column_id) = create_board_and_column(&mut app, "Board");
    let subject = create_card(&mut app, board_id, column_id, "Subject");
    app.selection.active_board_id = Some(board_id);
    app.reload_model();
    app.selection.active_card_id = Some(subject);

    invalidate_graph(&mut app);

    app.handle_manage_parents();

    let banner = app.ui_state.banner.expect("expected an error banner");
    let message = banner.message.to_lowercase();
    assert!(message.contains("relationship") || message.contains("loading"));
    assert_ne!(app.mode, AppMode::Dialog(DialogMode::ManageParents));
}

#[test]
fn test_manage_children_refuses_to_open_when_the_graph_is_not_loaded() {
    let mut app = App::test_default();
    let (board_id, column_id) = create_board_and_column(&mut app, "Board");
    let subject = create_card(&mut app, board_id, column_id, "Subject");
    app.selection.active_board_id = Some(board_id);
    app.reload_model();
    app.selection.active_card_id = Some(subject);

    invalidate_graph(&mut app);

    app.handle_manage_children();

    let banner = app.ui_state.banner.expect("expected an error banner");
    let message = banner.message.to_lowercase();
    assert!(message.contains("relationship") || message.contains("loading"));
    assert_ne!(app.mode, AppMode::Dialog(DialogMode::ManageChildren));
}

#[test]
fn test_manage_children_from_list_repopulates_and_opens_when_the_graph_is_not_loaded() {
    let mut app = App::test_default();
    let (board_id, column_id) = create_board_and_column(&mut app, "Board");
    let subject = create_card(&mut app, board_id, column_id, "Subject");
    app.reload_model();
    app.selection.active_board_id = Some(board_id);
    app.push_mode(AppMode::Normal);
    app.prepare_frame();
    if let Some(list) = app.view.strategy.get_active_task_list_mut() {
        list.set_selected_index(Some(0));
    }
    assert_eq!(
        app.get_selected_card_in_context().map(|c| c.id),
        Some(subject),
        "fixture must select the card before the graph is invalidated"
    );

    invalidate_graph(&mut app);

    app.handle_manage_children_from_list();

    assert_eq!(
        app.mode,
        AppMode::Dialog(DialogMode::ManageChildren),
        "opening the dialog first lets the mode-transition populate repair the graph, so the handler no longer dead-ends"
    );
    assert!(app.ui_state.banner.is_none());
}

#[test]
fn test_manage_parents_does_not_offer_a_descendant_when_the_graph_is_not_loaded() {
    let mut app = App::test_default();
    let (board_id, column_id) = create_board_and_column(&mut app, "Board");
    let a = create_card(&mut app, board_id, column_id, "A");
    let b = create_card(&mut app, board_id, column_id, "B");
    app.ctx.attach_child(a, b).unwrap();
    app.selection.active_board_id = Some(board_id);
    app.reload_model();
    app.selection.active_card_id = Some(a);

    app.relationship.card_ids = vec![Uuid::new_v4()];

    invalidate_graph(&mut app);

    app.handle_manage_parents();

    assert_ne!(
        app.mode,
        AppMode::Dialog(DialogMode::ManageParents),
        "the dialog must refuse to open on an unloaded graph"
    );
    assert!(
        !app.relationship.card_ids.contains(&b),
        "descendant B must never be offered as a legal parent of A"
    );
}

#[test]
fn test_an_existing_parent_is_pre_selected_so_toggling_it_detaches() {
    let mut app = App::test_default();
    let (board_id, column_id) = create_board_and_column(&mut app, "Board");
    let p = create_card(&mut app, board_id, column_id, "Parent");
    let c = create_card(&mut app, board_id, column_id, "Child");
    app.ctx.attach_child(p, c).unwrap();
    app.selection.active_board_id = Some(board_id);
    app.reload_model();
    app.selection.active_card_id = Some(c);
    warm_graph(&mut app);

    app.handle_manage_parents();

    assert_eq!(app.mode, AppMode::Dialog(DialogMode::ManageParents));
    assert!(app.relationship.selected.contains(&p));
}

#[test]
fn test_manage_parents_still_opens_and_filters_when_the_graph_is_loaded() {
    let mut app = App::test_default();
    let (board_id, column_id) = create_board_and_column(&mut app, "Board");
    let a = create_card(&mut app, board_id, column_id, "A");
    let b = create_card(&mut app, board_id, column_id, "B");
    app.ctx.attach_child(a, b).unwrap();
    app.selection.active_board_id = Some(board_id);
    app.reload_model();
    app.selection.active_card_id = Some(a);
    warm_graph(&mut app);

    app.handle_manage_parents();

    assert_eq!(app.mode, AppMode::Dialog(DialogMode::ManageParents));
    assert!(!app.relationship.card_ids.contains(&b));
}

#[test]
fn test_a_cross_board_graph_neighbour_body_is_fetched_for_card_detail() {
    let mut app = App::test_default();
    let (board_b_id, column_b_id) = create_board_and_column(&mut app, "Board B");
    let (board_c_id, column_c_id) = create_board_and_column(&mut app, "Board C");

    let subject = create_card(&mut app, board_b_id, column_b_id, "Subject");
    let cross = create_card(&mut app, board_c_id, column_c_id, "CrossBoardChildXYZ");
    app.ctx.attach_child(subject, cross).unwrap();

    app.selection.active_board_id = Some(board_b_id);
    app.selection.active_card_id = Some(subject);
    app.mode = AppMode::CardDetail;
    app.reload_model();

    let graph = app
        .model
        .graph_state()
        .loaded()
        .unwrap_or_else(|| Model::empty_graph());
    let children = graph.children(subject);

    let resolved = resolve_relationship_cards(&app.model, &children);
    assert_eq!(resolved.len(), 1);
    assert_eq!(resolved[0].id, cross);
    assert_eq!(resolved[0].title, "CrossBoardChildXYZ");
}
