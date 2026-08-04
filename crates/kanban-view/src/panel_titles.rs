use crate::filter_state::FilterState;
use crate::model::Model;
use kanban_domain::Board;

/// Builds the " - Unassigned Cards + sprint-1/Foo" suffix appended to the
/// tasks-panel title when filters are active. Returns `None` when no filter
/// is active. `board` is `None` when no board is currently open (browsing
/// the projects list); in that case any active sprint filters are silently
/// skipped, matching the pre-move behavior which could only resolve sprint
/// names against an active board.
pub fn build_filter_title_suffix(
    filter: &FilterState,
    model: &Model,
    board: Option<&Board>,
) -> Option<String> {
    let mut filters = vec![];

    if filter.hide_assigned_cards {
        filters.push("Unassigned Cards".to_string());
    }

    if !filter.active_sprint_filters.is_empty() {
        if let Some(board) = board {
            let mut sprint_names: Vec<String> = model
                .sprints()
                .iter()
                .filter(|s| filter.active_sprint_filters.contains(&s.id))
                .map(|s| s.formatted_name(board, "sprint"))
                .collect();
            sprint_names.sort();
            filters.extend(sprint_names);
        }
    }

    if filters.is_empty() {
        None
    } else {
        Some(format!(" - {}", filters.join(" + ")))
    }
}

/// Builds the tasks-panel title. Callers compute the App-native primitives
/// (active task list length, archived-board/archived-cards/focus flags) and
/// pass them in; this function owns only the string-formatting branching,
/// not any App-coupled state resolution.
#[allow(clippy::too_many_arguments)]
pub fn build_tasks_panel_title(
    active_task_list_len: usize,
    viewing_archived_board: bool,
    viewing_archived_cards: bool,
    focus_is_cards: bool,
    with_filter_suffix: bool,
    filter: &FilterState,
    model: &Model,
    board: Option<&Board>,
) -> String {
    let mut title = if viewing_archived_cards {
        format!("Archive [{}]", active_task_list_len)
    } else if viewing_archived_board {
        format!("[ARCHIVED] Tasks [2] ({})", active_task_list_len)
    } else if focus_is_cards {
        format!("Tasks [2] ({})", active_task_list_len)
    } else {
        "Tasks".to_string()
    };

    if with_filter_suffix && !viewing_archived_cards {
        if let Some(suffix) = build_filter_title_suffix(filter, model, board) {
            title.push_str(&suffix);
        }
    }

    title
}

#[cfg(test)]
mod tests {
    use super::*;
    use kanban_domain::{Board, DependencyGraph, Snapshot, Sprint};

    /// Builds a board plus `names.len()` sprints on it (each named from
    /// `names`, in order), and a `Model` loaded from a snapshot containing
    /// exactly that board and those sprints — pure kanban-domain
    /// construction, no kanban-service dependency (kanban-view must not
    /// depend on it).
    fn board_with_sprints(names: &[&str]) -> (Board, Model) {
        let mut board = Board::new("Test Board", None::<String>);
        let sprints: Vec<Sprint> = names
            .iter()
            .map(|name| {
                let name_index = board.add_sprint_name_at_used_index(*name);
                let number = board.get_next_sprint_number("sprint");
                Sprint::new(board.id, number, Some(name_index), None::<String>)
            })
            .collect();

        let mut model = Model::default();
        model.load_from_snapshot(Snapshot::from_data(
            vec![board.clone()],
            vec![],
            vec![],
            vec![],
            sprints,
            DependencyGraph::default(),
        ));
        (board, model)
    }

    #[test]
    fn test_build_filter_title_suffix_no_filters_returns_none() {
        let filter = FilterState::default();
        let model = Model::default();
        assert_eq!(build_filter_title_suffix(&filter, &model, None), None);
    }

    #[test]
    fn test_build_filter_title_suffix_unassigned_cards_flag() {
        let mut filter = FilterState::default();
        filter.hide_assigned_cards = true;
        let model = Model::default();
        assert_eq!(
            build_filter_title_suffix(&filter, &model, None),
            Some(" - Unassigned Cards".to_string())
        );
    }

    #[test]
    fn test_build_filter_title_suffix_sprint_filter_formats_sprint_name() {
        let (board, model) = board_with_sprints(&["Sprint"]);
        let mut filter = FilterState::default();
        filter.active_sprint_filters.insert(model.sprints()[0].id);

        let suffix = build_filter_title_suffix(&filter, &model, Some(&board));
        assert!(
            suffix.is_some(),
            "Expected Some suffix with active sprint filter"
        );
        let suffix = suffix.unwrap();
        assert!(suffix.starts_with(" - "), "Suffix should start with ' - '");
        assert!(
            suffix.contains("Sprint"),
            "Suffix should contain sprint name"
        );
    }

    #[test]
    fn test_build_filter_title_suffix_multiple_sprint_filters_sorted_and_joined() {
        let (board, model) = board_with_sprints(&["Sprint A", "Sprint B"]);
        let mut filter = FilterState::default();
        filter.active_sprint_filters.insert(model.sprints()[0].id);
        filter.active_sprint_filters.insert(model.sprints()[1].id);

        let suffix = build_filter_title_suffix(&filter, &model, Some(&board));
        assert_eq!(
            suffix,
            Some(" - sprint-1/Sprint A + sprint-2/Sprint B".to_string()),
            "multiple sprint filters must be sorted and joined with ' + '"
        );
    }

    #[test]
    fn test_build_filter_title_suffix_sprint_filter_without_board_is_skipped() {
        let mut filter = FilterState::default();
        filter
            .active_sprint_filters
            .insert(uuid::Uuid::new_v4());
        let model = Model::default();
        assert_eq!(
            build_filter_title_suffix(&filter, &model, None),
            None,
            "sprint filters can't be resolved to names without an active board"
        );
    }

    #[test]
    fn test_build_tasks_panel_title_default_focus_not_cards() {
        let filter = FilterState::default();
        let model = Model::default();
        assert_eq!(
            build_tasks_panel_title(0, false, false, false, false, &filter, &model, None),
            "Tasks"
        );
    }

    #[test]
    fn test_build_tasks_panel_title_archived_cards_view() {
        let filter = FilterState::default();
        let model = Model::default();
        assert_eq!(
            build_tasks_panel_title(0, false, true, false, false, &filter, &model, None),
            "Archive [0]"
        );
    }

    #[test]
    fn test_build_tasks_panel_title_archived_board() {
        let filter = FilterState::default();
        let model = Model::default();
        assert_eq!(
            build_tasks_panel_title(0, true, false, false, false, &filter, &model, None),
            "[ARCHIVED] Tasks [2] (0)"
        );
    }

    #[test]
    fn test_build_tasks_panel_title_cards_focus() {
        let filter = FilterState::default();
        let model = Model::default();
        assert_eq!(
            build_tasks_panel_title(0, false, false, true, false, &filter, &model, None),
            "Tasks [2] (0)",
            "empty board should show shortcut hint [2] and count (0)"
        );
    }

    #[test]
    fn test_build_tasks_panel_title_with_filter_suffix() {
        let mut filter = FilterState::default();
        filter.hide_assigned_cards = true;
        let model = Model::default();
        let title = build_tasks_panel_title(0, false, false, false, true, &filter, &model, None);
        assert!(
            title.ends_with(" - Unassigned Cards"),
            "Expected title to end with filter suffix, got: {}",
            title
        );
    }

    #[test]
    fn test_build_tasks_panel_title_archived_ignores_filter_suffix() {
        let mut filter = FilterState::default();
        filter.hide_assigned_cards = true;
        let model = Model::default();
        assert_eq!(
            build_tasks_panel_title(0, false, true, false, true, &filter, &model, None),
            "Archive [0]"
        );
    }
}
