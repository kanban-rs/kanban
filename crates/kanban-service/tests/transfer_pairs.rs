#![cfg(feature = "test-helpers")]

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use kanban_backend_memory::InMemoryStore;
use kanban_core::{AppConfig, Edge};
use kanban_domain::{
    Archived, ArchivedBoard, ArchivedCard, Board, Card, Column, DataStore, KanbanResult, Prefix,
    Sprint,
};
use kanban_persistence_json::{JsonDataStore, JsonFileStore};
use kanban_persistence_sqlite::SqliteBackend;
use kanban_service::test_helpers::contract::assert_card_eq;
use kanban_service::test_helpers::BackendFactory;
use kanban_service::{KanbanBackend, KanbanContext};
use tempfile::TempDir;
use uuid::Uuid;

fn json_backend_factory() -> BackendFactory {
    Box::new(|path: &Path| {
        Arc::new(JsonDataStore::new(Arc::new(JsonFileStore::new(path)))) as Arc<dyn KanbanBackend>
    })
}

fn sqlite_backend_factory() -> BackendFactory {
    Box::new(|path: &Path| {
        let path = path.to_path_buf();
        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_multi_thread()
                .worker_threads(1)
                .enable_all()
                .build()
                .expect("multi-thread runtime");
            let backend = rt
                .block_on(SqliteBackend::open(path.to_str().unwrap()))
                .expect("open sqlite backend");
            Arc::new(backend) as Arc<dyn KanbanBackend>
        })
        .join()
        .expect("sqlite open thread")
    })
}

fn in_memory_backend_factory() -> BackendFactory {
    let registry: Arc<Mutex<HashMap<PathBuf, Arc<dyn KanbanBackend>>>> =
        Arc::new(Mutex::new(HashMap::new()));
    Box::new(move |path: &Path| {
        let mut map = registry.lock().unwrap();
        map.entry(path.to_path_buf())
            .or_insert_with(|| Arc::new(InMemoryStore::new()) as Arc<dyn KanbanBackend>)
            .clone()
    })
}

fn backends() -> Vec<(&'static str, BackendFactory)> {
    vec![
        ("in_memory", in_memory_backend_factory()),
        ("json", json_backend_factory()),
        ("sqlite", sqlite_backend_factory()),
    ]
}

struct SeedFixture {
    boards: Vec<Board>,
    archived_board: Board,
    archived_board_marker: ArchivedBoard,
    columns: Vec<Column>,
    cards: Vec<Card>,
    archived_cards: Vec<ArchivedCard>,
    sprints: Vec<Sprint>,
    prefixes: Vec<Prefix>,
    dangling_card_id: Uuid,
    dangling_card_prior_column_id: Uuid,
    live_block_edge: (Uuid, Uuid),
    archived_endpoint_relate_edge: (Uuid, Uuid),
}

/// Seeds a non-trivial workspace directly through per-entity `DataStore`
/// calls (2 live boards, an archived board with its own subtree, an archived
/// card with a live board and one with a dangling `column_id`, dependency
/// edges including an archived endpoint, and prefix rows with non-zero
/// counters), then reads every entity back so the caller has exact expected
/// values rather than re-deriving them from the constructors.
fn seed_rich(store: &dyn DataStore) -> KanbanResult<SeedFixture> {
    let kan = Prefix {
        name: "kan".into(),
        card_counter: 5,
        sprint_counter: 2,
    };
    let arc = Prefix {
        name: "arc".into(),
        card_counter: 3,
        sprint_counter: 1,
    };
    store.upsert_prefix(kan.clone())?;
    store.upsert_prefix(arc.clone())?;

    let board_a = Board::new("Board A", Some("kan"));
    let mut col_a1 = Column::new(board_a.id, "Todo", 0);
    col_a1.wip_limit = Some(3);
    let mut col_a2 = Column::new(board_a.id, "Doing", 1);
    col_a2.wip_limit = Some(5);
    let col_a_temp = Column::new(board_a.id, "Temp", 2);

    let sprint_a = Sprint::new(board_a.id, 1, None, None::<String>);

    let mut card_a1 = Card::new(board_a.id, col_a1.id, "A1", 0);
    card_a1.prefix = "kan".into();
    card_a1.card_number = 1;
    let mut card_a2 = Card::new(board_a.id, col_a1.id, "A2", 1);
    card_a2.prefix = "kan".into();
    card_a2.card_number = 2;
    card_a2.sprint_id = Some(sprint_a.id);
    let mut card_a3 = Card::new(board_a.id, col_a2.id, "A3", 0);
    card_a3.prefix = "kan".into();
    card_a3.card_number = 3;
    let mut card_a_archived_live = Card::new(board_a.id, col_a1.id, "A archived", 2);
    card_a_archived_live.prefix = "kan".into();
    card_a_archived_live.card_number = 4;
    let mut card_a_dangling = Card::new(board_a.id, col_a_temp.id, "A dangling", 0);
    card_a_dangling.prefix = "kan".into();
    card_a_dangling.card_number = 5;

    store.upsert_board(board_a.clone())?;
    store.upsert_column(col_a1.clone())?;
    store.upsert_column(col_a2.clone())?;
    store.upsert_column(col_a_temp.clone())?;
    store.upsert_sprint(sprint_a.clone())?;
    store.upsert_card(card_a1.clone())?;
    store.upsert_card(card_a2.clone())?;
    store.upsert_card(card_a3.clone())?;
    store.upsert_card(card_a_archived_live.clone())?;
    store.upsert_card(card_a_dangling.clone())?;

    let archived_card_marker = ArchivedCard::new(card_a_archived_live.id, board_a.id);
    store.insert_archived_card(archived_card_marker)?;
    let dangling_marker = ArchivedCard::new(card_a_dangling.id, board_a.id);
    store.insert_archived_card(dangling_marker)?;
    store.delete_column(col_a_temp.id)?;

    let board_b = Board::new("Board B", Some("arc"));
    let mut col_b1 = Column::new(board_b.id, "Backlog", 0);
    col_b1.wip_limit = Some(2);
    let col_b2 = Column::new(board_b.id, "Review", 1);

    let sprint_b = Sprint::new(board_b.id, 1, None, None::<String>);

    let mut card_b1 = Card::new(board_b.id, col_b1.id, "B1", 0);
    card_b1.prefix = "arc".into();
    card_b1.card_number = 1;
    let mut card_b2 = Card::new(board_b.id, col_b1.id, "B2", 1);
    card_b2.prefix = "arc".into();
    card_b2.card_number = 2;
    card_b2.sprint_id = Some(sprint_b.id);
    let mut card_b3 = Card::new(board_b.id, col_b2.id, "B3", 0);
    card_b3.prefix = "arc".into();
    card_b3.card_number = 3;

    store.upsert_board(board_b.clone())?;
    store.upsert_column(col_b1.clone())?;
    store.upsert_column(col_b2.clone())?;
    store.upsert_sprint(sprint_b.clone())?;
    store.upsert_card(card_b1.clone())?;
    store.upsert_card(card_b2.clone())?;
    store.upsert_card(card_b3.clone())?;

    let board_c = Board::new("Archived board C", None::<String>);
    let col_c1 = Column::new(board_c.id, "Done", 0);
    let mut card_c1 = Card::new(board_c.id, col_c1.id, "C1", 0);
    card_c1.prefix = "arc".into();
    let sprint_c = Sprint::new(board_c.id, 1, None, None::<String>);

    store.upsert_board(board_c.clone())?;
    store.upsert_column(col_c1.clone())?;
    store.upsert_card(card_c1.clone())?;
    store.upsert_sprint(sprint_c.clone())?;
    let archived_board_marker = Archived::now(board_c.id);
    store.insert_archived_board(archived_board_marker)?;

    let live_block_edge = (card_a1.id, card_a2.id);
    let archived_endpoint_relate_edge = (card_c1.id, card_b1.id);
    let dangling_card_id = card_a_dangling.id;

    store.modify_graph(Box::new({
        let (a, b) = live_block_edge;
        move |g| g.set_block(a, b)
    }))?;
    store.modify_graph(Box::new({
        let (a, b) = archived_endpoint_relate_edge;
        move |g| g.relate(a, b)
    }))?;

    Ok(SeedFixture {
        boards: vec![board_a, board_b],
        archived_board: board_c,
        archived_board_marker,
        columns: vec![col_a1, col_a2, col_b1, col_b2, col_c1],
        cards: vec![
            card_a1,
            card_a2,
            card_a3,
            card_a_archived_live,
            card_a_dangling,
            card_b1,
            card_b2,
            card_b3,
            card_c1,
        ],
        archived_cards: vec![archived_card_marker, dangling_marker],
        sprints: vec![sprint_a, sprint_b, sprint_c],
        prefixes: vec![kan, arc],
        dangling_card_id,
        dangling_card_prior_column_id: col_a_temp.id,
        live_block_edge,
        archived_endpoint_relate_edge,
    })
}

fn assert_full_card(a: &Card, b: &Card) {
    assert_card_eq(a, b);
    assert_eq!(a.prefix, b.prefix, "card prefix");
}

fn find_by_id<T: Clone>(items: &[T], id: Uuid, get_id: impl Fn(&T) -> Uuid) -> T {
    items
        .iter()
        .find(|item| get_id(item) == id)
        .cloned()
        .unwrap_or_else(|| panic!("expected id {id} missing from destination"))
}

fn assert_transfer_matches(fixture: &SeedFixture, dst: &dyn DataStore) {
    let dst_boards_list = dst.list_boards().unwrap();
    for expected in &fixture.boards {
        let actual = find_by_id(&dst_boards_list, expected.id, |b| b.id);
        assert_eq!(&actual, expected, "live board {}", expected.id);
    }

    let dst_archived_board = dst
        .get_board(fixture.archived_board.id)
        .unwrap()
        .expect("archived board head must survive the transfer");
    assert_eq!(
        dst_archived_board, fixture.archived_board,
        "archived board head"
    );

    let dst_archived_boards = dst.list_archived_boards().unwrap();
    let actual_marker = find_by_id(
        &dst_archived_boards,
        fixture.archived_board_marker.entity_id,
        |a| a.entity_id,
    );
    assert_eq!(
        actual_marker, fixture.archived_board_marker,
        "archived board marker"
    );

    let dst_columns_list = dst.list_all_columns().unwrap();
    for expected in &fixture.columns {
        let actual = find_by_id(&dst_columns_list, expected.id, |c| c.id);
        assert_eq!(&actual, expected, "column {}", expected.id);
    }

    for expected in &fixture.cards {
        let actual = dst
            .get_card(expected.id)
            .unwrap()
            .unwrap_or_else(|| panic!("card {} missing on destination", expected.id));
        assert_full_card(&actual, expected);
    }

    let dangling = dst.get_card(fixture.dangling_card_id).unwrap().unwrap();
    assert_eq!(
        dangling.column_id, fixture.dangling_card_prior_column_id,
        "the archived card's dangling column_id must survive untouched"
    );

    let dst_archived_cards_list = dst.list_archived_cards().unwrap();
    for expected in &fixture.archived_cards {
        let actual = find_by_id(&dst_archived_cards_list, expected.entity_id, |a| {
            a.entity_id
        });
        assert_eq!(
            actual, *expected,
            "archived card marker {}",
            expected.entity_id
        );
    }

    let dst_sprints_list = dst.list_all_sprints().unwrap();
    for expected in &fixture.sprints {
        let actual = find_by_id(&dst_sprints_list, expected.id, |s| s.id);
        assert_eq!(&actual, expected, "sprint {}", expected.id);
    }

    let graph = dst.get_graph().unwrap();
    let blocks: Vec<(Uuid, Uuid)> = graph
        .blocks_edges()
        .iter()
        .map(|e| (e.source(), e.target()))
        .collect();
    assert!(
        blocks.contains(&fixture.live_block_edge),
        "block edge {:?} missing from destination edges {:?}",
        fixture.live_block_edge,
        blocks
    );

    let relates: Vec<(Uuid, Uuid)> = graph
        .relates_edges()
        .iter()
        .map(|e| {
            let (a, b) = (e.source(), e.target());
            if a <= b {
                (a, b)
            } else {
                (b, a)
            }
        })
        .collect();
    let (ea, eb) = fixture.archived_endpoint_relate_edge;
    let expected_relate = if ea <= eb { (ea, eb) } else { (eb, ea) };
    assert!(
        relates.contains(&expected_relate),
        "relate edge {expected_relate:?} missing from destination edges {relates:?}"
    );

    for expected in &fixture.prefixes {
        let actual = dst
            .get_prefix(&expected.name)
            .unwrap()
            .unwrap_or_else(|| panic!("prefix {} missing from destination", expected.name));
        assert_eq!(actual, *expected, "prefix {}", expected.name);
    }
}

async fn open_ctx(factory: &BackendFactory, path: &Path) -> KanbanContext {
    KanbanContext::open(factory(path), AppConfig::default())
        .await
        .unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn test_transfer_state_to_is_lossless_for_every_backend_pair() {
    for (src_name, src_factory) in backends() {
        for (dst_name, dst_factory) in backends() {
            let dir = TempDir::new().unwrap();
            let src_path = dir.path().join("src.store");
            let src_ctx = open_ctx(&src_factory, &src_path).await;
            let fixture = seed_rich(src_ctx.data_store()).unwrap();
            src_ctx.save().await.unwrap();

            let dst_path = dir.path().join("dst.store");
            let dst_ctx = open_ctx(&dst_factory, &dst_path).await;

            src_ctx
                .transfer_state_to(dst_ctx.data_store())
                .unwrap_or_else(|e| {
                    panic!("transfer {src_name} -> {dst_name} failed: {e}");
                });
            dst_ctx.save().await.unwrap();

            let reopened = open_ctx(&dst_factory, &dst_path).await;
            assert_transfer_matches(&fixture, reopened.data_store());
        }
    }
}

struct NoWholeStoreReads(InMemoryStore);

impl DataStore for NoWholeStoreReads {
    fn get_prefix(&self, name: &str) -> KanbanResult<Option<Prefix>> {
        self.0.get_prefix(name)
    }
    fn list_prefixes(&self) -> KanbanResult<Vec<Prefix>> {
        self.0.list_prefixes()
    }
    fn upsert_prefix(&self, prefix: Prefix) -> KanbanResult<()> {
        self.0.upsert_prefix(prefix)
    }
    fn snapshot(&self) -> KanbanResult<kanban_domain::Snapshot> {
        panic!("transfer_state_to must compose per-entity reads, not call snapshot()")
    }
    fn apply_snapshot(&self, _snapshot: kanban_domain::Snapshot) -> KanbanResult<()> {
        panic!("transfer_state_to must compose per-entity writes, not call apply_snapshot()")
    }
    fn get_board(&self, id: Uuid) -> KanbanResult<Option<Board>> {
        self.0.get_board(id)
    }
    fn list_boards(&self) -> KanbanResult<Vec<Board>> {
        self.0.list_boards()
    }
    fn upsert_board(&self, board: Board) -> KanbanResult<()> {
        self.0.upsert_board(board)
    }
    fn delete_board(&self, id: Uuid) -> KanbanResult<()> {
        self.0.delete_board(id)
    }
    fn get_column(&self, id: Uuid) -> KanbanResult<Option<Column>> {
        self.0.get_column(id)
    }
    fn list_columns_by_board(&self, board_id: Uuid) -> KanbanResult<Vec<Column>> {
        self.0.list_columns_by_board(board_id)
    }
    fn list_all_columns(&self) -> KanbanResult<Vec<Column>> {
        self.0.list_all_columns()
    }
    fn upsert_column(&self, column: Column) -> KanbanResult<()> {
        self.0.upsert_column(column)
    }
    fn delete_column(&self, id: Uuid) -> KanbanResult<()> {
        self.0.delete_column(id)
    }
    fn delete_columns_by_board(&self, board_id: Uuid) -> KanbanResult<()> {
        self.0.delete_columns_by_board(board_id)
    }
    fn get_card(&self, id: Uuid) -> KanbanResult<Option<Card>> {
        self.0.get_card(id)
    }
    fn list_all_cards(&self) -> KanbanResult<Vec<Card>> {
        self.0.list_all_cards()
    }
    fn list_cards_by_column(&self, column_id: Uuid) -> KanbanResult<Vec<Card>> {
        self.0.list_cards_by_column(column_id)
    }
    fn list_cards_by_sprint(&self, sprint_id: Uuid) -> KanbanResult<Vec<Card>> {
        self.0.list_cards_by_sprint(sprint_id)
    }
    fn count_cards_in_column(&self, column_id: Uuid) -> KanbanResult<usize> {
        self.0.count_cards_in_column(column_id)
    }
    fn count_cards_in_column_excluding(
        &self,
        column_id: Uuid,
        exclude: &[Uuid],
    ) -> KanbanResult<usize> {
        self.0.count_cards_in_column_excluding(column_id, exclude)
    }
    fn upsert_card(&self, card: Card) -> KanbanResult<()> {
        self.0.upsert_card(card)
    }
    fn delete_card(&self, id: Uuid) -> KanbanResult<()> {
        self.0.delete_card(id)
    }
    fn delete_cards_by_columns(&self, column_ids: &[Uuid]) -> KanbanResult<()> {
        self.0.delete_cards_by_columns(column_ids)
    }
    fn list_archived_cards(&self) -> KanbanResult<Vec<ArchivedCard>> {
        self.0.list_archived_cards()
    }
    fn list_archived_cards_by_board(&self, board_id: Uuid) -> KanbanResult<Vec<ArchivedCard>> {
        self.0.list_archived_cards_by_board(board_id)
    }
    fn insert_archived_card(&self, ac: ArchivedCard) -> KanbanResult<()> {
        self.0.insert_archived_card(ac)
    }
    fn get_archived_card(&self, card_id: Uuid) -> KanbanResult<Option<ArchivedCard>> {
        self.0.get_archived_card(card_id)
    }
    fn delete_archived_card(&self, card_id: Uuid) -> KanbanResult<()> {
        self.0.delete_archived_card(card_id)
    }
    fn get_archived_board(&self, board_id: Uuid) -> KanbanResult<Option<ArchivedBoard>> {
        self.0.get_archived_board(board_id)
    }
    fn list_archived_boards(&self) -> KanbanResult<Vec<ArchivedBoard>> {
        self.0.list_archived_boards()
    }
    fn insert_archived_board(&self, ab: ArchivedBoard) -> KanbanResult<()> {
        self.0.insert_archived_board(ab)
    }
    fn delete_archived_board(&self, board_id: Uuid) -> KanbanResult<()> {
        self.0.delete_archived_board(board_id)
    }
    fn get_sprint(&self, id: Uuid) -> KanbanResult<Option<Sprint>> {
        self.0.get_sprint(id)
    }
    fn list_sprints_by_board(&self, board_id: Uuid) -> KanbanResult<Vec<Sprint>> {
        self.0.list_sprints_by_board(board_id)
    }
    fn list_all_sprints(&self) -> KanbanResult<Vec<Sprint>> {
        self.0.list_all_sprints()
    }
    fn upsert_sprint(&self, sprint: Sprint) -> KanbanResult<()> {
        self.0.upsert_sprint(sprint)
    }
    fn delete_sprint(&self, id: Uuid) -> KanbanResult<()> {
        self.0.delete_sprint(id)
    }
    fn delete_sprints_by_board(&self, board_id: Uuid) -> KanbanResult<()> {
        self.0.delete_sprints_by_board(board_id)
    }
    fn clear_sprint_from_cards(
        &self,
        sprint_id: Uuid,
        timestamp: chrono::DateTime<chrono::Utc>,
    ) -> KanbanResult<()> {
        self.0.clear_sprint_from_cards(sprint_id, timestamp)
    }
    fn get_graph(&self) -> KanbanResult<kanban_domain::DependencyGraph> {
        self.0.get_graph()
    }
    fn set_graph(&self, graph: kanban_domain::DependencyGraph) -> KanbanResult<()> {
        self.0.set_graph(graph)
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn test_transfer_state_to_does_not_call_the_whole_store_trait_methods() {
    let dir = TempDir::new().unwrap();
    let src_path = dir.path().join("src.store");
    let src_ctx = open_ctx(&in_memory_backend_factory(), &src_path).await;
    seed_rich(src_ctx.data_store()).unwrap();

    let wrapped = NoWholeStoreReads(InMemoryStore::new());
    src_ctx
        .transfer_state_to(&wrapped)
        .expect("transfer must complete without calling snapshot()/apply_snapshot()");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_transfer_state_to_into_a_populated_target_is_an_upsert_not_a_wipe() {
    for (dst_name, dst_factory) in backends() {
        let dir = TempDir::new().unwrap();
        let dst_path = dir.path().join("dst.store");
        let dst_ctx = open_ctx(&dst_factory, &dst_path).await;

        let unrelated_prefix = Prefix {
            name: "unr".into(),
            card_counter: 9,
            sprint_counter: 1,
        };
        dst_ctx
            .data_store()
            .upsert_prefix(unrelated_prefix.clone())
            .unwrap();
        let unrelated_board = Board::new("Unrelated", Some("unr"));
        let unrelated_column = Column::new(unrelated_board.id, "Todo", 0);
        let mut unrelated_card = Card::new(unrelated_board.id, unrelated_column.id, "U1", 0);
        unrelated_card.prefix = "unr".into();
        unrelated_card.card_number = 1;
        let unrelated_sprint = Sprint::new(unrelated_board.id, 1, None, None::<String>);
        dst_ctx
            .data_store()
            .upsert_board(unrelated_board.clone())
            .unwrap();
        dst_ctx
            .data_store()
            .upsert_column(unrelated_column.clone())
            .unwrap();
        dst_ctx
            .data_store()
            .upsert_card(unrelated_card.clone())
            .unwrap();
        dst_ctx
            .data_store()
            .upsert_sprint(unrelated_sprint.clone())
            .unwrap();
        dst_ctx.save().await.unwrap();

        let src_path = dir.path().join("src.store");
        let src_ctx = open_ctx(&in_memory_backend_factory(), &src_path).await;
        let fixture = seed_rich(src_ctx.data_store()).unwrap();

        src_ctx
            .transfer_state_to(dst_ctx.data_store())
            .unwrap_or_else(|e| panic!("transfer into populated {dst_name} failed: {e}"));
        dst_ctx.save().await.unwrap();

        let reopened = open_ctx(&dst_factory, &dst_path).await;
        let store = reopened.data_store();

        let kept_board = store.get_board(unrelated_board.id).unwrap().unwrap();
        assert_eq!(
            kept_board, unrelated_board,
            "unrelated board must survive untouched"
        );
        let kept_column = store.get_column(unrelated_column.id).unwrap().unwrap();
        assert_eq!(
            kept_column, unrelated_column,
            "unrelated column must survive untouched"
        );
        let kept_card = store.get_card(unrelated_card.id).unwrap().unwrap();
        assert_full_card(&kept_card, &unrelated_card);
        let kept_sprint = store.get_sprint(unrelated_sprint.id).unwrap().unwrap();
        assert_eq!(
            kept_sprint, unrelated_sprint,
            "unrelated sprint must survive untouched"
        );
        let kept_prefix = store.get_prefix("unr").unwrap().unwrap();
        assert_eq!(
            kept_prefix.card_counter, unrelated_prefix.card_counter,
            "unrelated prefix counter must survive untouched"
        );

        assert_transfer_matches(&fixture, store);
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn test_transfer_state_to_leaves_the_source_untouched() {
    for (src_name, src_factory) in [
        ("sqlite", sqlite_backend_factory()),
        ("json", json_backend_factory()),
    ] {
        let dir = TempDir::new().unwrap();
        let src_path = dir.path().join("src.store");
        let src_ctx = open_ctx(&src_factory, &src_path).await;
        let fixture = seed_rich(src_ctx.data_store()).unwrap();
        src_ctx.save().await.unwrap();

        let dst_path = dir.path().join("dst.store");
        let dst_factory = sqlite_backend_factory();
        let dst_ctx = open_ctx(&dst_factory, &dst_path).await;

        src_ctx
            .transfer_state_to(dst_ctx.data_store())
            .unwrap_or_else(|e| panic!("transfer from {src_name} failed: {e}"));

        assert_transfer_matches(&fixture, src_ctx.data_store());
    }
}

struct UnsupportedArchivedBoards(InMemoryStore);

impl DataStore for UnsupportedArchivedBoards {
    fn get_prefix(&self, name: &str) -> KanbanResult<Option<Prefix>> {
        self.0.get_prefix(name)
    }
    fn list_prefixes(&self) -> KanbanResult<Vec<Prefix>> {
        self.0.list_prefixes()
    }
    fn upsert_prefix(&self, prefix: Prefix) -> KanbanResult<()> {
        self.0.upsert_prefix(prefix)
    }
    fn get_board(&self, id: Uuid) -> KanbanResult<Option<Board>> {
        self.0.get_board(id)
    }
    fn list_boards(&self) -> KanbanResult<Vec<Board>> {
        self.0.list_boards()
    }
    fn upsert_board(&self, board: Board) -> KanbanResult<()> {
        self.0.upsert_board(board)
    }
    fn delete_board(&self, id: Uuid) -> KanbanResult<()> {
        self.0.delete_board(id)
    }
    fn get_column(&self, id: Uuid) -> KanbanResult<Option<Column>> {
        self.0.get_column(id)
    }
    fn list_columns_by_board(&self, board_id: Uuid) -> KanbanResult<Vec<Column>> {
        self.0.list_columns_by_board(board_id)
    }
    fn list_all_columns(&self) -> KanbanResult<Vec<Column>> {
        self.0.list_all_columns()
    }
    fn upsert_column(&self, column: Column) -> KanbanResult<()> {
        self.0.upsert_column(column)
    }
    fn delete_column(&self, id: Uuid) -> KanbanResult<()> {
        self.0.delete_column(id)
    }
    fn delete_columns_by_board(&self, board_id: Uuid) -> KanbanResult<()> {
        self.0.delete_columns_by_board(board_id)
    }
    fn get_card(&self, id: Uuid) -> KanbanResult<Option<Card>> {
        self.0.get_card(id)
    }
    fn list_all_cards(&self) -> KanbanResult<Vec<Card>> {
        self.0.list_all_cards()
    }
    fn list_cards_by_column(&self, column_id: Uuid) -> KanbanResult<Vec<Card>> {
        self.0.list_cards_by_column(column_id)
    }
    fn list_cards_by_sprint(&self, sprint_id: Uuid) -> KanbanResult<Vec<Card>> {
        self.0.list_cards_by_sprint(sprint_id)
    }
    fn count_cards_in_column(&self, column_id: Uuid) -> KanbanResult<usize> {
        self.0.count_cards_in_column(column_id)
    }
    fn count_cards_in_column_excluding(
        &self,
        column_id: Uuid,
        exclude: &[Uuid],
    ) -> KanbanResult<usize> {
        self.0.count_cards_in_column_excluding(column_id, exclude)
    }
    fn upsert_card(&self, card: Card) -> KanbanResult<()> {
        self.0.upsert_card(card)
    }
    fn delete_card(&self, id: Uuid) -> KanbanResult<()> {
        self.0.delete_card(id)
    }
    fn delete_cards_by_columns(&self, column_ids: &[Uuid]) -> KanbanResult<()> {
        self.0.delete_cards_by_columns(column_ids)
    }
    fn list_archived_cards(&self) -> KanbanResult<Vec<ArchivedCard>> {
        self.0.list_archived_cards()
    }
    fn list_archived_cards_by_board(&self, board_id: Uuid) -> KanbanResult<Vec<ArchivedCard>> {
        self.0.list_archived_cards_by_board(board_id)
    }
    fn insert_archived_card(&self, ac: ArchivedCard) -> KanbanResult<()> {
        self.0.insert_archived_card(ac)
    }
    fn get_archived_card(&self, card_id: Uuid) -> KanbanResult<Option<ArchivedCard>> {
        self.0.get_archived_card(card_id)
    }
    fn delete_archived_card(&self, card_id: Uuid) -> KanbanResult<()> {
        self.0.delete_archived_card(card_id)
    }
    fn get_archived_board(&self, board_id: Uuid) -> KanbanResult<Option<ArchivedBoard>> {
        self.0.get_archived_board(board_id)
    }
    fn list_archived_boards(&self) -> KanbanResult<Vec<ArchivedBoard>> {
        self.0.list_archived_boards()
    }
    // Uses the trait default: Err(unsupported("insert_archived_board")).
    fn delete_archived_board(&self, board_id: Uuid) -> KanbanResult<()> {
        self.0.delete_archived_board(board_id)
    }
    fn snapshot(&self) -> KanbanResult<kanban_domain::Snapshot> {
        self.0.snapshot()
    }
    fn apply_snapshot(&self, snapshot: kanban_domain::Snapshot) -> KanbanResult<()> {
        self.0.apply_snapshot(snapshot)
    }
    fn get_sprint(&self, id: Uuid) -> KanbanResult<Option<Sprint>> {
        self.0.get_sprint(id)
    }
    fn list_sprints_by_board(&self, board_id: Uuid) -> KanbanResult<Vec<Sprint>> {
        self.0.list_sprints_by_board(board_id)
    }
    fn list_all_sprints(&self) -> KanbanResult<Vec<Sprint>> {
        self.0.list_all_sprints()
    }
    fn upsert_sprint(&self, sprint: Sprint) -> KanbanResult<()> {
        self.0.upsert_sprint(sprint)
    }
    fn delete_sprint(&self, id: Uuid) -> KanbanResult<()> {
        self.0.delete_sprint(id)
    }
    fn delete_sprints_by_board(&self, board_id: Uuid) -> KanbanResult<()> {
        self.0.delete_sprints_by_board(board_id)
    }
    fn clear_sprint_from_cards(
        &self,
        sprint_id: Uuid,
        timestamp: chrono::DateTime<chrono::Utc>,
    ) -> KanbanResult<()> {
        self.0.clear_sprint_from_cards(sprint_id, timestamp)
    }
    fn get_graph(&self) -> KanbanResult<kanban_domain::DependencyGraph> {
        self.0.get_graph()
    }
    fn set_graph(&self, graph: kanban_domain::DependencyGraph) -> KanbanResult<()> {
        self.0.set_graph(graph)
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn test_transfer_state_to_a_backend_that_cannot_accept_it_fails_loud() {
    let dir = TempDir::new().unwrap();
    let src_path = dir.path().join("src.store");
    let src_ctx = open_ctx(&in_memory_backend_factory(), &src_path).await;
    seed_rich(src_ctx.data_store()).unwrap();

    let target = UnsupportedArchivedBoards(InMemoryStore::new());
    let result = src_ctx.transfer_state_to(&target);

    assert!(
        result.is_err(),
        "a target that cannot accept insert_archived_board must surface the error, not swallow it"
    );
}
