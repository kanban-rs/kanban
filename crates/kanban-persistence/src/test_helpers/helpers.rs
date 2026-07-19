use chrono::Utc;
use kanban_domain::card::{Card, CardPriority, CardStatus};
use kanban_domain::sprint::{Sprint, SprintStatus};
use kanban_domain::Snapshot;
use kanban_domain::{Board, Column, DependencyGraph, SprintLog};
use uuid::Uuid;

pub fn fully_populated_snapshot() -> Snapshot {
    let board_id = Uuid::new_v4();
    let col_id = Uuid::new_v4();
    let sprint_id = Uuid::new_v4();
    let card_id = Uuid::new_v4();
    let archived_card_inner_id = Uuid::new_v4();
    let now = Utc::now();

    let board = Board {
        id: board_id,
        name: "Full Board".into(),
        description: Some("Board desc".into()),
        sprint_prefix: Some("sprint".into()),
        card_prefix: Some("FB".into()),
        task_sort_field: kanban_domain::board::SortField::Priority,
        task_sort_order: kanban_domain::board::SortOrder::Descending,
        sprint_duration_days: Some(14),
        sprint_names: vec!["Alpha".into(), "Beta".into()],
        sprint_name_used_count: 1,
        next_sprint_number: 3,
        active_sprint_id: Some(sprint_id),
        task_list_view: kanban_domain::task_list_view::TaskListView::GroupedByColumn,
        card_counter: 4,
        sprint_counters: {
            let mut m = std::collections::HashMap::new();
            m.insert("sprint".into(), 3u32);
            m
        },
        completion_column_id: Some(col_id),
        position: 0,
        created_at: now,
        updated_at: now,
    };

    let column = Column {
        id: col_id,
        board_id,
        name: "Full Col".into(),
        position: 0,
        wip_limit: Some(5),
        created_at: now,
        updated_at: now,
    };

    let sprint = Sprint {
        id: sprint_id,
        board_id,
        sprint_number: 2,
        name_index: Some(0),
        prefix: Some("sprint".into()),
        card_prefix: Some("TASK".into()),
        status: SprintStatus::Active,
        start_date: Some(now),
        end_date: Some(now),
        created_at: now,
        updated_at: now,
    };

    let card = Card {
        id: card_id,
        column_id: col_id,
        title: "Full Card".into(),
        description: Some("desc".into()),
        priority: CardPriority::High,
        status: CardStatus::InProgress,
        position: 0,
        due_date: Some(now),
        points: Some(3),
        card_number: 1,
        sprint_id: Some(sprint_id),
        created_at: now,
        updated_at: now,
        completed_at: None,
        sprint_logs: vec![SprintLog {
            sprint_id,
            sprint_number: 2,
            sprint_name: Some("Alpha".into()),
            started_at: now,
            ended_at: None,
            status: "Active".into(),
        }],
    };

    // Reference-marker model: the archived card stays LIVE in `.cards`; the marker
    // only references it by id.
    let archived_live_card = Card {
        id: archived_card_inner_id,
        column_id: col_id,
        title: "Archived Card".into(),
        description: Some("archived desc".into()),
        priority: CardPriority::Critical,
        status: CardStatus::Done,
        position: 1,
        due_date: Some(now),
        points: Some(5),
        card_number: 2,
        sprint_id: Some(sprint_id),
        created_at: now,
        updated_at: now,
        completed_at: Some(now),
        sprint_logs: vec![],
    };
    let archived_card = kanban_domain::Archived::with_context(
        archived_card_inner_id,
        kanban_domain::CardRestoreContext {
            board_id: Uuid::nil(),
        },
        kanban_domain::ArchiveMetadata::at(now),
    );

    use kanban_core::EdgeBase;
    use kanban_domain::{BlocksEdge, RelatesEdge, RelatesKind, Severity};
    let graph = DependencyGraph::from_validated_per_kind_edges(
        vec![],
        vec![BlocksEdge {
            base: EdgeBase {
                source: card_id,
                target: archived_card_inner_id,
                created_at: now,
                archived_at: None,
            },
            severity: Severity::default(),
        }],
        vec![RelatesEdge {
            base: EdgeBase {
                source: card_id,
                target: archived_card_inner_id,
                created_at: now,
                archived_at: Some(now),
            },
            kind: RelatesKind::default(),
        }],
    )
    .expect("test fixture edges must validate");

    Snapshot {
        archived_boards: Vec::new(),
        boards: vec![board],
        columns: vec![column],
        cards: vec![card, archived_live_card],
        archived_cards: vec![archived_card],
        sprints: vec![sprint],
        graph,
    }
}
