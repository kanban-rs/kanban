use super::super::{Command, CommandContext};
use super::CardCommand;
use crate::data_store::DataStore;
use crate::{CardUpdate, CreateCardOptions, DomainError, KanbanError, KanbanResult, NewCard};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Update card properties (title, description, priority, status, etc.)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UpdateCard {
    pub card_id: Uuid,
    pub updates: CardUpdate,
}

impl UpdateCard {
    pub fn execute(&self, context: &CommandContext) -> KanbanResult<()> {
        let mut card = context.get_card(self.card_id)?;
        // Validate a re-targeted column FK before mutating, mirroring MoveCard
        // (KAN-248). Without this an update could orphan card.column_id.
        if let Some(new_column_id) = self.updates.column_id {
            context.require_column(new_column_id)?;
        }
        card.update(self.updates.clone(), Utc::now());
        context.store.upsert_card(card)?;
        Ok(())
    }

    pub fn description(&self) -> String {
        "Update card".to_string()
    }

    /// Inverse: read the card's current state and synthesise an
    /// `UpdateCard` whose `updates` field-by-field restore each touched
    /// field to its prior value. Fields not touched by the forward
    /// command stay `None` / `NoChange` so the inverse is minimal.
    pub fn capture_inverse(&self, store: &dyn DataStore) -> KanbanResult<Vec<Command>> {
        use crate::field_update::FieldUpdate;
        let card = match store.get_card(self.card_id)? {
            Some(c) => c,
            None => return Err(KanbanError::not_found("Card", self.card_id)),
        };

        let upd = &self.updates;
        let inverse = CardUpdate {
            title: upd.title.as_ref().map(|_| card.title.clone()),
            description: match upd.description {
                FieldUpdate::NoChange => FieldUpdate::NoChange,
                _ => match card.description {
                    Some(v) => FieldUpdate::Set(v),
                    None => FieldUpdate::Clear,
                },
            },
            priority: upd.priority.map(|_| card.priority),
            status: upd.status.map(|_| card.status),
            position: upd.position.map(|_| card.position),
            column_id: upd.column_id.map(|_| card.column_id),
            due_date: match upd.due_date {
                FieldUpdate::NoChange => FieldUpdate::NoChange,
                _ => match card.due_date {
                    Some(v) => FieldUpdate::Set(v),
                    None => FieldUpdate::Clear,
                },
            },
            points: match upd.points {
                FieldUpdate::NoChange => FieldUpdate::NoChange,
                _ => match card.points {
                    Some(v) => FieldUpdate::Set(v),
                    None => FieldUpdate::Clear,
                },
            },
            sprint_id: match upd.sprint_id {
                FieldUpdate::NoChange => FieldUpdate::NoChange,
                _ => match card.sprint_id {
                    Some(v) => FieldUpdate::Set(v),
                    None => FieldUpdate::Clear,
                },
            },
        };

        Ok(vec![Command::Card(CardCommand::Update(UpdateCard {
            card_id: self.card_id,
            updates: inverse,
        }))])
    }
}

/// Create a new card in a column
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CreateCard {
    pub id: Uuid,
    pub card_number: u32,
    pub board_id: Uuid,
    pub column_id: Uuid,
    pub title: String,
    pub position: i32,
    pub options: CreateCardOptions,
    #[serde(default = "chrono::Utc::now")]
    pub timestamp: DateTime<Utc>,
}

impl CreateCard {
    pub fn execute(&self, context: &CommandContext) -> KanbanResult<()> {
        context.check_wip_limit(self.column_id, 1, &[])?;
        let board = context.get_board(self.board_id)?;

        let now = self.timestamp;
        // Funnel construction through the factory (no `Card { .. }` literal nor a
        // follow-up `CardUpdate` for create fields). The frozen command carries
        // `title` + `options`; translate them into a `NewCard` and build once via
        // `Card::create`. `sprint_id` is deliberately NOT funneled into the spec
        // (it would silently drop the SprintLog) — pass `None` and let the
        // post-create `assign_to_sprint` set the id and seed the log. The
        // server-managed `position` is applied post-create.
        let spec = NewCard {
            column_id: self.column_id,
            title: self.title.clone(),
            description: self.options.description.clone(),
            priority: self.options.priority.unwrap_or(crate::CardPriority::Medium),
            due_date: self.options.due_date,
            points: self.options.points,
            sprint_id: None,
        };
        // Resolved here rather than carried on the command: `CreateCard` is
        // replayed from serialized command logs, so its shape is frozen. The
        // value must match what the identifier reader resolves, so a sprint
        // override is applied below once the sprint is known.
        let board_prefix = crate::prefix::effective_card_prefix(
            board.card_prefix.as_deref(),
            None,
            crate::prefix_backfill::DEFAULT_CARD_PREFIX,
        );
        let mut card = crate::Card::create(
            spec,
            self.id,
            self.card_number,
            board_prefix,
            now,
            self.board_id,
        )?;
        card.position = self.position;

        if let Some(sprint_id) = self.options.sprint_id {
            let sprint = context.get_sprint(sprint_id)?;
            if sprint.board_id != self.board_id {
                return Err(KanbanError::Domain(DomainError::SprintBoardMismatch {
                    sprint_id,
                    sprint_board: sprint.board_id,
                    card_board: self.board_id,
                }));
            }
            let sprint_number = sprint.sprint_number;
            let sprint_name = sprint.get_name(&board).map(|s| s.to_string());
            let sprint_status = format!("{:?}", sprint.status);
            // The reader resolves `sprint.card_prefix -> board.card_prefix`, so
            // a card created into an overriding sprint is addressed under the
            // sprint's prefix and must be stored under it.
            card.prefix = crate::prefix::effective_card_prefix(
                board.card_prefix.as_deref(),
                sprint.card_prefix.as_deref(),
                crate::prefix_backfill::DEFAULT_CARD_PREFIX,
            );
            card.assign_to_sprint(sprint_id, sprint_number, sprint_name, sprint_status, now);
        }

        context.store.upsert_board(board)?;
        context.store.upsert_card(card)?;
        Ok(())
    }

    pub fn description(&self) -> String {
        format!("Create card: '{}'", self.title)
    }

    /// Inverse: delete the new card. `DeleteCard` is polymorphic over
    /// live / archived so it cleanly removes a freshly-created live
    /// card without leaving an archive trail. Redo via the original
    /// forward reproduces the same id and number.
    pub fn capture_inverse(&self, _store: &dyn DataStore) -> KanbanResult<Vec<Command>> {
        Ok(vec![Command::Card(CardCommand::Delete(DeleteCard {
            card_id: self.id,
        }))])
    }
}

/// Restore an archived card
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RestoreCard {
    pub card_id: Uuid,
    pub column_id: Uuid,
    pub position: i32,
    #[serde(default = "chrono::Utc::now")]
    pub timestamp: DateTime<Utc>,
}

impl RestoreCard {
    /// Inverse: archive the card again. The card id is in the forward
    /// command. ArchiveCards captures original column/position from the
    /// live card at capture time — by the time this runs the card has
    /// been restored to (self.column_id, self.position), so the
    /// re-archive will use those values as the new "original" location.
    pub fn capture_inverse(&self, _store: &dyn DataStore) -> KanbanResult<Vec<Command>> {
        Ok(vec![Command::Card(CardCommand::Archive(ArchiveCards {
            ids: vec![self.card_id],
        }))])
    }

    pub fn execute(&self, context: &CommandContext) -> KanbanResult<()> {
        context.check_wip_limit(self.column_id, 1, &[])?;
        if context.store.get_archived_card(self.card_id)?.is_none() {
            return Err(KanbanError::not_found("archived card", self.card_id));
        }
        // Reference-marker model: the card is already LIVE in `cards`. Fetch it,
        // apply the restore column/position, drop the marker, and re-upsert.
        // `delete_archived_card` removes both the marker and the card row (both
        // backends), so the following `upsert_card` re-materialises the live card
        // in its restored position — net effect: marker gone, card visible again.
        let mut card = context
            .store
            .get_card(self.card_id)?
            .ok_or_else(|| KanbanError::not_found("Card", self.card_id))?;
        card.column_id = self.column_id;
        // Keep board_id in sync with wherever the card actually lands -- the
        // normal capture_inverse-driven restore always targets the card's own
        // current column (a no-op here), but nothing else validates that
        // `column_id` belongs to the card's original board (KAN-963).
        card.board_id = context.require_column(self.column_id)?.board_id;
        card.position = self.position;
        card.updated_at = self.timestamp;

        context.store.delete_archived_card(self.card_id)?;
        context.store.upsert_card(card)?;

        // Cards still archived AFTER this restore (the marker for `card_id` was
        // just deleted above). Reviving `card_id`'s edges must not resurrect an
        // edge to a still-archived neighbor, so those endpoints are not live.
        let still_archived: std::collections::HashSet<Uuid> = context
            .store
            .list_archived_cards()?
            .into_iter()
            .map(|a| a.entity_id)
            .collect();

        let card_id = self.card_id;
        context.store.modify_graph(Box::new(move |graph| {
            graph.unarchive_node(card_id, &|other| !still_archived.contains(&other));
            Ok(())
        }))?;
        Ok(())
    }

    pub fn description(&self) -> String {
        format!("Restore card {}", self.card_id)
    }
}

/// Permanently delete a card. Operates on whichever list the card is
/// in — live or archived. Strips incident graph edges.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeleteCard {
    pub card_id: Uuid,
}

impl DeleteCard {
    pub fn execute(&self, context: &CommandContext) -> KanbanResult<()> {
        // Both store ops are idempotent on missing — calling both
        // covers a card in either list.
        context.store.delete_card(self.card_id)?;
        context.store.delete_archived_card(self.card_id)?;
        let card_id = self.card_id;
        context.store.modify_graph(Box::new(move |graph| {
            graph.remove_node(card_id);
            Ok(())
        }))?;
        Ok(())
    }

    pub fn description(&self) -> String {
        format!("Delete card {}", self.card_id)
    }

    /// Inverse: re-insert whichever state the card was in (live,
    /// archived, or — defensively — both) via `ImportEntities`, then
    /// re-add every incident graph edge.
    pub fn capture_inverse(&self, store: &dyn DataStore) -> KanbanResult<Vec<Command>> {
        let live = store.get_card(self.card_id)?;
        let archived = store.get_archived_card(self.card_id)?;
        if live.is_none() && archived.is_none() {
            return Err(KanbanError::not_found("Card", self.card_id));
        }
        let mut commands: Vec<Command> = vec![Command::Board(super::super::BoardCommand::Import(
            super::super::ImportEntities {
                cards: live.into_iter().collect(),
                archived_cards: archived.into_iter().collect(),
                ..Default::default()
            },
        ))];
        let graph = store.get_graph()?;
        let card_id = self.card_id;
        commands.extend(super::super::dependency_commands::edges_to_undo_commands(
            &graph,
            |s, t| s == card_id || t == card_id,
        ));
        Ok(commands)
    }
}

/// Archive one or more cards in a single command (single undo entry)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArchiveCards {
    pub ids: Vec<Uuid>,
}

impl ArchiveCards {
    /// Inverse: one `RestoreCard` per archived card, restoring each to its
    /// original column and position read from the live card BEFORE the
    /// archive runs.
    pub fn capture_inverse(&self, store: &dyn DataStore) -> KanbanResult<Vec<Command>> {
        let mut commands: Vec<Command> = Vec::new();
        for id in &self.ids {
            let card = match store.get_card(*id)? {
                Some(c) => c,
                None => continue, // skipped (matches ArchiveCards::execute's filter)
            };
            commands.push(Command::Card(CardCommand::Restore(RestoreCard {
                card_id: card.id,
                column_id: card.column_id,
                position: card.position,
                timestamp: chrono::Utc::now(),
            })));
        }
        Ok(commands)
    }

    pub fn execute(&self, context: &CommandContext) -> KanbanResult<()> {
        let valid_ids = context.filter_valid_card_ids(&self.ids, "ArchiveCards");
        if valid_ids.is_empty() && !self.ids.is_empty() {
            return Err(KanbanError::validation(
                "All card IDs in ArchiveCards batch are invalid",
            ));
        }
        for id in &valid_ids {
            // Idempotency guard: if a marker already exists for this id (a
            // re-issued archive command, an undo/redo replay, or a retry
            // after a flaky save), leave it untouched. Its board_id was
            // already durably settled on the FIRST archive; re-deriving it
            // here would read the card's CURRENT state, which may have
            // changed since (e.g. its column has since been legitimately
            // deleted), clobbering a correct value with a worse one.
            if context.store.get_archived_card(*id)?.is_some() {
                continue;
            }
            let card = context
                .store
                .get_card(*id)?
                .ok_or_else(|| KanbanError::not_found("Card", *id))?;
            // Reference-marker model: the card STAYS live in `cards`; we only
            // record the marker. `delete_card` is a guarded no-op on an archived
            // id (F1), so it is not called here — the card is the source of truth.
            let archived = crate::ArchivedCard::new(card.id, card.board_id);
            context.store.insert_archived_card(archived)?;
        }
        context.store.modify_graph(Box::new(move |graph| {
            for id in &valid_ids {
                graph.archive_node(*id);
            }
            Ok(())
        }))?;
        Ok(())
    }

    pub fn description(&self) -> String {
        format!("Archive {} card(s)", self.ids.len())
    }
}
