//! Every `DataStore` method the server has no route for declines with
//! `KanbanError::unsupported(<its own method name>)`, tagged with one of six
//! categories: `missing-route` (no route exists at all), `architecture-mismatch`
//! (the operation's shape doesn't map onto this transport), `write-backstop-via-RemoteWrites`
//! (writes route through `RemoteWrites`, so this decline firing at all is a
//! routing bug surfacing loudly), `archived-family-gap` (no route filters by
//! archival status), `count-methods-never-fake-O(1)` (`count_cards_in_column_filtered`
//! is implemented against `GET /v1/columns/{column_id}/cards/count`;
//! `count_cards_in_column` and `count_cards_in_column_excluding` still decline,
//! not for want of a route but because no remote caller reaches them, since
//! every remote count goes through `count_cards_in_column_filtered`), and
//! `bulk-deletes-never-fan-out` (no route deletes by parent).

use crate::conversions::{
    archived_board_from_response, archived_card_from_card_response, archived_card_from_response,
    board_from_response, card_from_response, column_from_response, prefix_from_response,
    sprint_from_response,
};
use crate::HttpBackend;
use kanban_api::{
    ArchivedBoardResponse, ArchivedCardResponse, BoardResponse, CardCountResponse, CardResponse,
    ColumnResponse, PrefixResponse, SprintResponse,
};
use kanban_domain::{
    ArchivedBoard, ArchivedCard, Board, Card, Column, DataStore, DependencyGraph, KanbanError,
    KanbanResult, Prefix, Sprint,
};
use uuid::Uuid;

impl HttpBackend {
    fn lookup_cards(&self, identifier: &str) -> KanbanResult<Vec<Card>> {
        self.block_on(async {
            let resp: Vec<CardResponse> = self
                .get_json_with_query("/v1/cards/lookup", &[("identifier", identifier)])
                .await?
                .unwrap_or_default();
            Ok(resp.iter().map(card_from_response).collect())
        })
    }
}

impl DataStore for HttpBackend {
    fn get_prefix(&self, name: &str) -> KanbanResult<Option<Prefix>> {
        self.block_on(async {
            let resp: Option<PrefixResponse> = self
                .get_json_with_query("/v1/prefixes", &[("name", name)])
                .await?;
            Ok(resp.as_ref().map(prefix_from_response))
        })
    }

    fn list_prefixes(&self) -> KanbanResult<Vec<Prefix>> {
        self.block_on(async {
            let resp: Vec<PrefixResponse> = self.get_json_list("/v1/prefixes").await?;
            Ok(resp.iter().map(prefix_from_response).collect())
        })
    }

    /// missing-route: no route creates or replaces a prefix directly.
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

    /// write-backstop-via-RemoteWrites: board writes route through `RemoteWrites`; this decline firing at all is a routing bug.
    fn upsert_board(&self, _board: Board) -> KanbanResult<()> {
        Err(KanbanError::unsupported("upsert_board"))
    }

    /// write-backstop-via-RemoteWrites: board deletes route through `RemoteWrites::delete_board`; this decline firing at all is a routing bug.
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
            let board: Option<BoardResponse> =
                self.get_json(&format!("/v1/boards/{board_id}")).await?;
            let Some(_) = board else {
                return Ok(Vec::new());
            };
            let resp: Vec<ColumnResponse> = self
                .get_json_list(&format!("/v1/boards/{board_id}/columns"))
                .await?;
            Ok(resp.iter().map(column_from_response).collect())
        })
    }

    /// architecture-mismatch: a whole-workspace flat column read; this transport deliberately never grows that route.
    fn list_all_columns(&self) -> KanbanResult<Vec<Column>> {
        Err(KanbanError::unsupported("list_all_columns"))
    }

    /// write-backstop-via-RemoteWrites: column writes route through `RemoteWrites`; this decline firing at all is a routing bug.
    fn upsert_column(&self, _column: Column) -> KanbanResult<()> {
        Err(KanbanError::unsupported("upsert_column"))
    }

    /// write-backstop-via-RemoteWrites: column deletes route through `RemoteWrites::delete_column`; this decline firing at all is a routing bug.
    fn delete_column(&self, _id: Uuid) -> KanbanResult<()> {
        Err(KanbanError::unsupported("delete_column"))
    }

    /// bulk-deletes-never-fan-out: no route deletes every column of a board in one call.
    fn delete_columns_by_board(&self, _board_id: Uuid) -> KanbanResult<()> {
        Err(KanbanError::unsupported("delete_columns_by_board"))
    }

    fn get_card(&self, id: Uuid) -> KanbanResult<Option<Card>> {
        self.block_on(async {
            let resp: Option<CardResponse> = self.get_json(&format!("/v1/cards/{id}")).await?;
            Ok(resp.as_ref().map(card_from_response))
        })
    }

    /// architecture-mismatch: a whole-workspace flat card read; this transport deliberately never grows that route.
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

    /// archived-family-gap: the server has no route filtering cards by
    /// archival status, so only `LiveOnly` can be served (by delegating to
    /// `list_cards_by_column`); any archived-aware filter declines under its
    /// own name.
    fn list_cards_by_column_filtered(
        &self,
        column_id: Uuid,
        archived: kanban_domain::ArchivedFilter,
    ) -> KanbanResult<Vec<Card>> {
        match archived {
            kanban_domain::ArchivedFilter::LiveOnly => self.list_cards_by_column(column_id),
            _ => Err(KanbanError::unsupported("list_cards_by_column_filtered")),
        }
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

    /// count-methods-never-fake-O(1): no caller reaches this remotely; callers get their count
    /// through `count_cards_in_column_filtered` against `GET /v1/columns/{column_id}/cards/count`.
    fn count_cards_in_column(&self, _column_id: Uuid) -> KanbanResult<usize> {
        Err(KanbanError::unsupported("count_cards_in_column"))
    }

    fn count_cards_in_column_filtered(
        &self,
        column_id: Uuid,
        archived: kanban_domain::ArchivedFilter,
    ) -> KanbanResult<usize> {
        let archived = archived_filter_param(archived);
        self.block_on(async {
            let resp: Option<CardCountResponse> = self
                .get_json_with_query(
                    &format!("/v1/columns/{column_id}/cards/count"),
                    &[("archived", archived)],
                )
                .await?;
            Ok(resp.map(|r| r.count).unwrap_or(0))
        })
    }

    /// count-methods-never-fake-O(1): same as `count_cards_in_column`.
    fn count_cards_in_column_excluding(
        &self,
        _column_id: Uuid,
        _exclude_ids: &[Uuid],
    ) -> KanbanResult<usize> {
        Err(KanbanError::unsupported("count_cards_in_column_excluding"))
    }

    /// write-backstop-via-RemoteWrites: card writes route through `RemoteWrites`; this decline firing at all is a routing bug.
    fn upsert_card(&self, _card: Card) -> KanbanResult<()> {
        Err(KanbanError::unsupported("upsert_card"))
    }

    /// write-backstop-via-RemoteWrites: card deletes route through `RemoteWrites::delete_card`; this decline firing at all is a routing bug.
    fn delete_card(&self, _id: Uuid) -> KanbanResult<()> {
        Err(KanbanError::unsupported("delete_card"))
    }

    /// bulk-deletes-never-fan-out: no route deletes every card of a set of columns in one call.
    fn delete_cards_by_columns(&self, _column_ids: &[Uuid]) -> KanbanResult<()> {
        Err(KanbanError::unsupported("delete_cards_by_columns"))
    }

    /// missing-route: no route clears a sprint id from live cards in bulk.
    fn clear_sprint_from_cards(
        &self,
        _sprint_id: Uuid,
        _timestamp: chrono::DateTime<chrono::Utc>,
    ) -> KanbanResult<()> {
        Err(KanbanError::unsupported("clear_sprint_from_cards"))
    }

    /// missing-route: no server route clears a sprint id from archived
    /// cards. Declines under its own name rather than inheriting the
    /// default, which would walk `list_archived_cards` and report that
    /// method's name instead.
    fn clear_sprint_from_archived_cards(
        &self,
        _sprint_id: Uuid,
        _timestamp: chrono::DateTime<chrono::Utc>,
    ) -> KanbanResult<()> {
        Err(KanbanError::unsupported("clear_sprint_from_archived_cards"))
    }

    fn get_archived_card(&self, card_id: Uuid) -> KanbanResult<Option<ArchivedCard>> {
        self.block_on(async {
            let resp: Option<CardResponse> = self.get_json(&format!("/v1/cards/{card_id}")).await?;
            Ok(resp.as_ref().and_then(archived_card_from_card_response))
        })
    }

    /// archived-family-gap: no whole-store archived-card list route exists; only the board-scoped one does.
    fn list_archived_cards(&self) -> KanbanResult<Vec<ArchivedCard>> {
        Err(KanbanError::unsupported("list_archived_cards"))
    }

    fn list_archived_cards_by_board(&self, board_id: Uuid) -> KanbanResult<Vec<ArchivedCard>> {
        self.block_on(async {
            let board: Option<BoardResponse> =
                self.get_json(&format!("/v1/boards/{board_id}")).await?;
            let Some(_) = board else {
                return Ok(Vec::new());
            };
            let resp: Vec<ArchivedCardResponse> = self
                .get_json_list(&format!("/v1/boards/{board_id}/archived-cards"))
                .await?;
            Ok(resp.iter().map(archived_card_from_response).collect())
        })
    }

    /// archived-family-gap: archiving happens via the card's own archive action server-side, not a direct marker insert.
    fn insert_archived_card(&self, _ac: ArchivedCard) -> KanbanResult<()> {
        Err(KanbanError::unsupported("insert_archived_card"))
    }

    /// archived-family-gap: no route deletes a single archived-card marker.
    fn delete_archived_card(&self, _card_id: Uuid) -> KanbanResult<()> {
        Err(KanbanError::unsupported("delete_archived_card"))
    }

    /// archived-family-gap: no route fetches a single archived-board marker by id.
    fn get_archived_board(&self, _board_id: Uuid) -> KanbanResult<Option<ArchivedBoard>> {
        Err(KanbanError::unsupported("get_archived_board"))
    }

    fn list_archived_boards(&self) -> KanbanResult<Vec<ArchivedBoard>> {
        self.block_on(async {
            let resp: Vec<ArchivedBoardResponse> =
                self.get_json_list("/v1/archived-boards").await?;
            Ok(resp.iter().map(archived_board_from_response).collect())
        })
    }

    /// archived-family-gap: archiving happens via the board's own archive action server-side, not a direct marker insert.
    fn insert_archived_board(&self, _ab: ArchivedBoard) -> KanbanResult<()> {
        Err(KanbanError::unsupported("insert_archived_board"))
    }

    /// archived-family-gap: no route deletes a single archived-board marker.
    fn delete_archived_board(&self, _board_id: Uuid) -> KanbanResult<()> {
        Err(KanbanError::unsupported("delete_archived_board"))
    }

    /// archived-family-gap: unarchiving happens via the board's own restore action server-side, not this trait method.
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
            let board: Option<BoardResponse> =
                self.get_json(&format!("/v1/boards/{board_id}")).await?;
            let Some(_) = board else {
                return Ok(Vec::new());
            };
            let resp: Vec<SprintResponse> = self
                .get_json_list(&format!("/v1/boards/{board_id}/sprints"))
                .await?;
            Ok(resp.iter().map(sprint_from_response).collect())
        })
    }

    /// architecture-mismatch: a whole-workspace flat sprint read; this transport deliberately never grows that route.
    fn list_all_sprints(&self) -> KanbanResult<Vec<Sprint>> {
        Err(KanbanError::unsupported("list_all_sprints"))
    }

    /// missing-route: sprint mutations have no `RemoteWrites` counterpart at all.
    fn upsert_sprint(&self, _sprint: Sprint) -> KanbanResult<()> {
        Err(KanbanError::unsupported("upsert_sprint"))
    }

    /// missing-route: sprint mutations have no `RemoteWrites` counterpart at all.
    fn delete_sprint(&self, _id: Uuid) -> KanbanResult<()> {
        Err(KanbanError::unsupported("delete_sprint"))
    }

    /// bulk-deletes-never-fan-out: no route deletes every sprint of a board in one call.
    fn delete_sprints_by_board(&self, _board_id: Uuid) -> KanbanResult<()> {
        Err(KanbanError::unsupported("delete_sprints_by_board"))
    }

    fn get_graph(&self) -> KanbanResult<DependencyGraph> {
        self.block_on(async {
            match self.get_json::<DependencyGraph>("/v1/graph").await? {
                Some(graph) => Ok(graph),
                None => Err(KanbanError::unsupported(
                    "get_graph (server has no /v1/graph route)",
                )),
            }
        })
    }

    /// architecture-mismatch: graph writes route server-side, never through this trait method.
    fn set_graph(&self, _graph: DependencyGraph) -> KanbanResult<()> {
        Err(KanbanError::unsupported("set_graph"))
    }

    /// architecture-mismatch: graph writes route server-side; the inherited
    /// default would call `get_graph()` then `set_graph()` as two
    /// operations, and `set_graph` always declines here, so the default
    /// would surface a transport/route error instead of an honest decline.
    fn modify_graph(&self, _f: kanban_domain::GraphMutFn) -> KanbanResult<()> {
        Err(KanbanError::unsupported("modify_graph"))
    }

    /// architecture-mismatch: the lookup route is prefix-keyed, not board-id-keyed; declines under its own name instead of inheriting the `list_all_cards`-walking default.
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

    fn list_cards_by_number(&self, card_number: u32) -> KanbanResult<Vec<Card>> {
        self.lookup_cards(&card_number.to_string())
    }

    /// Re-encodes the pair as `{prefix}-{card_number}` and re-parses it
    /// server-side, faithful because `parse_identifier` splits on the last
    /// dash and lowercases both sides. An empty `prefix` is the one shape
    /// that cannot round-trip through that reconstruction, but no caller
    /// reaches this method with one: `find_cards_by_identifier` only ever
    /// dispatches here with a prefix `parse_identifier` has already accepted,
    /// and it rejects the empty-prefix shape before dispatch.
    fn list_cards_by_prefix_and_number(
        &self,
        prefix: &str,
        card_number: u32,
    ) -> KanbanResult<Vec<Card>> {
        self.lookup_cards(&format!("{prefix}-{card_number}"))
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

fn archived_filter_param(archived: kanban_domain::ArchivedFilter) -> &'static str {
    match archived {
        kanban_domain::ArchivedFilter::LiveOnly => "live_only",
        kanban_domain::ArchivedFilter::ArchivedOnly => "archived_only",
        kanban_domain::ArchivedFilter::Include => "include",
    }
}
