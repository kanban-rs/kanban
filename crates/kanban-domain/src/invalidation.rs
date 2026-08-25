use std::collections::HashSet;
use uuid::Uuid;

/// The set of entities a command touched, scoped per collection.
///
/// `graph` is set when the command mutated the dependency graph. `prefixes`
/// is set when the command upserted a prefix row.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EntityIds {
    pub boards: HashSet<Uuid>,
    pub columns: HashSet<Uuid>,
    pub cards: HashSet<Uuid>,
    pub sprints: HashSet<Uuid>,
    pub graph: bool,
    pub prefixes: bool,
}

impl EntityIds {
    pub fn boards(ids: impl IntoIterator<Item = Uuid>) -> Self {
        Self {
            boards: ids.into_iter().collect(),
            ..Default::default()
        }
    }

    pub fn columns(ids: impl IntoIterator<Item = Uuid>) -> Self {
        Self {
            columns: ids.into_iter().collect(),
            ..Default::default()
        }
    }

    pub fn cards(ids: impl IntoIterator<Item = Uuid>) -> Self {
        Self {
            cards: ids.into_iter().collect(),
            ..Default::default()
        }
    }

    pub fn sprints(ids: impl IntoIterator<Item = Uuid>) -> Self {
        Self {
            sprints: ids.into_iter().collect(),
            ..Default::default()
        }
    }

    pub fn with_graph(mut self) -> Self {
        self.graph = true;
        self
    }

    pub fn with_prefixes(mut self) -> Self {
        self.prefixes = true;
        self
    }

    pub fn is_empty(&self) -> bool {
        self.boards.is_empty()
            && self.columns.is_empty()
            && self.cards.is_empty()
            && self.sprints.is_empty()
            && !self.graph
            && !self.prefixes
    }

    pub fn merge(&mut self, other: EntityIds) {
        self.boards.extend(other.boards);
        self.columns.extend(other.columns);
        self.cards.extend(other.cards);
        self.sprints.extend(other.sprints);
        self.graph |= other.graph;
        self.prefixes |= other.prefixes;
    }
}

/// What a batch of commands invalidated in a cache keyed on entity id.
///
/// `Entities` names exactly what changed. `All` is the safe fallback for any
/// batch containing a command whose full blast radius cannot be enumerated
/// from its own fields (an empty batch counts as unenumerable, never as "no
/// invalidation").
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Invalidation {
    Entities(EntityIds),
    All,
}

/// Derive the [`Invalidation`] a batch of commands implies, whether the batch
/// is a captured inverse or a forward batch about to be committed.
/// [`crate::commands::Command::touched_entities`] reads only the command's
/// own fields, so it applies to either direction.
///
/// Folds `touched_entities` over `inverse`, falling back to `All` the moment
/// any command in the batch returns `None`, or when the batch (or its
/// accumulated ids) is empty.
///
/// A forward batch and its own inverse are different commands and can imply
/// different results: `CreateCard` names its card and board, while its
/// inverse `DeleteCard` is unenumerable and yields `All`.
pub fn invalidation_from_inverse(inverse: &[crate::commands::Command]) -> Invalidation {
    if inverse.is_empty() {
        return Invalidation::All;
    }
    let mut acc = EntityIds::default();
    for cmd in inverse {
        match cmd.touched_entities() {
            Some(ids) => acc.merge(ids),
            None => return Invalidation::All,
        }
    }
    if acc.is_empty() {
        return Invalidation::All;
    }
    Invalidation::Entities(acc)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::cascade_commands::{
        CascadeCommand, DeleteArchivedCards, DeleteCardsByColumns,
    };
    use crate::commands::*;
    use crate::{ArchivedBoard, ArchivedCard, Board, CardUpdate, ColumnUpdate, SprintUpdate};
    use crate::{BoardUpdate, CreateCardOptions, DependencyGraph};
    use chrono::Utc;

    #[test]
    fn test_update_card_touched_entities_names_only_that_card() {
        let card_id = Uuid::new_v4();
        let cmd = Command::Card(CardCommand::Update(UpdateCard {
            card_id,
            updates: CardUpdate::default(),
        }));
        let ids = cmd.touched_entities().expect("enumerable");
        assert_eq!(ids.cards, HashSet::from([card_id]));
        assert!(ids.boards.is_empty());
        assert!(ids.columns.is_empty());
        assert!(ids.sprints.is_empty());
        assert!(!ids.graph);
    }

    #[test]
    fn test_create_card_touched_entities_includes_the_counter_bumping_board() {
        let id = Uuid::new_v4();
        let board_id = Uuid::new_v4();
        let cmd = Command::Card(CardCommand::Create(CreateCard {
            id,
            card_number: 1,
            board_id,
            column_id: Uuid::new_v4(),
            title: "t".into(),
            position: 0,
            options: CreateCardOptions::default(),
            timestamp: Utc::now(),
            default_card_prefix: "kan".into(),
        }));
        let ids = cmd.touched_entities().expect("enumerable");
        assert_eq!(ids.cards, HashSet::from([id]));
        assert_eq!(ids.boards, HashSet::from([board_id]));
    }

    #[test]
    fn test_create_card_touched_entities_marks_prefixes_dirty() {
        let cmd = Command::Card(CardCommand::Create(CreateCard {
            id: Uuid::new_v4(),
            card_number: 1,
            board_id: Uuid::new_v4(),
            column_id: Uuid::new_v4(),
            title: "t".into(),
            position: 0,
            options: CreateCardOptions::default(),
            timestamp: Utc::now(),
            default_card_prefix: "kan".into(),
        }));
        let ids = cmd.touched_entities().expect("enumerable");
        assert!(ids.prefixes);
    }

    #[test]
    fn test_create_sprint_touched_entities_includes_the_owning_board() {
        let id = Uuid::new_v4();
        let board_id = Uuid::new_v4();
        let cmd = Command::Sprint(SprintCommand::Create(CreateSprint {
            id,
            board_id,
            name: None,
            default_sprint_prefix: "kan".into(),
            explicit_prefix: None,
            auto_consume_name: false,
        }));
        let ids = cmd.touched_entities().expect("enumerable");
        assert_eq!(ids.sprints, HashSet::from([id]));
        assert_eq!(ids.boards, HashSet::from([board_id]));
    }

    #[test]
    fn test_create_sprint_touched_entities_marks_prefixes_dirty() {
        let cmd = Command::Sprint(SprintCommand::Create(CreateSprint {
            id: Uuid::new_v4(),
            board_id: Uuid::new_v4(),
            name: None,
            default_sprint_prefix: "kan".into(),
            explicit_prefix: None,
            auto_consume_name: false,
        }));
        let ids = cmd.touched_entities().expect("enumerable");
        assert!(ids.prefixes);
    }

    #[test]
    fn test_update_sprint_without_a_name_change_names_only_that_sprint() {
        let sprint_id = Uuid::new_v4();
        let cmd = Command::Sprint(SprintCommand::Update(UpdateSprint {
            sprint_id,
            updates: SprintUpdate {
                name: None,
                status: Some(crate::SprintStatus::Active),
                ..Default::default()
            },
        }));
        let ids = cmd.touched_entities().expect("enumerable");
        assert_eq!(ids.sprints, HashSet::from([sprint_id]));
        assert!(ids.boards.is_empty());
    }

    #[test]
    fn test_update_sprint_with_a_name_change_is_unenumerable() {
        let sprint_id = Uuid::new_v4();
        let cmd = Command::Sprint(SprintCommand::Update(UpdateSprint {
            sprint_id,
            updates: SprintUpdate {
                name: Some("Alpha".into()),
                ..Default::default()
            },
        }));
        assert!(cmd.touched_entities().is_none());
    }

    #[test]
    fn test_add_spawns_touched_entities_marks_the_graph_dirty() {
        let source = Uuid::new_v4();
        let target = Uuid::new_v4();
        let cmd = Command::Dependency(DependencyCommand::AddSpawns(AddSpawns {
            source,
            target,
            as_archived: false,
        }));
        let ids = cmd.touched_entities().expect("enumerable");
        assert_eq!(ids.cards, HashSet::from([source, target]));
        assert!(ids.graph);
    }

    #[test]
    fn test_create_subcard_touched_entities_marks_prefixes_dirty() {
        let cmd = Command::Dependency(DependencyCommand::CreateSubcard(CreateSubcardCommand {
            id: Uuid::new_v4(),
            parent_id: Uuid::new_v4(),
            board_id: Uuid::new_v4(),
            column_id: Uuid::new_v4(),
            title: "t".into(),
            description: None,
            position: 0,
            default_card_prefix: "kan".into(),
        }));
        let ids = cmd.touched_entities().expect("enumerable");
        assert!(ids.prefixes);
    }

    #[test]
    fn test_archive_cards_touched_entities_marks_the_graph_dirty() {
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        let cmd = Command::Card(CardCommand::Archive(ArchiveCards { ids: vec![a, b] }));
        let ids = cmd.touched_entities().expect("enumerable");
        assert_eq!(ids.cards, HashSet::from([a, b]));
        assert!(ids.graph);
    }

    #[test]
    fn test_delete_card_touched_entities_is_unenumerable() {
        let cmd = Command::Card(CardCommand::Delete(DeleteCard {
            card_id: Uuid::new_v4(),
        }));
        assert!(cmd.touched_entities().is_none());
    }

    #[test]
    fn test_delete_archived_cards_touched_entities_is_unenumerable() {
        let cmd = Command::Cascade(CascadeCommand::DeleteArchivedCards(DeleteArchivedCards {
            card_ids: vec![Uuid::new_v4()],
        }));
        assert!(cmd.touched_entities().is_none());
    }

    #[test]
    fn test_delete_cards_by_columns_touched_entities_is_unenumerable() {
        let cmd = Command::Cascade(CascadeCommand::DeleteCardsByColumns(DeleteCardsByColumns {
            column_ids: vec![Uuid::new_v4()],
        }));
        assert!(cmd.touched_entities().is_none());
    }

    #[test]
    fn test_import_entities_without_a_graph_enumerates_every_collection() {
        let board = Board::create(
            crate::NewBoard {
                name: "B".into(),
                description: None,
                sprint_prefix: None,
                card_prefix: None,
                task_sort_field: None,
                task_sort_order: None,
                sprint_duration_days: None,
                task_list_view: None,
            },
            Uuid::new_v4(),
            Utc::now(),
        )
        .unwrap();
        let board_id = board.id;
        let column = crate::Column::create(
            crate::NewColumn {
                board_id,
                name: "TODO".into(),
                wip_limit: None,
                default_status: None,
            },
            Uuid::new_v4(),
            0,
            Utc::now(),
        )
        .unwrap();
        let column_id = column.id;
        let card = crate::Card::new(board_id, column_id, "c1", 0);
        let card_id = card.id;
        let archived_card_id = Uuid::new_v4();
        let archived_card = ArchivedCard::new(archived_card_id, board_id);
        let archived_board_id = Uuid::new_v4();
        let archived_board: ArchivedBoard = crate::Archived::now(archived_board_id);
        let sprint = crate::Sprint::create(
            crate::NewSprint {
                board_id,
                sprint_number: 1,
                name_index: None,
                prefix: None,
                card_prefix: None,
            },
            Uuid::new_v4(),
            Utc::now(),
        )
        .unwrap();
        let sprint_id = sprint.id;

        let cmd = Command::Board(BoardCommand::Import(ImportEntities {
            boards: vec![board],
            columns: vec![column],
            cards: vec![card],
            archived_cards: vec![archived_card],
            archived_boards: vec![archived_board],
            sprints: vec![sprint],
            graph: None,
            ..Default::default()
        }));

        let ids = cmd.touched_entities().expect("enumerable");
        assert_eq!(ids.boards, HashSet::from([board_id, archived_board_id]));
        assert_eq!(ids.columns, HashSet::from([column_id]));
        assert_eq!(ids.cards, HashSet::from([card_id, archived_card_id]));
        assert_eq!(ids.sprints, HashSet::from([sprint_id]));
        assert!(!ids.graph);
        assert!(ids.prefixes);
    }

    #[test]
    fn test_import_entities_with_only_boards_still_marks_prefixes_dirty() {
        let cmd = Command::Board(BoardCommand::Import(ImportEntities::default()));
        let ids = cmd.touched_entities().expect("enumerable");
        assert!(ids.prefixes);
    }

    #[test]
    fn test_import_entities_with_a_graph_is_unenumerable() {
        let cmd = Command::Board(BoardCommand::Import(ImportEntities {
            graph: Some(DependencyGraph::default()),
            ..Default::default()
        }));
        assert!(cmd.touched_entities().is_none());
    }

    #[test]
    fn test_update_column_touched_entities_names_only_that_column() {
        let column_id = Uuid::new_v4();
        let cmd = Command::Column(ColumnCommand::Update(UpdateColumn {
            column_id,
            updates: ColumnUpdate::default(),
        }));
        let ids = cmd.touched_entities().expect("enumerable");
        assert_eq!(ids.columns, HashSet::from([column_id]));
    }

    #[test]
    fn test_update_board_touched_entities_names_only_that_board() {
        let board_id = Uuid::new_v4();
        let cmd = Command::Board(BoardCommand::Update(UpdateBoard {
            board_id,
            updates: BoardUpdate::default(),
        }));
        let ids = cmd.touched_entities().expect("enumerable");
        assert_eq!(ids.boards, HashSet::from([board_id]));
    }

    #[test]
    fn test_invalidation_from_empty_inverse_batch_is_all() {
        assert_eq!(invalidation_from_inverse(&[]), Invalidation::All);
    }

    #[test]
    fn test_invalidation_merges_ids_across_a_multi_command_batch() {
        let card_a = Uuid::new_v4();
        let board_b = Uuid::new_v4();
        let batch = vec![
            Command::Card(CardCommand::Update(UpdateCard {
                card_id: card_a,
                updates: CardUpdate::default(),
            })),
            Command::Board(BoardCommand::Update(UpdateBoard {
                board_id: board_b,
                updates: BoardUpdate::default(),
            })),
        ];
        match invalidation_from_inverse(&batch) {
            Invalidation::Entities(ids) => {
                assert_eq!(ids.cards, HashSet::from([card_a]));
                assert_eq!(ids.boards, HashSet::from([board_b]));
            }
            Invalidation::All => panic!("expected Entities, got All"),
        }
    }

    #[test]
    fn test_a_command_touching_no_entities_invalidates_everything() {
        let cmd = Command::Card(CardCommand::AssignToSprint(AssignCardsToSprint {
            ids: vec![],
            sprint_id: Uuid::new_v4(),
        }));
        assert_eq!(
            invalidation_from_inverse(std::slice::from_ref(&cmd)),
            Invalidation::All
        );
    }

    #[test]
    fn test_invalidation_from_a_mixed_batch_with_one_unenumerable_command_is_all() {
        let batch = vec![
            Command::Card(CardCommand::Update(UpdateCard {
                card_id: Uuid::new_v4(),
                updates: CardUpdate::default(),
            })),
            Command::Card(CardCommand::Delete(DeleteCard {
                card_id: Uuid::new_v4(),
            })),
        ];
        assert_eq!(invalidation_from_inverse(&batch), Invalidation::All);
    }
}
