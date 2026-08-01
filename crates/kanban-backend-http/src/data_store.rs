use crate::conversions::{board_from_response, card_from_response, column_from_response};
use crate::HttpBackend;
use kanban_api::{BoardResponse, CardResponse, ColumnResponse};
use kanban_domain::{
    ArchivedBoard, ArchivedCard, Board, Card, CardSummary, Column, DataStore, DependencyGraph,
    KanbanError, KanbanResult, Snapshot, Sprint,
};
use uuid::Uuid;

impl DataStore for HttpBackend {
    fn get_board(&self, id: Uuid) -> KanbanResult<Option<Board>> {
        self.block_on(async {
            let resp: Option<BoardResponse> =
                self.get_optional(&format!("/v1/boards/{id}")).await?;
            Ok(resp.map(board_from_response))
        })
    }

    fn list_boards(&self) -> KanbanResult<Vec<Board>> {
        self.block_on(async {
            let resps: Vec<BoardResponse> = self.get_list("/v1/boards").await?;
            Ok(resps.into_iter().map(board_from_response).collect())
        })
    }

    fn upsert_board(&self, _board: Board) -> KanbanResult<()> {
        Err(KanbanError::unsupported("upsert_board"))
    }

    fn delete_board(&self, _id: Uuid) -> KanbanResult<()> {
        Err(KanbanError::unsupported("delete_board"))
    }

    fn get_column(&self, id: Uuid) -> KanbanResult<Option<Column>> {
        self.block_on(async {
            let resp: Option<ColumnResponse> =
                self.get_optional(&format!("/v1/columns/{id}")).await?;
            Ok(resp.map(column_from_response))
        })
    }

    fn list_columns_by_board(&self, board_id: Uuid) -> KanbanResult<Vec<Column>> {
        self.block_on(async {
            let resps: Vec<ColumnResponse> = self
                .get_list(&format!("/v1/boards/{board_id}/columns"))
                .await?;
            Ok(resps.into_iter().map(column_from_response).collect())
        })
    }

    fn list_all_columns(&self) -> KanbanResult<Vec<Column>> {
        Err(KanbanError::unsupported("list_all_columns"))
    }

    fn upsert_column(&self, _column: Column) -> KanbanResult<()> {
        Err(KanbanError::unsupported("upsert_column"))
    }

    fn delete_column(&self, _id: Uuid) -> KanbanResult<()> {
        Err(KanbanError::unsupported("delete_column"))
    }

    fn delete_columns_by_board(&self, _board_id: Uuid) -> KanbanResult<()> {
        Err(KanbanError::unsupported("delete_columns_by_board"))
    }

    /// Stays unsupported: no route returns a `CardResponse` carrying its own
    /// `board_id` (see conversions.rs' `card_from_response` doc), and this
    /// method has no board_id in scope to supply it from, unlike
    /// `list_cards_by_column` which already resolved one via its column.
    fn get_card(&self, _id: Uuid) -> KanbanResult<Option<Card>> {
        Err(KanbanError::unsupported("get_card"))
    }

    fn list_all_cards(&self) -> KanbanResult<Vec<Card>> {
        Err(KanbanError::unsupported("list_all_cards"))
    }

    /// Resolves `column_id` to its owning board (`GET /v1/columns/{id}`),
    /// lists that board's cards filtered to the column (`CardSummary` --
    /// lighter than `Card`, missing `description`/`board_id`), then fetches
    /// each summary's full `CardResponse` (`GET
    /// /v1/boards/{board_id}/cards/{id}`) for a faithful `Card` -- one HTTP
    /// round-trip per card, by design (see AskUserQuestion decision on
    /// CardSummary fidelity). A summary that 404s on the detail fetch (raced
    /// with a delete) is dropped rather than erroring the whole list.
    fn list_cards_by_column(&self, column_id: Uuid) -> KanbanResult<Vec<Card>> {
        self.block_on(async {
            let Some(column): Option<ColumnResponse> = self
                .get_optional(&format!("/v1/columns/{column_id}"))
                .await?
            else {
                return Ok(Vec::new());
            };
            let board_id = column.board_id;

            let summaries: Vec<CardSummary> = self
                .get_list(&format!(
                    "/v1/boards/{board_id}/cards?column_id={column_id}"
                ))
                .await?;

            let mut cards = Vec::with_capacity(summaries.len());
            for summary in summaries {
                let detail: Option<CardResponse> = self
                    .get_optional(&format!("/v1/boards/{board_id}/cards/{}", summary.id))
                    .await?;
                if let Some(detail) = detail {
                    cards.push(card_from_response(detail, board_id));
                }
            }
            Ok(cards)
        })
    }

    fn list_cards_by_sprint(&self, _sprint_id: Uuid) -> KanbanResult<Vec<Card>> {
        Err(KanbanError::unsupported("list_cards_by_sprint"))
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

    fn get_sprint(&self, _id: Uuid) -> KanbanResult<Option<Sprint>> {
        Err(KanbanError::unsupported("get_sprint"))
    }

    fn list_sprints_by_board(&self, _board_id: Uuid) -> KanbanResult<Vec<Sprint>> {
        Err(KanbanError::unsupported("list_sprints_by_board"))
    }

    fn list_all_sprints(&self) -> KanbanResult<Vec<Sprint>> {
        Err(KanbanError::unsupported("list_all_sprints"))
    }

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
}
