use kanban_domain::{
    Board, BoardUpdate, Card, CardUpdate, Column, ColumnUpdate, Invalidation, KanbanResult,
    NewBoard, NewCard, NewColumn, RelatesKind, Severity, Sprint, SprintUpdate,
};
use uuid::Uuid;

/// Optional backend capability: delegate the nine core CRUD mutations
/// directly to a remote authority (a `kanban-server` instance) instead of
/// `KanbanContext`'s local command-execute-then-log path. When
/// `KanbanBackend::remote_writes()` returns `Some`, `KanbanContext`'s nine
/// v1 mutators divert to these methods before any local prework (id
/// minting, FK checks, cascade building) runs, and pass the returned
/// `Invalidation` straight through to the caller as the remote authority's
/// own answer. Local backends (JSON/SQLite/InMemory) never override
/// `KanbanBackend::remote_writes()`. `HttpBackend` is its production
/// implementor and returns `Some` unconditionally.
pub trait RemoteWrites: Send + Sync {
    fn create_board(
        &self,
        id: Option<Uuid>,
        spec: &NewBoard,
    ) -> KanbanResult<(Board, Invalidation)>;
    fn update_board(&self, id: Uuid, updates: &BoardUpdate) -> KanbanResult<(Board, Invalidation)>;
    fn delete_board(&self, id: Uuid) -> KanbanResult<Invalidation>;

    fn create_column(
        &self,
        board_id: Uuid,
        spec: &NewColumn,
    ) -> KanbanResult<(Column, Invalidation)>;
    fn update_column(
        &self,
        id: Uuid,
        updates: &ColumnUpdate,
    ) -> KanbanResult<(Column, Invalidation)>;
    fn delete_column(&self, id: Uuid) -> KanbanResult<Invalidation>;

    fn create_card(&self, id: Option<Uuid>, spec: &NewCard) -> KanbanResult<(Card, Invalidation)>;
    fn update_card(&self, id: Uuid, updates: &CardUpdate) -> KanbanResult<(Card, Invalidation)>;
    fn delete_card(&self, id: Uuid) -> KanbanResult<Invalidation>;
}

/// Per-family counterpart to [`RemoteWrites`]: each family is its own
/// `Option`-able seam on `KanbanBackend`, so a backend's support can degrade
/// per family instead of all-or-nothing.
pub trait RemoteBoardWrites: Send + Sync {
    fn archive_board(&self, id: Uuid) -> KanbanResult<Invalidation>;
    fn restore_board(&self, id: Uuid) -> KanbanResult<Invalidation>;
}

/// See [`RemoteBoardWrites`].
pub trait RemoteCardWrites: Send + Sync {
    fn archive_card(&self, id: Uuid) -> KanbanResult<Invalidation>;
    fn restore_card(&self, id: Uuid, column_id: Option<Uuid>)
        -> KanbanResult<(Card, Invalidation)>;
    fn move_card(
        &self,
        id: Uuid,
        column_id: Uuid,
        position: Option<i32>,
    ) -> KanbanResult<(Card, Invalidation)>;
    fn assign_card_to_sprint(
        &self,
        id: Uuid,
        sprint_id: Uuid,
    ) -> KanbanResult<(Card, Invalidation)>;
    fn unassign_card_from_sprint(&self, id: Uuid) -> KanbanResult<(Card, Invalidation)>;
}

/// Outcome of a batch mutation on [`RemoteBatchWrites`]: ids that succeeded,
/// and ids that failed paired with the remote authority's message for each.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RemoteBatchOutcome {
    pub succeeded: Vec<Uuid>,
    pub failed: Vec<(Uuid, String)>,
}

/// See [`RemoteBoardWrites`].
pub trait RemoteBatchWrites: Send + Sync {
    fn archive_cards(&self, ids: &[Uuid]) -> KanbanResult<(RemoteBatchOutcome, Invalidation)>;
    fn move_cards(
        &self,
        ids: &[Uuid],
        column_id: Uuid,
    ) -> KanbanResult<(RemoteBatchOutcome, Invalidation)>;
    fn assign_cards_to_sprint(
        &self,
        ids: &[Uuid],
        sprint_id: Uuid,
    ) -> KanbanResult<(RemoteBatchOutcome, Invalidation)>;
    fn update_cards(
        &self,
        updates: &[(Uuid, CardUpdate)],
    ) -> KanbanResult<(RemoteBatchOutcome, Invalidation)>;
}

/// See [`RemoteBoardWrites`].
pub trait RemoteSprintWrites: Send + Sync {
    fn create_sprint(
        &self,
        board_id: Uuid,
        id: Option<Uuid>,
        name: Option<&str>,
        prefix: Option<&str>,
    ) -> KanbanResult<(Sprint, Invalidation)>;
    fn update_sprint(
        &self,
        id: Uuid,
        updates: &SprintUpdate,
    ) -> KanbanResult<(Sprint, Invalidation)>;
    fn delete_sprint(&self, id: Uuid) -> KanbanResult<Invalidation>;
    fn activate_sprint(
        &self,
        id: Uuid,
        duration_days: Option<i32>,
    ) -> KanbanResult<(Sprint, Invalidation)>;
    fn complete_sprint(&self, id: Uuid) -> KanbanResult<(Sprint, Invalidation)>;
    fn cancel_sprint(&self, id: Uuid) -> KanbanResult<(Sprint, Invalidation)>;
    fn carry_over_sprint_cards(
        &self,
        from_sprint_id: Uuid,
        to_sprint_id: Uuid,
    ) -> KanbanResult<(usize, Invalidation)>;
}

/// See [`RemoteBoardWrites`].
pub trait RemoteGraphWrites: Send + Sync {
    fn attach_children(&self, parent: Uuid, children: &[Uuid]) -> KanbanResult<Invalidation>;
    fn detach_children(&self, parent: Uuid, children: &[Uuid]) -> KanbanResult<Invalidation>;
    fn block(&self, blocker: Uuid, blocked: Uuid, severity: Severity)
        -> KanbanResult<Invalidation>;
    fn unblock(&self, blocker: Uuid, blocked: Uuid) -> KanbanResult<Invalidation>;
    fn relate(&self, a: Uuid, b: Uuid, kind: RelatesKind) -> KanbanResult<Invalidation>;
    fn dissociate(&self, a: Uuid, b: Uuid) -> KanbanResult<Invalidation>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use kanban_domain::EntityIds;

    struct Probe;

    impl RemoteBatchWrites for Probe {
        fn archive_cards(&self, _ids: &[Uuid]) -> KanbanResult<(RemoteBatchOutcome, Invalidation)> {
            unimplemented!()
        }

        fn move_cards(
            &self,
            ids: &[Uuid],
            _column_id: Uuid,
        ) -> KanbanResult<(RemoteBatchOutcome, Invalidation)> {
            let outcome = RemoteBatchOutcome {
                succeeded: vec![ids[0]],
                failed: vec![(ids[1], "Card b not found".to_string())],
            };
            Ok((outcome, Invalidation::Entities(EntityIds::default())))
        }

        fn assign_cards_to_sprint(
            &self,
            _ids: &[Uuid],
            _sprint_id: Uuid,
        ) -> KanbanResult<(RemoteBatchOutcome, Invalidation)> {
            unimplemented!()
        }

        fn update_cards(
            &self,
            _updates: &[(Uuid, CardUpdate)],
        ) -> KanbanResult<(RemoteBatchOutcome, Invalidation)> {
            unimplemented!()
        }
    }

    #[test]
    fn test_a_remote_batch_writes_implementor_reports_partial_success_and_per_id_failures(
    ) -> KanbanResult<()> {
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        let probe = Probe;

        let (outcome, _invalidation) = probe.move_cards(&[a, b], Uuid::new_v4())?;

        assert_eq!(outcome.succeeded, vec![a]);
        assert_eq!(outcome.failed, vec![(b, "Card b not found".to_string())]);
        Ok(())
    }

    #[test]
    fn test_remote_batch_outcome_default_is_empty() {
        let outcome = RemoteBatchOutcome::default();

        assert!(outcome.succeeded.is_empty());
        assert!(outcome.failed.is_empty());
    }

    struct SprintProbe;

    impl RemoteSprintWrites for SprintProbe {
        fn create_sprint(
            &self,
            board_id: Uuid,
            id: Option<Uuid>,
            _name: Option<&str>,
            prefix: Option<&str>,
        ) -> KanbanResult<(Sprint, Invalidation)> {
            let mut sprint = Sprint::new(board_id, 1, None, prefix);
            if let Some(id) = id {
                sprint.id = id;
            }
            Ok((sprint, Invalidation::Entities(EntityIds::default())))
        }

        fn update_sprint(
            &self,
            _id: Uuid,
            _updates: &SprintUpdate,
        ) -> KanbanResult<(Sprint, Invalidation)> {
            unimplemented!()
        }

        fn delete_sprint(&self, _id: Uuid) -> KanbanResult<Invalidation> {
            Ok(Invalidation::Entities(EntityIds::default()))
        }

        fn activate_sprint(
            &self,
            _id: Uuid,
            duration_days: Option<i32>,
        ) -> KanbanResult<(Sprint, Invalidation)> {
            let sprint_number = duration_days.map(|d| d as u32).unwrap_or(0);
            let sprint = Sprint::new(Uuid::new_v4(), sprint_number, None, None::<String>);
            Ok((sprint, Invalidation::Entities(EntityIds::default())))
        }

        fn complete_sprint(&self, _id: Uuid) -> KanbanResult<(Sprint, Invalidation)> {
            let sprint = Sprint::new(Uuid::new_v4(), 1, None, None::<String>);
            Ok((sprint, Invalidation::Entities(EntityIds::default())))
        }

        fn cancel_sprint(&self, _id: Uuid) -> KanbanResult<(Sprint, Invalidation)> {
            let sprint = Sprint::new(Uuid::new_v4(), 1, None, None::<String>);
            Ok((sprint, Invalidation::Entities(EntityIds::default())))
        }

        fn carry_over_sprint_cards(
            &self,
            _from_sprint_id: Uuid,
            _to_sprint_id: Uuid,
        ) -> KanbanResult<(usize, Invalidation)> {
            Ok((3, Invalidation::Entities(EntityIds::default())))
        }
    }

    #[test]
    fn test_a_remote_sprint_writes_create_signature_carries_no_number_or_index() -> KanbanResult<()>
    {
        let board_id = Uuid::new_v4();
        let probe = SprintProbe;

        let (sprint, _invalidation) =
            probe.create_sprint(board_id, None, Some("Sprint Alpha"), Some("ALP"))?;

        assert_eq!(sprint.board_id, board_id);
        assert_eq!(sprint.prefix, Some("ALP".to_string()));
        Ok(())
    }

    #[test]
    fn test_a_remote_sprint_writes_delete_returns_an_invalidation_without_an_entity(
    ) -> KanbanResult<()> {
        let probe = SprintProbe;

        let invalidation = probe.delete_sprint(Uuid::new_v4())?;

        assert_eq!(invalidation, Invalidation::Entities(EntityIds::default()));
        Ok(())
    }

    #[test]
    fn test_activate_sprint_passes_none_through_as_none() -> KanbanResult<()> {
        let probe = SprintProbe;

        let (sprint, _invalidation) = probe.activate_sprint(Uuid::new_v4(), None)?;
        assert_eq!(sprint.sprint_number, 0);

        let (sprint, _invalidation) = probe.activate_sprint(Uuid::new_v4(), Some(7))?;
        assert_eq!(sprint.sprint_number, 7);
        Ok(())
    }

    #[test]
    fn test_carry_over_sprint_cards_returns_a_count_not_an_entity() -> KanbanResult<()> {
        let probe = SprintProbe;

        let (moved, _invalidation) =
            probe.carry_over_sprint_cards(Uuid::new_v4(), Uuid::new_v4())?;

        assert_eq!(moved, 3);
        Ok(())
    }
}
