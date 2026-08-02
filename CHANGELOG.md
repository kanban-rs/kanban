## [0.8.0] - 2026-08-02 ([#379](https://github.com/fulsomenko/kanban/pull/379))

### Other Changes (2026-08-02)

Internal maintenance with no user-visible effect: bumps the `rust-overlay` flake input so `nix develop` resolves stable Rust 1.97.1 instead of 1.93.1 (needed by an in-flight branch depending on a crate with a newer MSRV). The newer clippy this pulls in flags a handful of pre-existing patterns (`manual_checked_ops`, `unnecessary_sort_by`, `collapsible_match`) across `kanban-core`, `kanban-domain`, `kanban-backend-memory`, and `kanban-tui`; all are fixed mechanically with no behavior change.

### Other Changes (2026-08-02)

Correct the CLAUDE.md architecture reference: the dependency graph now shows the JSON and SQLite persistence crates depending on kanban-backend and kanban-backend-memory (they currently host their KanbanBackend adapters), both crate descriptions note that adapter placement, and the JSON envelope version is updated from V10 to the current V11.

### Other Changes (2026-08-02)

Add architecture READMEs (root + one per crate) with dependency diagrams reflecting the post-KAN-1027 workspace graph.

### Other Changes (2026-08-02)

`kanban-service`: `import_board_impl` now runs transactionally and appends an
audit-log entry, matching every other mutation's guarantee. Previously a
successful board import left no audit-log record and had no rollback
guarantee on partial failure. Import still clears undo history afterward
(unchanged) — it remains intentionally not undoable via the normal undo stack.

### KAN-1023 Extract In Memory Store Into Backend Memory Crate (2026-08-02)

Internal restructuring with no user-visible effect: the in-memory store now lives in its own `kanban-backend-memory` crate instead of inside `kanban-domain`. It becomes an ordinary backend alongside JSON, SQLite, and HTTP, so an application can choose to run with no storage backend, with one, or with a combination, rather than always inheriting an in-memory implementation from the domain layer. `kanban-domain` is now purely entities, commands, and trait definitions. All existing behaviour, on-disk formats, and command-line usage are unchanged.

### KAN-1024 Add Kanban Backend Factory Trait And Registry (2026-08-02)

Internal groundwork with no user-visible effect: backends can now be described by a factory registered against a URI scheme, rather than being named directly by the service layer. Nothing uses the registry yet, so behaviour, storage formats, and commands are unchanged. This is the seam that will later let an application decide for itself which backends exist, including pointing the CLI, TUI, or MCP server at a remote server instead of a local file.

### KAN-1025 Move Json Backend Adapter Out Of Service (2026-08-02)

Internal groundwork with no user-visible effect: the JSON backend adapter now lives in `kanban-persistence-json`, alongside the store it wraps, instead of `kanban-service`. A new `JsonBackendFactory` reports its backend name (`"json"`) and constructs a `JsonDataStore` directly from a locator. `kanban-service` still constructs the JSON backend directly; behaviour, storage formats, and commands are unchanged.

### KAN-1026 Move Sqlite Backend Adapter Out Of Service (2026-08-02)

Internal groundwork with no user-visible effect: the SQLite backend adapter now lives in `kanban-persistence-sqlite`, alongside the store it wraps, instead of `kanban-service`. The `KanbanBackendFactory` trait's `scheme()` method is renamed to `name()` to match the registry's dispatch-by-backend-name semantics. `kanban-service` still constructs the SQLite backend directly; behaviour, storage formats, and commands are unchanged.

### KAN-1029 Local Persistence Capability Accessor (2026-08-02)

Internal groundwork with no user-visible effect: `KanbanBackend::persistence_metadata()` is
replaced by a `local_persistence()` capability accessor returning `Option<&dyn
LocalPersistence>`, mirroring the existing `remote_writes()` pattern. Only the JSON and SQLite
backends implement `LocalPersistence`; the in-memory and HTTP backends simply have no
capability to return, instead of being forced to answer a question they cannot meaningfully
answer. The TUI's F12 diagnostics panel still shows the writer stamp exactly as before;
behaviour, storage formats, and commands are unchanged.
Completes the KAN-1021 epic.

### KAN-1032 Add Locator Dispatch To Backend Registry (2026-08-02)

Internal groundwork with no user-visible effect: `KanbanBackendFactory` gains a `matches_locator` method (default: never matches by content) and `KanbanBackendRegistry` gains `for_locator`, which reads up to the first 32 bytes of a locator and asks each registered factory in registration order whether it should handle it. This mirrors the existing `StoreRegistry::detect_backend` dispatch pattern one layer down. `SqliteBackendFactory` now matches SQLite magic bytes or a `.sqlite`/`.sqlite3`/`.db` extension on a new file; `JsonBackendFactory` is the catch-all. Nothing calls `for_locator` yet, so behaviour, storage formats, and commands are unchanged.

### KAN-1033 Registry Based Backend Dispatch (2026-08-02)

**BREAKING:** `McpServer::register_backend` and `CliApp::register_backend` now take two
arguments, `(Box<dyn StoreFactory>, Box<dyn KanbanBackendFactory>)`, instead of one. A
backend registered through the old single-argument signature could pass `has_backends()`
while still being unreachable from `make_backend`, since only the store-level registry was
populated. The new signature makes that failure mode structurally impossible: registering a
backend now always wires both dispatch paths together. Both types also gain a `backends()`
accessor mirroring the existing `registry()`.
Otherwise this is internal groundwork with no other user-visible effect: `StoreManager`
now dispatches `make_backend` purely through an injected `KanbanBackendRegistry` instead of
consulting `#[cfg(feature = ...)]` blocks internally, and each application (`kanban-cli`,
`kanban-mcp`, `kanban-tui`, `kanban-server`) assembles its own pair of registries rather than
asking `kanban-service` for a pre-populated one. Storage formats, commands, and CLI/MCP/TUI
behavior are unchanged.

### KAN-1034 Drop Json Feature From Kanban Service (2026-08-02)

Removes the `json` feature from `kanban-service`. `sqlite` is now the crate's
only optional persistence feature; `kanban-persistence-json` and
`kanban-backend-memory` moved from optional production dependencies to
unconditional dev-dependencies.
This is internal groundwork with no user-visible behavior change:
`kanban-service`'s production surface no longer depends on the JSON
persistence crate directly. `kanban-cli` and `kanban-mcp` already gate their
own `kanban-persistence-json` dependency independently, so nothing changes
for consumers of those binaries. Storage formats, commands, and CLI/MCP/TUI
behavior are unchanged.
Completes the KAN-1027 split (final child, after KAN-1032 and KAN-1033).

### KAN-675 Add Nixpkgs Stable Badge To Readme And Switch Web Nix Install To Stable (2026-08-02)

kanban is now packaged in nixpkgs 26.05 stable. The README gains a dedicated
"nixpkgs stable" version badge alongside the existing unstable one, so you can
see the latest version available on each channel at a glance. The project
website's install instructions now point at the stable channel, installing with
`nix run nixpkgs#kanban` instead of the unstable flake reference.

### KAN-712 Add Api Module To Kanban Service (2026-08-02)

Internal infrastructure change with no user-visible behaviour difference. Adds the `api` module to kanban-service (`kanban_service::api`) holding the shared HTTP wire types for the upcoming collaborative backend: ApiError (the HTTP error envelope) and ChangeEventFrame (the SSE frame carrying writer identity, correlation ID, and timestamp). These form the contract between the server and all clients. They live as a module in kanban-service rather than a standalone crate so the server and every KanbanBackend transport share one definition without a separate crate boundary.

### KAN-713 Board Column Api Dto Surface (2026-08-02)

Added the v1 HTTP API foundation and the full board/column DTO surface to the kanban-service api module (`kanban_service::api`).
Foundation: a `Patch<T>` wire type implementing JSON Merge Patch (RFC 7386) for PATCH bodies (absent = no change, `null` = clear, value = set), a paginated list envelope (`Page<T>`, `PageParams`), five new machine-readable `ErrorCode` variants (`BATCH_RESOLUTION_FAILED`, `CYCLE_DETECTED`, `SELF_REFERENCE`, `EDGE_NOT_FOUND`, `DUPLICATE_EDGE`) with a `KanbanError → ApiError`/HTTP-status mapping, and wire-enum mirrors (`SortFieldDto`, `SortOrderDto`, `TaskListViewDto`) that serialize as snake_case and decouple the HTTP contract from the domain enums.
Board/column: create (`CreateBoardRequest`/`CreateColumnRequest`), PATCH (`UpdateBoardRequest`/`UpdateColumnRequest`, now merge-patch), PUT full-replace (`ReplaceBoardRequest`/`ReplaceColumnRequest`), read responses (`BoardResponse`/`ColumnResponse`), and `ReorderColumnRequest`. Request types convert to the domain `BoardUpdate`/`ColumnUpdate` via exhaustive `From`/`TryFrom` conversions that exclude server-managed fields and validate at the boundary (e.g. non-negative `wip_limit`/`position`). `BoardResponse` omits internal allocation state (counters and sprint-name pools). These are wire types only; routing and handlers come with the server crate.

### KAN-728 Define Client Id Newtype (2026-08-02)

Added a `ClientId` newtype to `kanban-core`. This is an internal infrastructure type with no user-visible behaviour change. It provides typed identity for connected clients and will be used by the upcoming HTTP collaborative backend to attribute mutations in the audit log and real-time event stream.

### KAN-729 Wrap Commands In Commandbatch With Provenance (2026-08-02)

Internal infrastructure change with no user-visible behaviour difference. Every command batch executed by the tool now carries a correlation ID, a session ID, and app-surface attribution (which app issued it) through the audit log. Client identity is populated when commands are routed through the upcoming HTTP collaborative backend; locally it is left unset. This prepares the persistence layer for multi-client attribution in the command log and real-time event stream.

### KAN-732 Add Health Status And Health Checker (2026-08-02)

Added a `HealthStatus` enum and `HealthChecker` trait to `kanban-core`, and an optional `health_checker()` hook on the `KanbanBackend` trait. This is an internal infrastructure addition with no user-visible behaviour change. It will be used by the upcoming HTTP collaborative backend to power the `GET /health` endpoint, letting the server report whether its underlying storage backend is reachable and healthy.

### KAN-742 Add Kanban Api Server Http Backend Crate Stubs (2026-08-02)

Added three new crate stubs to the workspace -- `kanban-api`, `kanban-server`, and `kanban-http-backend` -- and registered the shared HTTP dependencies (`axum`, `reqwest`, `tower`, `tower-http`, `tokio-tungstenite`, `prometheus`) in the workspace manifest. These are empty scaffolds only; no features are available yet. This prepares the workspace for the HTTP collaborative backend implementation (KAN-684).

### KAN-752 Drop Tokio Tungstenite From Http Backend (2026-08-02)

Dropped the `tokio-tungstenite` dependency from `kanban-http-backend`. The collaborative transport is server-sent events (SSE), not WebSocket, so the dependency was unused.

### KAN-753 Remove The Empty Kanban Http Backend Stub Crate (2026-08-02)

Removed the empty `kanban-http-backend` stub crate (added by KAN-742) from the workspace. Per the sprint-19 decision, the HTTP KanbanBackend implementation will live as a module inside kanban-service (`kanban_service::http_backend`) alongside the JSON and SQLite backends, rather than as a standalone crate, so the placeholder crate is no longer needed. No behaviour change: the crate contained no code.

### KAN-755 Move Aur Packaging Into Packaging Dir (2026-08-02)

Repository housekeeping with no user-visible behaviour change. The AUR package sources (`PKGBUILD`, `.SRCINFO`) moved from the top-level `aur/` directory into `packaging/aur/`, making them a sibling of the existing `packaging/chocolatey/` sources. The release workflows were updated to the new path, and `packaging/` now carries an index README plus a per-package README for the AUR sources.

### KAN-762 Decompose Oversized Modules (2026-08-02)

Internal code-quality refactor with no user-visible behaviour change. Six of the largest modules in the codebase were broken up from single multi-thousand-line files into focused, single-responsibility submodules: the terminal UI application loop, the SQLite storage backend, the MCP server, the service-layer context, the in-memory store, and the card command set. All public interfaces, commands, and runtime behaviour are unchanged; this is purely a maintainability and readability improvement that makes the code easier to navigate and extend. Nothing to do as a user.

### KAN-769 Factory Rerun (2026-08-02)

Introduces a versioned v1 API DTO surface and a unified entity-construction factory across all four core entities (boards, columns, cards, sprints).
New capabilities:
- A versioned wire DTO layer (`kanban_service::api::v1`) with create, update, replace, and response types for every entity, decoupled from the internal domain model. Update requests follow JSON Merge Patch semantics (a field that is absent means no change, null means clear, a value means set); replace requests are true full replacements.
- Client-supplied IDs on create. You may now provide an id when creating an entity (enabling idempotent creation); a collision returns a conflict (ALREADY_EXISTS, HTTP 409 at the wire) instead of silently overwriting existing data.
- Every entity is now built through a single construction path: one funnel for brand-new entities and one for loading existing ones, with identity and timestamps supplied by the caller rather than generated deep inside the domain. Creation is now deterministic.
Behaviour changes to be aware of:
- The CLI `--json` output for entities is now projected through the v1 response types. Wire field names are snake_case and internal bookkeeping fields (per-board counters, sprint name-pool indices) are no longer exposed. Scripts that parse the CLI JSON output should expect the snake_case, counter-free shape.
- The MCP server now exposes the shared v1 create fields rather than its own per-tool create schemas, so the create inputs are identical across the MCP and HTTP surfaces.
- These wire-shape changes ship as a `minor` bump on purpose: pre-1.0, the CLI `--json` output and the MCP request shapes are not yet covered by semantic versioning. Treat them as evolving until the v1 API stabilises.
Reliability and internals:
- Persistence (both the JSON and SQLite backends) now round-trips every entity through a dedicated record type, so a field can no longer be silently dropped when saving or loading. On-disk and database formats are unchanged, so no data migration is required.
- Loading a board or column with a blank name (possible in older data) no longer fails. The blank name is migrated to "Untitled" on load and persisted on the next save; creating a new board or column with a blank name is still rejected.
- A compile-time lock keeps the DTO, persistence, and domain representations in sync: the boundary types are free of defaults and rest-patterns and every conversion is exhaustive, enforced by a CI guard, so adding a field fails the build until every layer handles it.

### KAN-822 Board Delete From The Tui (2026-08-02)

Added the ability to delete a board (project) directly from the TUI. Until now the only way to remove a board was the `kanban board delete` CLI command; there was no keybinding in the interface. Press `d` on the boards panel to delete the selected board, with a confirmation dialog that shows what will be removed (columns, tasks, archived tasks, and sprints) for a non-empty board, or a lighter prompt for an empty one. Deletion reuses the existing undoable cascade, so removing a board and everything in it can be undone with `u`, and the confirmation can be dismissed with `q`, `n`, or `Esc`.

### KAN-828 Shared Archival Abstraction (2026-08-02)

Internal foundation with no user-visible behaviour change. Adds a bounded shared archival abstraction to the domain layer: an `ArchiveMetadata` envelope and a thin `ArchivedEntity` trait (stable id + metadata), implemented first by `ArchivedCard`. Also adds a `KanbanError::unsupported` affordance (a `DomainError::Unsupported` variant) that later slices use as the default body for backend methods not yet implemented, so trait extensions compile cleanly. This is the groundwork the archived-cards retrofit and board archival build on; nothing changes for users yet.

### KAN-829 Archivedcard First Class Board Id (2026-08-02)

Internal domain change with no user-visible behaviour yet. `ArchivedCard` now carries its own `board_id` as a first-class field (D2) so board scoping can become a direct lookup rather than loading every column and post-filtering; the board-scoped query that uses it lands in a later slice. The archive command captures the board from the card's column best-effort, tolerating a dangling column by recording a nil board id rather than failing the archive. The field is additive with `#[serde(default)]`, so existing snapshots still load (nil until the persistence migration backfills a correct board id in a later slice); a nil board id is omitted from archived-card list output rather than surfaced as a zero UUID.

### KAN-830 Board Scoped Archived Card Query (2026-08-02)

Internal domain change with no user-visible behaviour. Adds an additive `DataStore::list_archived_cards_by_board` method (a functional default filtering the archived list by the `board_id` field first-class on `ArchivedCard`, with an in-memory override) that SQL backends will override with a single `WHERE board_id = ?` query once the persistence `board_id` column and its backfill land. Nothing consumes the method yet: the consumer-facing switch (repointing board-scoped archived listing and relaxing the column-delete guard) is deferred to land together with that backfill, so no query changes behaviour until `board_id` is actually populated. Also routes the archived-card map key through `ArchivedEntity::entity_id()` (no behaviour change).

### KAN-831 Json Board Id Backfill (2026-08-02)

Internal persistence change with no user-visible behaviour. The JSON backend
gains a V7->V8 format migration that backfills `board_id` onto historical
`archived_cards` entries that predate the first-class field, reconstructing the
value from each entry's `original_column_id` -> column -> board mapping. An
archived card whose original column no longer resolves keeps a nil board id
(unrecoverable, tolerated under the first-class model rather than failing the
load). The migration also defensively drops any live `cards` entry that shadows
an archived card by id, hardening the "archived cards are a discrete peer
collection" invariant. A `.v7.backup` is written before the destructive step
and removed on success, matching the existing chain policy. The board-scoping
consumer that reads the backfilled field is deferred to a later slice.

### KAN-832 Sqlite Board Id Column (2026-08-02)

Internal persistence change with no user-visible behaviour. The SQLite backend
schema advances 2 -> 3: `archived_cards` gains a first-class `board_id TEXT NOT
NULL` column (indexed via `idx_archived_cards_board_id`) and the 2 -> 3
migration backfills it from each row's `original_column_id` -> `columns.board_id`
mapping, falling back to a nil board id when the original column no longer
resolves (unrecoverable, tolerated rather than failing the open). The migration
runs before the schema is (re)applied, is idempotent on an already-migrated
database, and preserves archived sprint logs. The `cards.column_id -> columns(id)`
foreign key is dropped (the value stays `TEXT NOT NULL` and intact) so an
archived card survives deletion of its original column; live-card cleanup on
column delete is already explicit via the command tier, so the cascade was
redundant. Because dropping that FK also removed the database-level backstop
that rejected an orphaned live card, cross-backend store migration now enforces
live-card column integrity explicitly: an orphaned live card is repaired to the
first available column and, if none exists, the migration fails cleanly instead
of writing an orphan (archived cards remain exempt, keeping their historical
column). `list_archived_cards_by_board` now overrides with a direct `WHERE
board_id = ?` query, and the archived-card exclusion filters switch to `NOT
EXISTS`. The board-scoping consumer that reads the backfilled field is deferred
to a later slice.

### KAN-833 Board Scoped Archived Queries (2026-08-02)

Board-scoped archived-card listing now reads the first-class `board_id` field
instead of inferring the board from each archived card's (possibly stale)
original column, so an archived card whose original column was later deleted
still appears in its board's archived list. `DeleteColumn` no longer blocks on
archived cards: an archived card's `original_column_id` is historical, not a
live foreign key, so a column that holds only archived cards can be deleted. The
board-delete cascade now gathers archived cards by `board_id` and removes both
their records and their dependency-graph edges via a new `DeleteArchivedCards`
cascade command, closing an orphan leak that occurred when a card's original
column had been deleted after archival. Importing a board now backfills
`board_id` on archived cards that predate the first-class field (reconstructing
it from the archived card's original column), so a legacy export's archived
cards remain visible in board-scoped listing and are not leaked on board delete.
The board-delete cascade also no longer short-circuits a board that has sprints
but no columns or archived cards, so deleting such a board removes its sprints
and undo restores them. The now-unused `DeleteArchivedCardsByColumns` cascade
command and the `list_archived_cards_by_columns` store method have been removed.

### KAN-834 Archived Board (2026-08-02)

Internal foundation for board archiving (no user-facing change): boards can now be represented as first-class archived records via the shared archival abstraction (`ArchivedBoard = Archived<Board>`), and snapshots gain a discrete `archived_boards` collection alongside `archived_cards`. Older data without it loads unchanged. Commands, persistence, and UI for board archiving arrive in later slices.

### KAN-835 Board Archive Restore Domain Commands (2026-08-02)

Internal domain layer for board archiving (no user-facing change yet): a board can now be archived and restored as a discrete first-class record. Archiving moves the board head out of the live set into a separate archived collection while its columns, cards, and sprints stay in place; restore moves it back losslessly, and both operations undo symmetrically. The service wiring, persistence, and UI arrive in later slices.

### KAN-836 Board Archive Service Wiring (2026-08-02)

Board archiving is now usable through the service layer (no surface UI yet): a board can be archived, restored, and its archived records listed, all undoable. Permanent deletion works on a board whether it is live or archived, so applications can offer a safe archive-then-delete flow. Undoing a permanent delete restores the board to the state it was deleted from (archived boards come back archived), along with its columns, cards, and sprints.

### KAN-837 Json Archived Boards (2026-08-02)

The JSON file backend now supports archived boards: archiving a board persists it to the on-disk file and it reloads correctly, with the board's columns and cards kept in place. Older JSON files that predate this feature load unchanged. No file-format migration is needed.

### KAN-838 Sqlite Board Archival (2026-08-02)

Board archiving now works on the SQLite backend. Archiving a board persists it as archived, keeps its columns and cards in place, and reloads correctly; restore and permanent-delete (with undo) all behave the same as on the other backends. The database schema version is raised so an older version of the app safely refuses to open a database that uses this feature instead of misreading it; upgrading writes a one-time backup first.

### KAN-842 Hide Archived Board Descendants (2026-08-02)

Archiving a board now hides its columns, cards, and sprints from every live cross-board view (card lists, search, filters, and the TUI displays), instead of leaving them visible as if the board were still active. Restoring the board brings them back. Save/export/import still see the full contents so nothing is lost while a board is archived.

### KAN-843 Archived Card Response Dto (2026-08-02)

The CLI `card list --archived` output and the MCP `list_archived_cards` tool now serialize a stable v1 `ArchivedCardResponse` DTO instead of the internal domain summary. The archived-card JSON now nests the richer card projection (carrying `description` and `card_number`, hiding internal `sprint_logs`) and surfaces the archived card's `board_id` as a first-class field, matching the v1 DTO direction used elsewhere. An exhaustive mapping means a future `ArchivedCard` field must be deliberately represented (or omitted) in the DTO.

### KAN-845 Sqlite Durable Pre Migration Backup (2026-08-02)

The SQLite backend now writes a durable, transactionally-consistent backup
of the database (`<path>.v<from_version>.backup`, via `VACUUM INTO`) before
running an irreversible schema upgrade, so a user can roll back to the
previous binary version and recover their pre-upgrade data. The backup is
written atomically (to a scratch file, then renamed into place) so a crash
or disk-full mid-copy can never leave a truncated file mistaken for a valid
backup, and it is skipped if a backup for that source version already
exists. The naming convention matches the JSON backend's `.v{N}.backup`
scheme, though unlike the JSON backend's per-step backup it is kept
indefinitely rather than removed on success, since it exists to support
rollback across a binary downgrade rather than just crash recovery within
a single migration step.

### KAN-848 Deterministic Entity Ordering (2026-08-02)

Board, column, and card lists now have a deterministic, stable order. Entities that share the same `position` (for example legacy boards that predate the `position` field and default to `0`) previously fell back to hash-map iteration order, so the projects list and other views could reshuffle on every launch. Ordering now breaks ties by creation time and then by id, giving a total order that is stable across runs. A single shared helper owns this ordering policy so every read path stays consistent.

### KAN-849 Set Priority With P From List (2026-08-02)

Pressing `p` in the task list now sets the highlighted card's priority, matching the on-screen hint. The key was previously wired to the wrong action and did nothing, so priority could only be changed from the card editor. The single-card `p` binding mirrors the existing bulk priority flow and is scoped to the cards panel.

### KAN-850 Declutter Footer Panel Hints (2026-08-02)

The footer shortcut bar no longer lists the `panel N` focus bindings, which crowded out more useful hints (worst on the edit-task screen, which defines five of them). Panel numbers still appear as `[1]` and `[2]` in the panel titles, the number keys still switch panels, and the `?` help overlay still lists them, so nothing about panel switching is lost.

### KAN-855 Generic Archival Abstraction (2026-08-02)

Internal refactor (no user-facing change): archival is now a single reusable abstraction. Archived records are modeled generically as an entity plus its shared archive metadata and an entity-specific restore context, so archiving any entity is a matter of implementing one small trait rather than hand-rolling a bespoke type. Archived cards move onto this abstraction; already-saved archived-card data keeps loading unchanged.

### KAN-856 Json V7 Migration Test (2026-08-02)

Internal test coverage (no user-facing change): adds an end-to-end regression test that a legacy JSON file (format V7) migrates forward correctly and coexists with board archiving — archived cards keep their data with the board reference backfilled, the archived-boards collection defaults to empty, and archiving a board works on the migrated file.

### KAN-857 Json V9 Format Bump (2026-08-02)

The JSON save format is now version 9, marking files as archived-board-capable. An older version of the app that predates board archiving will cleanly refuse to open a version-9 file instead of loading it and silently discarding archived boards on its next save. Existing files upgrade automatically on open, writing a one-time backup first.

### KAN-860 Sqlite Snapshot Archived Boards (2026-08-02)

Fixes data loss where exporting or snapshotting a SQLite database silently dropped every archived board. Archived boards are now included in a SQLite snapshot and restored on import, so exporting a SQLite board to JSON (or copying between backends) preserves them.

### KAN-864 Unified Reference Marker Archival (2026-08-02)

Archived cards and boards are now fully functional everywhere. An archived
entity is just its ordinary card or board plus a marker, so you can view,
browse, edit, and restore archived items exactly like live ones. The
live/archived distinction is now a filter at each surface rather than a
separate, limited mode.
- CLI: `card list` and `board list` gain `--archived` (archived only) and
  `--include-archived` (live and archived together), plus `board archive`,
  `board restore`, and `board delete-archived` commands.
- MCP: `list_cards` and `list_boards` take an `archived` filter (`exclude`,
  `only`, `include`), and there are board archive, restore, and delete-archived
  tools. The separate `list_archived_cards` tool has been removed; use
  `list_cards` with `archived: "only"` instead.
- TUI: an archived boards view you can open into and browse like any live board,
  drilling into its columns and cards, with archive, restore, and
  permanent-delete affordances.
Under the hood the archival model was unified onto a single reference marker
(the entity stays live and a marker records that it is archived), which removes
the old separate archived collections and their drift. JSON files upgrade
automatically from format V9 to V10 on first open, lifting embedded archived
entities into references and writing a `.v9.backup` first; SQLite needs no
migration. The change is backward compatible.

### KAN-912 Remove Unneeded License Txt Verification Txt From Chocolatey Package (2026-08-02)

Repository housekeeping with no user-visible behaviour change. The Chocolatey package source (`packaging/chocolatey/tools/`) no longer ships `LICENSE.txt` or `VERIFICATION.txt` — Chocolatey moderation flagged both as unnecessary for a download-only package with no embedded binary payload, since `chocolateyinstall.ps1` already fetches the release ZIP with an inline SHA256 checksum. The release workflow and the packaging README were updated to match.

### KAN-939 Point Chocolatey Projecturl At Kanban Rs (2026-08-02)

Repository housekeeping with no user-visible behaviour change. The Chocolatey package's "Software Site" link on chocolatey.org now points at the project's dedicated website, https://kanban.rs, instead of the GitHub repo. The "Source" link is unaffected and still points at GitHub.

### KAN-946 Board List Sort (2026-08-02)

The board list is now sortable. You can order boards by position, name,
creation time, or archival recency, and persist a default so every surface
respects it.
- CLI: `kanban board list --sort <field> --order <dir>` sorts a single listing
  (`field` is one of `position`, `name`, `created_at`, `archived_at`; `dir` is
  `asc` or `desc`). `kanban board set-sort --sort <field> --order <dir>`
  persists a default in the app config; later `board list` calls without an
  explicit sort/order use it.
- MCP: `list_boards` gains `sort` and `order` parameters, and a new
  `set_board_sort` tool persists the default.
- TUI: the projects panel gets a sort field picker (`o`) and an order toggle
  (`s`) that work for both the live and archived board views, and the choice is
  persisted across sessions.
The archived-boards view defaults to recency (most recently archived first)
rather than position.

### KAN-958 Archived Board Drill In Parity (2026-08-02)

Fixed the TUI archived-board drill-in so it behaves exactly like a live board.
After opening an archived board's card list, `Esc` (or `q`) now backs out to the
archived-boards list instead of leaving you stranded, and card actions such as
archive (`d`), set priority (`p`), open detail, and move now work on an archived
board's cards, matching a live board. Previously the drill-in used a
boards-list-only key dispatch that silently dropped both the back-out and every
card key, even though the help overlay advertised them.

### KAN-961 Edge Revival Partial Restore (2026-08-02)

Fixed a dependency-graph integrity bug: restoring one of two archived cards that
shared a dependency edge (spawns/blocks/relates) revived the edge even though the
other card was still archived, leaving an active dependency pointing at a hidden
card. Restoring a card now only revives edges whose other endpoint is also live,
matching the invariant already enforced when an edge is created.

### KAN-962 Get Stamps Archived At (2026-08-02)

Fixed single-entity reads returning archived entities as if they were live.
`card get` and `board get` (CLI and MCP, by UUID or identifier) now stamp the
archival marker's `archived_at`, so an archived card or board is clearly marked
instead of being indistinguishable from a live one. This matches what
`card list --archived` already did and keeps the get and list views consistent.

### Other Changes (2026-08-02)

Documentation fix for contributors: `CONTRIBUTING.md` documented a PR title format (`<branch-name>`) that does not match the convention actually used in this repository, and a commit message format missing the crate scope. Both now reflect real practice.

### Other Changes (2026-08-02)

`kanban-server -V`/`--version`/`--help` no longer crash. The binary previously had no argument parsing at all, so any flag was ignored and it tried to open its data file before doing anything else — now these flags print immediately and exit without touching any file.

### Other Changes (2026-08-02)

Internal groundwork for the upcoming HTTP API (no user-facing change, `kanban-server` is not yet released for general use). `kanban-server`, when backed by the JSON store, now detects and reloads when the underlying file is changed by another process (the TUI, CLI, or MCP server) — previously it only ever read the file once at startup and could go stale until restarted.
Also fixes two bugs in the shared `FileWatcher` component (also used by the TUI): starting a watch on a locator whose file doesn't exist yet no longer fails, and starting a watch now waits until the OS-level watch is actually armed before returning, closing a race where a write landing immediately after startup could be missed.

### Other Changes (2026-08-02)

Internal groundwork for the upcoming HTTP API (no user-facing change, `kanban-server` is not yet released for general use). Fixes a critical data-loss bug: `kanban-server`, when backed by the JSON store, never wrote any board or column changes to disk — every mutation only ever existed in memory and was lost the moment the process exited, restarted, or crashed. Every write route now durably persists the change before reporting success back to the client.

### Other Changes (2026-08-02)

Internal groundwork for the upcoming HTTP API (no user-facing change, `kanban-server` is not yet released for general use). `kanban-server` now resolves its data file the same way `kanban-cli` and `kanban-mcp` already do: it reads the shared config file (`~/.config/kanban/config.toml`, or `KANBAN_CONFIG` override), falling back to the same default filename (`boards.json`) instead of a `kanban-server`-specific one. `KANBAN_FILE` still overrides everything, same as before.
Also improves the error message when the data file can't be opened or parsed: it now names the exact file path that failed, and shows a clean message instead of a raw internal error representation.

### Other Changes (2026-08-02)

Fixed an intermittent test failure and closed a related edge case in how the TUI detects file changes made by other processes. Own writes are now identified by comparing a stamped identifier in the saved file against the running instance, rather than guessing from how many filesystem events arrived in a short window — a guess that could occasionally be wrong under heavy load, causing the app to briefly (and incorrectly) treat its own save as an external change.

### Other Changes (2026-08-02)

Internal groundwork for the upcoming HTTP collaborative backend: `kanban-service` gained a `RemoteWrites` capability hook that lets a future backend delegate board/column/card create/update/delete directly to a remote server instead of applying them locally. This release has no user-visible effect — no backend uses the hook yet, and every existing local backend (JSON, SQLite, in-memory) behaves exactly as before.

### Other Changes (2026-08-02)

Fixes a `kanban-server` build break introduced by two independently-developed changes landing back to back: the flat-route integration tests (`tests/flat_routes.rs`) still referenced a test helper module that had been relocated in a parallel change, so `kanban-server` failed to compile with the `test-helpers` feature enabled. No functional change — the test file now imports from the correct location.

### Other Changes (2026-08-02)

`kanban-server` now exposes flat, non-board-scoped routes for columns and cards: `GET/PATCH/DELETE /v1/columns/{id}` and `GET/PATCH/DELETE /v1/cards/{id}`. These behave identically to the existing board-scoped routes (`/v1/boards/{board_id}/columns/{id}`, `/v1/boards/{board_id}/cards/{id}`) but don't require the caller to know which board owns the entity up front. The existing board-scoped routes are unchanged and continue to work exactly as before.

### Other Changes (2026-08-02)

`kanban-server` now accepts the data file path as a positional argument (`kanban-server <path>`), matching `kanban-cli` and `kanban-mcp`. Previously it only read the `KANBAN_FILE` environment variable, falling back to the config file's `storage_location`. Precedence is unchanged for existing setups: an explicit positional argument now wins over `KANBAN_FILE`, which still wins over the config-file default.

### Other Changes (2026-08-02)

Internal restructuring with no user-visible effect: the backend abstraction (`KanbanBackend`, `RemoteWrites`) and the HTTP wire types now live in their own `kanban-backend` and `kanban-api` crates instead of inside `kanban-service`. This lets storage and transport implementations be plugged in independently of the service layer, and is groundwork for connecting the CLI, TUI, and MCP server to a remote `kanban-server`. All existing behaviour, on-disk formats, and APIs are unchanged.

### Other Changes (2026-08-02)

Internal test reorganisation with no user-visible effect: `kanban-domain`'s command tests move from inline unit-test modules into integration tests. A test that needs a concrete storage implementation to run is an integration test, so this puts them where they belong, and it lets the in-memory store be extracted into its own crate later. Behaviour is unchanged. Six tests that exercised private validation helpers directly were removed as duplicates of existing tests covering the same scenarios through the public command API, and two scenarios that were only covered by such a test are now covered through the public API instead.

### Other Changes (2026-08-02)

`kanban` is now available on Windows via Chocolatey. Install with:
```powershell
choco install kanban
```
This installs both the `kanban` TUI/CLI and the `kanban-mcp` Model Context
Protocol server binary. The package is published to the Chocolatey Community
Repository under the id `kanban`.

### Other Changes (2026-08-02)

Internal groundwork for the upcoming HTTP API (no user-facing change). The
`kanban-server` binary now actually starts and serves a `/health` endpoint;
previously it was an empty stub that did nothing. This lays the foundation
that the rest of the HTTP API's routes will attach to in follow-up releases.

### Other Changes (2026-08-02)

`kanban-server` now exposes a live change stream: `GET /v1/events` is a
Server-Sent Events (SSE) endpoint that pushes a small notification to every
connected client whenever any client successfully creates, updates, or
deletes something on the server.
This lets multiple clients stay in sync without polling — open the stream
once and receive a notification the moment something changes elsewhere, with
periodic keep-alive pings so the connection survives idle periods and
proxies. This is server-side plumbing for real-time collaboration; client
support for consuming this stream lands in a future release.

### Other Changes (2026-08-02)

Internal groundwork for the upcoming HTTP collaborative backend: a new `kanban-backend-http` crate now exists, scaffolding `HttpBackend` (a `KanbanBackend` implementation that will eventually talk to a remote `kanban-server` over HTTP). This release has no user-visible effect — the crate isn't wired into any consumer yet, and every read/write method is an explicit stub returning "not supported." `kanban-service` itself gained no new dependencies from this work.

### Other Changes (2026-08-02)

Internal test infrastructure change only, no user-visible effect: `kanban-server`'s in-process test harness (`TestServer`, used to spin up a real server over a real socket for integration tests) moved from a test-only file into a `test-helpers`-feature-gated library module, so other crates in the workspace can reuse it in their own tests. Not included in the production `kanban-server` binary.

### Other Changes (2026-08-02)

Internal groundwork for the upcoming HTTP API (no user-facing change). Adds
the first two live routes in `kanban-server`: listing boards and fetching a
single board by id. This is the template every other entity's routes will
follow.

### Other Changes (2026-08-02)

Internal groundwork for the upcoming HTTP API (no user-facing change).
`kanban-server` can now create, replace, update, and delete boards over HTTP
(`POST`/`PUT`/`PATCH`/`DELETE /v1/boards`), not just read them. This is the
first entity to get a complete CRUD surface, establishing the pattern the
remaining entities (columns, cards, sprints) will follow.

### Other Changes (2026-08-02)

Internal groundwork for the upcoming HTTP API (no user-facing change). Adds
`kanban-server` routes for listing a board's columns and fetching one by id.
Second entity to get its read surface, following the same pattern boards
established.

### Other Changes (2026-08-02)

Internal groundwork for the upcoming HTTP API (no user-facing change, `kanban-server` is not yet released for general use). Adds column write routes to `kanban-server`: `POST`/`PUT`/`PATCH`/`DELETE` and a dedicated reorder endpoint under `/v1/boards/{board_id}/columns`. `PUT` is a true full replace — it can move a column's position, not just rename it. Every write route now verifies the target column actually belongs to the board named in the URL, matching the read routes.

### Other Changes (2026-08-02)

`kanban-server` now exposes read routes for cards:
- `GET /v1/boards/{board_id}/cards` — list a board's cards, optionally filtered
  by `column_id`, `sprint_id`, and a new `archived` query parameter
  (`live_only` [default], `archived_only`, or `include`). An unknown
  `board_id` returns an empty array rather than an error, matching the
  existing board/column list endpoints.
- `GET /v1/boards/{board_id}/cards/{id}` — fetch a single card. Returns 404 if
  the card doesn't exist or belongs to a different board than the one in the
  path.
Cards returned with `archived=include` or `archived=archived_only` carry a
stamped `archived_at` timestamp so clients can distinguish archived cards from
live ones without a separate lookup.

### Other Changes (2026-08-02)

`kanban-server` now supports creating, replacing, updating, and deleting cards
over HTTP:
- `POST /v1/columns/{column_id}/cards` — create a card in the given column.
- `PUT /v1/columns/{column_id}/cards/{id}` — idempotent create-or-replace for
  a card at a client-supplied id.
- `PATCH /v1/boards/{board_id}/cards/{id}` — partial update (JSON Merge
  Patch). Moving a card to a different column enforces that column's WIP
  limit, returning a 409 if it's full.
- `DELETE /v1/boards/{board_id}/cards/{id}` — delete a card.
Every mutation is durably persisted before the response is returned, and
broadcasts a change event to connected clients. A request for a card that
doesn't exist, or that belongs to a different board than the one in the URL,
returns 404.

### Other Changes (2026-08-02)

Internal groundwork for the upcoming HTTP API (no user-facing change). Errors
raised inside `kanban-server` now consistently convert to the right HTTP
status code and a flat JSON error body, instead of each future route having
to map that itself. This is the last piece the API's routes need before they
can start landing.

### Other Changes (2026-08-02)

Fixed a data-integrity bug where an archived card could permanently lose its
board association, making it invisible to that board's archived-cards list
and unreachable by the cleanup that runs when a board is deleted. This
happened in an edge case: archiving a card, then deleting its now-empty
column (allowed since archived cards don't block column deletion), then
archiving that same card again (e.g. a retried save or an undo/redo replay) —
the second archive used to silently overwrite the card's board with "unknown"
instead of leaving the already-correct value alone.
Under the hood, every card now carries its own durable reference to its
board, kept up to date whenever a card moves to a different column (including
moving it to a column on a different board, which continues to work exactly
as before). This removes the need to look up a card's board through its
column at archive time, so the board reference can no longer go stale even
if the column is later removed.
Existing boards, cards, and databases are migrated automatically the next
time they're opened — no action needed.

### Other Changes (2026-08-02)

Board, column, and card lists in the sortable/filtered views (and the SQLite backend's raw list queries) could still reshuffle nondeterministically when two entities shared the same position, or two cards additionally shared the same creation time — a gap left over from an earlier ordering fix that covered the in-memory store but not these paths. Boards, columns, and cards now break ties by creation time and then by id everywhere position ordering happens, matching the ordering the rest of the app already guarantees, so a duplicated position (for example after archiving a board and creating a new one) no longer produces an unstable order.

### Other Changes (2026-08-02)

The archived-boards view now always defaults to recency order (most recently archived first), independent of whatever sort preference is saved for the live boards list. Previously, changing the live list's default sort (for example to sort by name) also silently changed the archived view's default, since both shared the same saved preference. This is now consistent everywhere: at the service layer (MCP/CLI/API) and in the terminal UI. The archived-boards panel in the TUI is still independently sortable via its own `s`/`o` keys, but that choice is session-only and no longer bleeds into the live view's default or gets persisted.

### Other Changes (2026-08-02)

The JSON storage backend's V1-to-V2 and V8-to-V9 format migrations now write their results via the same atomic temp-file-and-rename pattern the rest of the migration chain already used. Previously these two steps wrote the migrated file in place, so a crash or power loss during either specific write could leave the board file truncated or corrupted. They now benefit from the same crash-safety guarantee as every other step in the chain.

### Other Changes (2026-08-02)

The `set_board_sort` MCP tool now reports the actual sort field and order it resolved and persisted, instead of echoing back exactly what the caller sent. Previously, omitting either `sort` or `order` (to leave that dimension unchanged) made the response report `null` for the omitted half, even though a concrete value was computed and saved. The response now always reflects the real, effective sort configuration, matching how the CLI's equivalent command already behaved.

### Other Changes (2026-08-02)

Creating a new board no longer accepts a completion-column setting, since a
brand-new board never has any columns yet for it to point at — that field
was always silently ineffective there (accepted and stored, but with nothing
to match against) and is now rejected instead. To set which column marks a
card as complete, create the board's columns first, then set it via the
board-update API.
This applies to MCP's board-creation tool and the underlying create API; the
separate "replace an existing board" API is unaffected and can still set the
completion column, since a board being replaced may already have columns.

### Other Changes (2026-08-02)

Fixed the TUI so viewing and restoring archived cards works on an archived
board, matching a live board. Previously, once you drilled into an archived
board, pressing `D` to view its archived cards silently did nothing, which
also made restore (`r`) and permanent-delete (`x`) for those cards
unreachable. `e` (edit) was similarly dead in that view despite being
advertised in the footer/help, and now works exactly as it does on a live
board.
`q`/`Q` in that view now correctly back out one level to the archived-boards
list, instead of silently doing nothing. This isn't identical to a live
board, where `q`/`Q` quits the application entirely — the drilled-in archived
view uses `q`/`Q` for back-navigation instead, matching how `Esc` already
behaved there.
Also fixed two related state-leak bugs surfaced while writing this: a stray
key sequence (e.g. `g` awaiting a second `g` for "jump to top") could survive
backing out with `q` and misfire on the next keystroke; and opening search
from the archived-cards view and closing it could strand you in the live
view with a stale reference to the archived board instead of returning you
to the archived-cards view.

### Other Changes (2026-08-02)

Three confirmation dialogs — sprint-prefix collision, storage conflict resolution, and the external-file-change prompt — now show footer hints that match what the keys actually do. Previously all three borrowed a generic list-picker's hints (`j`/`k`/`Enter`), which did nothing in any of these dialogs, while their real keys went unlisted. This was most serious on the external-change dialog: `k` (keep local changes, discarding the external write) looked like harmless "navigate up" in the footer, so it was easy to press by accident and silently lose someone else's saved changes. The footer now shows the real keys with accurate, specific descriptions, and the in-app help overlay (`?`) invoked from within any of these three dialogs now performs the same action the direct keypress does, instead of a mismatched or no-op one.

### Other Changes (2026-08-02)

Pressing `V`, `t`, `T`, or `1` while viewing archived tasks no longer leaks
into live state. Previously these keys fell through to the same handlers
used by the live tasks panel: `V` opened the task-list-view picker and
changed the board's display mode, `t` silently toggled the shared active-
sprint filter, `T` opened the filter-options dialog, and `1` jumped focus
out to the projects panel — all while nominally still looking at the
archived list. All four are now no-ops from the archived-cards view, and
are no longer advertised in its footer help.

### Other Changes (2026-08-02)

The Sprint Detail screen's footer hints now match what actually happens when you press the keys. Previously `s`, `y`, and `Y` were shown as available (assign to sprint, copy branch name, copy git checkout command) but did nothing when pressed; they now work, reusing the same assign-to-sprint picker and clipboard-copy behavior already available elsewhere in the app. Conversely, `d` (archive the selected task or tasks) was fully functional but never shown in the footer, so it could be pressed by accident with no indication of what it does; it's now advertised alongside the other keys.

### Other Changes (2026-08-02)

Actions invoked through the in-app help overlay (`?`) now actually run instead of silently doing nothing for four of them: copying a card's branch name (`y`) or git checkout command (`Y`) from the card or sprint detail views, carrying over a completed sprint's tasks (`M`), and exporting all boards (`x`). These all already worked as direct keypresses; only reaching them through the help menu was broken.

### Other Changes (2026-08-02)

**Breaking change for MCP clients**: all six MCP list tools (`list_boards`, `list_columns`, `list_sprints`, `list_card_parents`, `list_card_children`, and the already-paginated `list_cards`) now consistently return the same paginated envelope shape: `{ "items": [...], "total": N, "page": N, "page_size": N, "total_pages": N }`, with `page`/`page_size` request parameters on every one of them (default: page 1, page size 50).
Previously only `list_cards` used this shape. `list_boards` returned a bare array when no page parameters were supplied and the envelope only when they were — the same tool call could return two different shapes depending on input. `list_columns`, `list_sprints`, `list_card_parents`, and `list_card_children` always returned a bare array with no way to paginate at all.
Any client that read these tools' results as a bare JSON array needs to read `.items` instead, and should use `total` to detect when a result has more entries than fit on one page.

### Other Changes (2026-08-02)

Internal test coverage (no user-facing change): adds regression guards for
three SQLite behaviors that were correct but relied on fragile, previously
untested invariants — foreign-key enforcement is properly restored after the
old-format migration step that temporarily disables it, the safety backup
taken before upgrading an older database file is now verified at every
upgrade step (not just the first one), and restoring a saved snapshot is
confirmed to roll back cleanly instead of leaving the database partially
wiped if something goes wrong partway through.

### Other Changes (2026-08-02)

Hardened the board view's column ordering so it can no longer disagree with
itself in edge cases (two columns ending up sharing the same internal
position value, which can happen after deleting and recreating columns).
Column list rendering, the move-column-up/down commands, the rename/delete
column dialogs, and the board detail view now all resolve column order the
same consistent way instead of each computing it separately.

### Other Changes (2026-08-02)

Pressing `e` (edit) through the in-app Help overlay now works everywhere it
should. Previously, selecting "Edit card" from the Help menu (or pressing `?`
then jumping straight to `e`) silently did nothing in every context: the card
list, card detail (title, description, or metadata), sprint detail, and the
settings configuration editor. Pressing `e` directly, outside the Help menu,
already worked correctly in all of these places.
The two paths now share one dispatch, so editing a card through the Help
menu opens the same editor as pressing `e` directly, in every mode.

### Other Changes (2026-08-02)

Card counts and lists no longer include archived cards. Sprint Detail's
"Cards Assigned" count, Board Detail's per-sprint and per-column counts,
the Carry-Over Sprint popup's card count (and the check for whether that
popup should open at all), and a completed sprint's Uncompleted task
panel could previously keep including an archived card forever, since
archiving keeps a card's sprint and column assignment intact rather than
clearing it. All of these now reflect only live cards.

### Other Changes (2026-08-02)

Managing a card's children or parents (from either the tasks panel or Card
Detail) now defaults to offering only live cards as candidates. Previously
an archived card could show up in the picker as if it were a normal,
selectable relative, which wasn't particularly useful in the common case.
Managing relationships from an already-archived card (via the archived
tasks view) is unaffected: archived candidates are still fully available
there, matching how every other action on an archived card already works.

### Other Changes (2026-08-02)

The board delete/archive confirmation dialog's task count no longer
double-counts archived cards against the separate archived-task figure.
This was the last of a small cluster of display counts (see the previous
release's card-count fixes) that read from an internal collection without
excluding archived cards.

### Other Changes (2026-08-02)

Fixed a bug where moving multiple selected cards to another column at once
(multi-select `H`/`L`) could silently place a card on top of an existing
card in the destination column instead of appending it after the others.
The moved card kept its old position from the source column rather than
getting a fresh position in the destination, so it would only avoid
colliding by coincidence. Moving several cards into the same column in one
action now gives each of them its own position, in order.

### Other Changes (2026-08-02)

Fixed a bug where moving cards between columns without also changing their
status (for example the bulk multi-select "move left/right" action in the
TUI) could silently exceed a column's WIP limit, and could leave a card
pointing at the wrong board after a cross-board move. Both of these are now
enforced the same way a direct drag-and-drop move already enforces them, so
the two paths behave consistently.

### Other Changes (2026-08-02)

`kanban-server` is now available as a Nix package (`nix build .#kanban-server`),
alongside the existing `kanban-cli` and `kanban-mcp` packages.

### Other Changes (2026-08-02)

Internal groundwork for the upcoming HTTP API (no user-facing change). Aligns
how the board create/replace logic will handle PUT requests with the wire
convention the API's design already calls for (sprints already follow it;
columns and cards will be brought in line as their own write-route work
lands), ahead of the routes themselves landing.

### Other Changes (2026-08-02)

Updating a column (rename, position change, WIP limit) now rejects a negative position, matching the rule already enforced when creating a column. This closes a gap where reordering a column through the MCP server accepted a negative position with no validation at all.

### Other Changes (2026-08-02)

- docs: correct PR title and commit message formats in CONTRIBUTING

### Other Changes (2026-08-02)

`kanban-server`: fixes a write-path scoping hole across the card, column, and
sprint create-or-replace handlers. Each honours a client-supplied id for
idempotent create, but only the column PUT handler already guarded against
that id belonging to a different parent. A `PUT`/`POST` targeting an id that
already exists under a *different* column/board now returns 404 instead of
silently relocating a card or column, or silently editing a sprint that
belongs to a different board.


## [0.7.2] - 2026-06-10 ([#331](https://github.com/fulsomenko/kanban/pull/331))

### KAN-674 Fix Powershell Parser Error In Chocolatey Digest Read Step (2026-06-10)

The Chocolatey publish job now succeeds past the asset-readiness poll
step. A latent PowerShell parser bug in the `Read Windows ZIP digest
from GitHub Release` step (introduced by KAN-656) was masking the rest
of the Chocolatey publish path on every release since that change
landed: PowerShell interpreted `$asset:` as a scoped variable
reference (the same syntax as `$env:VAR`), so the step exited with a
parser error before the SHA256 could be written to the step outputs.
The user-facing effect was that Chocolatey was stuck at the last
version published before KAN-656, even though crates.io, AUR,
Homebrew, and GitHub Release shipped each new version normally. The
silent-failure caveat from KAN-649 plus the warning annotation from
KAN-667 made this visible at release time, but no actual fix for the
parser bug had landed until now.
The fix is one character — wrapping `$asset` in braces (`${asset}`) so
PowerShell stops looking for a scope qualifier after the colon.


## [0.7.1] - 2026-06-08 ([#329](https://github.com/fulsomenko/kanban/pull/329))

### KAN-667 Visible Failure Notification For Publish Chocolatey (2026-06-08)

The release workflow's `publish-chocolatey` job now surfaces a visible
warning when it fails, instead of silently going green at the workflow
level.
Background: KAN-649 marked the job `continue-on-error: true` so that a
stuck Chocolatey moderation queue would not turn the entire release
workflow red after crates.io, GitHub Release, AUR, and Homebrew had
already succeeded. The trade-off was that GitHub Actions only sends
notifications on workflow-level failures, so a failed Chocolatey
publish could become a silent miss for weeks.
A new `Surface chocolatey failure` step runs `if: failure()` at the
end of the job and writes a `::warning::` annotation plus a
`$GITHUB_STEP_SUMMARY` markdown block linking to
`packaging/chocolatey/RECOVERY.md`. The annotation is visible at the
top of the workflow run page and on the PR's checks panel without
clicking through. The step is itself `continue-on-error: true` so a
failure to write the annotation does not defeat the purpose of the
parent flag.
No behavioural change for end users installing the package.


## [0.7.0] - 2026-06-07 ([#327](https://github.com/fulsomenko/kanban/pull/327))

### KAN-174 Save Error Banner (2026-06-07)

Save errors are now shown to the user in the TUI instead of being
silently logged. A persistent red banner with a warning icon appears
between the main view and the footer whenever the save worker fails to
flush changes to disk (e.g. disk full, permission denied, conflict
detected). The banner clears automatically once a subsequent save
succeeds.

### KAN-175 Data Integrity Tests (2026-06-07)

Importing a board now validates that every card references a column
that exists in either the imported snapshot or the current store.
Previously a snapshot with dangling column references would import
partially and leave the board in a broken state. The import now returns
a validation error and writes nothing if any card has an invalid
`column_id`.

### KAN-191 Pure Command Replay Undo Redo (2026-06-07)

Undo and redo are now implemented as **inverse-command CRUD operations**
against current state — no more whole-board snapshot clones held in
RAM, no more wipe-and-replay when you press `u`.
**Unlimited undo within a session.** The previous 200-step cap is gone.
Undo can rewind through every change you've made since opening the
file, whether that's 5 edits or 5,000.
**Lower memory and CPU during heavy editing.** Each undo step now costs
the size of the commands that produced it (a few hundred bytes) instead
of a full clone of the entire board state. Sessions no longer
accumulate snapshot copies in RAM. The pre-execute snapshot that used
to run before every command — even for safe operations — is gone too.
Snapshots are only used as a rollback fallback if a batch fails
partway through.
**Failed undos and redos can be retried.** If an undo or redo encounters
an error (e.g. a validation rule fires on an inverse), the operation
rolls back cleanly and the undo stack stays where it was — the next
attempt sees the same entry, instead of skipping over it.
**Sprint history is no longer bloated by undo cycles.** Previously,
undoing a "card assigned to sprint" would push a *new* sprint-log
entry instead of removing the one the forward action added. A user
repeatedly toggling a sprint assignment could grow a card's
`sprint_logs` vec indefinitely. Inverses now restore the card's full
prior state, so sprint history round-trips cleanly.
**Two stories, properly separated.** The previous design conflated two
concerns under a single "command store":
- **UndoStack** lives in memory for the duration of a session and
  drives `u` / `Ctrl+R`. Closing the app discards it.
- **CommandLog** is the audit history of every action — what happened,
  when, by which forward command. Foundation for the upcoming
  audit-log UI (KAN-36). Currently session-scoped on both backends;
  the on-disk SQLite table is in the schema but is wired up in a
  separate follow-up.
**Cross-session undo is deferred.** The previous attempt at making undo
survive `close → reopen` shipped an `apply_snapshot(empty) + replay`
flow that conflicted with treating SQLite as a CRUD store. Cross-
session undo needs a separate design with conflict invalidation — it
will return as its own feature once that design lands.

### KAN-249 Audit Domainerror Helpers (2026-06-07)

Removes six public convenience constructors from
`kanban-domain::DomainError`: `board_not_found`, `card_not_found`,
`column_not_found`, `sprint_not_found`, `archived_card_not_found`,
and `tag_not_found`. End-user behaviour, error messages, error
variants, and matching behaviour (`is_not_found`) are unchanged, but
direct library consumers of the `kanban-domain` crate must switch to
`KanbanError::not_found(entity, id)`, which has been the standard
construction path in the rest of the workspace for some time.
The still-used `DomainError::wip_limit_exceeded` helper is retained.

### KAN-250 Required Str Sqlite Upserts (2026-06-07)

SQLite upserts for boards, columns, cards, sprint logs, and sprint name
lists now reject empty strings for `TEXT NOT NULL` fields instead of
silently writing them. A new `required_str` helper returns an error if
a required field is blank, preventing corrupted rows that would fail to
load on next open.

### KAN-402 Add Package Version Badges To Readme (2026-06-07)

The README now shows live version badges for the AUR, nixpkgs, and Homebrew tap packages alongside the existing crates.io and license badges, plus a CI status badge linking to the GitHub Actions workflow. Each badge tracks its package source directly so the README always reflects what is currently available to install.

### KAN-414 Welcome Bug Fixes In Contributing (2026-06-07)

`CONTRIBUTING.md` now lists bug fixes as an explicit area for contribution. Small targeted fixes, crash and regression fixes, and cross-platform fixes for Windows, macOS, and Linux are called out as especially welcome. The previous list only enumerated feature-shaped work, which implicitly framed bug fixes as out of scope.

### KAN-437 Selector Follows Card On Column Move (2026-06-07)

In the kanban (column) view, pressing `h` or `l` to move a card to an
adjacent column now keeps the card selected after the move. Previously
the selector stayed on whatever card was previously focused in the
target column, silently dropping focus on the moved card.
The same fix applies to the multi-select move path: after moving a
group of cards with `h`/`l`, the selector follows the first moved card
into the target column.

### KAN-451 Add Cli Init Subcommand To Decouple Storage Bootstrapping From Board Create (2026-06-07)

Add `kanban init` command for non-interactive board file initialization. Creates a new board file with an optional first board and exits cleanly without opening the TUI. Decouples storage bootstrapping from board creation, enabling scriptable first-time setup and fixing Homebrew formula tests that were hanging in non-TTY environments.
**Usage:**
```bash
kanban init boards.json --board "My Project"  # create file + first board, exit
kanban init --board "My Project"              # uses KANBAN_FILE or boards.json
kanban init                                   # creates boards.json with "My Board"
```
File path resolution follows the standard chain: positional argument > `KANBAN_FILE` env var > config file `storage_location` > compiled-in default (`boards.json`).

### KAN-457 Mcp Readme Schema Labels And Descriptions (2026-06-07)

The MCP server's README now matches the actual tool schemas. Tool reference tables previously listed `board: UUID`, `column: UUID`, `sprint: UUID`, and a comma-separated `cards: String` for bulk operations, but the implementation has accepted entity names (and sprint numbers, and card identifiers like `KAN-5`) for some time. Param types are now shown as `String` / `Vec<String>`, the three `Get a specific X by ID` rows now read "by UUID or name" or "by UUID, name, or number", and the formerly card-specific "Card Identifiers" section has been generalized to cover boards, columns, sprints, and bulk-cards inputs.

### KAN-465 Publish To Homebrew Via Personal Tap Fulsomenko Homebrew Kanban (2026-06-07)

Release workflow now auto-bumps the Homebrew tap formula on each version bump. The `release.yml` CI job computes the tarball SHA256, clones the `fulsomenko/homebrew-tap` repository, updates the formula's `url` and `sha256` fields, and pushes the changes — no manual intervention required. Updated README and web landing page with Homebrew install instructions.

### KAN-466 Kanban Mcp V Should Include Git Commit Hash (2026-06-07)

`kanban-mcp -V` now includes the git commit hash, matching the output of `kanban -V`:
```
kanban-mcp 0.6.0
commit: 1e2200b91bf854ca7dac456923fb38d903b67d28
```
Previously the commit line was missing because the `kanban-mcp` Nix build did not forward the `gitRev` parameter to the compiler environment. The nixpkgs package was already correct.

### KAN-469 Refactor Web Installation (2026-06-07)

Web landing page installation section refactored to collapse five separate install method blocks into a single unified code block. All methods (Cargo, Homebrew, Nix, AUR, from source) are now presented together with inline comments for clarity. Nix installation simplified from multi-step instructions to a single command: `nix run nixpkgs/nixpkgs-unstable#kanban`.

### KAN-480 Remove Redundant Git Mcp Server (2026-06-07)

Remove the `mcp-server-git` Nix package output and its `.mcp.json` entry. MCP-aware editors already provide git through their built-in shell tool, so the wrapper added no capability and only contributed permission-prompt noise plus a `nix run` startup cost per session. The `fulsomenko/servers` flake input, which existed solely to provide this package, is dropped as well.

### KAN-482 Kanban Init No Implicit My Board (2026-06-07)

`kanban init` (no flag) now creates only the storage file, with no boards, columns, sprints, or cards. The implicit "My Board" board that was previously created is gone.
Use `kanban init --board "<name>"` for the one-shot file + first board. Anyone who scripted the old default behavior can restore it with `kanban init --board "My Board"`.

### KAN-504 Expose Card Parent Child Via Graph Operations (2026-06-07)

Card parent/child relations are now reachable from the CLI and the MCP server, not only the TUI. A new top-level `kanban relation` subcommand exposes `add`, `remove`, `parents`, and `children`; four matching MCP tools (`tool_set_card_parent`, `tool_remove_card_parent`, `tool_list_card_parents`, `tool_list_card_children`) cover the same surface for LLM clients.
```
kanban relation add KAN-5 KAN-7              # KAN-7 is now a subtask of KAN-5
kanban relation add KAN-5 KAN-7 KAN-8 KAN-9  # attach several children in one atomic batch
kanban relation children KAN-5               # list direct children
kanban relation parents KAN-7                # list direct parents
kanban relation remove KAN-5 KAN-7
```
Multi-child invocations are atomic: a mid-list failure (cycle, self-reference, unknown card) rolls the whole batch back so neither the in-memory graph nor the on-disk file ever holds a partial state.
Under the hood this lands a substantial rework of the graph model. The graph machinery in `kanban-core` is generic in shape; the concrete edge kinds live in `kanban-domain` and carry per-kind metadata directly in their types.
- **Generic graph machinery.** `EdgeStore<E>`, `DagGraph<E: Edge>`, `UndirectedGraph<E: Edge>` are parameterised over any type that implements the `Edge` trait (`source`, `target`, `created_at`, `archived_at`, `archive`, `unarchive`, plus the `from_endpoints` constructor for cross-kind synthesis). External crates can instantiate the graph types with their own edge structs without modifying `kanban-core`. A small trait taxonomy keeps each capability orthogonal: `Graph` (minimal direction-agnostic edge contract), `Directed: Graph` (outgoing/incoming), `Undirected: Graph` (neighbors), `Cascadable: Graph` (archive/unarchive/remove node), `EdgeSet: Graph` (read-only edge counts and membership). All five are generic over `Graph::NodeId` — the kanban domain uses `Uuid` today, but the algorithms themselves can serve a heterogeneous-entity graph keyed on any `Copy + Eq + Hash` type without touching `kanban-core`. Direction is encoded by the sub-graph type. `EdgeStore::add_edge` is crate-private; the public entry points (`Graph::add_edge`, `DagGraph::add_edge_with_metadata`, `UndirectedGraph::add_edge_with_metadata`) run the relevant invariants before delegating to the storage push. Cycle / self-reference validation also runs at load time on `Deserialize`, so a corrupted file fails to load rather than silently rehydrating an invariant-violating graph.
- **Per-kind edge structs.** Three concrete types in `kanban_domain::dependencies::edges` each embed an `EdgeBase` (endpoints + timestamps) and add their own metadata:
  - `SpawnsEdge { base }` — parent/child hierarchy. No metadata today.
  - `BlocksEdge { base, severity: Severity }` — blocker→blocked. `Severity` is `Low / Medium / High / Critical` with `Default = Medium`, derived `Ord` so algorithms can rank blockers without translating.
  - `RelatesEdge { base, kind: RelatesKind }` — undirected. `RelatesKind` is `General / Duplicates / MentionedIn` with `Default = General`.
  Adding a new edge kind means adding a new struct that implements `Edge` and a new sub-graph instantiation — no changes to existing types. The on-disk shape per kind is exactly what the kind needs: no `edge_type` / `direction` / `weight` catch-all fields.
- **`DependencyGraph` holds three typed sub-graphs.**
  - `parent_child: DagGraph<SpawnsEdge>` (cycle + self-ref rejected)
  - `blocks: DagGraph<BlocksEdge>` (cycle + self-ref rejected)
  - `relates: UndirectedGraph<RelatesEdge>` (self-ref rejected, cycles permitted)
  Cross-cutting cascades (`archive_node`, `unarchive_node`, `remove_node`) iterate over `[&mut dyn Cascadable; 3]`; cross-cutting reads (`len`, `active_len`, `contains`) iterate over `[&dyn EdgeSet; 3]`. Per-kind convenience methods live on `DependencyGraph`: `set_parent` / `parents` / `children` / `ancestors` / `descendants` (Spawns), `set_block` / `set_block_with_severity` / `unblock` / `blocked` / `blockers` / `can_start` (Blocks), `relate` / `relate_with_kind` / `dissociate` / `related` (Relates). Persistence backends and tests use per-kind accessors (`spawns_edges()` / `blocks_edges()` / `relates_edges()`) and the validating constructor `from_validated_per_kind_edges`. There is no kind-agnostic edge removal; per-kind methods are the only user-facing remove surface so a single-kind handler cannot accidentally sever a multi-kind pair.
- **Per-kind commands.** `DependencyCommand` has variants `AddSpawns` / `AddBlocks(severity)` / `AddRelates(kind)` / `RemoveSpawns` / `RemoveBlocks` / `RemoveRelates` / `CreateSubcard`. Each carries its kind-specific metadata; undo replay sees the same severity / kind the forward saw. The `Remove*` structs carry a `tolerate_missing: bool` flag (default `false` via `#[serde(default)]`): user-initiated removes surface `EdgeNotFound`, while inverse-replay constructions set the flag so undo succeeds against an already-removed edge. Each `Add*::capture_inverse` emits the matching per-kind tolerant `Remove*` so a `[AddSpawns(a,b), AddBlocks(a,b)]` batch's reverse-order undo handles each forward independently, not as a single kind-agnostic wipe. `RemoveBlocks` / `RemoveRelates` capture metadata at inverse-capture time by reading the pre-remove graph, so undoing a remove restores the original severity/kind. The `Add*` structs symmetrically carry an `as_archived: bool` flag with `#[serde(default)]`: user-initiated paths leave it false (edges land active), while cascade-undo (`DeleteCardEdges` / `DeleteCard`) sets it from `!e.is_active()` per incident edge so archived edges restore as archived instead of silently reviving to active. Three new helpers on `DependencyGraph` (`add_archived_spawns` / `add_archived_blocks` / `add_archived_relates`) provide the typed insertion path; `add_edge_with_metadata` already skips duplicate / cycle checks for archived edges, so these helpers route through the existing infrastructure. A small `edges_to_undo_commands` helper centralises the kind→`Add*` mapping used by the two cross-cutting capture-inverse sites (`DeleteCardEdges`, `DeleteCard`).
- **`GraphOperations` consolidated on plural primitives.** One canonical method per per-kind operation, using directed-graph verbs for the parent/child surface: `attach_children` / `attach_child` and `detach_children` / `detach_child` (Spawns), `block(severity)` / `unblock` (Blocks), `relate(kind)` / `dissociate` (Relates), plus `list_children_of` / `list_parents_of` / `list_blocked_by` / `list_blockers_of` / `list_related_to`. The plural is the atomic primitive — singular methods are default-impl forwards that wrap a single id in `vec![]` and call the plural, so every mutation routes through the same `KanbanContext::execute(Vec<Command>)` transactional path (this matches the project precedent set by `archive_card` calling `archive_cards(vec![id])`). The earlier shape exposed four near-duplicate ways to add a parent edge with three different argument orderings; the consolidated trait is one shape per operation. No `kind: CardEdgeType` parameter anywhere — the type system expresses what's being mutated. Existence guards on add/remove and the list paths reject unknown card UUIDs symmetrically before the command reaches the graph.
- **App alignment.** All three apps (TUI / CLI / MCP) route every graph mutation through `GraphOperations` on their respective context wrappers. The TUI's relationship popup now goes through `attach_child` / `detach_child` like the other two apps; the direct `Command::Dependency(...)` construction in `popup_handlers.rs` is gone. No app reaches into `.graph.parent_child` / `.graph.blocks` / `.graph.relates` directly — the service layer is the only mutation gate.
- **SQLite per-kind tables.** The single `card_edges` table is replaced by `spawns_edges` / `blocks_edges` / `relates_edges`. Each table has just the columns its kind needs: `blocks_edges.severity` with `CHECK (severity IN ('Low','Medium','High','Critical'))`, `relates_edges.kind` with `CHECK (kind IN ('General','Duplicates','MentionedIn'))`. No `edge_type` / `direction` / `weight` catch-all columns. `SqliteStore::open` drops the pre-KAN-504 `card_edges` table on first encounter; nothing of this graph work is live yet on `develop` so there is no installed-base of data to preserve.
- **JSON V5→V6 split-graph migration.** Old shape: `graph.cards.edges: [{ edge_type, direction, weight, ... }]`. New shape: `graph.{ parent_child, blocks, relates }.edges: [{ source, target, created_at, archived_at, severity? / kind? }]`. The migration strips `edge_type` / `direction` / `weight` from each migrated edge and populates per-kind defaults (`Medium` severity for migrated Blocks rows, `General` kind for migrated Relates rows). Files at V1..V5 are auto-migrated on load through the appropriate legacy chain followed by the split-graph step; the chain writes `.v{N}.backup` before the split-graph step for V3/V4/V5 starting points so an upgrade can be rolled back. The split-graph transform is idempotent: invoking it on an already-V6 envelope returns immediately without overwriting the sub-graphs. Migration and sync paths use `AtomicWriter::write_atomic_sync` with a unique random temp file. The new post-migration shape matches what a freshly saved file produces byte-for-byte.
- **Typed error boundaries.** `KanbanCliError` (`Domain` / `Resolution` / `Io` / `Serialization`) and `KanbanMcpError` (`Domain` / `Resolution`) wrap `KanbanError` so handlers thread every failure through `?` uniformly. Both surfaces use the same `Resolution { hint }` variant for handler-built enrichment of anonymous domain errors, and both render the hint verbatim with no wrapper prefix — the MCP enrich helpers route exclusively through `Resolution`, so a `messages::parent_cycle(...)` hint produces byte-identical user-facing messages on both surfaces. The wire-level MCP error code stays `INVALID_PARAMS` because the `From<KanbanMcpError> for McpError` conversion maps `Resolution` to it. Identifier-resolution failures flow through `Domain` directly so the structured `DomainError::NotFoundByName` / `Ambiguous` variants stay introspectable. The MCP `locked_read` / `locked_write` helpers are generic over `E: Into<McpError>` so typed closures plug in without per-handler conversion boilerplate.
- **Correctness hardening.** Graph mutations got four review-driven fixes that close real edge cases:
  - **Duplicate-edge rejection.** `GraphError::Duplicate` (mapped to `DependencyError::DuplicateEdge`) is returned when an active edge with the same endpoints already exists. DAG checks the directed orientation; undirected checks either ordering. Archived edges don't count, so re-add after archive still works. Closes a silent bug where `set_parent` twice would put `child` in `parents()` / `children()` twice and bias future cycle decisions via a duplicated adjacency entry.
  - **`EdgeSet::contains` aligned to active-only.** Previously `EdgeSet::contains` reported true for archived edges while `Graph::contains_edge` (similar name) reported only active — silent divergence between two near-identical methods. Both now mean "is this here right now?". The any-state lookup remains available via the new `EdgeSet::contains_archived` / `DependencyGraph::contains_archived` for callers that genuinely need to consult history.
  - **`remove_edge` preserves archived records.** `EdgeStore::remove_directed_edge` / `remove_undirected_edge` used to retain over the full list, sweeping archived edges alongside the active one — so `Graph::remove_edge` and the per-kind `remove_parent` / `unblock` / `dissociate` all silently destroyed history. They now filter on `is_active()`, so archive records survive a remove.
  - **Load errors carry context.** `from_validated_per_kind_edges` wraps each per-kind add error with the kind tag and offending source/target endpoints (e.g. `"load failed on blocks edge <s> -> <t>: cycle detected"`), so a user inspecting a corrupt-file diagnostic can grep the source for the named UUIDs. The bare `DependencyError` variants only said "cycle detected" with no clue which kind or which edge.
  - **`DependencyGraph` sub-graph fields private.** `parent_child` / `blocks` / `relates` were `pub(crate)`; now fully private. No call site reaches in, but the visibility allowed future regression of the validation gate. External access stays via `*_edges()` accessors and `from_validated_per_kind_edges`.
- **Shared parent-relation messages.** `kanban_domain::dependencies::messages` holds the formatters CLI and MCP both consume — cycle / self-reference / edge-not-found messages name both sides of the offending edge using the user's raw identifiers. The two surfaces produce identical wording for the same failure. Both `enrich_*` helpers in CLI/MCP match exhaustively on `DependencyError` so a new variant fails to compile until the maintainer handles it. The CLI batch-mode enrichment widens the wording to name all children in the batch when the variant alone doesn't pinpoint the offender.
- **Service-level card-existence validation.** `KanbanContext`'s per-kind edge methods reject unknown card ids up front via `require_card_exists`, returning `NotFound { entity: "card", id }` before the command reaches the graph. Closes a data-integrity hole: the CLI's identifier resolver parses raw UUIDs but does not look them up, so a stale or fabricated UUID would silently land in the graph as a dangling edge whose endpoints reference no card.
`CardEdgeType` remains as a discriminator for parameterised tests and cross-kind utilities; production code is per-kind throughout. The transitive `LegacyEdge` struct used during the refactor is gone. Cross-board parent/child is permitted, matching the prior TUI behavior. Cross-kind algorithms can take `&impl Edge` or `&dyn Edge` for uniform read access without knowing concrete metadata; per-kind algorithms take `&DagGraph<BlocksEdge>` directly and see severity as a typed field.

### KAN-520 Card Detail Enter On Parent Child Does Not Navigate To That Card Active Card Id Stale (2026-06-07)

Pressing Enter on a card in the Parents or Children box of the card detail view now reloads the detail view against that card, as it always should have. The same fix applies at the other entry points into the detail view (Enter and 'e' on a sprint-detail card row) and to Backspace returning through the navigation history. Previously the detail view appeared to stay on the original card while the parents box silently emptied out.
The underlying drift between the active-card index and the active-card UUID is now prevented at the type level: the two fields have been collapsed into a single struct whose constructor requires both values, so future handlers cannot reintroduce this bug class.

### KAN-522 Refuse On Version Mismatch In Persistence Backends Writer Stamp Metadata Startup Banner (2026-06-07)

Kanban now refuses to silently mishandle data files written by a newer
version of itself, and surfaces enough information for you to diagnose
version mismatches at a glance.
**Refuse-on-future-version.** Opening a JSON file whose `version` is higher
than the binary supports, or a SQLite database whose `schema_version`
exceeds the binary's, now returns a typed error rather than silently
coercing the file to a lower format (which previously dropped fields the
old reader did not understand). The error message tells you the file's
version, the binary's maximum, and asks you to upgrade. Refused files are
left untouched on disk — no schema bump, no column ALTER, nothing rewritten
before the refusal fires.
**MCP error category.** Pointing the `kanban-mcp` server at a future-format
file now surfaces as `INVALID_PARAMS` to the MCP client (was
`INTERNAL_ERROR`). That category change tells the LLM the input is
unusable, not that the server is broken — so the client can suggest
pointing at a different file or upgrading kanban instead of treating it
as a server bug.
**Writer stamp on save.** Every save now records which kanban produced the
file: a semver version string and the build's git commit. Old files that
lack the stamp continue to load cleanly; the new fields show up the first
time the file is rewritten.
**F12 diagnostics popup.** The "Error Log" popup that already lived behind
F12 has been renamed to **Diagnostics** and now shows:
- File path
- Format version (read live from the file, not assumed from the binary)
- Writer (the kanban that last wrote this file)
- Binary (the kanban you're running right now)
- Last saved timestamp
- Log entries in a separate, titled section below
When the file's writer is a newer semver than the running binary, the
Writer line is highlighted in yellow with a `(newer than this binary)`
suffix, so a mismatch is one keystroke away from being diagnosed instead
of buried in tracing output.
**SQLite schema bump.** The SQLite metadata table gains `writer_version`
and `writer_commit` columns and the on-disk `schema_version` is bumped to
2. Existing databases are upgraded transparently on open via the same
idempotent `ALTER TABLE ADD COLUMN` mechanism used for previous SQLite
schema additions — no manual migration required.
**Renamed const.** Internal API: `kanban_core::VERSION` is renamed to
`kanban_core::CLI_VERSION_DISPLAY` to distinguish the multi-line clap
display string from the new raw `KANBAN_VERSION` / `KANBAN_COMMIT`
components used by the writer stamp. This is only relevant to anyone
embedding kanban-core as a library.
Library consumers exhaustively matching on `kanban_domain::KanbanError`
will need to add an arm for the new `UnsupportedFutureVersion` variant.

### KAN-530 Rename Json Parent Child Graph Field To Spawns To Match Domain Naming (2026-06-07)

The JSON storage backend now uses `spawns` as the key for the parent/child
dependency-graph bucket, matching the name used everywhere else in the app
(the `SpawnsEdge` type, the `spawns_edges` SQLite table). Previously the
JSON file alone exposed this bucket as `parent_child`, a leftover from an
older field name.
Existing kanban files written in the older format are upgraded
automatically on the next load. A `.v6.backup` copy of the original file
is written before the upgrade and removed once it completes successfully,
so a failed upgrade leaves a recoverable file in place. No manual action
is needed.
The on-disk envelope version advances from 6 to 7. Older builds of the
app will refuse to open V7 files (the existing future-version guard) to
prevent silently dropping data they don't understand. SQLite storage is
unaffected, since it already used the `spawns_edges` name internally.

### KAN-534 Fix Detail View Card Lookup To Use Id Not Index After External File Reload (2026-06-07)

Actions you take on the currently-open card right after an external write (a CLI update, an MCP tool call, another TUI saving) now operate on the card you are actually viewing, instead of silently operating on whichever card happens to occupy that slot in the freshly reloaded list.
Affected interactions include pressing `e` on the card detail Metadata section, opening the Manage Parents and Manage Children dialogs, the parent and child counts shown in the detail sidebar, the priority popup, the sprint-assign popup, the points dialog, editing the card title or description, copying the branch name or git checkout command, and the current-priority and current-sprint indicators in their respective dialogs. Pressing Backspace to return through the detail-view navigation history also now resolves the previous card by identity rather than by position, so back-navigation lands on the originally visited card even after the cards list has been re-sorted underneath it.
The underlying bug was that the TUI tracked the active card by both its stable UUID and its position in the cards list. After an external write the file watcher reloaded the list sorted by most recently updated, but the stored position pointed at a now-different card. Every action that resolved the active card by position silently operated on the wrong target. All such call sites now resolve the active card through the model's UUID-keyed lookup, the navigation history stores UUIDs instead of positions, and the active-card type no longer carries a position at all, so this class of bug can no longer be reintroduced by future handlers.

### KAN-538 Author Packaging Chocolatey Skeleton Nuspec Install Scripts License Verification (2026-06-07)

Internal: added the Chocolatey package source files under
`packaging/chocolatey/` (nuspec, install/uninstall scripts, LICENSE
placeholder, VERIFICATION). No user-visible change in this release —
the files sit unused until the Chocolatey publish workflow lands in
a follow-up commit, at which point `choco install kanban` becomes
available on Windows.

### KAN-539 Release Yml Build And Upload Windows Release Archive On Tag Push (2026-06-07)

GitHub releases now include a Windows release archive built directly
by CI. Each release page surfaces
`kanban-v$VERSION-x86_64-pc-windows-msvc.zip` containing prebuilt
`kanban.exe` and `kanban-mcp.exe` binaries (alongside `LICENSE.md`
and `README.md`), plus a `SHA256SUMS` file for integrity verification.
Windows users can download and run the binaries directly from the
GitHub release page without compiling from source. The same archive
is the substrate for the upcoming Chocolatey publish workflow.

### KAN-541 Release Yml Add Chocolatey Publish Step (2026-06-07)

`kanban` is now available on Chocolatey. After this release reaches
moderation approval on community.chocolatey.org (typically 1-7 days
for a first version), Windows users can install via:
    choco install kanban
The package installs both `kanban` (TUI/CLI) and `kanban-mcp` (MCP
server) and adds shims for both onto PATH. Release CI handles
packaging and publishing automatically on every release with
changesets; the `CHOCO_API_KEY` repo secret authenticates the push.
A smoke install on the Windows runner gates the push, so a broken
package never reaches the registry.

### KAN-545 Choco Cosmetic Polish Iconurl Verification Wording Readme Clarity (2026-06-07)

The Chocolatey package page now displays a `kanban` brand icon
instead of the generic placeholder. The full bold-icon family
(PNG + SVG, three transparency variants each) is also committed
to `assets/` for use by other registries, docs, and downstream
distributors.
Two small documentation tweaks ship alongside: the
`VERIFICATION.txt` step that points users at the chocolatey.org
package page is reworded to be less circular, and the
`packaging/chocolatey/README.md` developer example now sets `$VERSION`
and `$SHA` as real PowerShell variables so the snippet is
copy-paste-runnable on Windows without ambiguity.

### KAN-550 Add Claude To Gitignore Cli Tool Scratch Per Machine Settings (2026-06-07)

Internal: `.claude/` is now gitignored. Contributors using the
Claude Code CLI will no longer risk accidentally committing
per-machine settings (`.claude/settings.local.json`) or
agent scratch worktrees (`.claude/worktrees/`, which can grow
to tens of MB during parallel agent runs). No user-visible
change.

### KAN-551 Retain Card Selection After Toggle Completion (2026-06-07)

In the kanban (column) view, toggling a card's completion status now keeps the card
selected after the toggle. Previously, the card would be moved to the Done column
by the service layer, but the selection would silently drop on the next render frame
because the view was not refreshed before the selection was updated.
`select_card_by_id` has also been made robust for any view: if the card is not found
in the currently active column list it now searches all column lists, navigates to the
column that holds the card, and selects it there. This prevents silent selection drops
whenever a card moves between columns as a side effect of an operation.

### KAN-556 Add Optional Sprint Assignment To Card Creation (2026-06-07)

Cards can now be assigned to a sprint at creation time, in a single
action, across all three surfaces.
In the TUI, the Create Task dialog gains a sprint picker below the
title input. If the board has exactly one active (non-ended) sprint,
that sprint is pre-selected, so pressing Enter creates the card already
attached to the active sprint. With no active sprint or with multiple,
the picker defaults to "None" and the user can pick deliberately. Tab
toggles focus between the title input and the sprint picker; Down or
Esc on the title focus drops focus into the picker (Esc on the picker
focus closes the dialog); Up/Down or j/k navigate the picker; Enter
confirms from either side. The focused side is signalled with a bright
border on the title input or the picker block.
From the CLI, `kanban card create` accepts a new `--assign` / `-a`
flag. Pass a sprint UUID, name, or number to assign the new card to a
specific sprint, or pass the flag with no value to use the board's
sole active sprint. The flag fails with a clear message when there
are zero or multiple active sprints on the board.
The MCP `create_card` tool gains an optional `sprint_id` parameter
that accepts a UUID, sprint name, or sprint number and resolves it via
the same in-board sprint resolution used by `assign_card_to_sprint`.
The schema description hints to LLM callers that when a board has a
single active sprint, passing its id at create time avoids the extra
assign round-trip.
The on-disk format is unchanged. Existing scripts and integrations
keep working with no migration required; omitting the new flag/field
preserves the previous "create then assign separately" workflow.

### KAN-557 Extract Radiolist Sprintpicker (2026-06-07)

Internal refactor with no user-visible behaviour change. The two sprint-assignment
dialogs (single-card and bulk) and the existing list-component navigation now
share a single set of reusable building blocks, making future selection dialogs
quicker and safer to add.
The sprint-assignment dialogs render the same Active / Planned and Completed /
Ended sections, the same green-bold "(current)" indicator, the same sticky
section header when scrolling past it, and the same colour coding for Completed
(green) versus Ended (red) sprints. Keyboard navigation, dialog framing, and
selection persistence are unchanged.
Under the hood the rendering and navigation pieces are now factored as:
- `RadioList<T>` — a domain-agnostic single-select list with optional sticky
  section-header overlay, used by both sprint-assignment dialogs.
- `SprintPicker` — a thin adapter on top of `RadioList<Option<Uuid>>` that
  knows about sprint sections, the "(current)" suffix, and the pre-selection
  rule for the create-card flow that's coming next.
- `list_nav` — pure selectable-skipping navigation helpers shared by
  `RadioList`, `sprint_assign_list`, and `ListComponent`. The duplicate
  index-step helpers on `Page` in `kanban-core` have been removed.
The refactor unlocks two upcoming changes: sprint selection at card creation
time (KAN-556) reuses `RadioList` + `SprintPicker` directly, and the planned
multi-select picker (KAN-558) will share the same `ListItem<T>` shape and
`list_nav` primitives rather than duplicating them.

### KAN-580 Refactor Domain Apply Nuanced String Str Impl Into String Rule To Domain Params (2026-06-07)

Internal: domain constructors and mutators that always store their string
input now accept `impl Into<String>` instead of `String`. This means
callers can pass `"foo"` or `String::from("foo")` interchangeably without
a trailing `.to_string()`, and ownership decisions stay at the call site
rather than being forced at the domain boundary.
There is no behaviour change for users. Saved files, the CLI surface,
the MCP tool schemas, and the TUI all work exactly as before. The
refactor is API-source-compatible for any external caller already
passing `String` or `Some("...".to_string())`, and only loosens what
those APIs accept. One ergonomic note: parameters that became
`Option<impl Into<String>>` can no longer infer the type of a bare
`None`, so external callers that previously wrote `update_prefix(None)`
must now write `update_prefix(None::<String>)` (or any concrete
`Option::<T>::None`). This only affects the `None` case; `Some("...")`
callers are unchanged.
Call sites across the service, persistence-sqlite, and TUI test suites
were updated to drop the now-redundant `.to_string()` allocations, which
removes a small amount of test setup noise. The contributor guide gains
a short note describing the "unconditional store rule" so future domain
APIs follow the same convention.

### KAN-643 Fix Domain Accept Yyyy Mm Dd In External Editor Due Date Field (2026-06-07)

Editing a card's metadata through the external editor (`e` on the
Metadata section of Card Detail) now accepts plain `YYYY-MM-DD` dates
in addition to full RFC 3339 timestamps. Previously the editor
silently dropped any value that wasn't full RFC 3339, leaving the
field unchanged with no feedback. This matches what the CLI and MCP
already accepted and what the TUI already displays.
A date like `2024-01-15` is stored as midnight UTC on that day. A
full timestamp like `2024-01-15T14:30:00Z` is stored at the exact
instant the user supplied. When you re-open the editor, midnight-UTC
values are written back as `2024-01-15` (so the format you typed is
the format you see), and any non-midnight value is written back as
RFC 3339.
Malformed dates such as `"yesterday"` no longer disappear silently:
the editor now surfaces a clear error banner explaining the supported
formats. ISO 8601 zero-padding is required (`2024-1-5` is rejected
with the same banner), keeping behaviour predictable.
No file-format changes. Existing kanban files load unchanged and
existing RFC 3339 values continue to round-trip exactly as before.

### KAN-644 Add Due Date Sort Field For Cards (2026-06-07)

Cards can now be sorted by their due date in every view across all three
frontends.
**New features:**
- The TUI's "Order Tasks By" popup gains a **Due Date** option alongside
  the existing Points / Priority / Status / etc. Cards without a due date
  sort last in ascending order (matching the existing behaviour for
  cards without points).
- `kanban card list` accepts `--sort` and `--order` flags. When omitted,
  the listing falls back to the board's persisted default sort. The
  flags also apply to `kanban card list --archived`.
- `kanban board update` accepts `--sort-field` and `--sort-order` to set
  the board's default task sort from the CLI. Previously this was only
  reachable through the TUI popup.
- The MCP `update_board` tool exposes `task_sort_field` and
  `task_sort_order` so agents can persist a board's default sort.
- The MCP `list_cards` and `list_archived_cards` tools accept `sort` and
  `order` parameters; when omitted they inherit the board's default.
  `list_archived_cards` also gains a `board` parameter so archives can be
  scoped to one board.
- `kanban relation parents` and `kanban relation children` accept
  `--sort due-date` to order related cards by due date.
**Supporting improvements:**
- Filtering and sorting now share one pure domain helper,
  `filter_and_sort_cards`, generic over `T: Borrow<Card> + Clone` so
  archived cards flow through the same predicate via the existing
  `Borrow<Card> for ArchivedCard` impl. `KanbanContext::list_cards`,
  `KanbanOperations::list_archived_cards_sorted` (default impl),
  `CardQueryBuilder::execute`, and the TUI render path all delegate to
  it. CLI, MCP and the TUI inherit consistent ordering and filtering
  from one source instead of each re-implementing them.
- `CardListFilter` carries the three filters the TUI used to apply
  client-side: any-of sprint membership (`sprint_ids`), `hide_assigned`,
  and full-text `search`. The TUI's `get_sorted_board_cards`,
  `get_board_card_count`, and the layout-strategy `CardQueryBuilder`
  delegate to the domain helper directly on the model snapshot, so the
  render path no longer touches the backend on every redraw.
- A new `count_filtered_cards` shares the same predicate without
  allocating a result vector or sorting; the TUI badge/count path uses
  it. A regression test pins parity between
  `count_filtered_cards(filter)` and `list_cards(filter).len()` across
  every non-trivial filter combination.
- The (override → board default → none) sort-resolution rule and the
  `OrderedSorter` / `get_sorter_for_field` plumbing have been collapsed
  into two pure helpers, `resolve_sort` and `sort_cards_in_place`. The
  duplicated resolution logic in `KanbanContext` and the
  `KanbanOperations` trait default is gone.
- The TUI sort-field popup is now driven by a single
  `SORT_FIELD_POPUP_ORDER` table; adding a future sort field only
  requires editing one slice instead of three separate index matches.
- MCP descriptions for `task_sort_field`, `sort` and the archived-card
  `sort` now explain that `default` orders by card number and that date
  fields and points place None values last in ascending order.
Library consumers exhaustively matching on `kanban_domain::SortField` or
`kanban_domain::SortBy` will need to add an arm for the new `DueDate`
variant.

### KAN-646 Split Operations Into Trait And Filter Sort (2026-06-07)

Internal cleanup with no user-visible change. The filter+sort engine that
backs `kanban card list`, the TUI's board view, and the MCP `list_cards` /
`list_archived_cards` tools moved into its own module
(`kanban_domain::query::filter_sort`), separated from the
`KanbanOperations` service-contract trait it used to share a file with.
Every public name a downstream user might depend on (`CardListFilter`,
`ArchivedCardListFilter`, `filter_and_sort_cards`, `count_filtered_cards`,
`KanbanOperations`) is still importable from the `kanban_domain` crate
root.
**Supporting improvements:**
- The filter+sort engine is now in a single-purpose module instead of
  buried under the trait surface. Future work in this area (e.g. KAN-645
  generalising sort across listable entities) has a smaller, focused
  file to edit.
- The `KanbanOperations` trait file shrinks from 586 to 442 lines,
  bringing it closer to the project's per-file size guideline. The full
  trait split is tracked separately (KAN-645).

### KAN-649 Make Release Yml Idempotent On Partial Failure Re Runs (2026-06-07)

Hardened the release workflow against partial-failure re-runs. The Tag
version step now guards both the local tag and the push so re-running
after a half-completed prior run no longer crashes on tag collision.
The Publish to Chocolatey job is now marked continue-on-error so a
stuck moderation queue or transient API failure surfaces as a warning
rather than turning the entire workflow red after crates.io, GitHub
Release, AUR, and Homebrew have already succeeded.
A new docs/release-recovery.md runbook enumerates the per-step recovery
procedure: which steps are safe to re-run from the GitHub Actions UI,
what state to expect on origin after each failure mode, and the manual
fallback commands for the cases where a re-run is not enough.
No user-visible runtime behaviour changes; this only affects how the
release pipeline recovers when something goes wrong.

### KAN-650 Sync Migration Backups (2026-06-07)

The synchronous JSON migration path used by `kanban` (CLI), the TUI
startup, and `kanban-mcp` now writes a `.v{N}.backup` file before
running the shape-changing V→V7 migration chain, removes it on
success, and preserves it on failure. Previously only the asynchronous
load path produced these rollback artefacts; the sync path would
overwrite files in place with no recourse if a step failed
mid-chain.
End users upgrading a V3/V4/V5/V6 JSON file to V7 via any sync entry
point now see the same backup-and-cleanup behaviour as users who go
through the async path: a successful migration leaves no extra files
on disk, and a failed migration leaves a `.v3.backup` / `.v4.backup`
/ `.v5.backup` / `.v6.backup` alongside the original file with the
path surfaced in the error log.
V1→V2 keeps its existing in-step `.v1.backup`; V2→V3 is shape-stable
and continues to need no backup.
No API or message changes for library consumers. The
source-version-to-backup-path policy is now shared by both
orchestrators in a single `migration::backup::pre_v7_backup_path_for`
helper so future migration additions only need to update one site.

### KAN-655 Card Create Assign Negative Tests (2026-06-07)

The cross-board sprint check on `card create --assign` now returns a
typed `DomainError::SprintBoardMismatch { sprint_id, sprint_board,
card_board }` error variant instead of an untyped `Validation`
message. End-user error text is unchanged across the CLI, MCP, and
TUI (the message format is preserved verbatim by the new variant's
`Display` impl), but library consumers can now match on the variant
structurally and get the three relevant UUIDs without parsing a
string.
A new `KanbanError::is_sprint_board_mismatch()` predicate follows
the same shape as the other `is_*` helpers.
Negative-path test coverage has been added for `card create
--assign` across all three surfaces:
- `kanban-service` integration tests cover the unknown-sprint-UUID
  case (returns `NotFound { entity: "sprint" }`) and the
  cross-board-sprint case (returns the new typed variant).
- `kanban-cli` integration tests invoke the real binary and assert
  that stderr surfaces "Sprint" and the offending name or UUID when
  `--assign` is given an identifier the resolver cannot find.
- `kanban-mcp` integration tests exercise the same negative paths
  through the actual `tool_create_card` handler so the wire-level
  error message is pinned for LLM clients.
No behavioural change for end users; this fills a coverage gap and
strengthens the error contract for library and MCP consumers.
Library consumers exhaustively matching on `kanban_domain::DomainError`
will need to add an arm for `SprintBoardMismatch`.

### KAN-656 Chocolatey Asset Poll Via Gh Api (2026-06-07)

The release workflow's Chocolatey publish job now reads the Windows
ZIP's SHA256 directly from the GitHub Release API's `digest` field
(exposed since June 2025) and uses `state == "uploaded"` as the
asset-readiness signal. Two release.yml steps collapse into one:
the previous `HEAD`-based poll and the separate download-and-hash
step both go away. The `publish-chocolatey` job also gains an
explicit `permissions: contents: read` scope so a future tightening
of org-default token permissions cannot silently break the digest
lookup, and per-iteration `gh release view` stderr is suppressed so
the action log stays clean while the release tag is still being
created upstream.
The `HEAD` poll was latently broken. GitHub release-download URLs
302-redirect to S3-style presigned URLs that are cryptographically
signed for a specific HTTP method; `Invoke-WebRequest -Method Head`
auto-follows the redirect and gets a 403 even when `GET` on the same
URL would succeed. The bug would have surfaced on the first 0.7.x
release attempt that produced Windows artifacts.
No behavioural change for users. The chocolatey nupkg is templated
with the same `$checksum64$` value as before; the change is only in
how the workflow obtains that value.

### KAN-657 Chocolatey Recovery Runbook (2026-06-07)

The release workflow's `publish-chocolatey` job is now safe to
re-run after a transient post-push failure, and surfaces an
actionable error message that points at a recovery runbook on
real failures.
Chocolatey rejects re-pushing the same `id + version`
permanently, which means a workflow re-run after the underlying
push has already succeeded would silently surface a fresh red
"failure" that obscures the previous success. The job now does a
pre-check against
`community.chocolatey.org/api/v2/Packages(Id='kanban',Version=...)`
and exits 0 with an explanatory message when the version is
already published. On a genuine push failure, the job prints a
pointer to `packaging/chocolatey/RECOVERY.md` along with a clear
"do not simply re-run this job" warning.
The new `packaging/chocolatey/RECOVERY.md` runbook documents the
four real failure scenarios (push-succeeded-but-reported-failure,
malformed nupkg, rejected API key, moderation backlog) with
diagnosis steps for each, an anti-patterns section, and a
"reading this in a hurry" table at the bottom.
`packaging/chocolatey/README.md` cross-links to it.
No behavioural change for end users installing the package.

### KAN-659 Capitalize Not Found Entity Tags (2026-06-07)

Error messages for "not found" lookups now use consistent capitalization
across all code paths. Previously the same error category rendered
differently depending on whether the lookup was by UUID
(`"sprint <uuid> not found"`, lowercase) or by name
(`"Sprint 'foo' not found"`, capitalized). Both forms now read with the
sentence-leading capitalized noun.
User-visible impact: error messages for unknown card / column / sprint
/ board UUIDs now start with a capital letter to match the existing
name-lookup messages. No structural change to error types or
diagnostics; only the first letter of the entity name in the rendered
message changes.
Library consumers exhaustively matching on `DomainError::NotFound`
should note that the `entity` field is now always a capitalized noun
(`"Card"`, `"Column"`, `"Sprint"`, `"Board"`) rather than the previous
lowercase form. The `NotFoundByName` variant's casing is unchanged. A
doc comment on both variants now documents the convention so future
not-found additions inherit the same casing.

### KAN-660 V1 V2 Backup Wrap (2026-06-07)

The synchronous and asynchronous JSON migration paths now also write a
`.v{N}.backup` for V1 and V2 source files before running the V→V7
chain, extending the rollback coverage that KAN-650 added for
V3/V4/V5/V6 sources.
Pre-fix, a V1 file going through the V1→V2→V3→V6→V7 chain only had
a transient `.v1.backup` written by the V1→V2 step itself, and that
backup was removed on V1→V2 success — leaving no rollback artifact if
the subsequent destructive steps (split-graph or v6→v7 rename) failed.
V2 files had no backup at all. Both gaps are now closed: the outer
backup is taken before the first per-step migration runs and is removed
only after the full V→V7 chain succeeds.
Users opening a V1 or V2 file with kanban 0.7.x+ via any normal entry
point (CLI command, MCP tool call, TUI startup) will now see a
`.v1.backup` or `.v2.backup` preserved on disk if a mid-chain step
fails, mirroring the V3..V6 behaviour.
One subtle behaviour change for direct library consumers of the
`kanban-persistence-json` crate: invoking `Migrator::migrate(V1, V2,
path)` or the `V1ToV2Migration` strategy wrapper as a standalone
*V1→V2* step (not chained through to V7) no longer writes its own
`.v1.backup`. The per-step backup mechanism was removed in favour of
the outer V→V7 wrap, which doesn't fire for the standalone case.
Library consumers wanting backup protection should use
`Migrator::migrate(V1, V7, path)` instead, which provides the outer
wrap that covers the entire chain. No in-repo callers were affected.

### KAN-668 Fix Validate Release Sh Step 5 Silently Passing When Every Dry Run Fails (2026-06-07)

The release-tooling pre-publish validation now actually catches packaging
defects. Step 5 of `validate-release` previously ran `cargo publish --dry-run`
in offline mode and swallowed the resulting failure, so every release reported
"All crates passed dry-run validation" regardless of manifest state. Step 5
now runs `cargo package --no-verify` and fails the release on any non-zero
exit, so packaging defects (missing required fields, broken readme/license
file references, file-exclusion regressions) are caught before crates.io
publish rather than mid-publish where some sibling crates have already
shipped.
Internal dev-dependencies on sibling workspace crates are now path-only
(no version constraint), per the existing project convention. The previous
version constraints made `cargo package` fail to resolve sibling features
added between releases against the published version. Step 3 of
`validate-release` now enforces this convention so the regression cannot
recur.
No user-visible runtime change; this only affects the release pipeline's
ability to detect manifest defects before publishing.


## [0.6.0] - 2026-05-15 ([#276](https://github.com/fulsomenko/kanban/pull/276))

### CAT-323 Fix Misleading Card Not Found Error When Board File Does Not Exist (2026-05-15)

Running `kanban missing.json card get KAN-1` (or any subcommand) when the
file does not exist now reports `Board file not found: 'missing.json'`
instead of the misleading `Card not found: 'KAN-1'`. The check covers
every path that can supply the file — positional argument, `KANBAN_FILE`
environment variable, and `storage_location` in the config file.
`board create` previously had an implicit dual role: it created the domain
entity AND initialised the storage file when it did not exist yet. That
responsibility has moved to `kanban <file>` with no subcommand. Running
`kanban newboard.json` now creates an empty storage file if one does not
exist and exits cleanly, making it safe to use in scripts and CI without a
live terminal. In a TTY the TUI launches as before.

### Other Changes (2026-05-15)

Refactor editor functionality to handle arbitrary EDITOR strings
- Allow the user-defined EDITOR to fully determine what editor launches and how, while preserving limited fallback behavior to `notepad` and `vi` for Windows and non-Windows respectively
- VS Code is still broken due to issues with `code --wait`, so editors that stay in the terminal are heavily preferred
- Vim-like editors are the most well-tested for this project and expected to work on every OS without issue
- Separate installs are currently recommended for Windows and WSL, as switching between them with the same binary can trigger consistent recompiles
- On Windows, it is strongly recommended to set your `EDITOR` to a program that is in your `PATH`, for example `$env:EDITOR = "vim.exe"` in PowerShell. Path resolution for Windows-like paths in your `EDITOR` will cause issues.

### KAN-328 Sprint Filter Toast (2026-05-15)

Pressing `t` to toggle the active-sprint filter on a board that has no
active sprint now surfaces an error banner reading "No active sprint set
for filtering" instead of failing silently. Previously the keypress
appeared to do nothing while quietly emitting a warning to the trace log,
leaving users confused about why the card list did not change.

### KAN-400 Accept Names Everywhere (2026-05-15)

Every CLI command and MCP tool now accepts a human-readable name (or sprint
number) anywhere it previously required a raw UUID. Plain UUIDs continue to
work, so existing scripts that already use UUIDs still resolve correctly.
You can now write things like:
```
kanban board get Kanban
kanban column list --board Kanban
kanban sprint activate yarara-release
kanban sprint get 15
kanban card create --board Kanban --column TODO --title "Hi"
kanban card move KAN-12 --column Doing
kanban card move-cards --cards KAN-1,KAN-2 --column Doing
kanban card assign-cards-to-sprint --cards KAN-1,KAN-2 --sprint yarara-release
```
The same input flexibility applies to every MCP tool. Board, column, and
sprint fields in tool schemas now read "UUID or name" (or "UUID, name, or
number" for sprints) instead of demanding a UUID. The batch tools
`archive_cards`, `move_cards`, and `assign_cards_to_sprint` take a JSON array
of card UUIDs or identifiers (for example `["KAN-1", "KAN-2"]`) in place of
the old comma-separated string.
**Breaking flag and field renames.** Now that these inputs accept names, the
old `--*-id` flag names and `*_id` schema fields were misleading and have
been replaced with the bare entity name:
CLI flags:
| Old                                  | New                              |
| ------------------------------------ | -------------------------------- |
| `--board-id`                         | `--board`                        |
| `--column-id`                        | `--column`                       |
| `--sprint-id`                        | `--sprint`                       |
| `--ids` (on `card archive-cards`, `card move-cards`, `card assign-cards-to-sprint`) | `--cards` |
Positional arguments now show `<BOARD>` / `<COLUMN>` / `<CARD>` / `<SPRINT>`
in `--help` output instead of the generic `<ID>`.
MCP tool input fields:
| Old                  | New              |
| -------------------- | ---------------- |
| `board_id`           | `board`          |
| `column_id`          | `column`         |
| `sprint_id`          | `sprint`         |
| `card_id`            | `card`           |
| `ids` (batch tools)  | `cards`          |
| `from_sprint_id`     | `from_sprint`    |
| `to_sprint_id`       | `to_sprint`      |
No aliases are kept. Update scripts and MCP clients to the new spellings.
When a name does not match, the error tells you exactly what is available,
for example: `Column 'done' not found. Available: 'TODO', 'Doing', 'Complete'`.
When a name is ambiguous across boards, the error names the boards in
conflict and asks you to disambiguate by UUID or a unique name.
For `card move-cards` and `card assign-cards-to-sprint`, the selection must
now share a single board so the target column or sprint can be resolved
unambiguously within it; mixing cards from different boards produces a clear
"Batch operation requires all cards on the same board" error.
Names are case-insensitive. Sprints accept either a name (matched against
the board's stored sprint names) or a sprint number. Cards accept their
prefix-number identifier (such as `KAN-5`) or a bare card number, in
addition to the full UUID.
The TUI is unchanged in this release, but the same resolver functions are
now available to it for future text-input features (command palette,
jump-to-board, and so on).

### KAN-455 No Scrolling In Board Edit View Sprint And Column Lists (2026-05-15)

The sprint and column lists in the board edit view, and the sprint
section of the card list filter popup, now scroll to keep the selected
item visible. Previously these three lists tracked the j/k cursor but
never scrolled, so items past the visible area of the panel were
unreachable on small terminals or on boards with many sprints or
columns.
Scrolling matches the minimal-scroll behavior of the main card list:
the viewport only shifts when the cursor crosses an edge, so navigating
back and forth inside the visible area no longer reshuffles the list.


## [0.5.1] - 2026-05-14 ([#270](https://github.com/fulsomenko/kanban/pull/270))

### KAN-449 Make Apply Config Edit Test Sandbox Safe (2026-05-14)

Make settings_ui_tests `apply_config_edit` non-default-content test sandbox-safe (KAN-449)
- `test_apply_config_edit_with_non_default_content_writes_config` now pins `configuration_location` to a `tempfile::tempdir()` path before building the DTO. Without this, `AppConfigDto::from_config` resolves `configuration_location` via `effective_configuration_location` → `dirs::config_dir()` → `$HOME/.config/kanban/config.toml`, and `config::save`'s `create_dir_all` fails with `EACCES` in build sandboxes (nixpkgs, etc.) where `$HOME` is non-writable.
- No production code change. Same failure class as the 2026-05-07 nixpkgs-update log that KAN-396 closed for the other `apply_config_edit` tests; this is the one new instance that landed in #267 and slipped past that fix.


## [0.5.0] - 2026-05-14 ([#251](https://github.com/fulsomenko/kanban/pull/251))

### KAN-330 Startup Choose Storage File Dialog (2026-05-14)

Show a "choose storage file" dialog on TUI startup when no file is configured (KAN-330)
- Opening `kanban` with no file argument, no `KANBAN_FILE` env var, and no `storage_location` config now shows a startup dialog explaining both modes instead of silently opening an ephemeral in-memory board
- The dialog has a JSON/SQLite radio (default JSON); pressing `Tab` toggles the selection and swaps the filename's extension to match (`.json` ↔ `.sqlite`)
- The filename input is pre-filled with `boards.json` and shows a "Will be saved at: <abs path>" preview that updates as you type — pressing Enter creates the file at that path
- Pressing Escape dismisses the dialog and continues in memory, with the existing `x` export available to save work to a file at any time
- Choosing a file fully adopts that backend: the in-memory state is transferred to the new on-disk backend, the file is created, undo state is reinitialised, and subsequent changes (creating boards, etc.) are persisted normally
- If the chosen path cannot be opened (e.g. parent directory missing) the dialog stays open with the input preserved and an error banner explains what went wrong, so the user can correct the path and retry
- Layout reads top-to-bottom as description → filename input + path preview → format selector → action keys, with `x`, `Tab`, `Enter`, and `Esc` rendered in bold so the keyboard hints stand out from the surrounding prose

### KAN-332 Require Explicit File Path No Implicit Kanban Json Default (2026-05-14)

Require explicit file path — no implicit kanban.json default (KAN-332)
- Running `kanban <subcommand>` without a file argument, `KANBAN_FILE` env var, or `storage_location` config setting now fails with a clear error that lists all three ways to provide a file, instead of silently falling back to `kanban.json` in the current directory
- `kanban` with no args and no configured file now opens the TUI backed by an in-memory store instead of silently creating `kanban.json` — the TUI is fully usable without a file; data is not persisted until a storage location is configured from within the settings
- `kanban` with `KANBAN_FILE` or a `storage_location` config setting continues to open the TUI with that file as before
- `kanban completions` and `kanban migrate` are not affected — they do not operate on a data file
- README Quick Start updated to remove the implication that `kanban.json` is created automatically on first run

### KAN-394 Mcp Sync Card Status With Completion Column (2026-05-14)

Fix: sync card status ↔ completion column across CLI, MCP, and TUI (KAN-394)
Card status and column position now stay in lockstep with the board's completion column, regardless of which surface initiates the change:
- Marking a card as done via `kanban card update --status done` (CLI), the MCP `update_card` tool, or the TUI's `c` key automatically moves it to the board's resolved completion column and stamps `completed_at`.
- Moving a card *into* the completion column via CLI `kanban card move`, MCP `move_card`, the TUI's `h`/`l` keys, or any of the multi-select batch equivalents now sets `status=done` and stamps `completed_at`. Moving back out clears both.
- Multi-select batch operations (`c`, `h`/`l` on multiple cards, sprint-detail batch toggle) execute as a **single undo unit** — one `undo()` reverses every card and every chained command together — and produce **distinct positions** in the destination column instead of all colliding on the same one.
- Atomic updates that already specify both `column_id` and `status` explicitly are respected as-is; auto-sync only fires when the caller leaves one side unspecified.
Internally, the sync is orchestrated at the service layer (`KanbanContext::update_card`, `move_card`, `update_cards`, `move_cards`) by composing chained commands on top of the existing `execute(Vec<Command>)` atomic-batch infrastructure. Domain commands (`UpdateCard`, `MoveCard`, `MoveCards`) remain pure single-responsibility primitives. A new trait method `KanbanOperations::update_cards(Vec<(Uuid, CardUpdate)>)` provides the batched entry point used by the TUI multi-select handlers.

### KAN-397 Ci Wire Aur Publish Into Release Workflow (2026-05-14)

Wire AUR publish into release workflow
- Move AUR publish steps inline into release.yml so they run automatically on every release
- Remove the dead `release: [published]` trigger from aur-publish.yml (GitHub Actions does not fire it when GITHUB_TOKEN creates the release)
- Keep aur-publish.yml as a workflow_dispatch fallback for manual re-runs

### KAN-403 Regression Card Selector Newly Created Card (2026-05-14)

After creating a new card in the TUI, the selector now jumps to the new card immediately — so the very next action (edit, move, mark complete, open details) lands on the card you just made. Previously, when another card was already selected, the selector stayed on that prior card and the next keystroke acted on it instead.
This restores the demo-recording flow (Beat 2 creates a card, Beat 3 edits it) and matches the pre-regression behaviour.

### KAN-405 Json Backend Should Not Persist Command Log Between Sessions (2026-05-14)

Fix undo crash and strip command log from persistence (KAN-404, KAN-405)
- Undo is now in-session only for all backends — the command log is never written to `kanban.json` or SQLite
- Opening a file with a stale `commands` section no longer causes a crash or corrupts board state when pressing undo
- Existing data files with embedded command logs are cleaned up on the next open — JSON files are rewritten in place and SQLite files have the legacy `command_log`/`undo_state` tables dropped. Both backends announce the cleanup via the application log
- After upgrading, downgrading to a pre-405 build is not supported on SQLite databases — the legacy `command_log` and `undo_state` tables are dropped on first open
- Card sort is now deterministic when multiple cards tie on the primary sort key — tied cards order by ascending `card_number` regardless of how the backend yielded them, so cards no longer visibly jump on every render
- Archiving a card and triggering the resulting column compaction now form a single undo step instead of two, so one undo restores the previous state cleanly
- Archive selection now stays anchored to the focused card's column when archiving across multiple columns, instead of jumping to an unrelated card

### KAN-406 Sprint Assignment Dialog Group Sprints By Status With Completed Sprints In Separate Section (2026-05-14)

Group sprints by status in the sprint assignment dialog (KAN-406)
- Sprint assignment dialog (single-card and multi-card) now splits sprints into two headed sections: `Active / Planned` and `Completed / Ended`.
- Completed sprints render in green, Ended sprints (Active sprints whose `end_date` has passed) in red, so retrospective assignment targets are visually distinct.
- Cards can now be assigned to Completed and Ended sprints — useful for logging work against past sprints in retrospect.
- `j`/`k` navigation skips section headers; the dialog scrolls to keep the selected entry on-screen when the list overflows the viewport.
- When the list is scrolled past a section's header, the relevant header label stays pinned at the top row so the active section is always visible.
- `Cancelled` sprints remain hidden.

### KAN-418 Fix Kanban V Output (2026-05-14)

Fix `kanban -V` / `--version` / `--help` output (KAN-418)
- `kanban -V` and `kanban --version` now write to **stdout** with exit code **0** instead of stderr with exit 1, and no longer carry the spurious `Error:` prefix
- The trailing blank line after the version output is gone — output ends in a single newline
- `kanban --help` is fixed by the same path: stdout, exit 0, no `Error:` prefix
- Real argument errors (e.g. unknown flags) are unaffected — they continue to surface on stderr with a non-zero exit code
- The `commit:` line in `-V` output is still omitted when no commit hash is available at build time (e.g. `cargo install kanban` from crates.io)

### KAN-419 Show Sprint History On Cards With 1 Plus Sprint Assigned (2026-05-14)

Show sprint history on cards with 1+ sprint assigned (KAN-419)
- Sprint history box now appears in the card details view as soon as a card has 1 or more sprints assigned, instead of requiring 2+ sprints
- This makes it easier to see what sprints a card is on without needing to view multiple sprint transitions

### KAN-420 Bump Minimum Rust Version (2026-05-14)

Bump minimum Rust version to 1.74 (KAN-420)
- `CONTRIBUTING.md` prerequisites now correctly state Rust 1.74+ instead of 1.70+
- `rust-version = "1.74"` added to the workspace `Cargo.toml` so `cargo` enforces the minimum at build time
- The actual floor is set by `ratatui 0.29` and `clap 4.5`, both of which declare a 1.74 MSRV

### KAN-421 Add Build Time Debug Info For Target Os And Cfg Flags (2026-05-14)

Add raw key event trace logging to EventHandler (KAN-421)
- Setting `RUST_LOG=trace` logs every raw key event (code, kind, modifiers) before the Windows key filter runs — on Windows this captures both Press and Release events, which is the exact signal needed to diagnose key-doubling issues

### KAN-426 Replace Cfg Gated Keyeventkind Filter (2026-05-14)

Make KeyEventKind::Press filter unconditional across all platforms (KAN-426)
- The Windows-only `#[cfg(target_os = "windows")]` gate on the key event filter is removed — the filter now runs on all platforms
- On Linux/macOS crossterm only emits `Press` events in standard terminal mode, so the filter is a no-op there; behaviour is unchanged on all platforms
- Removes platform-specific code divergence and makes the filter testable on any OS

### KAN-427 Lift Delete Board To Service (2026-05-14)

Lift `DeleteBoard` cascade orchestration from the domain layer to the service layer (KAN-427)
- `BoardCommand::Delete(DeleteBoard)` is now atomic — it only deletes the board record.
- The cascade (dependency-graph edges → active cards → archived cards → columns → sprints → board) is composed in `KanbanContext::delete_board` and executed as a single `execute(...)` batch, which gives one undo unit and snapshot-based rollback on partial failure.
- New `Command::Cascade(CascadeCommand)` variant groups the validation-bypassing cascade primitives: `DeleteCardEdges`, `DeleteCardsByColumns`, `DeleteArchivedCardsByColumns`, `DeleteColumnsByBoard`, `DeleteSprintsByBoard`.
- New `commands::cascade::delete_board(store, id)` is the canonical batch builder.
- New `DataStore::list_cards_by_columns` (SQLite-optimised) eliminates a per-column N+1 read in the cascade.
- User-visible behaviour is unchanged.

### KAN-428 Lift Move Cards To Service (2026-05-14)

Lift MoveCards batch position calculation from domain to service (KAN-428)
- Add pure `kanban_domain::card_lifecycle::compute_move_positions` that returns target positions for a batch move given the current column contents and the moving card IDs.
- Add pure helper `kanban_domain::card_lifecycle::dedup_preserving_order<T: Hash + Eq + Copy>(items: &[T]) -> Vec<T>`, used internally by `compute_move_positions` and by the service-layer move orchestration.
- Remove the `MoveCards` (`CardCommand::MoveMultiple`) domain command — its position-computation orchestration is now performed in the service layer.
- `KanbanContext::move_cards_detailed` and `KanbanContext::move_cards` build a batch of atomic `MoveCard` commands plus the existing chained status updates, executed in a single `execute` call (one undo unit, snapshot rollback on failure).
- `build_move_cards_batch` performs a single batch-level WIP pre-check using the column listing it already fetches for position computation, so a `WipLimitExceeded` error from a batch move is reported once at the batch level instead of per-card. The pre-check compares against the deduplicated mover count, so callers passing duplicates that would fit under the limit are not falsely rejected.
- `InMemoryStore` is now indexed by column: `count_cards_in_column`, `count_cards_in_column_excluding`, and `list_cards_by_column` run in O(column_size) instead of O(total_cards). The index is maintained transactionally across `upsert_card`, `delete_card`, `delete_cards_by_columns`, and `apply_snapshot`. SQLite already does the equivalent indexed lookup via `WHERE column_id = ?`, so behaviour is consistent across backends.
- `KanbanContext::move_cards` and `move_cards_detailed` validate input ids via per-id `backend.get_card(id)` instead of an upfront `list_all_cards()` HashSet — strictly cheaper for typical small batches. Validation is consolidated inside `build_move_cards_batch` so an unknown id surfaces as `not_found` before the WIP pre-check can miscount it. `move_cards_detailed` also dedupes its input upfront so both `succeeded` and `failed` report each id at most once.
- **Behaviour change**: `KanbanContext::move_cards` (and the MCP `move_cards` tool) now error and roll back the entire batch when any input card ID is unknown, instead of silently dropping invalid IDs. Callers that need partial-success semantics should use `move_cards_detailed`, which continues to report per-ID failures without rolling back the rest of the batch.

### KAN-430 Lift Migrate Sprint Logs To Service (2026-05-14)

Lift MigrateSprintLogs from domain to service layer (KAN-430)
- The `CardCommand::MigrateSprintLogs` domain command and its associated struct are removed
- A new `KanbanContext::migrate_sprint_logs()` method takes its place — wraps the pure `card_lifecycle::migrate_sprint_logs()` function with the read → transform → persist-changed loop
- TUI invokes the service method directly via a new `TuiContext::migrate_sprint_logs()` delegation
- Behaviour change: this is now a pure data migration that does not record on the undo stack — sprint-log backfills should not be undoable

### KAN-431 Extract Update Sprint Validators (2026-05-14)

Refactor UpdateSprint to extract validators into pure functions (KAN-431)
- Extract `validate_card_prefix_not_locked`, `validate_card_prefix_unique`, and `allocate_sprint_name` from the inline body of `UpdateSprint::execute`
- Slim `execute` into a thin coordinator that calls the extracted helpers in sequence
- Behavior is unchanged — existing integration tests pass without modification; new focused unit tests added for each extracted function

### KAN-434 Collapse Singular Service Methods (2026-05-14)

Service layer cleanup: singular card mutations now share orchestration with their batch counterparts (KAN-434)
- `update_card` is a one-line shorthand over `update_cards(vec![(id, updates)])`. The status ↔ completion-column auto-sync now fires symmetrically on `update_card` as well — a column-only update into the completion column auto-sets `status=Done`, and a column-only update out of it clears `Done`. Previously only status-driven updates triggered the column move, so column-only callers silently missed the sync. No production caller exercised that path before this release, so existing behaviour is preserved and the gap is closed.
- `assign_card_to_sprint` is a one-line shorthand over `assign_cards_to_sprint(vec![card_id], sprint_id)`. Behaviour is unchanged — both implementations dispatched the same underlying domain command.
- Both singulars retain their original public signature (`KanbanResult<Card>`) and trailing `get_card` for the return value. CLI, MCP, and TUI delegations are untouched.
The design principle: **singular builds on plural, not the other way around.** The atomic-transaction infrastructure (`KanbanContext::execute(Vec<Command>)`) is the fundamental unit at the service layer; the per-card singular is a convenience wrapper for the batch-of-one case. This keeps orchestration in one place — when future tweaks land (e.g. the per-board auto-sync opt-out tracked as KAN-432), they only need to touch the plural.

### KAN-435 Sprint Detail Card Lists (2026-05-14)

The sprint-detail card lists now behave more like the main-board lists:
- **Scrolling works.** Pressing `j` / `k` past the visible viewport in either the Uncompleted or Completed panel scrolls the list to keep the selected card on-screen. Previously the selection moved off-screen and got truncated. Both panels scroll independently of each other.
- **Multi-select works on the Completed panel.** `v` and `V` now toggle multi-selection on completed cards in addition to uncompleted ones. Batch actions you initiate from sprint detail can target either panel.
- **Movement actions are enabled on both panels.** Action configs are aligned — every card action available on the main-board list is also available here.
- **Sort order applies on populate.** Opening a sprint detail with a non-default board sort (e.g. priority, due date) now shows both panels already ordered the way the main board orders. Previously the lists used raw iteration order until you opened the sort dialog manually.
Known gaps remaining (tracked as follow-up cards):
- Search filter (`/` query) does not yet propagate to sprint-detail panels on every frame — only on initial populate.
- Toggling a card from Completed back to Uncompleted in sprint detail still routes to the second-to-last board column (KAN-394 default) rather than the card's pre-completion column. The original-column tracking the user proposed is a separate change.

### KAN-445 Fix Validate Path Windows Unc (2026-05-14)

Fix Windows path handling across `validate_path`, TUI startup, and storage migrations (KAN-445)
- On Windows, launching `kanban` with an existing data file (e.g. `kanban
  kanban.json`) no longer fails with the misleading error `Path traversal
  not allowed: 'kanban.json' resolves outside current directory`
- The path validator now uses `dunce::canonicalize`, which returns the
  ordinary `C:\…` form on Windows instead of the verbatim `\?\C:\…` UNC
  form that `std::fs::canonicalize` emits. The traversal guard's prefix
  comparison against the current working directory now succeeds for paths
  inside the cwd, as intended
- The current working directory is canonicalized through the same path,
  so the comparison is robust even when the cwd itself is in non-canonical
  form (e.g. a UNC-shaped Windows cwd, or a `/var` → `/private/var`
  symlink on macOS)
- The TUI's startup `--file` resolution follows the same canonical form,
  so `app_config.storage_location` no longer leaks `\?\C:\…` paths into
  settings rendering, migration source paths, and other downstream
  consumers
- Absolute paths that point at existing files are likewise returned in
  their plain form, so downstream consumers no longer see surprise UNC
  prefixes leaking out of the service layer
- On Windows, a failed storage migration (`kanban migrate`) now actually
  removes the partially-written destination file instead of leaving an
  orphan that blocks retries. The SQLite store now exposes an async
  `close()` that drains its connection pool before the cleanup `remove_file`
  runs — Windows refuses to delete files with live handles, and the
  previous `drop(store)` was synchronous-only and didn't wait for in-flight
  connections. POSIX behaviour is unchanged
- No change to the path traversal protection — escapes via `..` are
  still rejected

### Other Changes (2026-05-14)

Fix duplicated key presses on Windows
- Filter out non-Press KeyEventKind variants on Windows so each keystroke registers once instead of twice (Press + Release)
- Resolves text input duplicating, backspace deleting two characters at a time, and the help menu not staying open
- Linux behavior unchanged (compile-time cfg gate)


## [0.4.1] - 2026-05-07 ([#242](https://github.com/fulsomenko/kanban/pull/242))

### KAN-396 Fix Tui Make Settings Config Edit Tests Sandbox Safe For Nixpkgs (2026-05-07)

Fix settings_config_edit_tests failing in Nix build sandbox
- 5 tests in kanban-tui called apply_config_edit without a configuration_location
  in the JSON, causing save() to fall back to $HOME/.config/kanban/config.toml
- The Nix sandbox sets $HOME to a non-writable stub, so create_dir_all failed
  with Permission denied
- Fix: each test now creates a TempDir and passes its path as configuration_location
  so save() writes to $TMPDIR (writable in sandbox) instead of $HOME/.config


## [0.4.0] - 2026-05-04 ([#208](https://github.com/fulsomenko/kanban/pull/208))

### CAT-245 Surface Command Errors To User Via Banner In Tui Handlers (2026-05-04)

Adds an in-app error log panel that captures WARN and ERROR tracing events
without corrupting the TUI display.
Previously, `tracing::warn!` and `tracing::error!` calls would write directly
to stderr during raw mode, bleeding into the terminal buffer and garbling the
UI. Log output was also lost once the session ended with no way to review it.
The fix is two-pronged. A custom `InMemoryLogLayer` replaces the stderr
subscriber in TUI mode, intercepting all WARN/ERROR events into a shared
in-memory buffer. The buffer is then surfaced through a dedicated `ErrorLog`
panel that auto-opens whenever a new ERROR is captured, and can be toggled
on demand with F12 and dismissed with Escape. The footer shows a `[!] N errors`
badge while there are unread errors.

### CAT-260 Invert Storage Backend Plugin Architecture (2026-05-04)

- refactor(cli): extract run_with_args from run to enable injection of CLI args in tests
- fix(service): include root cause in export_to_sqlite error when sqlite backend is absent
- feat(cli,mcp): add json/sqlite forwarding features; gate with_defaults on cfg
- fix(cli): early-return Completions before no-backends guard
- refactor(tui): drop direct backend deps, use kanban_service::default_registry()
- refactor(mcp): drop direct backend deps, use kanban_service::default_registry()
- refactor(cli): drop direct backend deps, use kanban_service::default_registry()
- feat(service): add json/sqlite optional features and default_registry()
- fix(mcp): add empty-registry guard, use shared validate_path, remove local fn
- fix(cli): add empty-registry guard, validate file path, align tracing with env-filter
- refactor(service): extract shared validate_path from kanban-mcp
- refactor(service): add StoreManager::has_backends
- refactor(persistence): add StoreRegistry::is_empty
- refactor(cli): restrict internal module visibility to pub(crate)
- fix(service): improve export_to_sqlite error for unregistered sqlite backend
- fix(cli,mcp): use try_init to prevent double-init panic
- docs(service): document export_to_sqlite registry requirement
- fix(mcp): warn on backend auto-correction in McpContext
- refactor(cli,mcp): invert storage backend ownership via builders
- feat(service): introduce StoreManager with injectable StoreRegistry

### CAT-264 Lift Undo Redo Historymanager Into Kanbancontext (2026-05-04)

History-aware execute, StateManager slimming, and TuiContext encapsulation
- Unify `execute()` and `execute_batch()` into a single `execute(Vec<Box<dyn Command>>)` — fixes spurious undo-on-failure bug and provides one uniform API with atomic rollback semantics
- Make `execute()` capture undo history by default — all `KanbanOperations` consumers get undo/redo for free
- Add native batch commands (`ArchiveCards`, `MoveCards`, `AssignCardsToSprint`) with single undo entry
- Extract `clear_history()` from `reload()` — callers decide whether to clear
- Move conflict detection (`has_conflict`/`set_conflict`/`clear_conflict`) from StateManager to KanbanContext
- Slim StateManager to purely a save coordinator (channels + file watcher)
- Add MCP `undo` and `redo` tools
- Encapsulate `TuiContext` by removing `Deref` and making `inner` private
- Remove all `_mut()` accessors from `TuiContext`, routing every mutation through domain commands
- Add `ImportEntities`, `ApplyBoardSettings`, `ApplyCardMetadata`, `CompactColumnPositions`, `MigrateSprintLogs` commands
- Lift sprint counter/name logic into `CreateSprint` command, eliminating caller-side board mutations

### CAT-302 Starting The Tui Doesnt Select A Board (2026-05-04)

- feat(tui): preselect first board and refresh card view on startup
- test(tui): preselect first board and refresh card view on startup

### CAT-312 Adjust Landing Roadmap (2026-05-04)

- fix(web): correct completed elements output

### CAT-341 Redesign Card Identifier Model Drop Stored Prefixes Lock Board Sprint Prefix Simplify Counters (2026-05-04)

- fix(persistence-json): renumber colliding cards instead of aborting V2→V3 migration
- refactor(tui): remove assigned_prefix management from sprint assignment handlers
- test(service,persistence): update contract tests for card_counter
- fix(mcp,cli): remove dead card_prefix/assigned_prefix fields from CardUpdate
- feat(persistence-sqlite): schema v1→v2 migration; card_counter; drop prefix columns
- feat(persistence-json): add V2→V3 migration; strip prefix fields, set card_counter
- refactor(domain): update all Card::new call sites to drop prefix argument
- feat(domain): lock sprint card_prefix after card assigned; enforce prefix uniqueness
- feat(domain): lock board card_prefix after first card is created
- feat(domain): two-level identifier resolution (sprint.card_prefix → board.card_prefix)
- feat(domain): drop assigned_prefix and card_prefix from Card; simplify Card::new
- feat(domain): replace prefix_counters with card_counter on Board
- feat(persistence): add FormatVersion::V3

### KAN-159 Implement Sqlite Storage Backend (2026-05-04)

- docs: add .db extension to SQLite backend documentation
- test(service): add make_store test for .db extension
- fix(service): use store instance_id instead of throwaway UUID in save
- docs(service): document registry registration order rationale
- fix(persistence-json): exclude SQLite extensions from catch-all match
- feat(persistence-sqlite): add .db extension to SqliteStoreFactory
- docs(web): update landing page for SQLite backend support
- docs(service,cli,mcp): update READMEs for pluggable storage
- docs(persistence): add READMEs for JSON and SQLite backends
- docs(persistence): rewrite README as trait abstraction layer
- docs: update workspace structure and diagrams for pluggable persistence
- fix(persistence-sqlite): replace len comparison with is_empty in concurrent test
- test(persistence-json): align roundtrip test with shared snapshot
- refactor(persistence): replace hardcoded make_store with StoreFactory registry
- fix(persistence-sqlite): wrap load in read transaction and batch sync deletes
- docs(persistence-sqlite): document delete-and-reinsert pattern in upserts
- fix(persistence-sqlite): add points range validation
- refactor(persistence-sqlite): replace fragile enum parsing with parse_enum helper
- test(persistence-sqlite): add concurrent access test
- fix(persistence-sqlite): narrow mutex scope in save and add schema migration skeleton
- fix(service): remove dead let _ = ext binding
- fix(persistence-sqlite): require NOT NULL fields in upserts instead of silent defaults
- fix(persistence-sqlite): propagate parse errors for optional UUID and DateTime fields
- fix(persistence-sqlite): narrow sqlx features with default-features = false
- test(service): strengthen make_store no-extension test with save/load roundtrip
- refactor(persistence-sqlite): deduplicate fully_populated_snapshot in roundtrip test
- fix(service): make_store returns Result instead of panicking
- fix(persistence-sqlite): deterministic card order, async mutex, builder API, and pool size
- fix(persistence-sqlite): validate NOT NULL fields and sync edges incrementally
- fix(persistence-sqlite): return errors for unknown enum variants instead of silent fallback
- docs(persistence-sqlite): document missing FK rationale on card_edges table
- test(cli): add sqlite-to-sqlite migration roundtrip test
- test(persistence): add conflict detection contract test for stale metadata
- fix(service): guard .db extension when sqlite-storage feature is disabled
- refactor(persistence-sqlite): move WAL pragma to connection options and document sprint_logs FK
- refactor(persistence-sqlite): split sqlite_store.rs into helpers, builders, and upserts modules
- style: apply cargo fmt across workspace
- refactor(persistence): replace contract test glue with macro
- test(cli): add bidirectional migration tests
- refactor(cli): remove direct persistence crate dependencies
- feat(cli): make migrate command backend-agnostic
- refactor(persistence): remove tests superseded by contract suite
- test(persistence): wire contract tests to JSON and SQLite backends
- feat(service): add test-helpers feature with contract test suite
- test(service): add make_store factory dispatch tests
- refactor(service): extract shared make_store factory from CLI, MCP, and TUI
- feat(persistence): add instance_id() to PersistenceStore trait
- fix(persistence-sqlite): prevent silent data degradation on load
- fix(persistence-sqlite): normalize schema types and add missing constraints
- refactor(persistence): delete dead store/ and migration/ modules
- test(persistence-sqlite): add KanbanContext integration tests for SQLite backend
- test(persistence-sqlite): add dependency graph edges to roundtrip test
- feat(persistence-sqlite): normalize schema — replace JSON columns with relational tables
- fix: update error types and imports after rebase onto develop
- docs(contributing): add 'adding a domain field' checklist for schema enforcement
- test(persistence): add fully-populated roundtrip tests for JSON and SQLite plugins
- feat(domain): add PartialEq to domain types and graph primitives for roundtrip test assertions
- feat(cli/mcp/tui): wire up persistence plugin architecture
- feat(service): decouple KanbanContext from concrete persistence implementations
- feat(persistence-sqlite): add SQLite storage backend with typed struct enforcement in row_to_* functions
- feat(persistence-json): add JSON file store plugin crate
- feat(persistence): refactor trait crate to remove embedded store implementations
- feat(ci): add persistance-* crates to publish script
- fix(tui): remove unused PersistenceStore import missed during rebase
- fix(tui): restore TuiSnapshot export and fix SaveChannel type after rebase conflict
- fix(sqlite): address code review feedback
- refactor(sqlite): use Table enum instead of string literals
- fix(sqlite): use transaction for all save operations
- fix(tui): resolve clippy warnings for type complexity and unused import
- feat(tui,cli): add pluggable storage backend support
- feat(persistence): add JSON to SQLite migration utilities
- feat(persistence): implement SqliteStore with PersistenceStore trait
- feat(persistence): add SQLite schema for kanban data

### KAN-171 Replace Silent Failures With Proper Errors (2026-05-04)

- fix(domain): replace silent failure with NotFound error in CreateSubcardCommand (ef6aa323bdeeca39a07eb6afa273901719158b5f)
- fix(domain): replace silent failures with NotFound errors in sprint commands (6828c75d20bff02169e4f04e41d0b5fdc54de0e4)
- fix(domain): replace silent failure with NotFound error in UpdateColumn (a70a5c449f8236d8cb19e2ee59bec6fd3d9779fd)
- fix(domain): replace silent failures with NotFound errors in card commands (00d17452e29ed77e4de1edcdeaa69b7aec5cd878)
- fix(domain): replace silent failures with NotFound errors in board commands (d3810135a1131be6b0e9ce5acc4643248443433b)
- feat(domain): add CommandContext lookup helpers that return NotFound errors (0e2817bb9573a2e60bb0734f5b5b691ac04b610b)

### KAN-210 Carry Over Cards From Ended Sprint (2026-05-04)

- feat: rebind carry-over from R to M, always moves all uncompleted cards
- test: verify carry_over_sprint_cards skips Done cards
- feat: add R carry-over keybinding to sprint detail help
- feat: add R carry-over and bulk c/d actions in sprint detail view
- feat: add carry-over sprint popup navigation and confirm handler
- feat: trigger carry-over dialog on sprint completion with uncompleted cards
- feat: add CarryOverSprintDialog render component
- feat: add CarryOverSprint dialog mode and state
- feat: add carry_over_sprint_cards MCP tool
- feat: add sprint carry-over CLI subcommand
- feat: implement carry_over_sprint_cards on KanbanContext
- feat: add carry_over_sprint_cards to KanbanOperations trait

### KAN-211 Disambiguate Card Identifier Lookup Across Boards (2026-05-04)

- test: add CLI integration tests for ambiguous identifier resolution
- feat: return all matches from card get for ambiguous identifier
- test: add find_cards_by_identifier integration tests for MCP context
- feat: return ambiguity error when multiple cards match identifier
- refactor: rename find_card_by_identifier to find_cards_by_identifier returning Vec

### KAN-230 Submit Kanban To Aur (2026-05-04)

- ci: add AUR auto-publish workflow on release
- docs: add AUR installation instructions

### KAN-232 Fix C Key Can No Longer Complete Sprint From Sprint Detail View (2026-05-04)

- fix: suppress no planning sprint toast on c key sprint completion
- fix: restore c key sprint completion from sprint detail view

### KAN-233 Sync Web Index Html Roadmap With Readme Md (2026-05-04)

- feat: add roadmap item

### KAN-235 Per Layer Error Types Domainerror Persistenceerror Kanbanerror (2026-05-04)

- refactor(mcp,cli): use kanban_domain error types
- refactor(tui): use kanban_domain error types
- refactor(service): use kanban_domain::KanbanError with typed constructors
- feat(persistence): add PersistenceError/PersistenceResult
- feat(domain): add DomainError, DependencyError, and KanbanError wrapper
- refactor(core): slim to CoreError/CoreResult, remove KanbanError

### KAN-256 Fix Sqlite Db File Loading Read To String On Binary File (2026-05-04)

- fix: load SQLite files via persistence store instead of read_to_string

### KAN-258 Unify Initial File Loading Path Follow Up To Kan 256 (2026-05-04)

- test(tui): update tests to use async load_initial_state()
- fix(tui): unify initial file loading into async load_initial_state()
- feat(persistence-json): implement JSON content detection with BOM support
- feat(persistence-sqlite): implement SQLite content detection via magic bytes
- feat(persistence): add content-based detection to StoreFactory trait

### KAN-259 Make Sqlite A Default Feature In Kanban Cli (2026-05-04)

- feat: make sqlite-storage a default feature and remove redundant sqlite feature flags

### KAN-263 Rework Migrate Cli Backend As Positional Arg Filename As Option (2026-05-04)

- feat(cli): rework migrate command to use positional backend arg
- feat(service): add make_store_for_backend for explicit backend selection
- feat(persistence): add create_by_name and available_backend_names to StoreRegistry

### KAN-274 Settings Page Ui (2026-05-04)

## Settings page UI (`S`)
Press `S` from the boards view to open a two-column settings screen:
- **Configuration** panel — editing format, card/sprint prefixes, storage backend and location, config format and path. Navigate with `j`/`k` across rows, `h`/`l` or `1`/`2`/`3` to jump between panels.
- **Config File** panel — shows the resolved config path, whether it is loaded, and the serialization format.
- **Storage** panel — shows backend and data-file path; bottom row triggers the export dialog.
Press `e` or `Enter` (on Configuration panel) to open the config in an external editor. The file format respects `editing_format` (json or toml). Changes are validated and applied live; invalid values are rejected with an error banner.
## Storage backend switching
Changing `storage_location` in the editor triggers an async migration: data is copied to the new file, the store swaps in-place, and the UI reloads. If the destination already exists, data is loaded from it instead of migrated. The source backend is auto-detected from the file extension; mismatches between the configured backend and the actual file are corrected automatically with a warning.
## Export boards dialog (`x` in Settings)
Opens a board-selection checklist, then an options step to choose JSON or SQLite output and set a filename. JSON export is synchronous; SQLite export is async and reports success or failure via a banner when complete.
## `kanban migrate` CLI
```
kanban migrate <source> <backend> [--output <path>] [--source-backend <override>]
```
Source backend is auto-detected from the file extension. The output path defaults to the source stem with the target backend's extension.
## Config persistence (`~/.config/kanban/config.toml`)
Config is written only when at least one value differs from the compiled-in defaults. Default values are stripped before saving so the file stays minimal. Both TOML and JSON serialization formats are supported (`configuration_format`). The `editing_format` field now accepts `"toml"` in addition to `"json"`.
## Service layer additions
- `kanban_service::config::resolve_storage_location` — resolves relative storage paths to absolute (cwd join extracted from `kanban-core`, which is now a pure data crate).
- `kanban_service::migrate_store` — copies a snapshot between any two stores.
- `kanban_service::validate_and_load_store` — opens an existing store and verifies it is readable.
- `kanban_service::detect_backend` — infers the backend from a locator string.
- `KanbanContext::load_with_defaults` — convenience constructor used throughout tests.

### KAN-278 Hero Demo (2026-05-04)

Create polished hero demo for kanban TUI application
- Add pre-crafted JSON fixture with realistic development board
- Implement single VHS recording script showcasing core workflow
- Add Nix shell environment with vhs and neovim integration
- Create reproducible nvim wrapper for demo editing
- Build record script with automatic fixture reset
- Add comprehensive README documentation
- Replace fragile multi-tape setup with self-contained demo

### KAN-299 Extract Ui Rs Into Reusable Components And View Submodules (2026-05-04)

Splits the monolithic `ui.rs` (2,100 lines) in `kanban-tui` into focused, testable modules.
**New reusable components** (each with integration tests):
- `components/footer.rs` — keybinding footer bar
- `components/help_popup.rs` — help overlay and viewport height calculator
- `components/conflict_popup.rs` — file conflict and external-change dialogs
- `components/relationship_popup.rs` — parent/child card relationship picker
- `components/filter_popup.rs` — sprint/date/tag filter dialog
**View submodules** under `ui/`:
- `ui/mod.rs` — render entry point and dispatcher (~130 lines)
- `ui/main_view.rs` — main kanban board view
- `ui/settings_view.rs` — settings view
- `ui/card_detail.rs` — card detail view
- `ui/board_detail.rs` — board detail view
- `ui/sprint_detail.rs` — sprint detail view
- `ui/dialogs.rs` — thin dialog wrapper functions
No behaviour changes. All existing tests pass.

### KAN-300 Make Version Readout Of Web Landing Dynamic (2026-05-04)

- feat(web): inject version from workspace Cargo.toml at build time
- feat(web): replace hardcoded version with @VERSION@ placeholder

### KAN-305 Fix Config File Corruption And Unnecessary Writes (2026-05-04)

- fix(tui): clear cli_file_provided after migration so storage shows under Storage fields
- fix(tui): use correct selection indices for Active Storage rows in cli-only mode
- feat(tui): use Active Storage labels when storage source is CLI arg, Storage labels when config
- test(tui): add red-green tests for absolute path in Storage Location settings UI
- fix(tui): show resolved absolute path for Storage Location in settings UI
- fix(tui): unload cli_file_override when user explicitly provides storage in editor
- test(tui): add test for cli override unload when storage fields uncommented
- refactor(tui): extract is_storage_line helper; revert annotate editor change
- fix(tui): use annotate_storage_fields in editor when CLI file override is active
- fix(tui): add annotate_storage_fields to show storage as active lines with comment
- fix(tui): don't inject absolute storage path when CLI arg matches config default
- fix(tui): reset config storage to original values when DTO storage is unchanged
- test(tui): add tests for startup-injected absolute storage path not written to config
- fix(tui): strip unchanged storage from DTO to prevent spurious config writes
- test(tui): add test for unchanged storage not written to config
- fix(tui): CLI-supplied storage path is always session-only
- test(service): fix vacuous temp-file leak assertion in config write test
- fix(tui): skip config save when editor exits without changes
- fix(service): atomic write for config file to prevent corruption
- fix(service): promote tempfile to regular dependency

### KAN-326 Hide Grayed Config Storage Rows When Storage Is Not Set In Config (2026-05-04)

Hide grayed config storage rows when storage not set in config
- Only show grayed 'Storage Backend' / 'Storage Location' rows in the Configuration panel when storage is explicitly configured (original_storage_backend or original_storage_location is Some)
- When config defines no storage and a CLI file overrides the default, only Active Storage rows are shown, avoiding the misleading implication that CWD-resolved defaults are configured values

### KAN-339 Address Pr 208 0 4 0 Code Review Feedback (2026-05-04)

- fix(ci): validate release tag and fix sed delimiter in aur-publish
- fix(domain): cap redo stack at MAX_HISTORY_DEPTH
- test(domain): add redo stack bounded test
- fix(domain): validate column exists before restoring card
- test(domain): add restore card column validation test
- feat(domain): enforce WIP limits in CreateCard, MoveCard, MoveCards
- fix(domain): enforce WIP limits in RestoreCard
- test(domain): add failing WIP limit enforcement tests
- test(domain): add WIP limit enforcement test for RestoreCard
- feat(domain): add WipLimitExceeded error variant and predicate
- test(domain): add error.rs predicate and From conversion tests

### KAN-348 Refactor Storage To On Demand Querying Instead Of Full Snapshot In Memory (2026-05-04)

### Added
- **SQLite storage backend** — use `.sqlite`, `.sqlite3`, or `.db` file extensions to store kanban data in a relational database instead of JSON
- **Command-replay undo/redo** — all mutations are recorded as replayable commands with full history persistence across sessions
- **Indexed snapshots** — undo/redo on SQLite is O(1) via compressed snapshots stored alongside each command, eliminating full replay from baseline
- **Board ordering** — boards now have an explicit `position` field for deterministic sort order
- **Magic bytes detection** — CLI and MCP automatically detect whether a file is SQLite or JSON by reading file headers, with extension-based fallback for new files
### Changed
- `undo()` and `redo()` now return `KanbanResult<bool>` instead of `bool`, propagating storage errors to callers
- Board import clears command history after completion — imported data is baked into the baseline snapshot and cannot be individually undone
- `MigrateSprintLogs` selectively persists only cards whose sprint logs actually changed, reducing unnecessary writes
### Fixed
- SQLite databases created before the `card_counter` feature now auto-migrate on open instead of crashing with "no such column: card_counter"
- Input lag when holding navigation keys — buffered key events are now drained before each redraw
- TUI no longer renders at 60fps when idle — redraws are event-driven, reducing CPU usage to near zero when not interacting
- Eliminated O(n²) card cloning in the render loop (was cloning all cards per visible card per frame)
- Eliminated N+1 SQL query pattern when loading sprint logs and board auxiliary data on the SQLite backend
### Removed
- `SqliteBlobStore` and `SqliteStoreFactory` — replaced by `SqliteStore` (formerly `SqliteDataStore`), wired directly through `StoreManager`
- `InMemoryDataStore` type alias — use `InMemoryStore` directly
- `UndoPointId` and snapshot-based undo-point methods from `DataStore` trait — superseded by command-replay undo
- Command log methods from `PersistenceStore` trait — moved to the dedicated `CommandStore` trait
### Internal
- `DataStore` trait provides on-demand entity queries (get/list/upsert/delete) replacing full in-memory snapshot
- `CommandStore` trait handles command persistence and indexed snapshot storage
- `KanbanBackend` supertrait combines `DataStore + CommandStore` with manual impls per backend
- Create commands embed deterministic UUIDs for reproducible replay
- TUI render path reads from `ViewState` cache populated by `refresh_view()` — no storage queries during frame rendering

### KAN-364 Fix Tui Card Selection Opens Wrong Details (2026-05-04)

Fixed a bug where opening the card detail view would display the wrong card. The
detail view was resolving the selected card by indexing into
`cards_by_id.values()`, but `HashMap` iteration order is non-deterministic and
does not match the ordered position stored in `active_card_index`. This caused
the wrong card to be shown whenever the HashMap's internal order diverged from
the selection order.
The fix stores the selected card's UUID in `SelectionHub.active_card_id` when
entering the detail view and looks the card up directly by ID via the new
`App::get_card_for_detail_view` method, eliminating the ordering dependency.

### KAN-365 Block Quit During Migration With Double Q Ui (2026-05-04)

Pressing `q` while a storage migration is in progress no longer silently
abandons the migration. The app now shows a warning banner and requires a
second `q` to confirm the abort. If the migration completes before the
second `q` is pressed, the confirmation clears automatically and the next
`q` exits cleanly with no data loss.
This fixes a data loss scenario where triggering a JSON→SQLite migration
via the config editor and immediately pressing `q` would leave the
destination file unwritten.
Also fixes a startup regression where supplying an explicit file argument
(e.g. `kanban myboard.json`) was incorrectly treated as a SQLite file when
the config had `storage_backend = "sqlite"` set, causing a load error.

### KAN-366 Description Doesnt Load In Card Details (2026-05-04)

## Fixes
- Card descriptions now display correctly when opening card details — previously the description field appeared empty even when content existed
- Editing a card or board field in the detail view now immediately reflects changes without requiring a manual refresh
- Empty card descriptions now show a placeholder prompt instead of a blank field
- Snapshot load errors during rendering are now logged as warnings instead of being silently swallowed
- Stale model reads after `execute_command` eliminated by capturing card/column UUIDs upfront before state mutation
- Archived cards are now indexed in `Model` for O(1) lookup and cached as a flat list to avoid per-frame clones
- Scroll offset is now preserved in `ColumnListsLayout.refresh_lists` after mutations
- Archived cards panel title is now dynamic (shows live card count) instead of hardcoded
- `ArchivedCardsView` is excluded from the global `q` quit intercept — `q` now closes the view instead of quitting the app
## Refactors
- Replaced the manual `refresh_view()` call pattern with an automatic per-frame render loop (`prepare_frame`), eliminating a class of stale-data bugs where UI state could fall out of sync after mutations
- Introduced a `Model` struct as the single source of truth for all board, column, card, sprint, and dependency graph data rendered each frame
- Removed the intermediate `RenderData`/`ViewState` layer in favour of direct `Model` reads
- Removed granular cache-invalidation methods (`invalidate_boards`, `invalidate_cards`, etc.) — the per-frame full reload makes them unnecessary
- Removed cloning accessors (`boards()`, `sprints()`) from `TuiContext`; callers now read from `Model` or the domain snapshot directly
## Features
- `SqliteStore` now implements `PersistenceStore` — `path` and `instance_id` fields added; `instance_id` is persisted in the `metadata` table and survives reopens
- `SqliteStoreFactory` added to `kanban-persistence-sqlite`, implementing `StoreFactory` with magic-byte content sniffing (`SQLite format 3 `)
- `SqliteStoreFactory` registered first in `default_registry()` so SQLite files are detected by content before JSON extension matching
- `is_sqlite` / `open_sqlite` bypass removed from `McpContext` and `CliContext` — all storage backends now routed uniformly through the registry
- `VERSION` constant extracted to a shared module; MCP and CLI now share a single version string source
- MCP server handles `-V` / `--version` flag cleanly — responds with version string and exits without error output

### KAN-371 Kanban Sqlite Add Explicit Wal Checkpoint On App Exit (2026-05-04)

SQLite storage now flushes pending writes to the main database file after
every save. Previously, SQLite's WAL mode accumulated changes in a
`.wal` sidecar file that could grow to several MB between checkpoints,
meaning a backup of just the `.db` file could be missing recent data.
Every write — whether from the TUI, CLI, or MCP server — now triggers a
`PRAGMA wal_checkpoint(TRUNCATE)`, keeping the WAL file at near-zero size
after each operation. Backups of the `.sqlite` file are now always
complete and self-contained.

### KAN-383 Bug X In Archived Cards View Restores Card Instead Of Hard Deleting It Sqlite (2026-05-04)

### Bug fix: permanently deleting an archived card no longer restores it as active (SQLite)
When using a SQLite-backed board, pressing `x` on a card in the Archived Cards view is supposed to
permanently remove it. Instead, the card reappeared in the normal kanban view as if it had been
restored — as though the action had triggered a restore rather than a deletion.
**The card is now fully removed in both tables** when hard-deleted. It will no longer ghost back
into the active board after pressing `x`.
This fix also closes a broader durability gap: every mutation on the SQLite backend (create, update,
move, archive, undo, redo) now immediately checkpoints the write-ahead log, so the database file on
disk always reflects the latest state. Previously the WAL was only flushed when the app exited
cleanly — meaning a crash or force-quit could silently discard recent changes. That risk is now
eliminated regardless of which interface (TUI, CLI, or MCP) is used.

### KAN-384 Architecture Unified Backends Via True Deferred Reads (2026-05-04)

## Description
Unified the storage backend architecture so that both JSON and SQLite
backends are opened with zero I/O at construction time. Data is loaded
lazily on the first read, keeping startup fast and making the two
backends interchangeable through a single `open_context()` entry point.
## New Features
- **`open_context(locator, config)`** — single async function that
  opens any supported backend (JSON or SQLite) by detecting the file
  type automatically from magic bytes or extension, then returns a
  ready-to-use `KanbanContext`. No per-backend wiring required in
  callers.
- **Lazy JSON backend (`JsonDataStore`)** — wraps a JSON persistence
  store with an in-memory cache that is populated only on the first
  read. Subsequent reads are served from the cache; writes set a dirty
  flag and are flushed to disk explicitly via `save()` or by the
  background save worker.
- **`KanbanBackend` lifecycle methods** — `flush()`, `reload()`,
  `needs_flush()`, `needs_save_worker()`, and `on_undo_state_changed()`
  give callers a uniform interface for durability and conflict detection
  across all backend types.
## Improvements
- `KanbanContext::open` is now the single zero-I/O constructor for all
  backends. The legacy `open_sqlite` / `open_json` constructors are
  retained for backward compatibility but delegate to the new path.
- The TUI flush signal replaces the old snapshot-save channel, removing
  a layer of indirection and aligning JSON saves with the SQLite
  checkpoint model.
- Backend type is auto-detected from file content (magic bytes for
  SQLite, leading `{` / `[` for JSON), so files without a recognised
  extension are handled correctly.
## Fixes
- `StoreManager::make_backend` now correctly detects SQLite databases
  that have no file extension by reading the SQLite magic-byte header,
  preventing them from being opened as (invalid) JSON stores.
## Deprecations
None.
## Testing
Full contract coverage added for the new architecture:
- `KanbanBackend` lifecycle tests for `SqliteStore` (needs_flush, WAL
  checkpoint, reload no-op).
- `JsonDataStore` command-log round-trip (flush → reopen → command
  count matches).
- `StoreManager::make_backend` — JSON path, SQLite path, magic-byte
  detection, and content-sniffing for extension-less files.
- `KanbanContext::open` integration suite — zero-I/O construction,
  lazy load on first read, undo/redo with lazy baseline, save/reload
  delegation, and external-change pickup after `reload()`.
- `open_context()` end-to-end suite — JSON round-trip, SQLite
  round-trip, magic-byte auto-detection, new-file-starts-empty.

### KAN-391 Fix Validate Release Staleness (2026-05-04)

- fix(ci): derive release-script crate list dynamically via cargo metadata
- fix(ci): propagate list-crates failures cleanly to release-script consumers
- fix(ci): broaden crate-list-sync drift regex to catch inline arrays
- test(ci): add crate list sync invariant guard


## [0.3.5] - 2026-03-22 ([#193](https://github.com/fulsomenko/kanban/pull/193))

### KAN-229 Fix Publish Crates Order Add Kanban Service Before Kanban Mcp (2026-03-22)

- fix(ci): add kanban-service to publish script and order mcp as last


## [0.3.4] - 2026-03-22 ([#191](https://github.com/fulsomenko/kanban/pull/191))

### KAN-123 Escape Bind To Clear Search Enter To Apply (2026-03-22)

- fix: remove trailing spaces from active search footer text
- KAN-123: update search mode keybinding descriptions
- KAN-123: show active search filter indicator in footer
- KAN-123: highlight search matches in card titles
- KAN-123: split Enter/Esc in search mode and add n/N navigation

### KAN-221 Help Menu List Doesnt Scroll (2026-03-22)

- fix(help): fixed header/footer layout with ListComponent scroll in render_help_popup
- refactor(help): replace help_selection+help_page with help_list ListComponent
- refactor(generic_list): delegate get_adjusted_viewport_height to Page
- refactor(pagination): add get_adjusted_viewport_height to Page
- refactor: use render_scroll_indicators helper at all scroll indicator sites
- feat: add scroll support to help menu popup (KAN-221)
- refactor: generalize render_scroll_indicators to accept plain args and label

### KAN-222 Fix Post Search Ux Issues Gg Scroll Unicode Panic Footer Hint N N Nav (2026-03-22)

- fix: remove n/N search-navigation shortcuts — n is for new card only
- fix: drop n/N from active-search footer hint — redundant with j/k when results are filtered
- fix: active-search footer shows navigation hint alongside ESC
- refactor: add SearchState::active_query() and collapse repeated search_query expressions
- fix: Unicode panic in build_title_spans — map lowercase byte offsets back to original
- fix: gg jumps to top but doesn't scroll view — call ensure_selected_visible

### KAN-224 Decompose App Rs Into Focused Modules (2026-03-22)

- refactor: decompose app.rs into focused sub-modules
- Split 2060-line `app.rs` into 12 focused sub-modules under `app/`
- Each concern now lives in its own file: `mode`, `focus`, `selection`, `filter`, `multi_select`, `dialog_input`, `sprint_view`, `relationship`, `view`, `animation`, `persistence`, `ui_state`
- Zero behavioral change — all types re-exported from `app/mod.rs`

### KAN-226 Extract Kanban Service Crate From Kanban Cli Handlers (2026-03-22)

- docs: use graph LR for dependency diagram in README to match other docs
- docs: replace repeated workspace graphs in crate READMEs with CONTRIBUTING.md links
- docs: add kanban-service to architecture section with Mermaid diagram
- docs: update CONTRIBUTING.md workspace structure to 7 crates with Mermaid diagram
- docs: update CLAUDE.md workspace structure to 7 crates with Mermaid diagram
- docs: replace StateManager references with KanbanContext in persistence README
- docs: rewrite kanban-mcp README for in-process KanbanContext architecture
- docs: add kanban-service README
- test: verify reload() picks up external changes to the kanban file
- feat: reload from disk before every mutating_op in kanban-mcp
- feat: add reload() to KanbanContext to re-read state from disk
- test: restore McpContext in kanban-mcp integration tests, add persistence coverage
- test: add KanbanContext persistence round-trip tests
- refactor: replace parking_lot::Mutex with tokio::sync::Mutex in kanban-mcp
- refactor: remove instance_id field and save_sync from KanbanContext
- chore: remove kanban binary dep from kanban-mcp Nix build
- test: rewrite kanban-mcp integration tests to use KanbanContext directly
- refactor: migrate McpContext to KanbanContext, delete subprocess executor
- refactor: delegate CliContext to KanbanContext from kanban-service
- feat: add kanban-service crate with KanbanContext over PersistenceStore


## [0.3.3] - 2026-03-18 ([#184](https://github.com/fulsomenko/kanban/pull/184))

### KAN-220 Fix Kanban Binary Discovery In Mcp Integration Tests For Nix Builds (2026-03-18)

- fix: check direct target profiles before triple subdirs in kanban_bin()
- fix: discover kanban binary across target triples and profiles in integration tests


## [0.3.2] - 2026-03-18 ([#182](https://github.com/fulsomenko/kanban/pull/182))

### KAN-217 Mcp List Cards Pagination Returns Max 50 Cards Instead Of All Cards (2026-03-18)

- fix: pass page/page_size through to CLI subprocess in MCP list_cards
- refactor: change list_cards to return Vec<CardSummary> instead of Vec<Card>

### KAN-218 Gate Kanban Tui Behind Default Feature Flag In Kanban Cli (2026-03-18)

- ci: add no-tui build check
- fix: improve no-tui error message to point to --help
- feat: build kanban-mcp with no-tui kanban binary to skip wayland/xcb
- feat: gate kanban-tui behind optional 'tui' default feature


## [0.3.1] - 2026-03-17 ([#179](https://github.com/fulsomenko/kanban/pull/179))

### KAN-216 Changelog Md Grouping By Card (2026-03-17)

- docs: retroactively group CHANGELOG entries by changeset for 0.1.11–0.3.0
- fix: group changelog entries by changeset in aggregate-changelog.sh


## [0.3.0] - 2026-03-17 ([#175](https://github.com/fulsomenko/kanban/pull/175))

### KAN-193 Bring Mcp To Full Feature Parity With Cli Tui Via Kanbanoperations Trait (2026-03-17)

- test: add integration tests for MCP round-trips
- test: add unit tests for MCP helpers and ArgsBuilder
- feat: update MCP server tools for full CLI parity
- feat: bring MCP context to full parity with CLI
- fix: error handling in MCP executor
- feat: add sprint update fields to CLI (name, dates, clear flags)
- feat: add --clear-wip-limit flag to CLI column update
- feat: rewrite MCP server with 37 tools via KanbanOperations trait
- feat: remove McpTools trait, replaced by KanbanOperations from kanban-domain
- feat: add McpContext implementing KanbanOperations trait
- feat: replace async CliExecutor with sync SyncExecutor
- feat: add kanban-domain, kanban-core, uuid, chrono, tempfile deps to kanban-mcp
- fix: remove create_card_full bypass, use trait two-step create+update pattern
- fix: remove update_sprint_full bypass, route through trait's update_sprint
- feat: add name field to SprintUpdate for MCP name passthrough
- fix: remove broken clear_description and clear_points MCP flags
- refactor: remove 4 dead pre-animation functions from TUI card_handlers

### KAN-196 Redesign Release Workflow Defer Version Bump To Master Merge (2026-03-17)

- fix: address PR review findings for release workflow
- fix: quote variable in parameter expansion to satisfy shellcheck SC2295
- chore: wire all scripts into nix dev shell
- fix: use robust frontmatter parsing in changeset-check
- fix: reorder release workflow to validate before push
- refactor: extract changelog aggregation into standalone script
- fix: exclude README.md from changeset detection in bump-version.sh
- fix: defer version bump to master merge

### KAN-197 Add Card Identifier Search Prefix Number (2026-03-17)

- feat: add card identifier search (KAN-197)

### KAN-208 Fix Shift Y Branch Copy Crash On Linux Nixos Wayland (2026-03-17)

- docs: document Linux clipboard manager requirement
- refactor(tui): replace last_error with unified Banner system
- feat(tui): add reusable Banner component
- feat(tui): enable Wayland support with clipboard manager handoff
- chore: add Wayland/X11 clipboard dependencies

### KAN-209 Multi Select Cards (2026-03-17)

- feat(tui): add bulk priority popup rendering
- feat(tui): add selection mode indicator to footer
- feat(tui): handle SetMultipleCardsPriority dialog in event loop
- feat(tui): add keyboard shortcuts for multi-select
- feat(tui): wire keybinding actions in execute_action
- feat(tui): add bulk priority popup handler
- feat(tui): update escape handler for selection mode
- feat(tui): add auto-select on navigation in selection mode
- feat(tui): implement vim-style selection mode toggle
- feat(tui): add bulk move for selected cards
- feat(tui): add card selection handler functions
- feat(tui): add card list keybindings for bulk operations
- feat(tui): register bulk priority dialog provider
- feat(tui): add BulkPriorityDialog component
- feat(tui): add keybinding actions for multi-select operations
- feat(tui): add SetMultipleCardsPriority dialog mode
- feat(tui): add selection_mode_active field to App

### KAN-210 Find Cards By Prefix Increment Identifier E G Kan 5 (2026-03-17)

- feat(mcp): resolve card identifier (e.g. KAN-5) in all card tools
- feat(cli): accept card identifier (e.g. KAN-5) in all card commands
- feat(cli,tui,mcp): implement find_card_by_identifier in all contexts
- feat(domain): add find_card_by_identifier to KanbanOperations trait
- fix(domain): use sprint card_prefix in identifier resolution
- fix(domain): PrefixAndNumber with no resolved prefix returns no match instead of falling back to "task"
- fix(cli): remove redundant find-by-identifier subcommand (card get KAN-5 already works)

### KAN-212 Add Compact Names Only Flag To Card Listing For Token Efficient Search (2026-03-17)

- feat(core): add PaginatedList<T> with paginate() helper and resolve_page_params() utility
- feat(domain): add ArchivedCardSummary with From<&ArchivedCard> impl
- feat(cli): card list defaults to CardSummary (no description); use card get for full details
- feat(cli): add --page, --page-size flags to card, board, column, sprint list
- feat(cli): archived card list returns PaginatedList<ArchivedCardSummary>
- feat(mcp): tool_list_cards and tool_list_archived_cards return PaginatedList<CardSummary>
- test(cli): card list pagination, summary shape, out-of-bounds page

### KAN-215 Version Flag (2026-03-17)

- nix: inject self.rev as GIT_COMMIT_HASH in Nix builds
- fix: suppress commit: line in -V when git hash is unknown
- fmt: wrap long lines

## [0.2.0] - 2026-02-01

### KAN-134 Undo Action (2026-02-01)

- feat(tui): register undo/redo keybindings in CardList provider
- feat(tui): register undo/redo keybindings in BoardDetail provider
- feat(tui): register undo/redo keybindings in CardDetail provider
- feat(tui): register undo/redo keybindings in NormalMode providers
- feat(tui): add Undo and Redo KeybindingAction variants
- feat(tui): add undo() and redo() methods to App
- feat(tui): capture snapshots before command execution for undo history
- feat(tui): integrate HistoryManager into StateManager
- feat(tui): create HistoryManager module for undo/redo support

### KAN-170 Cascade Cleanup Delete Operations (2026-02-01)

- test: add cycle detection tests for dependency graph
- test: add integration tests for cascade cleanup operations
- feat(domain): unassign cards when deleting sprints
- feat(domain): add validation to DeleteColumn command
- feat(domain): implement cascade cleanup in card deletion and archival
- feat(domain): add cascade cleanup methods to DependencyGraph trait

### KAN-177 Parent And Child Relationship Boxes Layout (2026-02-01)

- feat(tui): implement backward wrap-around navigation from title to children
- feat(tui): add scrolling support to parent/child relationship boxes
- feat(tui): Implement interactive navigation for parent/child relationship boxes
- feat(tui): add infrastructure for parent/child relationship navigation
- feat(tui): Display parent/child relationship boxes side-by-side with increased height

### KAN-178 Tui To Domain Refactoring Migration (2026-02-01)

Extract business logic from kanban-tui into kanban-domain and kanban-core, establishing a clean layered architecture.

### kanban-core
- Add `InputState`, `SelectionState`, and `PageInfo` modules for reusable UI-agnostic state primitives

### kanban-domain
- Add `sort`, `filter`, `search`, and `query` modules for card filtering/sorting pipeline
- Add `CardQueryBuilder` with fluent API for composing card queries
- Add `card_lifecycle` module for card movement, completion toggling, and archival logic
- Add `HistoryManager` for bounded undo/redo (capped at 100 entries)
- Add `export`/`import` modules with `BoardExporter` and `BoardImporter`
- Add `Snapshot` serialization (`to_json_bytes`/`from_json_bytes`) directly on the domain type
- Add sprint query functions and `CardFilters` struct
- Replace dyn dispatch with enum dispatch in search and sort

### kanban-tui
- Remove re-export wrappers and thin delegation layers that proxied domain logic
- Replace inline business logic in handlers with `card_lifecycle` calls
- Replace duplicated filter/sort service with `CardQueryBuilder`
- Fix multi-byte UTF-8 cursor handling via core `InputState`

### KAN-6 Card Dependencies (2026-02-01)

- feat(tui): Add TUI for managing parent-child card relationships
- feat(domain): Add commands for parent-child card relationships
- feat(domain): Add ParentOf edge type for hierarchical card grouping
- feat(tui,cli): integrate dependency graph into persistence
- feat(domain): add dependency management commands
- feat(domain): add card dependency graph types
- feat(core): add graph-related error variants
- feat(core): add graph cycle detection algorithms
- feat(core): add generic Graph<E> data structure
- feat(core): add graph module with edge types and GraphNode trait


## [0.1.16] - 2025-12-21

### Other Changes (2025-12-21)

- chore: bump version to 16

### KAN-154 P Dialog Does Not Correctly Set Points (2025-12-21)

- fix: points dialog now correctly updates card from detail view

## [0.1.15] - 2025-12-21

### KAN-129 Include Commit Hash In V (2025-12-21)

- feat(cli): include git commit hash in version output

### KAN-139 If No Sprints Cant Scroll To Column Settings (2025-12-21)

- fix(tui): skip empty sprints section when navigating board details

### KAN-140 Filter Out Completed Sprints From Assign List (2025-12-21)

- fix: filter out completed and cancelled sprints from assign list

### KAN-141 Scrolling Up From Column Options Lands The Cursor On The First Sprint In The List (2025-12-21)

- fix: navigate to last sprint when scrolling up from columns in board settings

### KAN-142 Updating Fields Jumps The User Back To Board 2 (2025-12-21)

- fix: preserve navigation mode during auto-reload from external changes

### KAN-143 Gg G Works Poorly (2025-12-21)

- chore: cargo fmt
- fix(tui): fix gg/G vim navigation in grouped-by-column view
- chore: remove wip file

### KAN-144 Kanban View Switches Column On The Second To Last Item (2025-12-21)

- fix: prevent premature column switching in handle_navigation_down

### KAN-145 We Broke The File Watcher Having A Conflict With One Instance (2025-12-21)

- fix: Centralize file watcher pause/resume in StateManager

### KAN-146 Kanban Mcp (2025-12-21)

- feat: add kanban-mcp server
- feat(mcp): add McpTools trait for compile-time parity with KanbanOperations
- docs(mcp): add subprocess architecture documentation and Nix wrapper
- feat(mcp): add CLI executor for subprocess-based operations
- feat(mcp): enhance card operations and add delete/archive functionality
- feat: add kanban-mcp: Model Context Protocol server implementation

### KAN-147 Multiselecting And Assigning Cards Causes Write Race Condition (2025-12-21)

- fix: batch card creation with optional status update
- fix: batch card movements with conditional status updates
- fix: batch sprint activation and completion with board updates
- fix: batch column position swaps
- fix: batch card unassignment from sprint
- fix: batch card completion toggles
- fix: batch card moves when deleting column
- fix: batch default column creation to prevent conflict dialog on new board
- refactor: use batch command execution in sprint assignment handlers
- feat: add execute_commands_batch for race-free command execution
- fix: enhance AssignCardToSprint to handle sprint log transitions

### KAN-148 Archiving Deleting Cards Is Broken (2025-12-21)

- fix: batch card archive and delete operations in animation completion

### KAN-15 Progressive Saving Detect Changes To Current Json (2025-12-21)

- feat(persistence): create kanban-persistence crate structure
- feat(state): create Command trait and StateManager
- feat(domain): add CreateBoard command
- feat(domain): add active_sprint_id field to BoardUpdate
- feat(state): add debouncing to StateManager::save_if_needed()
- feat(persistence): add automatic V1→V2 migration on load
- feat(core,persistence): add conflict detection for multi-instance saves
- feat(persistence): detect file conflicts before save
- feat(state): propagate conflict errors in StateManager
- feat(tui): Implement conflict resolution dialog and event loop integration
- feat(tui): Integrate FileWatcher with App event loop
- feat(state): Add view refresh tracking to StateManager
- feat(tui): Add ExternalChangeDetected dialog
- feat(tui): add user-visible error display banner
- feat(app): prevent quit with pending saves
- feat(app): add save completion receiver to App struct
- feat(state): add bidirectional save completion channel

### KAN-150 File Path To A Non Existant File Crashes The App (2025-12-21)

- feat: Add migration verification and automatic backup cleanup
- fix: Add instance ID check to file watcher to prevent false positives
- fix: Remove redundant version fields from PersistenceMetadata

### KAN-151 Kanban Cli (2025-12-21)

- fix(tui): restoring restoring cards
- fix(cli): restoring to a non existing column
- docs: add CLI quick start section to root README
- docs: update CLI README with command documentation
- fix: use get_selected_card_in_context for points dialog
- feat: add TuiContext struct with KanbanOperations implementation
- feat: implement KanbanOperations trait for TUI App
- test: update CLI tests for positional ID arguments
- feat: make ID positional argument for single-resource commands
- fix: return descriptive errors for invalid priority and status values
- feat: add API version to CLI output and document never type
- feat: simplify CLI file argument and add shell completions
- fix: CLI context bugs and improve error messages
- fix: Support positional file argument for TUI mode
- test: Add comprehensive integration tests for CLI
- feat: Implement full CLI with subcommand interface
- feat: Add KanbanOperations trait for TUI/CLI feature parity

### KAN-152 Dont Include Description Of Card For Get Cards (2025-12-21)

- feat(mcp): omit description and sprint_logs from card list responses
- feat(cli): include git commit hash in version output (#132)

### KAN-155 Publish New Version (2025-12-21)

- fix: stabilize release pipeline for v0.1.15

### KAN-30 Vim Motions (2025-12-21)

Jumping cards

- fix: jump by actual visible cards count from render_info, not cards_to_show
- feat: add vim jump motions to normal mode keybinding display
- feat: add vim jump motions to card list keybinding display
- feat: wire up vim jump motions to keybinding handlers
- feat: add jump motion handlers
- feat: add jump methods to CardList
- feat: add jump_to_first and jump_to_last methods to SelectionState
- feat: add jump action variants to KeybindingAction enum
- feat: add pending_key field to App struct for multi-key sequences

### KAN-93 Dialogs Always Return To Main When Opened (2025-12-21)

Refactored dialog mode handling to use nested AppMode::Dialog(DialogMode) enum
for type-safe dialog management. Dialogs now correctly display their parent
view in the background instead of hardcoded destinations.

- Added DialogMode enum with all 23 dialog variants
- Simplified is_dialog_mode() to matches!(self.mode, AppMode::Dialog(_))
- Added get_base_mode() to determine parent view from mode_stack
- Two-phase rendering: base view first, then dialog overlay
- Converted all push_mode(AppMode::X) calls to open_dialog(DialogMode::X)

## [0.1.14] - 2025-11-17 ([#patch](https://github.com/fulsomenko/kanban/pull/patch))

### KAN-111 Sprint Binding Help Is Wrong (2025-11-17)

- refactor: integrate card list into keybinding registry
- refactor: unify keybinding management for footer and help popup

### KAN-118 Unfilter Tasks List On Completed Sprint (2025-11-17)

Remove filtering of cards from completed sprints

- fix: remove auto-hiding of completed sprint cards from app methods
- fix: remove auto-hiding of completed sprint cards from view strategies

### KAN-130 Three Card List Components To Become One (2025-11-17)

A refactoring.

- refactor: simplify navigation handlers to work with unified view strategy
- refactor: simplify card handlers to work with unified view strategy
- refactor: update app initialization to use UnifiedViewStrategy
- refactor: simplify render_tasks to use unified view strategy
- refactor: introduce UnifiedViewStrategy to compose layout and render strategies
- refactor: create render strategy abstraction for card list rendering
- refactor: create layout strategy abstraction for card list management
- refactor: extract card filtering and sorting logic into card_filter_service
- KAN-118/unfilter-tasks-list-on-completed-sprint (#93)
- KAN-111/sprint-binding-help-is-wrong (#92)
- KAN-33: Add Help mode with context-aware keybindings (#91)
- ci: automatically sync develop with master after release (#90)

### KAN-132 Urgent Migrations (2025-11-17)

- migration: add reconciliation of branch_prefix and sprint_prefix to migrate old boards
- migration: add serde default to support migration to archived cards board

### KAN-133 Scrolling Doesnt Work In Grouped By Columns List (2025-11-17)

- feat: Synchronize navigation viewport with grouped view column headers
- feat: Implement unified scrolling rendering for grouped view
- feat: Wire up VirtualUnifiedLayout for grouped view mode
- feat: Add VirtualUnifiedLayout for unified card scrolling in grouped view

### KAN-196 Make Help Menu Items Selectable And Activateable (2025-11-17)

- fix: help menu keybinding matching for special keys and /
- fix: implement missing action handlers for help menu
- refactor: couple keybindings with actions
- feat: add visual selection to help popup
- feat: add generic list component

### KAN-20 Remove A Card (2025-11-17)

- chore: simplify archived cards view keybindings
- refactor: rename delete to archive, permanent delete to delete
- refactor: consolidate keybinding providers into CardListProvider
- feat: add animation state infrastructure and types
- feat: add yellow border for deleted cards view visual distinction
- feat: add card deletion from detail view
- fix: card lookup in DeletedCardsView mode
- feat: add deleted cards UI rendering
- feat: add keybindings for card deletion
- feat: implement card deletion with position compacting
- feat: add DeletedCardsView mode to App
- feat: add deleted_cards persistence
- feat: add DeletedCard domain model

### KAN-33 Add Binding (2025-11-17)

Add help dialogue for keybindings.

- feat: implement Help popup rendering with context-aware keybindings
- feat: add global ? key handler for help across all modes
- refactor: make CardFocus and BoardFocus Copy
- feat: add Help app mode with context preservation
- feat: create keybinding registry to route contexts
- feat: implement keybinding providers for all contexts
- feat: create keybindings module with traits and data structures
- refactor: add keybindings module to lib
- ci: automatically sync develop with master after release (#90)

### KAN-55 Scroll In Cards List (2025-11-17)

- fix: ensure forward progress when viewport shrinks during down navigation
- fix: correct viewport height calculation across all renderers
- feat: add viewport calculation infrastructure to CardList
- fix: allow scrolling down to show the final card
- feat: update navigation to account for scroll indicator space
- feat: add scroll indicators showing tasks above and below viewport
- feat: use actual viewport_height instead of hardcoded value
- feat: calculate and update viewport_height during rendering
- feat: add viewport_height tracking to App
- fix: eliminate selector jitter by moving selection with scroll
- refactor: remove preemptive ensure_selected_visible calls
- refactor: update CardListComponent navigate methods for viewport awareness
- refactor: implement scroll-on-boundary logic in navigate methods
- feat: wire up automatic scroll adjustment on navigation
- feat: implement scroll-aware rendering for sprint detail panels
- feat: implement scroll-aware rendering in all card list views
- feat: expose scroll management in CardListComponent
- feat: add scroll offset tracking to CardList


## [0.1.12] - 2025-11-02 ([#patch](https://github.com/fulsomenko/kanban/pull/patch))

### KAN-117 Workflows And Releases (2025-11-02)

Update release flow

- chore: remove unnecessary backup logic from update-changelog script
- chore: update bump-version script to output new version
- ci: enhance release workflow with version bump and changelog
- ci: simplify aggregate-changesets workflow
- fix: prevent stdout pollution of GITHUB_OUTPUT in release workflow


## [0.1.11] - 2025-11-02 ([#patch](https://github.com/fulsomenko/kanban/pull/patch))

### KAN-105 We Probably Should Move Sprint Prefix Into Sprint Level Settings (2025-11-02)

- refactor: fix clippy enum variant naming warnings
- chore: cargo fmt
- refactor: consolidate copy methods with generic implementation
- refactor: create generic prefix dialog handler abstraction
- refactor: remove dead code render_sprint_task_panel
- fix: remove used for filtering output
- fix: scope sprint counter initialization to board context
- feat: show active sprint card prefix override in board details
- feat: add board_context module for board-related queries
- feat: initialize sprint counter when prefix is assigned
- feat: initialize sprint counter when creating new sprints
- feat: add Board::ensure_sprint_counter_initialized method
- fix: separate default prefixes for sprints and cards
- test: fix import test to include new card_prefix field
- test: add integration tests for export/import with prefixes (Phase 4D)
- test: add backward compatibility tests (Phase 4C)
- test: add card prefix hierarchy tests (Phase 4B)
- feat: display separate sprint and card prefix fields in UI
- feat: add UI rendering for SetSprintCardPrefix dialog
- fix: rename branch_prefix to sprint_prefix throughout codebase
- feat: update sprint creation to use per-prefix sprint counters
- feat: add separate sprint_prefix and card_prefix to BoardSettingsDto
- feat: add card_prefix field to Card domain model
- feat: add card_prefix field to Sprint domain model
- feat: add card_prefix field to Board domain model
- chore: cargo fmt
- chore: add changeset
- feat: add help text for sprint prefix collision confirmation
- feat: set assigned_prefix when assigning cards to sprints
- feat: add sprint prefix collision confirmation mode
- test: update Card::new() call sites to use prefix parameter
- fix: resolve borrow checker constraint in create_card handler
- feat: update Card::new() signature to accept and use prefix parameter
- feat: add assigned_prefix field to Card domain model
- feat: add prefix registry system to Board domain model
- feat: Implement sprint prefix editing UI and handlers
- feat: Add sprint prefix settings support to domain and app modes
- refactor: simplify effective_prefix() using or() instead of or_else()
- refactor: remove board.sprint_prefix from TUI layer
- refactor: add Sprint.effective_prefix() and update branch name logic
- refactor: remove sprint_prefix from Board and BoardSettingsDto
- refactor: rename Sprint.prefix_override to Sprint.prefix

### KAN-109 Choose Which Sprint To Filter By (2025-11-02)

Adding a dialogue to chose card filters

- feat: support filtering by multiple sprints simultaneously
- feat: display all active filters in card list header
- chore: cargo fmt
- fix: simplify Space key handler to remove clippy single-match warning
- refactor: merge unassigned sprints into sprints section with graphical separation
- feat: apply filters immediately when toggled in dialog
- feat: implement filter dialog item selection and cursor feedback
- feat: add filter dialog UI rendering
- feat: implement filter dialog handlers
- feat: add filters module with FilterOptions AppMode

### KAN-113 Dont Add To Sprint Log For A Card If The Same Sprint Is Added (2025-11-02)

- fix: prevent duplicate sprint log entries when reassigning to same sprint

### KAN-95 Marketing (2025-11-02)

- add demo

### MVP-108 Keep A Log Of Sprints For A Card (2025-11-02)

Implement log for sprints that a card has seen

- chore: cargo fmt
- feat: integrate sprint logging into card-to-sprint assignment
- feat: add sprint logging to Card domain model
- feat: add SprintLog struct for tracking sprint history
- feat: add logging abstraction to kanban-core

### MVP-110 In Card Metadata Show The Sprint Log For A Card (2025-11-02)

Adding a sprint history view to Card Details

- feat: increase sprint history display to 4 elements
- feat: show sprint history tail with correct absolute indexing
- feat: migrate sprint logs for existing assigned cards
- feat: display sprint history in card detail view

### MVP-40 Make Card Meta Data Editing Like Board Settings Edit (2025-11-02)

Introduce JSON editing for card meta

- refactor: swap keybindings - 'p' for points, 'P' for priority in card detail
- chore: cargo fmt
- refactor: remove unused BoardSettingsDto import from app.rs
- chore: update Cargo.lock
- feat: use generic editor for card metadata and board settings
- feat: add generic edit_entity_json_impl method for JSON-based entity editing
- feat: add BoardSettingsDto and CardMetadataDto with Editable implementations
- feat: add Editable<T> trait for entity subset editing


## [0.1.10] - 2025-11-02

### KAN-105 We Probably Should Move Sprint Prefix Into Sprint Level Settings (2025-11-02 15:57)

- refactor: fix clippy enum variant naming warnings
- chore: cargo fmt
- refactor: consolidate copy methods with generic implementation
- refactor: create generic prefix dialog handler abstraction
- refactor: remove dead code render_sprint_task_panel
- fix: remove used for filtering output
- fix: scope sprint counter initialization to board context
- feat: show active sprint card prefix override in board details
- feat: add board_context module for board-related queries
- feat: initialize sprint counter when prefix is assigned
- feat: initialize sprint counter when creating new sprints
- feat: add Board::ensure_sprint_counter_initialized method
- fix: separate default prefixes for sprints and cards
- test: fix import test to include new card_prefix field
- test: add integration tests for export/import with prefixes (Phase 4D)
- test: add backward compatibility tests (Phase 4C)
- test: add card prefix hierarchy tests (Phase 4B)
- feat: display separate sprint and card prefix fields in UI
- feat: add UI rendering for SetSprintCardPrefix dialog
- fix: rename branch_prefix to sprint_prefix throughout codebase
- feat: update sprint creation to use per-prefix sprint counters
- feat: add separate sprint_prefix and card_prefix to BoardSettingsDto
- feat: add card_prefix field to Card domain model
- feat: add card_prefix field to Sprint domain model
- feat: add card_prefix field to Board domain model
- chore: cargo fmt
- chore: add changeset
- feat: add help text for sprint prefix collision confirmation
- feat: set assigned_prefix when assigning cards to sprints
- feat: add sprint prefix collision confirmation mode
- test: update Card::new() call sites to use prefix parameter
- fix: resolve borrow checker constraint in create_card handler
- feat: update Card::new() signature to accept and use prefix parameter
- feat: add assigned_prefix field to Card domain model
- feat: add prefix registry system to Board domain model
- feat: Implement sprint prefix editing UI and handlers
- feat: Add sprint prefix settings support to domain and app modes
- refactor: simplify effective_prefix() using or() instead of or_else()
- refactor: remove board.sprint_prefix from TUI layer
- refactor: add Sprint.effective_prefix() and update branch name logic
- refactor: remove sprint_prefix from Board and BoardSettingsDto
- refactor: rename Sprint.prefix_override to Sprint.prefix

### KAN-109 Choose Which Sprint To Filter By (2025-11-02 15:57)

Adding a dialogue to chose card filters
- feat: support filtering by multiple sprints simultaneously
- feat: display all active filters in card list header
- chore: cargo fmt
- fix: simplify Space key handler to remove clippy single-match warning
- refactor: merge unassigned sprints into sprints section with graphical separation
- feat: apply filters immediately when toggled in dialog
- feat: implement filter dialog item selection and cursor feedback
- feat: add filter dialog UI rendering
- feat: implement filter dialog handlers
- feat: add filters module with FilterOptions AppMode

### KAN-113 Dont Add To Sprint Log For A Card If The Same Sprint Is Added (2025-11-02 15:57)

- fix: prevent duplicate sprint log entries when reassigning to same sprint

### KAN-95 Marketing (2025-11-02 15:57)

- add demo

### MVP-108 Keep A Log Of Sprints For A Card (2025-11-02 15:57)

Implement log for sprints that a card has seen
- chore: cargo fmt
- feat: integrate sprint logging into card-to-sprint assignment
- feat: add sprint logging to Card domain model
- feat: add SprintLog struct for tracking sprint history
- feat: add logging abstraction to kanban-core

### MVP-110 In Card Metadata Show The Sprint Log For A Card (2025-11-02 15:57)

Adding a sprint history view to Card Details
- feat: increase sprint history display to 4 elements
- feat: show sprint history tail with correct absolute indexing
- feat: migrate sprint logs for existing assigned cards
- feat: display sprint history in card detail view

### MVP-40 Make Card Meta Data Editing Like Board Settings Edit (2025-11-02 15:57)

Introduce JSON editing for card meta
- refactor: swap keybindings - 'p' for points, 'P' for priority in card detail
- chore: cargo fmt
- refactor: remove unused BoardSettingsDto import from app.rs
- chore: update Cargo.lock
- feat: use generic editor for card metadata and board settings
- feat: add generic edit_entity_json_impl method for JSON-based entity editing
- feat: add BoardSettingsDto and CardMetadataDto with Editable implementations
- feat: add Editable<T> trait for entity subset editing


## [0.1.10] - 2025-10-26 ([#75](https://github.com/fulsomenko/kanban/pull/75))

### MVP-77 Changeset Script To Add Timestamp Of Changeset Creation And Card Name (2025-10-26)
- feat: group changelog entries by card with timestamps and branch names

### MVP-101 Add Column Header For Non-Assigned Filter (2025-10-26)
- refactor: extract tasks panel title builder
- refactor: extract filter title suffix helper
- feat: add unassigned cards header to filter view

### MVP-29 Search In Cards List (2025-10-26)
- style: make search query text white for better visibility
- feat: add vim-style search query display in footer
- refactor: consolidate refresh_view and refresh_preview functions
- Add Search mode help text to footer
- Integrate search functionality into App
- Add search query parameter to view strategies
- Add search module to crate exports
- Add search module with trait-based architecture

### MVP-49 Hitting 'Q' In Dialogue Quits The Program (2025-10-26)
- fix: exclude AppModes with text input form the `q` to quit binding

### MVP-86 Missing Sprint Header For Sprint Filter In Kanban View (2025-10-26)
- feat: add sprint filter indicator to kanban view

### MVP-90 Moving Cards From Last Column Doesn't Uncomplete (2025-10-26)
- Update CardListAction::MoveColumn handler to reflect card status changes
- Fix handle_move_card_right to complete cards moved to last column
- Fix handle_move_card_left to uncomplete cards moved from last column

### KAN-81 J K Doesn't Work On Empty Columns (2025-10-26)
- Fix j/k navigation on empty card lists. Pressing j/k on an empty column now correctly navigates to adjacent columns instead of doing nothing.

### MVP-60 Move Card Out Of Completed Column Doesn't Unmark As Complete (2025-10-26)
- fix: moving cards from the last column should uncomplete said card

### KAN-94 When Opening A Dialogue Put Selector On The Currently Selected Item (2025-10-26)
- refactor: delegate dialog rendering to SelectionDialog components
- refactor: use dialog selection state helpers in event handlers
- refactor: export SelectionDialog component from components module
- refactor: create SelectionDialog trait and implementations
- refactor: add dialog selection state helpers to app
- Implement CardListComponent for reusable card list interactions (#65)

### MVP-35 Make J K Work For Changing Panels (2025-10-26)
- Add vim-style j/k navigation for panel changes in detail views
- Enable j/k keys to navigate between panels in CardDetail view (Title, Metadata, Description)
- Enable j/k keys to navigate between panels in BoardDetail view (Name, Description, Settings, Sprints, Columns)
- Wrap navigation at list boundaries: reaching the end of Sprints/Columns lists transitions to next panel
- Both arrow keys and vim-style j/k keys work consistently across all views

### MVP-68 Treesitter For Syntax Highlighting (2025-10-26)
- Add markdown rendering support for task and board descriptions
- Integrate pulldown-cmark for markdown parsing
- Support bold, italic, inline code, and code blocks with proper spacing
- Code blocks render as plain text with top/bottom margins and left indent for readability
- Enhance card detail view with formatted markdown descriptions
- Enhance board detail view with formatted markdown descriptions
- Add comprehensive integration tests for markdown renderer (9 tests)
- Note: Chose markdown-only approach over syntax highlighting to maintain simplicity and performance

### MVP-64 Create Task In The Focused Column (2025-10-26)
- feat: auto-complete cards created in last column when >2 columns exist
- feat: create cards in focused column of grouped and kanban views
- feat: add helper method to get focused column ID from view strategy
- KAN-59/fix card movement and completion display (#61)

### KAN-59 Fix Card Movement And Completion Display (2025-10-26)
- Add view refresh for card movement (H/L keys) in all view modes
- Add view refresh for card completion (c key) in all view modes
- Add smart column navigation: cards move to last column when marked Done, and to second-to-last column when unmarked from Done

## [0.1.8] - 2025-10-21 && [0.1.9] - 2025-10-21 ([#patch](https://github.com/fulsomenko/kanban/pull/patch))

- Fix critical release workflow issues that prevented successful publishing to crates.io:
- Fix Nix script path resolution in publish-crates (validate-release now called directly)
- Use portable sed syntax compatible with both Linux and macOS
- Preserve .changeset/README.md when cleaning up changesets
- Correct changeset description parsing in update-changelog script
- Add runtime dependencies (cargo, git, grep, sed, find) to Nix shell applications
- Add concurrency control to aggregate workflow to prevent race conditions
- Remove error suppression that was hiding failures
- Extract repository URL from git remote instead of hardcoding

## [0.2.1] - 2025-10-20 ([#patch](https://github.com/fulsomenko/kanban/pull/patch))

- Testing the release flow
- Created aggregate-changesets.sh: collects all changesets and determines highest priority bump type
- Created update-changelog.sh: merges changesets into CHANGELOG.md with version header and date
- Modified aggregate-changesets.yml: aggregates all pending changesets into single version bump, updates changelog, cleans up changesets
- Modified release.yml: uses version comparison (Cargo.toml vs git tags) instead of changeset checking - idempotent and race-condition free
- Eliminates race conditions by not pushing back to trigger branch
- Single version bump per release cycle instead of per feature
- Full changelog history preserved in CHANGELOG.md
- Fix CI workflow and publish workflow issues
- Implement monorepo versioning and release validation to prevent cross-crate API mismatches during publishing. Adds validate-release.sh script that runs in CI to catch version skew and dependency resolution issues before they reach crates.io.
- Fix cross-crate dependency version specifications to enable crates.io publishing. All workspace dependencies now include required version specs.


## [0.2.0] - 2025-10-20

---
Testing the release flow
- Created aggregate-changesets.sh: collects all changesets and determines highest priority bump type
- Created update-changelog.sh: merges changesets into CHANGELOG.md with version header and date
- Modified aggregate-changesets.yml: aggregates all pending changesets into single version bump, updates changelog, cleans up changesets
- Modified release.yml: uses version comparison (Cargo.toml vs git tags) instead of changeset checking - idempotent and race-condition free
- Eliminates race conditions by not pushing back to trigger branch
- Single version bump per release cycle instead of per feature
- Full changelog history preserved in CHANGELOG.md
---
Fix CI workflow and publish workflow issues
---
Implement monorepo versioning and release validation to prevent cross-crate API mismatches during publishing. Adds validate-release.sh script that runs in CI to catch version skew and dependency resolution issues before they reach crates.io.
---
Fix cross-crate dependency version specifications to enable crates.io publishing. All workspace dependencies now include required version specs.


## [0.2.0] - 2025-10-19 ([#40](https://github.com/fulsomenko/kanban/pull/40))

- Fix CI workflow and publish workflow issues
- Implement monorepo versioning and release validation to prevent cross-crate API mismatches during publishing. Adds validate-release.sh script that runs in CI to catch version skew and dependency resolution issues before they reach crates.io.


## [0.1.7] - 2025-10-18 ([#32](https://github.com/fulsomenko/kanban/pull/32))

- - update CONTRIBUTING.md with branching and release workflow
- check for changesets onm develop branch
- add create-changeset.sh
- Fix card selection in kanban column view
- Fix card selection in kanban column view
- Fixed bug where card operations (edit, move, toggle completion) were using incorrect card indices
- Card selection index now correctly maps to cards within the focused column in kanban view
- Added get_selected_card_id() helper method to resolve selection properly
- CI/CD improvements and grouped view navigation fixes
- Add comprehensive CI workflow with format, clippy, test, and build checks
- Add sync-develop workflow to prevent branch divergence
- Refactor GroupedViewStrategy to use per-column TaskLists
- Fix navigation and sorting in grouped by column view
- Add seamless column wrapping for grouped and kanban views
- Document required GitHub secrets in CONTRIBUTING.md
- Set cursor to newly created task after creation
- - feat: add kanban column navigation
- feat: implement three task list view modes
- feat: add column and view selection UI state
- feat: add task list view support to Board domain
- feat: add column management handlers
- feat: add TaskListView domain enum


## [0.1.6] - 2025-10-16 ([#25](https://github.com/fulsomenko/kanban/pull/25))

- Enable direct card description editing from task list
- Add 'e' key binding to edit card description when focus is on Cards
- Previously required entering CardDetail mode first (Enter then 'e')


## [0.1.5] - 2025-10-14 ([#24](https://github.com/fulsomenko/kanban/pull/24))

- - only show prefix+number as task label on filtered by sprint task list


## [0.1.4] - 2025-10-14 ([#23](https://github.com/fulsomenko/kanban/pull/23))

- Show branch name in sprint-filtered task list and fix UI issues
- Show branch name instead of redundant sprint name when task list filtered by sprint
- Fix duplicate title rendering in tasks panel (removed redundant title call)
- Change LABEL_TEXT color from Gray to DarkGray for better visual separation


## [0.1.3] - 2025-10-14 ([#22](https://github.com/fulsomenko/kanban/pull/22))

- Extract theme system and reusable UI components
- Add theme module with semantic colors and style functions
- Create composable components (ListItem, Panel, Popup, DetailView, CardListItem, SelectionList)
- Refactor ui.rs using new components (1227→869 lines, 29% reduction)
- Improve code reusability and maintainability through composition
- CardListItem provides reusable task list rendering for board and sprint views


## [0.1.2] - 2025-10-13 ([#20](https://github.com/fulsomenko/kanban/pull/20))

- KAN-45: Automated release workflow with changeset-based versioning
- Add GitHub Actions workflow for automated crates.io publishing
- Implement changeset system for version management
- Add changeset validation check for PRs to master
- Create Nix-based bump-version and publish-crates scripts
- Configure deploy key authentication for protected branch bypass
- Update `CHANGELOG.md` generation with PR links
- Add unified workspace versioning across all crates
- Document changeset workflow in `README.md` and `CONTRIBUTING.md`
- Add semantic commit message guidelines
- Add PR title and description format guidelines
- Cross-reference `CLAUDE.md`, `CONTRIBUTING.md`, and `README.md`


## [0.1.1] - 2025-10-13 ([#19](https://github.com/fulsomenko/kanban/pull/19))

- # Changesets
When creating a PR, add a changeset file to describe your changes.
## Creating a Changeset
Create a file `.changeset/<descriptive-name>.md`:
```md
Brief description of changes for the changelog
```
## Bump Types
- `patch` - Bug fixes, small changes (0.1.0 → 0.1.1)
- `minor` - New features, backwards compatible (0.1.0 → 0.2.0)
- `major` - Breaking changes (0.1.0 → 1.0.0)
## Example
`.changeset/add-vim-keybindings.md`:
```md
Add vim-style keybindings for navigation
```
On merge to master, this will:
1. Update CHANGELOG.md with the description
2. Bump version according to the highest bump type
3. Tag and publish to crates.io
4. Delete processed changesets
- Add automated release workflow with changeset-based version management


# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2025-10-10

- Initial release
- Terminal-based kanban board interface
- Nix development environment
