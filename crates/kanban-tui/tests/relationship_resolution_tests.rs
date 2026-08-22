mod helpers;

use kanban_domain::{CreateCardOptions, GraphOperations, KanbanOperations};
use kanban_tui::components::resolve_relationship_cards;
use kanban_tui::App;
use kanban_view::model::Model;
use uuid::Uuid;

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
    app.reload_model();

    assert_eq!(app.model.cards_state().loaded_or_empty().len(), 50);

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
    app.reload_model();

    assert!(app.model.archived_card_ids().contains(&parent));
    assert!(!app.model.live_cards().iter().any(|c| c.id == parent));

    let resolved = resolve_relationship_cards(&app.model, &[parent]);

    assert_eq!(resolved.len(), 1);
    assert_eq!(resolved[0].id, parent);
    assert_eq!(resolved[0].title, "ArchivedParentXYZ");
}

#[test]
fn test_resolve_relationship_cards_resolves_cross_board_related_card() {
    let mut app = App::test_default();
    let (board_b_id, column_b_id) = create_board_and_column(&mut app, "Board B");
    let (board_c_id, column_c_id) = create_board_and_column(&mut app, "Board C");

    let subject = create_card(&mut app, board_b_id, column_b_id, "Subject");
    let cross = create_card(&mut app, board_c_id, column_c_id, "CrossBoardChildXYZ");

    app.ctx.attach_child(subject, cross).unwrap();
    app.reload_model();
    app.selection.active_board_id = Some(board_b_id);

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
    app.reload_model();

    let unknown = Uuid::new_v4();
    let resolved = resolve_relationship_cards(&app.model, &[known, unknown]);

    let resolved_ids: Vec<Uuid> = resolved.iter().map(|c| c.id).collect();
    assert_eq!(resolved_ids, vec![known]);
}

#[test]
fn test_resolve_relationship_cards_resolves_same_set_as_full_collection_scan() {
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
    app.reload_model();

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

    let expected: Vec<Uuid> = ids
        .iter()
        .filter_map(|id| {
            app.model
                .cards_state()
                .loaded_or_empty()
                .iter()
                .find(|c| c.id == *id)
                .cloned()
        })
        .map(|c| c.id)
        .collect();
    assert_eq!(expected.len(), 3);

    let actual: Vec<Uuid> = resolve_relationship_cards(&app.model, &ids)
        .iter()
        .map(|c| c.id)
        .collect();

    assert_eq!(actual, expected);
}

#[test]
fn test_resolve_relationship_cards_with_no_ids_returns_empty() {
    let mut app = App::test_default();
    let (board_id, column_id) = create_board_and_column(&mut app, "Board");
    for i in 0..50 {
        create_card(&mut app, board_id, column_id, &format!("Card {i}"));
    }
    app.reload_model();

    assert_eq!(app.model.cards_state().loaded_or_empty().len(), 50);

    let resolved = resolve_relationship_cards(&app.model, &[]);
    assert!(resolved.is_empty());
}

#[test]
fn test_card_detail_children_box_shows_cross_board_child_title() {
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
    app.relationship.children_list.update_item_count(1);

    let output =
        helpers::render_widget_to_string(120, 40, |frame| kanban_tui::ui::render(&mut app, frame));

    assert!(output.contains("CrossBoardChildXYZ"));
    assert!(!output.contains("UnrelatedDecoyABC"));
}
