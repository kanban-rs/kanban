use crate::filter_state::FilterState;
use chrono::{DateTime, Utc};
use kanban_domain::Board;
use kanban_domain::Model;
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

/// A tasks-panel entry count, distinguishing a genuinely empty collection
/// from one whose load state means the count cannot be trusted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PanelCount {
    Known(usize),
    NotLoaded,
    Failed,
}

/// Everything a renderer needs to title the tasks panel: which panel it is,
/// how many entries it holds, and the active filter labels. Carries no
/// separators, no counts baked into prose, and no keyboard hints.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TasksPanelTitle {
    pub kind: TasksPanelKind,
    pub count: PanelCount,
    /// How many of the board's sprints are `Active` but past their
    /// `end_date`. `NotLoaded`/`Failed` when the sprint tier can't be
    /// trusted, never collapsed to `Known(0)`.
    pub ended_sprints: PanelCount,
    pub filters: Vec<String>,
}

/// Counts a board's `Active` sprints past their `end_date`. `NotLoaded` when
/// there is no board, or when the sprint tier is `NotLoaded` or `Missing`;
/// `Failed` when the sprint tier failed to load. An untrustworthy tier never
/// reports `Known(0)`: no claim and a claim of none are different answers.
pub fn ended_sprint_count(model: &Model, board: Option<&Board>, now: DateTime<Utc>) -> PanelCount {
    let Some(board) = board else {
        return PanelCount::NotLoaded;
    };
    match model.board_sprints_state(board.id) {
        kanban_domain::LoadState::Loaded(sprints) => {
            PanelCount::Known(sprints.iter().filter(|s| s.is_ended(now)).count())
        }
        kanban_domain::LoadState::Failed(_) => PanelCount::Failed,
        kanban_domain::LoadState::NotLoaded | kanban_domain::LoadState::Missing => {
            PanelCount::NotLoaded
        }
    }
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
            match model.board_sprints_state(board.id) {
                kanban_domain::LoadState::Loaded(sprints) => {
                    let mut sprint_names: Vec<String> = sprints
                        .iter()
                        .filter(|s| filter.active_sprint_filters.contains(&s.id))
                        .map(|s| s.formatted_name(board, None))
                        .collect();
                    sprint_names.sort();
                    filters.extend(sprint_names);
                }
                _ => {
                    filters.push(format!(
                        "{} sprint filter(s)",
                        filter.active_sprint_filters.len()
                    ));
                }
            }
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
    active_task_list: PanelCount,
    ended_sprints: PanelCount,
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
        count: active_task_list,
        ended_sprints,
        filters,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{DateTime, Duration, Utc};
    use kanban_domain::resolved::Collection;
    use kanban_domain::SprintStatus;
    use kanban_domain::{
        Board, DependencyGraph, KanbanError, LoadState, Resolved, Snapshot, Sprint,
    };
    use std::collections::HashMap;
    use std::sync::Arc;

    fn sprint_at(
        board_id: uuid::Uuid,
        n: u32,
        status: SprintStatus,
        end_date: Option<DateTime<Utc>>,
    ) -> Sprint {
        Sprint {
            status,
            end_date,
            ..Sprint::new(board_id, n, None, None::<String>)
        }
    }

    /// Builds a board plus `names.len()` sprints on it (each named from
    /// `names`, in order), and a `Model` loaded from a snapshot containing
    /// exactly that board and those sprints — pure kanban-domain
    /// construction, no kanban-service dependency (kanban-view must not
    /// depend on it).
    fn board_with_sprints(names: &[&str]) -> (Board, Vec<Sprint>, Model) {
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
        let _ = model.load_from_snapshot(Snapshot::from_data(
            vec![board.clone()],
            vec![],
            vec![],
            vec![],
            sprints.clone(),
            DependencyGraph::default(),
        ));
        (board, sprints, model)
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
        let (board, sprints, model) = board_with_sprints(&["Sprint"]);
        let mut filter = FilterState::default();
        filter.active_sprint_filters.insert(sprints[0].id);

        let parts = build_filter_title_parts(&filter, &model, Some(&board));
        assert_eq!(parts.len(), 1, "one active sprint filter yields one label");
        assert!(
            parts[0].contains("Sprint"),
            "label should contain the sprint name"
        );
    }

    #[test]
    fn test_build_filter_title_parts_multiple_sprint_filters_sorted() {
        let (board, sprints, model) = board_with_sprints(&["Sprint A", "Sprint B"]);
        let mut filter = FilterState::default();
        filter.active_sprint_filters.insert(sprints[0].id);
        filter.active_sprint_filters.insert(sprints[1].id);

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
        let title = build_tasks_panel_title(
            PanelCount::Known(0),
            PanelCount::NotLoaded,
            false,
            false,
            false,
            false,
            &filter,
            &model,
            None,
        );
        assert_eq!(title.kind, TasksPanelKind::UnfocusedTasks);
        assert!(title.filters.is_empty());
    }

    #[test]
    fn test_build_tasks_panel_title_archived_cards_view() {
        let filter = FilterState::default();
        let model = Model::default();
        let title = build_tasks_panel_title(
            PanelCount::Known(3),
            PanelCount::NotLoaded,
            false,
            true,
            false,
            false,
            &filter,
            &model,
            None,
        );
        assert_eq!(title.kind, TasksPanelKind::Archive);
        assert_eq!(title.count, PanelCount::Known(3));
    }

    #[test]
    fn test_build_tasks_panel_title_archived_board_takes_precedence_over_focus() {
        let filter = FilterState::default();
        let model = Model::default();
        let title = build_tasks_panel_title(
            PanelCount::Known(5),
            PanelCount::NotLoaded,
            true,
            false,
            false,
            false,
            &filter,
            &model,
            None,
        );
        assert_eq!(title.kind, TasksPanelKind::ArchivedBoardTasks);
        assert_eq!(title.count, PanelCount::Known(5));
    }

    #[test]
    fn test_build_tasks_panel_title_cards_focus() {
        let filter = FilterState::default();
        let model = Model::default();
        let title = build_tasks_panel_title(
            PanelCount::Known(0),
            PanelCount::NotLoaded,
            false,
            false,
            true,
            false,
            &filter,
            &model,
            None,
        );
        assert_eq!(title.kind, TasksPanelKind::FocusedTasks);
        assert_eq!(title.count, PanelCount::Known(0));
    }

    #[test]
    fn test_build_tasks_panel_title_with_filters_carries_labels() {
        let filter = FilterState {
            hide_assigned_cards: true,
            ..Default::default()
        };
        let model = Model::default();
        let title = build_tasks_panel_title(
            PanelCount::Known(0),
            PanelCount::NotLoaded,
            false,
            false,
            false,
            true,
            &filter,
            &model,
            None,
        );
        assert_eq!(title.filters, vec!["Unassigned Cards".to_string()]);
    }

    #[test]
    fn test_build_tasks_panel_title_archived_cards_view_drops_filters() {
        let filter = FilterState {
            hide_assigned_cards: true,
            ..Default::default()
        };
        let model = Model::default();
        let title = build_tasks_panel_title(
            PanelCount::Known(0),
            PanelCount::NotLoaded,
            false,
            true,
            false,
            true,
            &filter,
            &model,
            None,
        );
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
        let title = build_tasks_panel_title(
            PanelCount::Known(0),
            PanelCount::NotLoaded,
            false,
            false,
            true,
            false,
            &filter,
            &model,
            None,
        );
        assert!(
            !format!("{:?}", title).contains("[2]"),
            "the [2] panel hotkey is a terminal concern, not view data"
        );
    }

    #[test]
    fn test_build_tasks_panel_title_passes_the_count_through_unchanged() {
        let filter = FilterState::default();
        let model = Model::default();

        for count in [
            PanelCount::Known(3),
            PanelCount::NotLoaded,
            PanelCount::Failed,
        ] {
            let title = build_tasks_panel_title(
                count,
                PanelCount::NotLoaded,
                false,
                false,
                true,
                false,
                &filter,
                &model,
                None,
            );
            assert_eq!(title.count, count);
        }
    }

    fn board_with_sprint_state(state: LoadState<Vec<Sprint>>) -> (Board, Model) {
        let board = Board::new("Test Board", None::<String>);
        let mut model = Model::default();
        let resolved = Resolved {
            boards: Collection {
                all: LoadState::Loaded(vec![board.clone()]),
                ..Default::default()
            },
            sprints: Collection {
                by_parent: HashMap::from([(board.id, state)]),
                ..Default::default()
            },
            ..Default::default()
        };
        let _ = model.apply_resolved(resolved);
        (board, model)
    }

    #[test]
    fn test_an_active_sprint_filter_over_an_unloaded_sprint_tier_still_shows_a_filter_label() {
        let (board, model) = board_with_sprint_state(LoadState::NotLoaded);
        let mut filter = FilterState::default();
        filter.active_sprint_filters.insert(uuid::Uuid::new_v4());

        let parts = build_filter_title_parts(&filter, &model, Some(&board));
        assert!(
            !parts.is_empty(),
            "an active sprint filter must show a label even when the sprint tier is not loaded"
        );
    }

    #[test]
    fn test_a_loaded_sprint_tier_still_shows_the_sprint_names() {
        let sprint_a = Sprint::new(uuid::Uuid::new_v4(), 1, None, None::<String>);
        let sprint_a_id = sprint_a.id;
        let sprint_b = Sprint::new(uuid::Uuid::new_v4(), 2, None, None::<String>);
        let (board, model) =
            board_with_sprint_state(LoadState::Loaded(vec![sprint_a.clone(), sprint_b]));
        let mut filter = FilterState::default();
        filter.active_sprint_filters.insert(sprint_a_id);

        let parts = build_filter_title_parts(&filter, &model, Some(&board));
        assert_eq!(parts, vec![sprint_a.formatted_name(&board, None)]);
    }

    #[test]
    fn test_a_failed_sprint_tier_shows_a_filter_label_rather_than_silence() {
        let (board, model) = board_with_sprint_state(LoadState::Failed(Arc::new(
            KanbanError::unsupported("boom"),
        )));
        let mut filter = FilterState::default();
        filter.active_sprint_filters.insert(uuid::Uuid::new_v4());

        let parts = build_filter_title_parts(&filter, &model, Some(&board));
        assert!(
            !parts.is_empty(),
            "a failed sprint tier must still show a filter label, not silently drop it"
        );
    }

    #[test]
    fn test_ended_sprint_count_over_a_loaded_tier_counts_only_active_sprints_past_their_end_date() {
        let placeholder = Board::new("Placeholder", None::<String>);
        let now = Utc::now();
        let sprints = vec![
            sprint_at(
                placeholder.id,
                1,
                SprintStatus::Active,
                Some(now - Duration::days(1)),
            ),
            sprint_at(
                placeholder.id,
                2,
                SprintStatus::Active,
                Some(now + Duration::days(1)),
            ),
            sprint_at(placeholder.id, 3, SprintStatus::Active, None),
            sprint_at(
                placeholder.id,
                4,
                SprintStatus::Completed,
                Some(now - Duration::days(1)),
            ),
            sprint_at(
                placeholder.id,
                5,
                SprintStatus::Cancelled,
                Some(now - Duration::days(1)),
            ),
        ];
        let (board, model) = board_with_sprint_state(LoadState::Loaded(sprints));
        assert_eq!(
            ended_sprint_count(&model, Some(&board), now),
            PanelCount::Known(1)
        );
    }

    #[test]
    fn test_ended_sprint_count_at_the_exact_end_date_is_not_yet_ended() {
        let placeholder = Board::new("Placeholder", None::<String>);
        let now = Utc::now();
        let sprints = vec![sprint_at(
            placeholder.id,
            1,
            SprintStatus::Active,
            Some(now),
        )];
        let (board, model) = board_with_sprint_state(LoadState::Loaded(sprints));
        assert_eq!(
            ended_sprint_count(&model, Some(&board), now),
            PanelCount::Known(0)
        );
    }

    #[test]
    fn test_ended_sprint_count_over_an_untrustworthy_tier_is_not_a_zero() {
        for state in [LoadState::NotLoaded, LoadState::Missing] {
            let (board, model) = board_with_sprint_state(state.clone());
            let result = ended_sprint_count(&model, Some(&board), Utc::now());
            assert_eq!(
                result,
                PanelCount::NotLoaded,
                "{state:?} must report no claim"
            );
            assert_ne!(
                result,
                PanelCount::Known(0),
                "{state:?} must not be reported as a count of zero"
            );
        }
    }

    #[test]
    fn test_ended_sprint_count_over_a_failed_tier_reports_failed_not_zero() {
        let (board, model) = board_with_sprint_state(LoadState::Failed(Arc::new(
            KanbanError::unsupported("boom"),
        )));
        assert_eq!(
            ended_sprint_count(&model, Some(&board), Utc::now()),
            PanelCount::Failed
        );
    }

    #[test]
    fn test_ended_sprint_count_without_a_board_makes_no_claim() {
        assert_eq!(
            ended_sprint_count(&Model::default(), None, Utc::now()),
            PanelCount::NotLoaded
        );
    }

    #[test]
    fn test_build_tasks_panel_title_passes_the_ended_sprint_count_through_unchanged() {
        let filter = FilterState::default();
        let model = Model::default();

        for ended in [
            PanelCount::Known(2),
            PanelCount::Known(0),
            PanelCount::NotLoaded,
            PanelCount::Failed,
        ] {
            let title = build_tasks_panel_title(
                PanelCount::Known(0),
                ended,
                false,
                false,
                true,
                false,
                &filter,
                &model,
                None,
            );
            assert_eq!(title.ended_sprints, ended);
        }
    }

    #[test]
    fn test_no_active_sprint_filter_adds_no_label_regardless_of_load_state() {
        for state in [
            LoadState::NotLoaded,
            LoadState::Loaded(vec![]),
            LoadState::Failed(Arc::new(KanbanError::unsupported("boom"))),
        ] {
            let (board, model) = board_with_sprint_state(state);
            let filter = FilterState::default();
            assert!(build_filter_title_parts(&filter, &model, Some(&board)).is_empty());
        }
    }
}
