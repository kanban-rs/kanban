use crate::filter_state::FilterState;
use crate::model::Model;
use kanban_domain::Board;
use serde::{Deserialize, Serialize};

/// Which tasks panel a title describes. The renderer decides how each kind
/// is spelled and whether it shows `count`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TasksPanelKind {
    /// The archived-cards view.
    Archive,
    /// A live tasks list whose board head is archived.
    ArchivedBoardTasks,
    /// The tasks list while the cards panel holds focus.
    FocusedTasks,
    /// The tasks list while focus is elsewhere.
    UnfocusedTasks,
}

/// Everything a renderer needs to title the tasks panel: which panel it is,
/// how many entries it holds, and the active filter labels. Carries no
/// separators, no counts baked into prose, and no keyboard hints.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TasksPanelTitle {
    pub kind: TasksPanelKind,
    pub count: usize,
    pub filters: Vec<String>,
}

/// The active filter labels, e.g. `["Unassigned Cards", "sprint-1/Foo"]`.
/// Empty when no filter is active. `board` is `None` when no board is
/// currently open (browsing the projects list); in that case any active
/// sprint filters are silently skipped, since sprint names can only be
/// resolved against a board.
pub fn build_filter_title_parts(
    filter: &FilterState,
    model: &Model,
    board: Option<&Board>,
) -> Vec<String> {
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
                .map(|s| s.formatted_name(board, None))
                .collect();
            sprint_names.sort();
            filters.extend(sprint_names);
        }
    }

    filters
}

/// Builds the tasks-panel title data. Callers compute the App-native
/// primitives (active task list length, archived-board/archived-cards/focus
/// flags) and pass them in; this function owns only the branching, not any
/// App-coupled state resolution and not the rendering.
#[allow(clippy::too_many_arguments)]
pub fn build_tasks_panel_title(
    active_task_list_len: usize,
    viewing_archived_board: bool,
    viewing_archived_cards: bool,
    focus_is_cards: bool,
    with_filters: bool,
    filter: &FilterState,
    model: &Model,
    board: Option<&Board>,
) -> TasksPanelTitle {
    let kind = if viewing_archived_cards {
        TasksPanelKind::Archive
    } else if viewing_archived_board {
        TasksPanelKind::ArchivedBoardTasks
    } else if focus_is_cards {
        TasksPanelKind::FocusedTasks
    } else {
        TasksPanelKind::UnfocusedTasks
    };

    let filters = if with_filters && !viewing_archived_cards {
        build_filter_title_parts(filter, model, board)
    } else {
        vec![]
    };

    TasksPanelTitle {
        kind,
        count: active_task_list_len,
        filters,
    }
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
            .enumerate()
            .map(|(i, name)| {
                let name_index = board.add_sprint_name_at_used_index(*name);
                let number = i as u32 + 1;
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
    fn test_build_filter_title_parts_no_filters_returns_empty() {
        let filter = FilterState::default();
        let model = Model::default();
        assert!(build_filter_title_parts(&filter, &model, None).is_empty());
    }

    #[test]
    fn test_build_filter_title_parts_unassigned_cards_flag() {
        let filter = FilterState {
            hide_assigned_cards: true,
            ..Default::default()
        };
        let model = Model::default();
        assert_eq!(
            build_filter_title_parts(&filter, &model, None),
            vec!["Unassigned Cards".to_string()]
        );
    }

    #[test]
    fn test_build_filter_title_parts_returns_bare_labels_without_separators() {
        let filter = FilterState {
            hide_assigned_cards: true,
            ..Default::default()
        };
        let model = Model::default();
        for part in build_filter_title_parts(&filter, &model, None) {
            assert!(
                !part.starts_with(" - ") && !part.contains(" + "),
                "parts must carry no assembled separators: {}",
                part
            );
        }
    }

    #[test]
    fn test_build_filter_title_parts_sprint_filter_formats_sprint_name() {
        let (board, model) = board_with_sprints(&["Sprint"]);
        let mut filter = FilterState::default();
        filter.active_sprint_filters.insert(model.sprints()[0].id);

        let parts = build_filter_title_parts(&filter, &model, Some(&board));
        assert_eq!(parts.len(), 1, "one active sprint filter yields one label");
        assert!(
            parts[0].contains("Sprint"),
            "label should contain the sprint name"
        );
    }

    #[test]
    fn test_build_filter_title_parts_multiple_sprint_filters_sorted() {
        let (board, model) = board_with_sprints(&["Sprint A", "Sprint B"]);
        let mut filter = FilterState::default();
        filter.active_sprint_filters.insert(model.sprints()[0].id);
        filter.active_sprint_filters.insert(model.sprints()[1].id);

        assert_eq!(
            build_filter_title_parts(&filter, &model, Some(&board)),
            vec![
                "sprint-1/Sprint A".to_string(),
                "sprint-2/Sprint B".to_string()
            ],
            "multiple sprint filters must come back sorted"
        );
    }

    #[test]
    fn test_build_filter_title_parts_sprint_filter_without_board_is_skipped() {
        let mut filter = FilterState::default();
        filter.active_sprint_filters.insert(uuid::Uuid::new_v4());
        let model = Model::default();
        assert!(
            build_filter_title_parts(&filter, &model, None).is_empty(),
            "sprint filters can't be resolved to names without an active board"
        );
    }

    #[test]
    fn test_build_tasks_panel_title_default_focus_not_cards() {
        let filter = FilterState::default();
        let model = Model::default();
        let title = build_tasks_panel_title(0, false, false, false, false, &filter, &model, None);
        assert_eq!(title.kind, TasksPanelKind::UnfocusedTasks);
        assert!(title.filters.is_empty());
    }

    #[test]
    fn test_build_tasks_panel_title_archived_cards_view() {
        let filter = FilterState::default();
        let model = Model::default();
        let title = build_tasks_panel_title(3, false, true, false, false, &filter, &model, None);
        assert_eq!(title.kind, TasksPanelKind::Archive);
        assert_eq!(title.count, 3);
    }

    #[test]
    fn test_build_tasks_panel_title_archived_board_takes_precedence_over_focus() {
        let filter = FilterState::default();
        let model = Model::default();
        let title = build_tasks_panel_title(5, true, false, false, false, &filter, &model, None);
        assert_eq!(title.kind, TasksPanelKind::ArchivedBoardTasks);
        assert_eq!(title.count, 5);
    }

    #[test]
    fn test_build_tasks_panel_title_cards_focus() {
        let filter = FilterState::default();
        let model = Model::default();
        let title = build_tasks_panel_title(0, false, false, true, false, &filter, &model, None);
        assert_eq!(title.kind, TasksPanelKind::FocusedTasks);
        assert_eq!(title.count, 0);
    }

    #[test]
    fn test_build_tasks_panel_title_with_filters_carries_labels() {
        let filter = FilterState {
            hide_assigned_cards: true,
            ..Default::default()
        };
        let model = Model::default();
        let title = build_tasks_panel_title(0, false, false, false, true, &filter, &model, None);
        assert_eq!(title.filters, vec!["Unassigned Cards".to_string()]);
    }

    #[test]
    fn test_build_tasks_panel_title_archived_cards_view_drops_filters() {
        let filter = FilterState {
            hide_assigned_cards: true,
            ..Default::default()
        };
        let model = Model::default();
        let title = build_tasks_panel_title(0, false, true, false, true, &filter, &model, None);
        assert_eq!(title.kind, TasksPanelKind::Archive);
        assert!(
            title.filters.is_empty(),
            "the archived-cards view never shows the filter summary"
        );
    }

    #[test]
    fn test_build_tasks_panel_title_carries_no_terminal_hotkey_hint() {
        let filter = FilterState::default();
        let model = Model::default();
        let title = build_tasks_panel_title(0, false, false, true, false, &filter, &model, None);
        assert!(
            !format!("{:?}", title).contains("[2]"),
            "the [2] panel hotkey is a terminal concern, not view data"
        );
    }
}
