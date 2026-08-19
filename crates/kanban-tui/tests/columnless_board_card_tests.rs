use crossterm::event::KeyCode;
use kanban_domain::{CardStatus, KanbanOperations};
use kanban_tui::app::dialog_input::CreateCardFocus;
use kanban_tui::app::focus::Focus;
use kanban_tui::view_strategy::UnifiedViewStrategy;
use kanban_tui::App;
use kanban_view::card_list::CardListId;
use ratatui::backend::TestBackend;
use ratatui::style::Color;
use ratatui::Terminal;

fn setup_app_without_columns() -> App {
    let mut app = App::test_default();
    let board = app.ctx.create_board("Board".to_string(), None).unwrap();
    app.reload_model();
    app.prepare_frame();
    app.board_list.inner_mut().set_selected_index(Some(0));
    app.selection.active_board_id = Some(board.id);
    app
}

fn setup_app_with_two_columns() -> App {
    let mut app = App::test_default();
    let board = app.ctx.create_board("Board".to_string(), None).unwrap();
    let _c1 = app
        .ctx
        .create_column(board.id, "Backlog".to_string(), Some(0))
        .unwrap();
    let _c2 = app
        .ctx
        .create_column(board.id, "Doing".to_string(), Some(1))
        .unwrap();
    app.reload_model();
    app.prepare_frame();
    app.board_list.inner_mut().set_selected_index(Some(0));
    app.selection.active_board_id = Some(board.id);
    app
}

fn render_to_string(app: &mut App, width: u16, height: u16) -> String {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| {
            kanban_tui::ui::render(app, frame);
        })
        .unwrap();
    let buffer = terminal.backend().buffer().clone();
    let mut result = String::new();
    for y in 0..buffer.area.height {
        for x in 0..buffer.area.width {
            result.push_str(buffer.cell((x, y)).map(|c| c.symbol()).unwrap_or(" "));
        }
        result.push('\n');
    }
    result
}

fn render_with_colors(app: &mut App, width: u16, height: u16) -> Vec<(String, Option<Color>)> {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| {
            kanban_tui::ui::render(app, frame);
        })
        .unwrap();
    let buffer = terminal.backend().buffer().clone();
    let mut result = Vec::new();
    for y in 0..buffer.area.height {
        for x in 0..buffer.area.width {
            let cell = buffer.cell((x, y)).unwrap();
            result.push((cell.symbol().to_string(), cell.style().fg));
        }
    }
    result
}

fn row_containing(grid: &str, substring: &str) -> Option<usize> {
    grid.lines()
        .enumerate()
        .find(|(_, line)| line.contains(substring))
        .map(|(i, _)| i)
}

fn color_of_row(
    grid: &[(String, Option<Color>)],
    width: usize,
    _height: usize,
    row: usize,
    substring: &str,
) -> Option<Color> {
    let line: String = (0..width)
        .map(|x| grid[row * width + x].0.clone())
        .collect();
    let start = line.find(substring)?;
    let char_start = line[..start].chars().count();
    grid[row * width + char_start].1
}

#[test]
fn test_the_column_field_is_editable_and_prefilled_with_the_template_name_on_a_columnless_board() {
    let mut app = setup_app_without_columns();
    app.focus.active = Focus::Cards;
    app.handle_create_card_key();

    assert_eq!(
        app.dialog_input.create_card_column_input.as_str(),
        kanban_domain::DEFAULT_TEMPLATE_COLUMNS[0].0
    );
    assert!(app.dialog_input.create_card_column_is_editable());
}

#[test]
fn test_the_column_field_is_disabled_and_names_the_destination_column_when_the_board_has_columns() {
    let mut app = setup_app_with_two_columns();
    app.focus.active = Focus::Cards;
    app.handle_create_card_key();

    assert!(!app.dialog_input.create_card_column_is_editable());
    assert_eq!(
        app.dialog_input.create_card_column_input.as_str(),
        "Backlog"
    );
}

#[test]
fn test_the_column_field_names_the_column_the_card_actually_lands_in() {
    // (a) board with columns, focused on the SECOND column's card list
    let mut app = setup_app_with_two_columns();
    app.focus.active = Focus::Cards;
    app.view.strategy = Box::new(UnifiedViewStrategy::kanban());
    app.prepare_frame();
    assert!(app.view.strategy.navigate_right(false));
    assert_eq!(
        app.view
            .strategy
            .get_active_task_list()
            .map(|l| l.id.clone()),
        Some(CardListId::Column(
            app.model
                .columns()
                .iter()
                .find(|c| c.name == "Doing")
                .unwrap()
                .id
        ))
    );
    app.handle_create_card_key();
    let snapshot = app
        .dialog_input
        .create_card_column_input
        .as_str()
        .to_string();
    assert_eq!(
        snapshot, "Doing",
        "the field must name the focused (second) column, not fall back to the first"
    );
    for ch in "Task".chars() {
        app.handle_create_card_dialog(KeyCode::Char(ch));
    }
    app.handle_create_card_dialog(KeyCode::Enter);
    app.reload_model();
    let card = app
        .model
        .all_cards()
        .iter()
        .find(|c| c.title == "Task")
        .expect("card created")
        .clone();
    let col = app
        .model
        .columns()
        .iter()
        .find(|c| c.id == card.column_id)
        .expect("column exists");
    assert_eq!(col.name, snapshot);

    // (b) columnless board
    let mut app2 = setup_app_without_columns();
    app2.focus.active = Focus::Cards;
    app2.handle_create_card_key();
    let snapshot2 = app2
        .dialog_input
        .create_card_column_input
        .as_str()
        .to_string();
    for ch in "Task2".chars() {
        app2.handle_create_card_dialog(KeyCode::Char(ch));
    }
    app2.handle_create_card_dialog(KeyCode::Enter);
    app2.reload_model();
    let cols2 = app2.model.columns();
    assert_eq!(cols2.len(), 1);
    assert_eq!(cols2[0].name, snapshot2);
}

#[test]
fn test_focus_never_lands_on_the_column_field_when_the_board_has_columns() {
    let mut app = setup_app_with_two_columns();
    app.focus.active = Focus::Cards;
    app.handle_create_card_key();

    for _ in 0..6 {
        app.handle_create_card_dialog(KeyCode::Tab);
        assert_ne!(app.dialog_input.create_card_focus, CreateCardFocus::Column);
    }
}

#[test]
fn test_focus_cycles_title_then_column_then_sprint_on_a_columnless_board() {
    let mut app = setup_app_without_columns();
    app.focus.active = Focus::Cards;
    app.handle_create_card_key();

    assert_eq!(app.dialog_input.create_card_focus, CreateCardFocus::Title);
    app.handle_create_card_dialog(KeyCode::Tab);
    assert_eq!(app.dialog_input.create_card_focus, CreateCardFocus::Column);
    app.handle_create_card_dialog(KeyCode::Tab);
    assert_eq!(app.dialog_input.create_card_focus, CreateCardFocus::Sprint);
    app.handle_create_card_dialog(KeyCode::Tab);
    assert_eq!(app.dialog_input.create_card_focus, CreateCardFocus::Title);
}

#[test]
fn test_typing_in_the_column_field_edits_it_and_leaves_the_title_alone() {
    let mut app = setup_app_without_columns();
    app.focus.active = Focus::Cards;
    app.handle_create_card_key();

    for ch in "Task".chars() {
        app.handle_create_card_dialog(KeyCode::Char(ch));
    }
    app.handle_create_card_dialog(KeyCode::Tab);
    for _ in 0..4 {
        app.handle_create_card_dialog(KeyCode::Backspace);
    }
    for ch in "Inbox".chars() {
        app.handle_create_card_dialog(KeyCode::Char(ch));
    }

    assert_eq!(app.dialog_input.create_card_column_input.as_str(), "Inbox");
    assert_eq!(app.input.as_str(), "Task");
}

#[test]
fn test_typing_cannot_change_the_column_field_while_it_is_disabled() {
    let mut app = setup_app_with_two_columns();
    app.focus.active = Focus::Cards;
    app.handle_create_card_key();

    app.handle_create_card_dialog(KeyCode::Tab);
    app.handle_create_card_dialog(KeyCode::Char('x'));
    app.handle_create_card_dialog(KeyCode::Char('y'));
    app.handle_create_card_dialog(KeyCode::Char('z'));
    app.handle_create_card_dialog(KeyCode::Tab);
    app.handle_create_card_dialog(KeyCode::Backspace);
    app.handle_create_card_dialog(KeyCode::Tab);
    app.handle_create_card_dialog(KeyCode::Char('q'));

    assert_eq!(
        app.dialog_input.create_card_column_input.as_str(),
        "Backlog"
    );
}

#[test]
fn test_creating_a_card_on_a_columnless_board_creates_the_template_named_column_with_its_default_status(
) {
    let mut app = setup_app_without_columns();
    app.focus.active = Focus::Cards;
    app.handle_create_card_key();
    for ch in "Probe".chars() {
        app.handle_create_card_dialog(KeyCode::Char(ch));
    }
    app.handle_create_card_dialog(KeyCode::Enter);
    app.reload_model();

    let cols = app.model.columns();
    assert_eq!(cols.len(), 1);
    assert_eq!(cols[0].name, "TODO");
    assert_eq!(cols[0].default_status, Some(CardStatus::Todo));
}

#[test]
fn test_an_edited_column_name_is_used_for_the_created_column() {
    let mut app = setup_app_without_columns();
    app.focus.active = Focus::Cards;
    app.handle_create_card_key();
    for ch in "Probe".chars() {
        app.handle_create_card_dialog(KeyCode::Char(ch));
    }
    app.handle_create_card_dialog(KeyCode::Tab);
    for _ in 0..app
        .dialog_input
        .create_card_column_input
        .as_str()
        .chars()
        .count()
    {
        app.handle_create_card_dialog(KeyCode::Backspace);
    }
    for ch in "Inbox".chars() {
        app.handle_create_card_dialog(KeyCode::Char(ch));
    }
    app.handle_create_card_dialog(KeyCode::Enter);
    app.reload_model();

    let cols = app.model.columns();
    assert_eq!(cols.len(), 1);
    assert_eq!(cols[0].name, "Inbox");
    assert_eq!(cols[0].default_status, Some(CardStatus::Todo));
    let card = app
        .model
        .all_cards()
        .iter()
        .find(|c| c.title == "Probe")
        .unwrap();
    assert_eq!(card.column_id, cols[0].id);
}

#[test]
fn test_an_emptied_column_name_falls_back_to_the_template_name() {
    let mut app = setup_app_without_columns();
    app.focus.active = Focus::Cards;
    app.handle_create_card_key();
    for ch in "Probe".chars() {
        app.handle_create_card_dialog(KeyCode::Char(ch));
    }
    app.handle_create_card_dialog(KeyCode::Tab);
    for _ in 0..app
        .dialog_input
        .create_card_column_input
        .as_str()
        .chars()
        .count()
    {
        app.handle_create_card_dialog(KeyCode::Backspace);
    }
    app.handle_create_card_dialog(KeyCode::Enter);
    app.reload_model();

    let cols = app.model.columns();
    assert_eq!(cols.len(), 1);
    assert_eq!(cols[0].name, "TODO");
    let card = app
        .model
        .all_cards()
        .iter()
        .find(|c| c.title == "Probe")
        .unwrap();
    assert_eq!(card.column_id, cols[0].id);
    assert!(app.ui_state.banner.is_none());
}

#[test]
fn test_undoing_the_card_create_also_removes_the_invented_column() {
    let mut app = setup_app_without_columns();
    app.focus.active = Focus::Cards;
    app.handle_create_card_key();
    for ch in "Probe".chars() {
        app.handle_create_card_dialog(KeyCode::Char(ch));
    }
    app.handle_create_card_dialog(KeyCode::Enter);
    app.reload_model();
    assert_eq!(app.model.all_cards().len(), 1);
    assert_eq!(app.model.columns().len(), 1);

    app.ctx.undo().unwrap();
    app.reload_model();

    assert!(app.model.all_cards().is_empty());
    assert!(app.model.columns().is_empty());
}

#[test]
fn test_the_create_card_dialog_has_the_same_shape_with_and_without_columns() {
    let mut app_no_cols = setup_app_without_columns();
    app_no_cols.focus.active = Focus::Cards;
    app_no_cols.handle_create_card_key();
    let grid_no_cols = render_to_string(&mut app_no_cols, 120, 40);

    let mut app_cols = setup_app_with_two_columns();
    app_cols.focus.active = Focus::Cards;
    app_cols.handle_create_card_key();
    let grid_cols = render_to_string(&mut app_cols, 120, 40);

    assert!(grid_no_cols.contains("Column:"));
    assert!(grid_cols.contains("Column:"));
    assert!(grid_no_cols.contains("Sprint:"));
    assert!(grid_cols.contains("Sprint:"));

    let sprint_row_no_cols = row_containing(&grid_no_cols, "Sprint:").unwrap();
    let sprint_row_cols = row_containing(&grid_cols, "Sprint:").unwrap();
    assert_eq!(sprint_row_no_cols, sprint_row_cols);
}

#[test]
fn test_the_disabled_column_field_is_greyed_and_the_editable_one_is_not() {
    let width = 120u16;
    let height = 40u16;

    let mut app_no_cols = setup_app_without_columns();
    app_no_cols.focus.active = Focus::Cards;
    app_no_cols.handle_create_card_key();
    let grid_no_cols = render_to_string(&mut app_no_cols, width, height);
    let colors_no_cols = render_with_colors(&mut app_no_cols, width, height);
    let row = row_containing(&grid_no_cols, kanban_domain::DEFAULT_TEMPLATE_COLUMNS[0].0).unwrap();
    let color = color_of_row(
        &colors_no_cols,
        width as usize,
        height as usize,
        row,
        kanban_domain::DEFAULT_TEMPLATE_COLUMNS[0].0,
    );
    assert_eq!(color, Some(Color::White));

    let mut app_cols = setup_app_with_two_columns();
    app_cols.focus.active = Focus::Cards;
    app_cols.handle_create_card_key();
    let grid_cols = render_to_string(&mut app_cols, width, height);
    let colors_cols = render_with_colors(&mut app_cols, width, height);
    let row2 = row_containing(&grid_cols, "Backlog").unwrap();
    let color2 = color_of_row(
        &colors_cols,
        width as usize,
        height as usize,
        row2,
        "Backlog",
    );
    assert_eq!(color2, Some(Color::DarkGray));
}

#[test]
fn test_the_dialog_fits_within_an_80x24_terminal_without_collapsing() {
    let width = 80u16;
    let height = 24u16;

    fn assert_title_input_has_content_row(grid: &str) {
        let title_row = row_containing(grid, "Task Title:").unwrap();
        let lines: Vec<&str> = grid.lines().collect();
        let top_border = lines[title_row + 1];
        let content = lines[title_row + 2];
        let bottom_border = lines[title_row + 3];
        assert!(
            top_border.contains('┌') && top_border.contains('┐'),
            "no top border for the title input box at 80x24:\n{grid}"
        );
        assert!(
            bottom_border.contains('└') && bottom_border.contains('┘'),
            "no bottom border for the title input box at 80x24:\n{grid}"
        );
        assert!(
            !content.contains('┌') && !content.contains('└'),
            "title input box has no distinct content row at 80x24 (top and bottom border collapsed together):\n{grid}"
        );
    }

    let mut app_no_cols = setup_app_without_columns();
    app_no_cols.focus.active = Focus::Cards;
    app_no_cols.handle_create_card_key();
    let grid_no_cols = render_to_string(&mut app_no_cols, width, height);
    assert_title_input_has_content_row(&grid_no_cols);
    assert!(
        grid_no_cols.contains("(None)"),
        "sprint picker did not render its first entry at 80x24:\n{grid_no_cols}"
    );

    let mut app_cols = setup_app_with_two_columns();
    app_cols.focus.active = Focus::Cards;
    app_cols.handle_create_card_key();
    let grid_cols = render_to_string(&mut app_cols, width, height);
    assert_title_input_has_content_row(&grid_cols);
    assert!(
        grid_cols.contains("(None)"),
        "sprint picker did not render its first entry at 80x24:\n{grid_cols}"
    );
}

#[test]
fn test_a_board_with_existing_columns_gains_no_new_column() {
    let mut app = setup_app_with_two_columns();
    app.focus.active = Focus::Cards;
    app.handle_create_card_key();
    for ch in "Task".chars() {
        app.handle_create_card_dialog(KeyCode::Char(ch));
    }
    app.handle_create_card_dialog(KeyCode::Enter);
    app.reload_model();

    let cols = app.model.columns();
    assert_eq!(cols.len(), 2);
    let card = app
        .model
        .all_cards()
        .iter()
        .find(|c| c.title == "Task")
        .unwrap();
    assert_eq!(card.column_id, cols[0].id);
}
