# Kanban

[![CI](https://github.com/fulsomenko/kanban/actions/workflows/ci.yml/badge.svg)](https://github.com/fulsomenko/kanban/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/kanban-cli.svg)](https://crates.io/crates/kanban-cli)
[![AUR](https://img.shields.io/aur/version/kanban?label=AUR)](https://aur.archlinux.org/packages/kanban)
[![nixpkgs stable](https://repology.org/badge/version-for-repo/nix_stable_26_05/kanban.svg?header=nixpkgs%20stable)](https://search.nixos.org/packages?show=kanban&channel=26.05)
[![nixpkgs unstable](https://repology.org/badge/version-for-repo/nix_unstable/kanban.svg?header=nixpkgs%20unstable)](https://search.nixos.org/packages?show=kanban&channel=unstable)
[![Homebrew](https://img.shields.io/badge/dynamic/regex?url=https%3A%2F%2Fraw.githubusercontent.com%2Ffulsomenko%2Fhomebrew-tap%2Fmaster%2FFormula%2Fkanban.rb&search=refs%2Ftags%2Fv%28.%2A%29%5C.tar%5C.gz&replace=%241&label=homebrew)](https://github.com/fulsomenko/homebrew-tap)
[![Chocolatey](https://img.shields.io/chocolatey/v/kanban.svg)](https://community.chocolatey.org/packages/kanban)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE.md)

**Keyboard-first project management for the terminal.**

![Kanban Demo](demo/demo.gif)

*Inspired by [lazygit](https://github.com/jesseduffield/lazygit) · Built on [ratatui](https://ratatui.rs)*

---

## Why Kanban?

- **Zero latency** — pure keyboard flow — hjkl, never reach for the mouse
- **Your data is a file on your disk** — private, offline, always yours
- **Git-native** — generate branch names and `git checkout` commands from any card
- **LLM-native** — full MCP server (47 tools) works with Claude Code, Cursor, and any MCP client
- **Offline-first** — works anywhere; JSON and SQLite backends, atomic writes, live conflict detection

---

## Quick Start

### TUI

```bash
kanban                  # launch in-memory; pick or skip a file from the startup dialog
kanban boards.json      # open or create a JSON board file
kanban boards.sqlite    # open or create a SQLite board file
```

Press `?` at any time to see context-sensitive help.

### CLI

```bash
export KANBAN_FILE=boards.json   # or pass the path as the first argument

kanban board create --name "My Project"
kanban board list
kanban card create --board "My Project" --column TODO --title "Fix the bug" --priority high
kanban card list --board "My Project"
kanban sprint create --board "My Project"
kanban sprint activate yarara-release --duration-days 14
kanban card assign-sprint KAN-5 --sprint yarara-release
kanban relation add --parent KAN-5 --child KAN-7   # KAN-7 is now a subtask of KAN-5
kanban relation children KAN-5                     # list direct children of KAN-5
```

### Init (non-interactive setup)

```bash
kanban init boards.json --board "My Project"  # create file + first board, exit
kanban init --board "My Project"              # uses KANBAN_FILE or boards.json
kanban init                                   # creates the file with no entities
```

Every entity argument accepts either a UUID or a human-readable name (sprint
numbers also work for sprints; cards accept their `KAN-N` identifier). When a
name doesn't match, the error lists what's available.

All commands output JSON. Use `kanban --help` for full reference.

### MCP Server

**Claude Code**

```json
{
  "mcpServers": {
    "kanban": {
      "command": "kanban-mcp",
      "args": ["boards.json"]
    }
  }
}
```

---

## Installation

### From crates.io
```bash
cargo install kanban-cli
```

### From source
```bash
git clone https://github.com/fulsomenko/kanban
cd kanban
cargo install --path crates/kanban-cli
```

### Homebrew
```bash
brew install fulsomenko/tap/kanban
```

### Using Nix
```bash
nix run github:fulsomenko/kanban
```

### Arch Linux (AUR)
```bash
yay -S kanban
```

### Chocolatey
```powershell
choco install kanban
```

### winget
```powershell
winget install fulsomenko.kanban
```

### Linux Clipboard Support

For `y`/`Y` clipboard operations to persist after the app exits, you need a clipboard manager:

- **Wayland**: `wl-clip-persist`, `cliphist`, `clipman`, or your DE's built-in manager
- **X11**: Most desktop environments include one by default

### Windows and WSL

If your setup is Windows and WSL, and you often switch between them, then it is recommended to install separate binaries for each system to avoid constant recompiles.

---

## EDITOR configuration

Changes are made in an external editor as defined by your `EDITOR`. Neovim, nano, or some other terminal-based editor is recommended, both for easier switching between edits and browsing, and because editors that leave the terminal may cause issues.

VS Code is known not to work in the current implementation.

`kanban` is well-tested on supported OSes and is designed to be shell-agnostic. If your `EDITOR` is not set, it will default to `notepad` on Windows and `vi` otherwise.

---

## Features

### Boards & Cards
- Multiple boards, each with custom columns and WIP limits
- Rich cards: title, description, priority (Low/Medium/High/Critical), status (Todo/InProgress/Blocked/Done), story points, due dates
- Card numbering with configurable prefix (e.g. `KAN-42`)
- Card relations: parent/child (Spawns), blocking (with severity), and undirected relates (with sub-kind) — each with cycle / self-reference detection and dedicated `kanban relation` CLI + MCP tools
- Archive and restore cards
- Archive and restore whole boards: `board archive` / `board restore` / `board delete-archived`, an archived-boards TUI view you can drill into like a live board, `board list --archived` / `--include-archived`, a three-state MCP `archived` filter (`exclude` / `only` / `include`), and matching MCP archive/restore/delete-archived tools

### Sprint Planning
- Full sprint lifecycle: Planning → Active → Completed / Cancelled
- Carry uncompleted cards to the next sprint with one key
- Per-sprint card prefix overrides
- Sprint logs track assignment history per card

### Views & Navigation
- **3 view modes**: Flat list / Grouped by column / Kanban board — toggle with `V`
- Real-time `/` search
- Sort cards by priority, points, status, or position
- Sort the board list by position, name, creation time, or archival recency: `board list --sort <field> --order <dir>`, a persisted default via `board set-sort`, and a TUI field picker (`o`) / order toggle (`s`) for both live and archived boards
- Filter by sprint, status, or search result
- Multi-select for bulk archive / move / sprint-assign

### Productivity
- Undo/redo (`u`/`U`)
- External editor for descriptions (respects `$EDITOR`)
- Clipboard: `y` copies git branch name, `Y` copies `git checkout` command
- Import/export boards as JSON

### Storage & Sync
- JSON and SQLite storage backends
- Atomic writes (temp file → rename) prevent corruption
- Live file watching: auto-reload when another instance writes
- Conflict detection with user prompt when local edits clash

### Interfaces
- **TUI** — full keyboard-driven terminal UI
- **CLI** — scriptable; all operations, JSON output, pagination
- **MCP server** — 47 tools for LLM integration

---

## Key Bindings

Press `?` in the app to see bindings for the current context.

### Boards Panel

| Key | Action |
|-----|--------|
| `j`/`↓` | Navigate down |
| `k`/`↑` | Navigate up |
| `gg` | Jump to top |
| `G` | Jump to bottom |
| `Enter`/`Space` | Open board detail |
| `n` | New board |
| `r` | Rename board |
| `e` | Edit board |
| `d` | Delete board |
| `x` | Export board |
| `X` | Export all boards |
| `i` | Import board from file |
| `u` | Undo |
| `U` | Redo |
| `S` | Open settings |
| `1`/`2` | Focus boards/cards panel |
| `q` | Quit |
| `?` | Help |

### Cards Panel

| Key | Action |
|-----|--------|
| `j`/`↓`, `k`/`↑` | Navigate down/up |
| `gg` / `G` | Jump to top/bottom |
| `{` / `}` | Half-page up/down |
| `h`/`l` | Previous/next column |
| `H`/`L` | Move card left/right column |
| `Enter`/`Space` | Open card detail |
| `n` | New card |
| `e` | Edit card |
| `c` | Toggle done |
| `p` | Set priority |
| `d` | Archive card(s) |
| `D` | View archived cards |
| `v` | Toggle card selection |
| `Ctrl+a` | Select all visible cards |
| `Esc` | Clear selection |
| `P` | Set priority (bulk) |
| `a` | Assign to sprint |
| `o` | Sort cards |
| `O` | Toggle sort order |
| `t` | Toggle sprint filter |
| `T` | Filter options |
| `/` | Search |
| `s` | Manage child cards |
| `V` | Toggle view mode |
| `u` / `U` | Undo / Redo |
| `1`/`2` | Focus boards/cards panel |
| `q` | Quit |
| `?` | Help |

### Card Detail View

| Key | Action |
|-----|--------|
| `1`–`5` | Focus Title / Metadata / Description / Parents / Children panel |
| `e` | Edit current panel |
| `r` | Manage parent cards |
| `R` | Manage child cards |
| `y` | Copy git branch name to clipboard |
| `Y` | Copy `git checkout` command to clipboard |
| `a` | Assign to sprint |
| `d` | Delete card |
| `u` / `U` | Undo / Redo |
| `Esc` | Back |
| `?` | Help |

### Board Detail View

| Key | Action |
|-----|--------|
| `1`–`5` | Focus Name / Description / Settings / Sprints / Columns panel |
| `e` | Edit current panel |
| `p` | Set branch prefix |
| `n` | New sprint (Sprints panel) / New column (Columns panel) |
| `r` | Rename column (Columns panel) |
| `d` | Delete column (Columns panel) |
| `J`/`K` | Reorder column up/down (Columns panel) |
| `j`/`k` | Navigate within panel |
| `Enter`/`Space` | Open sprint detail (Sprints panel) |
| `u` / `U` | Undo / Redo |
| `Esc` | Back |
| `?` | Help |

### Sprint Detail View

| Key | Action |
|-----|--------|
| `h`/`l` | Switch between uncompleted/completed panels |
| `j`/`k` | Navigate cards |
| `a` | Activate sprint |
| `c` | Complete sprint |
| `p` | Set sprint prefix |
| `C` | Set card prefix override |
| `o`/`O` | Sort / Toggle sort order |
| `v` | Select card(s) |
| `u` / `U` | Undo / Redo |
| `Esc` | Back |
| `?` | Help |

### Archived Cards View

| Key | Action |
|-----|--------|
| `j`/`k` | Navigate |
| `gg`/`G` | Jump to top/bottom |
| `{`/`}` | Half-page up/down |
| `r` | Restore card(s) |
| `x` | Delete card(s) permanently |
| `v` | Select for bulk operation |
| `V` | Toggle view mode |
| `u` / `U` | Undo / Redo |
| `Esc` | Back |

---

## Architecture

**How it fits, in 30 seconds:** `kanban-domain` holds all business rules with
zero I/O. `kanban-persistence` and `kanban-backend` are two stacked plugin
points — one for storage *format* (JSON/SQLite), one for storage *backend*
(in-memory/JSON/SQLite/HTTP) — each with a factory trait a new implementation
registers against. `kanban-service` is the single seam every frontend
(`kanban-cli`, `kanban-tui`, `kanban-mcp`, `kanban-server`) goes through to
reach a backend; each frontend picks which concrete backends to compile in
and registers them itself at startup. The detailed graph below shows exactly
which crate depends on which.

The workspace is layered so that every dependency points inward, toward pure
domain logic, and outward-facing concerns (storage format, transport, UI) stay
swappable:

```
crates/
├── kanban-core               → Shared types, error handling, config, reusable state primitives
├── kanban-domain             → Domain models, business logic, filtering & sorting
├── kanban-api                → Wire-format DTOs shared by kanban-server and HTTP backend clients
├── kanban-persistence        → Persistence trait layer — pure trait definitions, all I/O lives in backend crates
├── kanban-backend            → KanbanBackend / KanbanBackendFactory abstractions over a pluggable backend
├── kanban-backend-memory     → In-memory KanbanBackend (ephemeral, no persistence)
├── kanban-backend-http       → KanbanBackend implementation talking to a remote kanban-server
├── kanban-persistence-json   → JSON file storage backend (implements kanban-persistence + kanban-backend)
├── kanban-persistence-sqlite → SQLite storage backend (implements kanban-persistence + kanban-backend)
├── kanban-service            → KanbanContext, persistence orchestration, undo/redo
├── kanban-view               → Renderer-agnostic view-model layer shared by kanban-tui and kanban-web
├── kanban-tui                → Terminal UI with ratatui
├── kanban-cli                → CLI entry point (clap)
├── kanban-mcp                → Model Context Protocol server
└── kanban-server             → HTTP API server (axum)
```

**Pluggable backends, registered by the app, not the service layer** (KAN-1027):
`kanban-persistence` defines `StoreFactory` / `StoreRegistry` for the storage
format layer, and `kanban-backend` defines the equivalent `KanbanBackendFactory`
/ `KanbanBackendRegistry` one layer up, dispatching to the right backend by
content-sniffing a locator (`KanbanBackendRegistry::for_locator`) or by explicit
name (`for_name`). `kanban-service` depends only on the `kanban-backend`
abstraction — it has no production dependency on any concrete backend crate.
Each application (`kanban-cli`, `kanban-mcp`, `kanban-tui`, `kanban-server`)
builds its own registry and registers the concrete backends (JSON, SQLite,
in-memory, HTTP) it wants to ship with. This is the payoff of the KAN-1027
refactor: adding a new storage backend no longer means touching
`kanban-service`.

### Workspace dependency graph

Solid arrows are normal (`[dependencies]`) edges. Dotted arrows are
optional/feature-gated edges (the dependency only activates when the named
Cargo feature is enabled — most are on by default). Dev-only edges (test
fixtures) are omitted here; see the note below.

```mermaid
graph TD
    subgraph "Foundation"
        CORE[kanban-core]
        DOM[kanban-domain]
    end
    subgraph "Domain-adjacent traits"
        API[kanban-api]
        PER[kanban-persistence]
    end
    subgraph "Backend abstraction"
        BE[kanban-backend]
        BEMEM[kanban-backend-memory]
        BEHTTP[kanban-backend-http]
    end
    subgraph "Concrete storage backends"
        JSON[kanban-persistence-json]
        SQL[kanban-persistence-sqlite]
    end
    subgraph "Service"
        SVC[kanban-service]
    end
    subgraph "View layer"
        VIEW[kanban-view]
    end
    subgraph "Applications"
        CLI[kanban-cli]
        MCP[kanban-mcp]
        TUI[kanban-tui]
        SRV[kanban-server]
    end

    DOM --> CORE
    API --> CORE
    API --> DOM
    PER --> CORE
    PER --> DOM

    BE --> CORE
    BE --> DOM
    BE --> PER
    BEMEM --> DOM
    BEMEM --> BE
    BEHTTP --> CORE
    BEHTTP --> DOM
    BEHTTP --> BE
    BEHTTP --> API

    JSON --> CORE
    JSON --> DOM
    JSON --> PER
    JSON --> BE
    JSON --> BEMEM
    SQL --> CORE
    SQL --> DOM
    SQL --> PER
    SQL --> BE
    SQL --> BEMEM

    SVC --> CORE
    SVC --> DOM
    SVC --> PER
    SVC --> API
    SVC --> BE
    SVC -.->|feature: sqlite, default-on| SQL

    VIEW --> CORE
    VIEW --> DOM

    CLI --> CORE
    CLI --> DOM
    CLI --> PER
    CLI --> BE
    CLI --> SVC
    CLI -.->|feature: json, default-on| JSON
    CLI -.->|feature: sqlite, default-on| SQL
    CLI -.->|feature: tui, default-on| TUI

    MCP --> CORE
    MCP --> DOM
    MCP --> PER
    MCP --> BE
    MCP --> SVC
    MCP -.->|feature: json, default-on| JSON
    MCP -.->|feature: sqlite, default-on| SQL

    TUI --> CORE
    TUI --> DOM
    TUI --> PER
    TUI --> BE
    TUI --> BEMEM
    TUI --> JSON
    TUI --> SQL
    TUI --> SVC
    TUI --> VIEW

    SRV --> CORE
    SRV --> DOM
    SRV --> PER
    SRV --> BE
    SRV --> JSON
    SRV --> SQL
    SRV --> SVC
    SRV -.->|feature: test-helpers| BEMEM
```

**Not shown above (test-only, dev-dependencies):** `kanban-persistence-json`
and `kanban-persistence-sqlite` each dev-depend on `kanban-service` (feature
`test-helpers`) to run the shared service-layer contract tests against their
backend, and `kanban-service` dev-depends back on both of them — a
dev-dependency-only cycle that Cargo permits but a production dependency graph
never would. Similarly `kanban-domain` and `kanban-persistence` dev-depend on
`kanban-backend-memory` for lightweight in-memory test fixtures, and
`kanban-backend-http` dev-depends on `kanban-server` (feature `test-helpers`)
for integration tests against a real server. None of these are reachable from
a release build — `cargo build --release` never touches them.

The key structural change from before KAN-1027: `kanban-service` used to
depend directly on `kanban-persistence-json` (behind a `json` feature) and on
`kanban-backend-memory`. Both are gone from its production dependency graph;
it depends only on the `kanban-backend`/`kanban-persistence` abstractions, and
the four application crates now compose the concrete backends themselves.

| Crate | Description | README |
|-------|-------------|--------|
| `kanban-core` | Shared types, config, errors, graph, pagination | [→](crates/kanban-core/README.md) |
| `kanban-domain` | Domain models, business logic | [→](crates/kanban-domain/README.md) |
| `kanban-api` | REST wire DTOs shared by the server and the HTTP backend | [→](crates/kanban-api/README.md) |
| `kanban-persistence` | Persistence trait layer | [→](crates/kanban-persistence/README.md) |
| `kanban-backend` | `KanbanBackend` / `RemoteWrites` abstractions and the backend registry | [→](crates/kanban-backend/README.md) |
| `kanban-backend-memory` | In-memory `KanbanBackend` (ephemeral) | [→](crates/kanban-backend-memory/README.md) |
| `kanban-backend-http` | `KanbanBackend` over HTTP against a remote `kanban-server` | [→](crates/kanban-backend-http/README.md) |
| `kanban-persistence-json` | JSON file backend | [→](crates/kanban-persistence-json/README.md) |
| `kanban-persistence-sqlite` | SQLite backend | [→](crates/kanban-persistence-sqlite/README.md) |
| `kanban-service` | Service layer, KanbanContext, undo/redo | [→](crates/kanban-service/README.md) |
| `kanban-view` | Renderer-agnostic view-model layer shared by kanban-tui/kanban-web | [→](crates/kanban-view/README.md) |
| `kanban-tui` | Terminal UI | [→](crates/kanban-tui/README.md) |
| `kanban-cli` | CLI entry point | [→](crates/kanban-cli/README.md) |
| `kanban-mcp` | MCP server | [→](crates/kanban-mcp/README.md) |
| `kanban-server` | HTTP API server | [→](crates/kanban-server/README.md) |

### Where do I make a change?

| I want to... | Touch these crates |
|---|---|
| Add a new storage backend (e.g. Postgres) | `kanban-persistence` (implement `StoreFactory`/`PersistenceStore`), `kanban-backend` (implement `KanbanBackendFactory`), and whichever app crate(s) should ship it (`kanban-cli`/`kanban-mcp`/`kanban-tui`/`kanban-server`) to `register_backend` it |
| Add a field to `Card`/`Board`/`Column`/`Sprint` | `kanban-domain` (model + `*Update` struct), `kanban-persistence-json` (envelope version bump + migration), `kanban-persistence-sqlite` (schema migration), `kanban-api` (DTOs, if exposed over REST) |
| Add a CLI command | `kanban-cli` (clap subcommand + handler in `src/handlers/`), `kanban-service` (a new `KanbanContext` method, if the operation doesn't exist yet) |
| Add an MCP tool | `kanban-mcp` (tool handler), `kanban-service` (a new `KanbanContext` method, if needed) |
| Add a REST endpoint | `kanban-server` (axum handler + route), `kanban-api` (request/response DTOs), `kanban-service` |
| Add a TUI dialog or view | `kanban-tui` (`AppMode`/`DialogMode` variant, key handler, `ui::` renderer) |
| Add or change a card relation kind (spawns / blocks / relates) | `kanban-domain` (`DependencyGraph`, `GraphOperations`), `kanban-persistence-sqlite` (edge table), `kanban-persistence-json` (migration), `kanban-service` (`GraphOperations` impl on `KanbanContext`) |
| Change undo/redo behavior | `kanban-service` (`src/undo_stack.rs`, `src/context/undo.rs`), `kanban-domain` (`commands/` inverse capture) |

### Request flow: `card create` end to end

The static dependency graph above shows structure; here's the same layering
in motion for one representative write path:

1. `kanban card create --board ... --column ... --title ...` is parsed by clap in `kanban-cli` (`src/handlers/card.rs`), which resolves the board/column arguments to UUIDs.
2. The handler calls `ctx.create_card(board_id, column_id, title, options)` — the `KanbanOperations` entry point implemented on `KanbanContext` (`kanban-service/src/context/cards.rs`).
3. `create_card_impl` builds a `NewCard` spec and calls `create_card_from_spec`, which constructs a `Command::Card(CardCommand::Create(..))` and calls `self.execute(vec![cmd])`.
4. `KanbanContext::execute` runs the command through `backend.with_transaction(...)`: the command mutates state via the `DataStore` trait on whichever concrete `KanbanBackend` is active, then the batch is appended to the command log via `backend.append_batch` for the undo/audit trail.
5. The handler calls `ctx.save().await?`, which delegates to `backend.flush()` — an atomic temp-file-then-rename write for the JSON backend, or a transaction commit for SQLite.
6. The CLI serializes the returned `Card` to JSON and prints it to stdout.

---

## Data & Persistence

### JSON Backend (default)

- **Envelope format** (current version V11): `{ "version": 11, "metadata": {...}, "data": {...} }`
- **Automatic migrations**: older files (V1..V11) upgrade in place on open, writing a one-time `.v{N}.backup` before the upgrade
- **Atomic writes**: crash-safe — every write is atomic (temp file → rename)
- **Debounced saving**: 500ms minimum interval between saves
- Default for any plain file path

### SQLite Backend

- **WAL mode** with foreign key enforcement
- **Connection pool**: max 2 connections
- **Relational schema**: boards, columns, cards, archived cards, sprints, sprint logs, dependency graph edges, and more
- **Schema versioning with active migrations** (current schema version 5): older databases upgrade on open, each guarded by a durable pre-migration backup (`VACUUM INTO` snapshot to `.v{N}.backup`)
- File selected by `.sqlite`, `.sqlite3`, or `.db` extension

### Multi-Instance Support

- **File watching**: detects changes written by other TUI or MCP instances
- **Auto-reload**: applies external changes automatically when the local state is clean
- **Conflict prompt**: when local edits clash with an external write, you choose to reload or keep

---

## Roadmap

- [x] Progressive auto-save
- [x] Full CLI interface
- [x] Card relations (parent/child, blocks with severity, relates with sub-kind)
- [x] Multiple storage backends (JSON + SQLite)
- [x] MCP server for LLM integration
- [x] Full undo/redo
- [x] Sprint planning lifecycle
- [x] Bulk operations
- [ ] Configurable keybindings
- [ ] Attachments
- [ ] Audit log
- [ ] HTTP API for remote access
- [ ] Collaborative / sync features
- [ ] Search anything, anywhere

---

## Building, Running, Testing

```bash
nix develop            # enter the dev shell (Rust toolchain, cargo-watch, bacon, ...)

cargo build             # build all crates
cargo build --release   # optimized production build
cargo run                # launch the TUI
cargo run -- tui          # explicit TUI mode
cargo run -- init --name "My Project"  # non-interactive board init

cargo test                            # run all tests
cargo test --package kanban-domain    # test a single crate
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt --all
```

See [CONTRIBUTING.md](CONTRIBUTING.md) for the full development workflow, code style, and testing guidelines, and each crate's own README (linked in the architecture table above) for its scoped dependency diagram and public API.

---

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for development workflow, code style, and testing guidelines.

## License

Apache 2.0 — see [LICENSE.md](LICENSE.md)
