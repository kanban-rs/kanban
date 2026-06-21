use super::KanbanContext;
use kanban_domain::commands::{
    AddBlocks, AddRelates, AddSpawns, Command, DependencyCommand, RemoveBlocks, RemoveRelates,
    RemoveSpawns,
};
use kanban_domain::{GraphOperations, KanbanError, KanbanResult, RelatesKind, Severity};
use uuid::Uuid;

impl KanbanContext {
    /// Reject edge mutations against unknown card ids before the
    /// command reaches the graph. Without this guard a stale or
    /// fabricated UUID would silently land in the graph as a dangling
    /// edge — the CLI's identifier-resolution layer parses raw UUIDs
    /// without looking them up, so service-level enforcement is the
    /// right boundary.
    fn require_card_exists(&self, id: Uuid) -> KanbanResult<()> {
        match self.backend.get_card(id)? {
            Some(_) => Ok(()),
            None => Err(KanbanError::not_found("Card", id)),
        }
    }
}

impl GraphOperations for KanbanContext {
    fn attach_children(&mut self, parent: Uuid, children: Vec<Uuid>) -> KanbanResult<()> {
        self.require_card_exists(parent)?;
        for child in &children {
            self.require_card_exists(*child)?;
        }
        let commands: Vec<Command> = children
            .into_iter()
            .map(|child| {
                Command::Dependency(DependencyCommand::AddSpawns(AddSpawns {
                    source: parent,
                    target: child,
                    as_archived: false,
                }))
            })
            .collect();
        self.execute(commands)
    }

    fn detach_children(&mut self, parent: Uuid, children: Vec<Uuid>) -> KanbanResult<()> {
        self.require_card_exists(parent)?;
        for child in &children {
            self.require_card_exists(*child)?;
        }
        let commands: Vec<Command> = children
            .into_iter()
            .map(|child| {
                Command::Dependency(DependencyCommand::RemoveSpawns(RemoveSpawns {
                    source: parent,
                    target: child,
                    tolerate_missing: false,
                }))
            })
            .collect();
        self.execute(commands)
    }

    fn list_children_of(&self, parent: Uuid) -> KanbanResult<Vec<Uuid>> {
        self.require_card_exists(parent)?;
        Ok(self.backend.get_graph()?.children(parent))
    }

    fn list_parents_of(&self, child: Uuid) -> KanbanResult<Vec<Uuid>> {
        self.require_card_exists(child)?;
        Ok(self.backend.get_graph()?.parents(child))
    }

    fn block(&mut self, blocker: Uuid, blocked: Uuid, severity: Severity) -> KanbanResult<()> {
        self.require_card_exists(blocker)?;
        self.require_card_exists(blocked)?;
        self.execute(vec![Command::Dependency(DependencyCommand::AddBlocks(
            AddBlocks {
                source: blocker,
                target: blocked,
                severity,
                as_archived: false,
            },
        ))])
    }

    fn unblock(&mut self, blocker: Uuid, blocked: Uuid) -> KanbanResult<()> {
        self.require_card_exists(blocker)?;
        self.require_card_exists(blocked)?;
        self.execute(vec![Command::Dependency(DependencyCommand::RemoveBlocks(
            RemoveBlocks {
                source: blocker,
                target: blocked,
                tolerate_missing: false,
            },
        ))])
    }

    fn list_blocked_by(&self, blocker: Uuid) -> KanbanResult<Vec<Uuid>> {
        self.require_card_exists(blocker)?;
        Ok(self.backend.get_graph()?.blocked(blocker))
    }

    fn list_blockers_of(&self, blocked: Uuid) -> KanbanResult<Vec<Uuid>> {
        self.require_card_exists(blocked)?;
        Ok(self.backend.get_graph()?.blockers(blocked))
    }

    fn relate(&mut self, a: Uuid, b: Uuid, kind: RelatesKind) -> KanbanResult<()> {
        self.require_card_exists(a)?;
        self.require_card_exists(b)?;
        self.execute(vec![Command::Dependency(DependencyCommand::AddRelates(
            AddRelates {
                source: a,
                target: b,
                kind,
                as_archived: false,
            },
        ))])
    }

    fn dissociate(&mut self, a: Uuid, b: Uuid) -> KanbanResult<()> {
        self.require_card_exists(a)?;
        self.require_card_exists(b)?;
        self.execute(vec![Command::Dependency(DependencyCommand::RemoveRelates(
            RemoveRelates {
                source: a,
                target: b,
                tolerate_missing: false,
            },
        ))])
    }

    fn list_related_to(&self, card: Uuid) -> KanbanResult<Vec<Uuid>> {
        self.require_card_exists(card)?;
        Ok(self.backend.get_graph()?.related(card))
    }
}
