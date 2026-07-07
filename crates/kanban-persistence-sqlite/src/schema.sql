-- SQLite schema for kanban persistence
-- Version: 3 (KAN-832: archived_cards.board_id + cards column_id FK dropped so
-- archived cards survive column deletion — see migrate_v2_to_v3_archived_cards.
-- Version: 2 (KAN-522: writer-stamp columns added; schema_version begins
-- to be authoritative — see SqliteStore::migrate for the ALTER fallbacks)

-- Metadata table for tracking persistence state and conflict detection
CREATE TABLE IF NOT EXISTS metadata (
    id INTEGER PRIMARY KEY CHECK (id = 1),  -- Singleton row
    instance_id TEXT NOT NULL,
    saved_at TEXT NOT NULL,
    schema_version INTEGER NOT NULL DEFAULT 3,
    writer_version TEXT,
    writer_commit TEXT
);

-- Boards table
CREATE TABLE IF NOT EXISTS boards (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT,
    sprint_prefix TEXT,
    card_prefix TEXT,
    task_sort_field TEXT NOT NULL DEFAULT 'Default',
    task_sort_order TEXT NOT NULL DEFAULT 'Ascending',
    sprint_duration_days INTEGER,
    sprint_name_used_count INTEGER NOT NULL DEFAULT 0,
    next_sprint_number INTEGER NOT NULL DEFAULT 1,
    active_sprint_id TEXT,
    task_list_view TEXT NOT NULL DEFAULT 'Flat',
    card_counter INTEGER NOT NULL DEFAULT 1,
    completion_column_id TEXT,
    position INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (active_sprint_id) REFERENCES sprints(id) ON DELETE SET NULL DEFERRABLE INITIALLY DEFERRED,
    FOREIGN KEY (completion_column_id) REFERENCES columns(id) ON DELETE SET NULL DEFERRABLE INITIALLY DEFERRED
);

-- Board sprint names
CREATE TABLE IF NOT EXISTS board_sprint_names (
    board_id TEXT NOT NULL,
    position INTEGER NOT NULL,
    name TEXT NOT NULL,
    PRIMARY KEY (board_id, position),
    FOREIGN KEY (board_id) REFERENCES boards(id) ON DELETE CASCADE
);

-- Board sprint counters
CREATE TABLE IF NOT EXISTS board_sprint_counters (
    board_id TEXT NOT NULL,
    prefix TEXT NOT NULL,
    counter INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (board_id, prefix),
    FOREIGN KEY (board_id) REFERENCES boards(id) ON DELETE CASCADE
);

-- Columns table
CREATE TABLE IF NOT EXISTS columns (
    id TEXT PRIMARY KEY,
    board_id TEXT NOT NULL,
    name TEXT NOT NULL,
    position INTEGER NOT NULL,
    wip_limit INTEGER,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (board_id) REFERENCES boards(id) ON DELETE CASCADE
);

-- Sprints table (defined before cards since cards reference sprints)
CREATE TABLE IF NOT EXISTS sprints (
    id TEXT PRIMARY KEY,
    board_id TEXT NOT NULL,
    sprint_number INTEGER NOT NULL,
    name_index INTEGER,
    prefix TEXT,
    card_prefix TEXT,
    status TEXT NOT NULL DEFAULT 'Planning',
    start_date TEXT,
    end_date TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (board_id) REFERENCES boards(id) ON DELETE CASCADE
);

-- Cards table (holds both active and archived cards)
-- NOTE (schema 3): column_id carries NO foreign key to columns. An archived
-- card keeps its (now historical, possibly dangling) column_id in this table,
-- and must survive deletion of that column. A live-card FK cascade would
-- delete the archived card's row when its column is dropped; instead, live-card
-- cleanup on column delete is performed explicitly by the command tier
-- (DeleteCardsByColumns), so no cascade is needed here.
-- KEEP IN SYNC: the 2->3 migration rebuilds this table as `cards_new` in
-- `init.rs::migrate_v2_to_v3_archived_cards` (same columns, same non-FK shape).
-- Adding/removing a column here must be mirrored in that CREATE + its INSERT
-- SELECT list, or migrating users silently lose the column's data on the swap.
CREATE TABLE IF NOT EXISTS cards (
    id TEXT PRIMARY KEY,
    column_id TEXT NOT NULL,
    title TEXT NOT NULL,
    description TEXT,
    priority TEXT NOT NULL DEFAULT 'Medium',
    status TEXT NOT NULL DEFAULT 'Todo',
    position INTEGER NOT NULL,
    due_date TEXT,
    points INTEGER CHECK (points >= 0 AND points <= 255),
    card_number INTEGER NOT NULL DEFAULT 0,
    sprint_id TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    completed_at TEXT,
    FOREIGN KEY (sprint_id) REFERENCES sprints(id) ON DELETE SET NULL
);

-- Sprint logs
-- Note: No FK on sprint_id — sprint logs are historical records
-- and must survive sprint deletion.
CREATE TABLE IF NOT EXISTS sprint_logs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    card_id TEXT NOT NULL,
    sprint_id TEXT NOT NULL,
    sprint_number INTEGER NOT NULL,
    sprint_name TEXT,
    started_at TEXT NOT NULL,
    ended_at TEXT,
    status TEXT NOT NULL,
    FOREIGN KEY (card_id) REFERENCES cards(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_sprint_logs_card_id ON sprint_logs(card_id);

-- Archived cards metadata (card data lives in cards table)
-- Extension table over `cards` (1:0..1). board_id (schema 3) makes board
-- scoping a direct WHERE rather than a column walk.
CREATE TABLE IF NOT EXISTS archived_cards (
    card_id TEXT PRIMARY KEY,
    board_id TEXT NOT NULL,
    archived_at TEXT NOT NULL,
    original_column_id TEXT NOT NULL,
    original_position INTEGER NOT NULL,
    FOREIGN KEY (card_id) REFERENCES cards(id) ON DELETE CASCADE
);

-- Card dependency edges: one table per kind.
-- Splitting the single card_edges table into kind-specific tables
-- mirrors the in-memory split (DagGraph<SpawnsEdge> /
-- DagGraph<BlocksEdge> / UndirectedGraph<RelatesEdge>) and lets each
-- table carry the metadata its kind actually needs without nullable
-- catch-all columns. No FK on source_id/target_id — edges are
-- bulk-replaced on every save (DELETE-all + re-INSERT).

CREATE TABLE IF NOT EXISTS spawns_edges (
    source_id TEXT NOT NULL,
    target_id TEXT NOT NULL,
    created_at TEXT NOT NULL,
    archived_at TEXT,
    PRIMARY KEY (source_id, target_id)
);

CREATE TABLE IF NOT EXISTS blocks_edges (
    source_id TEXT NOT NULL,
    target_id TEXT NOT NULL,
    severity TEXT NOT NULL DEFAULT 'Medium'
        CHECK (severity IN ('Low', 'Medium', 'High', 'Critical')),
    created_at TEXT NOT NULL,
    archived_at TEXT,
    PRIMARY KEY (source_id, target_id)
);

CREATE TABLE IF NOT EXISTS relates_edges (
    source_id TEXT NOT NULL,
    target_id TEXT NOT NULL,
    kind TEXT NOT NULL DEFAULT 'General'
        CHECK (kind IN ('General', 'Duplicates', 'MentionedIn')),
    created_at TEXT NOT NULL,
    archived_at TEXT,
    PRIMARY KEY (source_id, target_id)
);

CREATE INDEX IF NOT EXISTS idx_spawns_edges_source ON spawns_edges(source_id);
CREATE INDEX IF NOT EXISTS idx_spawns_edges_target ON spawns_edges(target_id);
CREATE INDEX IF NOT EXISTS idx_blocks_edges_source ON blocks_edges(source_id);
CREATE INDEX IF NOT EXISTS idx_blocks_edges_target ON blocks_edges(target_id);
CREATE INDEX IF NOT EXISTS idx_relates_edges_source ON relates_edges(source_id);
CREATE INDEX IF NOT EXISTS idx_relates_edges_target ON relates_edges(target_id);

-- Indexes for common queries
CREATE INDEX IF NOT EXISTS idx_columns_board_id ON columns(board_id);
CREATE INDEX IF NOT EXISTS idx_columns_position ON columns(board_id, position);

CREATE INDEX IF NOT EXISTS idx_sprints_board_id ON sprints(board_id);
CREATE INDEX IF NOT EXISTS idx_sprints_status ON sprints(status);

CREATE INDEX IF NOT EXISTS idx_cards_column_id ON cards(column_id);
CREATE INDEX IF NOT EXISTS idx_cards_sprint_id ON cards(sprint_id);
CREATE INDEX IF NOT EXISTS idx_cards_position ON cards(column_id, position);
CREATE INDEX IF NOT EXISTS idx_cards_status ON cards(status);
CREATE INDEX IF NOT EXISTS idx_cards_priority ON cards(priority);
CREATE INDEX IF NOT EXISTS idx_cards_updated_at ON cards(updated_at);

CREATE INDEX IF NOT EXISTS idx_archived_cards_board_id ON archived_cards(board_id);
CREATE INDEX IF NOT EXISTS idx_archived_cards_archived_at ON archived_cards(archived_at);

-- Command log: per-batch JSON serialisation for cross-session undo (KAN-191).
-- batch_index is a logical, dense, monotonically increasing cursor — it does
-- not need to match SQLite's ROWID. Truncate-after-N is implemented with a
-- DELETE WHERE batch_index >= N; pruning the oldest N is a DELETE WHERE
-- batch_index < N followed by a renumber.
CREATE TABLE IF NOT EXISTS command_log (
    batch_index INTEGER PRIMARY KEY,
    commands_json TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_command_log_batch ON command_log(batch_index);
