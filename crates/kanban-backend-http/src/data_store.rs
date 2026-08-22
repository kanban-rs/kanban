use crate::conversions::{
    board_from_response, card_from_response, column_from_response, sprint_from_response,
};
use crate::HttpBackend;
use kanban_api::{BoardResponse, CardResponse, ColumnResponse, SprintResponse};
use kanban_domain::{
    ArchivedBoard, ArchivedCard, Board, Card, Column, DataStore, DependencyGraph, KanbanError,
    KanbanResult, Prefix, Snapshot, Sprint,
};
use uuid::Uuid;

impl DataStore for HttpBackend {
    fn get_prefix(&self, _name: &str) -> KanbanResult<Option<Prefix>> {
        Err(KanbanError::unsupported("get_prefix"))
    }

    fn list_prefixes(&self) -> KanbanResult<Vec<Prefix>> {
        Err(KanbanError::unsupported("list_prefixes"))
    }

    fn upsert_prefix(&self, _prefix: Prefix) -> KanbanResult<()> {
        Err(KanbanError::unsupported("upsert_prefix"))
    }

    fn get_board(&self, id: Uuid) -> KanbanResult<Option<Board>> {
        self.block_on(async {
            let resp: Option<BoardResponse> = self.get_json(&format!("/v1/boards/{id}")).await?;
            Ok(resp.as_ref().map(board_from_response))
        })
    }

    fn list_boards(&self) -> KanbanResult<Vec<Board>> {
        self.block_on(async {
            let resp: Vec<BoardResponse> = self.get_json_list("/v1/boards").await?;
            Ok(resp.iter().map(board_from_response).collect())
        })
    }

    /// `ReplaceBoardRequest` has no `position` and no `active_sprint_id`, both
    /// of which `DataStore::upsert_board` must write verbatim, so a PUT-based
    /// implementation here would silently drop them on every write.
    /// `with_transaction` already declines and no `RemoteWrites` impl exists,
    /// so this path is unreachable from `execute_with_extra` today regardless.
    fn upsert_board(&self, _board: Board) -> KanbanResult<()> {
        Err(KanbanError::unsupported("upsert_board"))
    }

    fn delete_board(&self, _id: Uuid) -> KanbanResult<()> {
        Err(KanbanError::unsupported("delete_board"))
    }

    fn get_column(&self, id: Uuid) -> KanbanResult<Option<Column>> {
        self.block_on(async {
            let resp: Option<ColumnResponse> = self.get_json(&format!("/v1/columns/{id}")).await?;
            Ok(resp.as_ref().map(column_from_response))
        })
    }

    fn list_columns_by_board(&self, board_id: Uuid) -> KanbanResult<Vec<Column>> {
        self.block_on(async {
            let resp: Vec<ColumnResponse> = self
                .get_json_list(&format!("/v1/boards/{board_id}/columns"))
                .await?;
            Ok(resp.iter().map(column_from_response).collect())
        })
    }

    fn list_all_columns(&self) -> KanbanResult<Vec<Column>> {
        Err(KanbanError::unsupported("list_all_columns"))
    }

    /// `ReplaceColumnRequest` drops only the timestamps, but there is no route
    /// that upserts a column by id outside its board (`PUT
    /// /v1/boards/{bid}/columns/{id}`), and `DataStore::upsert_column` carries
    /// no board_id to route through. `with_transaction` already declines and
    /// no `RemoteWrites` impl exists, so this path is unreachable regardless.
    fn upsert_column(&self, _column: Column) -> KanbanResult<()> {
        Err(KanbanError::unsupported("upsert_column"))
    }

    fn delete_column(&self, _id: Uuid) -> KanbanResult<()> {
        Err(KanbanError::unsupported("delete_column"))
    }

    fn delete_columns_by_board(&self, _board_id: Uuid) -> KanbanResult<()> {
        Err(KanbanError::unsupported("delete_columns_by_board"))
    }

    fn get_card(&self, id: Uuid) -> KanbanResult<Option<Card>> {
        self.block_on(async {
            let resp: Option<CardResponse> = self.get_json(&format!("/v1/cards/{id}")).await?;
            Ok(resp.as_ref().map(card_from_response))
        })
    }

    fn list_all_cards(&self) -> KanbanResult<Vec<Card>> {
        Err(KanbanError::unsupported("list_all_cards"))
    }

    fn list_cards_by_column(&self, column_id: Uuid) -> KanbanResult<Vec<Card>> {
        self.block_on(async {
            let column: Option<ColumnResponse> =
                self.get_json(&format!("/v1/columns/{column_id}")).await?;
            let Some(column) = column else {
                return Ok(Vec::new());
            };
            let resp: Vec<CardResponse> = self
                .get_json_list(&format!(
                    "/v1/boards/{}/cards?column_id={}",
                    column.board_id, column_id
                ))
                .await?;
            let mut cards: Vec<Card> = resp.iter().map(card_from_response).collect();
            cards.sort_by_key(|c| c.position);
            Ok(cards)
        })
    }

    fn list_cards_by_sprint(&self, sprint_id: Uuid) -> KanbanResult<Vec<Card>> {
        self.block_on(async {
            let sprint: Option<SprintResponse> =
                self.get_json(&format!("/v1/sprints/{sprint_id}")).await?;
            let Some(sprint) = sprint else {
                return Ok(Vec::new());
            };
            let resp: Vec<CardResponse> = self
                .get_json_list(&format!(
                    "/v1/boards/{}/cards?sprint_id={}",
                    sprint.board_id, sprint_id
                ))
                .await?;
            let mut cards: Vec<Card> = resp.iter().map(card_from_response).collect();
            cards.sort_by_key(|c| c.position);
            Ok(cards)
        })
    }

    fn count_cards_in_column(&self, _column_id: Uuid) -> KanbanResult<usize> {
        Err(KanbanError::unsupported("count_cards_in_column"))
    }

    fn count_cards_in_column_excluding(
        &self,
        _column_id: Uuid,
        _exclude_ids: &[Uuid],
    ) -> KanbanResult<usize> {
        Err(KanbanError::unsupported("count_cards_in_column_excluding"))
    }

    /// The only card-write route (`PUT /v1/columns/{cid}/cards/{id}`) takes
    /// `CreateCardRequest`, which has no `status`, `position`, `card_number`,
    /// `prefix`, `completed_at` or `board_id` -- a PUT-based upsert here would
    /// silently drop six fields of the row it was told to write.
    /// `with_transaction` already declines and no `RemoteWrites` impl exists,
    /// so this path is unreachable regardless.
    fn upsert_card(&self, _card: Card) -> KanbanResult<()> {
        Err(KanbanError::unsupported("upsert_card"))
    }

    fn delete_card(&self, _id: Uuid) -> KanbanResult<()> {
        Err(KanbanError::unsupported("delete_card"))
    }

    fn delete_cards_by_columns(&self, _column_ids: &[Uuid]) -> KanbanResult<()> {
        Err(KanbanError::unsupported("delete_cards_by_columns"))
    }

    fn clear_sprint_from_cards(
        &self,
        _sprint_id: Uuid,
        _timestamp: chrono::DateTime<chrono::Utc>,
    ) -> KanbanResult<()> {
        Err(KanbanError::unsupported("clear_sprint_from_cards"))
    }

    fn get_archived_card(&self, _card_id: Uuid) -> KanbanResult<Option<ArchivedCard>> {
        Err(KanbanError::unsupported("get_archived_card"))
    }

    fn list_archived_cards(&self) -> KanbanResult<Vec<ArchivedCard>> {
        Err(KanbanError::unsupported("list_archived_cards"))
    }

    fn insert_archived_card(&self, _ac: ArchivedCard) -> KanbanResult<()> {
        Err(KanbanError::unsupported("insert_archived_card"))
    }

    fn delete_archived_card(&self, _card_id: Uuid) -> KanbanResult<()> {
        Err(KanbanError::unsupported("delete_archived_card"))
    }

    fn get_archived_board(&self, _board_id: Uuid) -> KanbanResult<Option<ArchivedBoard>> {
        Err(KanbanError::unsupported("get_archived_board"))
    }

    fn list_archived_boards(&self) -> KanbanResult<Vec<ArchivedBoard>> {
        Err(KanbanError::unsupported("list_archived_boards"))
    }

    fn insert_archived_board(&self, _ab: ArchivedBoard) -> KanbanResult<()> {
        Err(KanbanError::unsupported("insert_archived_board"))
    }

    fn delete_archived_board(&self, _board_id: Uuid) -> KanbanResult<()> {
        Err(KanbanError::unsupported("delete_archived_board"))
    }

    fn unarchive_board(&self, _board_id: Uuid) -> KanbanResult<()> {
        Err(KanbanError::unsupported("unarchive_board"))
    }

    fn get_sprint(&self, id: Uuid) -> KanbanResult<Option<Sprint>> {
        self.block_on(async {
            let resp: Option<SprintResponse> = self.get_json(&format!("/v1/sprints/{id}")).await?;
            Ok(resp.as_ref().map(sprint_from_response))
        })
    }

    fn list_sprints_by_board(&self, board_id: Uuid) -> KanbanResult<Vec<Sprint>> {
        self.block_on(async {
            let resp: Vec<SprintResponse> = self
                .get_json_list(&format!("/v1/boards/{board_id}/sprints"))
                .await?;
            Ok(resp.iter().map(sprint_from_response).collect())
        })
    }

    fn list_all_sprints(&self) -> KanbanResult<Vec<Sprint>> {
        Err(KanbanError::unsupported("list_all_sprints"))
    }

    /// `ReplaceSprintRequest` has no `status`, `start_date`, `end_date`,
    /// `sprint_number` or `name_index` -- a PUT-based upsert here would
    /// silently drop five fields of the row it was told to write.
    /// `with_transaction` already declines and no `RemoteWrites` impl exists,
    /// so this path is unreachable regardless.
    fn upsert_sprint(&self, _sprint: Sprint) -> KanbanResult<()> {
        Err(KanbanError::unsupported("upsert_sprint"))
    }

    fn delete_sprint(&self, _id: Uuid) -> KanbanResult<()> {
        Err(KanbanError::unsupported("delete_sprint"))
    }

    fn delete_sprints_by_board(&self, _board_id: Uuid) -> KanbanResult<()> {
        Err(KanbanError::unsupported("delete_sprints_by_board"))
    }

    fn get_graph(&self) -> KanbanResult<DependencyGraph> {
        Err(KanbanError::unsupported("get_graph"))
    }

    fn set_graph(&self, _graph: DependencyGraph) -> KanbanResult<()> {
        Err(KanbanError::unsupported("set_graph"))
    }

    fn snapshot(&self) -> KanbanResult<Snapshot> {
        Err(KanbanError::unsupported("snapshot"))
    }

    fn apply_snapshot(&self, _snapshot: Snapshot) -> KanbanResult<()> {
        Err(KanbanError::unsupported("apply_snapshot"))
    }

    /// No route filters cards by a bare `board_id` + `card_number` pair, and
    /// the inherited default would fetch every card in the workspace
    /// (`list_all_cards`, itself `unsupported` here) to answer a one-row
    /// lookup. Declines under its own name instead of inheriting.
    fn get_card_by_board_and_number(
        &self,
        _board_id: Uuid,
        _card_number: u32,
    ) -> KanbanResult<Option<Card>> {
        Err(KanbanError::unsupported("get_card_by_board_and_number"))
    }

    /// One list request plus a client-side find, bounded to one sprint --
    /// the same shape as the inherited default, written out so it declines
    /// under its own name if `list_cards_by_sprint` ever stops answering it.
    fn get_card_by_sprint_and_number(
        &self,
        sprint_id: Uuid,
        card_number: u32,
    ) -> KanbanResult<Option<Card>> {
        Ok(self
            .list_cards_by_sprint(sprint_id)?
            .into_iter()
            .find(|c| c.card_number == card_number))
    }

    /// No route filters cards by a bare `card_number` across every namespace,
    /// and the inherited default would fetch every card in the workspace to
    /// answer a one-row lookup. Declines under its own name instead of
    /// inheriting.
    fn list_cards_by_number(&self, _card_number: u32) -> KanbanResult<Vec<Card>> {
        Err(KanbanError::unsupported("list_cards_by_number"))
    }

    /// No route filters cards by `(prefix, card_number)`, and the inherited
    /// default would both fetch every card in the workspace AND re-implement
    /// `Prefix::normalize` client-side, a second source of truth for a server
    /// rule. Declines under its own name instead of inheriting.
    fn list_cards_by_prefix_and_number(
        &self,
        _prefix: &str,
        _card_number: u32,
    ) -> KanbanResult<Vec<Card>> {
        Err(KanbanError::unsupported("list_cards_by_prefix_and_number"))
    }

    /// The cards route accepts a single `column_id` filter
    /// (`CardQuery.column_id: Option<Uuid>`), so one request per column is
    /// the floor until a multi-column filter exists -- and each of those is
    /// itself two requests, because `list_cards_by_column` must resolve the
    /// column to its board first. Written out so a future backend change to
    /// `list_cards_by_column` doesn't silently change this cost too.
    fn list_cards_by_columns(&self, column_ids: &[Uuid]) -> KanbanResult<Vec<Card>> {
        let mut out = Vec::new();
        for col_id in column_ids {
            out.extend(self.list_cards_by_column(*col_id)?);
        }
        Ok(out)
    }
}
