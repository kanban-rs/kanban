use clap::{Args, Parser, Subcommand, ValueEnum};

use kanban_core::CLI_VERSION_DISPLAY;

#[derive(Parser)]
#[command(name = "kanban")]
#[command(about = "A terminal-based kanban board", long_about = None)]
#[command(version = CLI_VERSION_DISPLAY, arg_required_else_help = false)]
pub struct Cli {
    /// Path to kanban data file (or set KANBAN_FILE env var)
    #[arg(value_name = "FILE", env = "KANBAN_FILE")]
    pub file: Option<String>,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Board operations
    Board(BoardCommand),
    /// Column operations
    Column(ColumnCommand),
    /// Card operations
    Card(CardCommand),
    /// Card-relation operations (parent/child)
    Relation(RelationCommand),
    /// Sprint operations
    Sprint(SprintCommand),
    /// Export board data
    Export(ExportArgs),
    /// Import board data
    Import(ImportArgs),
    /// Generate shell completions
    Completions {
        #[arg(value_enum)]
        shell: clap_complete::Shell,
    },
    /// Migrate data between storage backends
    Migrate(MigrateArgs),
    /// Initialize a new board file with an optional first board
    Init {
        /// Name of the first board to create. If omitted, the file is created with no entities.
        #[arg(long)]
        board: Option<String>,
    },
}

// Board commands
#[derive(Args)]
pub struct BoardCommand {
    #[command(subcommand)]
    pub action: BoardAction,
}

#[derive(Subcommand)]
pub enum BoardAction {
    /// Create a new board
    Create {
        #[arg(long)]
        name: String,
        #[arg(long)]
        card_prefix: Option<String>,
        /// Also create TODO/Doing/Complete and set Complete as the completion column.
        #[arg(long)]
        with_default_columns: bool,
    },
    /// List boards (live by default; use --archived / --include-archived).
    List {
        /// Show only archived boards (mutually exclusive with --include-archived).
        #[arg(long, conflicts_with = "include_archived")]
        archived: bool,
        /// Include archived boards alongside live ones.
        #[arg(long)]
        include_archived: bool,
        /// Sort key. When omitted, falls back to the persisted
        /// `board_sort_field` default (else board position).
        #[arg(long, value_enum)]
        sort: Option<BoardSortKey>,
        /// Sort direction. When omitted, falls back to the persisted
        /// `board_sort_order` default (else ascending).
        #[arg(long, value_enum)]
        order: Option<SortDir>,
        #[arg(long)]
        page: Option<u32>,
        #[arg(long)]
        page_size: Option<u32>,
    },
    /// Get a specific board by UUID or name
    Get {
        /// Board UUID or name
        board: String,
    },
    /// Update a board
    Update(BoardUpdateArgs),
    /// Delete a board by UUID or name
    Delete {
        /// Board UUID or name
        board: String,
    },
    /// Archive a board by UUID or name
    Archive {
        /// Board UUID or name
        board: String,
    },
    /// Restore an archived board by UUID or name
    Restore {
        /// Board UUID or name
        board: String,
    },
    /// Permanently delete an archived board by UUID or name
    DeleteArchived {
        /// Board UUID or name
        board: String,
    },
    /// Persist the default board-list sort (written to AppConfig).
    SetSort {
        /// Board sort dimension to persist as the default.
        #[arg(long, value_enum)]
        sort: Option<BoardSortKey>,
        /// Board sort direction to persist as the default.
        #[arg(long, value_enum)]
        order: Option<SortDir>,
    },
}

/// Sort key for `kanban board list` / the persisted board-list default.
/// The board-view dimensions (NOT the card `SortKey`): board position, name,
/// creation time, and archival recency. Each multi-word variant also accepts
/// its snake_case canonical `Display` token (e.g. `created_at`) as an alias so
/// the arg surface matches the on-disk form (R1).
#[derive(Copy, Clone, Debug, ValueEnum)]
pub enum BoardSortKey {
    Position,
    Name,
    #[value(alias = "created_at")]
    CreatedAt,
    #[value(alias = "archived_at")]
    ArchivedAt,
}

impl BoardSortKey {
    pub fn to_board_sort_field(self) -> kanban_domain::BoardSortField {
        use kanban_domain::BoardSortField;
        match self {
            BoardSortKey::Position => BoardSortField::Position,
            BoardSortKey::Name => BoardSortField::Name,
            BoardSortKey::CreatedAt => BoardSortField::CreatedAt,
            BoardSortKey::ArchivedAt => BoardSortField::ArchivedAt,
        }
    }
}

#[derive(Args)]
pub struct BoardUpdateArgs {
    /// Board UUID or name
    pub board: String,
    #[arg(long)]
    pub name: Option<String>,
    #[arg(long)]
    pub description: Option<String>,
    #[arg(long)]
    pub sprint_prefix: Option<String>,
    #[arg(long)]
    pub card_prefix: Option<String>,
    /// Default sort key for the task list view.
    #[arg(long, value_enum)]
    pub sort_field: Option<SortKey>,
    /// Default sort direction for the task list view.
    #[arg(long, value_enum)]
    pub sort_order: Option<SortDir>,
    /// Columns that mean "complete", most-primary first. Accepts names or
    /// UUIDs, comma-separated. Pass an empty string to disable status/column
    /// auto-sync for this board.
    #[arg(long, value_delimiter = ',')]
    pub completion_columns: Option<Vec<String>>,
}

// Column commands
#[derive(Args)]
pub struct ColumnCommand {
    #[command(subcommand)]
    pub action: ColumnAction,
}

#[derive(Subcommand)]
pub enum ColumnAction {
    /// Create a new column
    Create {
        /// Board UUID or name
        #[arg(long)]
        board: String,
        #[arg(long)]
        name: String,
        #[arg(long)]
        position: Option<i32>,
    },
    /// List columns for a board
    List {
        /// Board UUID or name
        #[arg(long)]
        board: String,
        #[arg(long)]
        page: Option<u32>,
        #[arg(long)]
        page_size: Option<u32>,
    },
    /// Get a specific column by UUID or name
    Get {
        /// Column UUID or name
        column: String,
    },
    /// Update a column
    Update(ColumnUpdateArgs),
    /// Delete a column by UUID or name
    Delete {
        /// Column UUID or name
        column: String,
    },
    /// Reorder a column by UUID or name
    Reorder {
        /// Column UUID or name
        column: String,
        #[arg(long)]
        position: i32,
    },
}

#[derive(Args)]
pub struct ColumnUpdateArgs {
    /// Column UUID or name
    pub column: String,
    #[arg(long)]
    pub name: Option<String>,
    #[arg(long)]
    pub position: Option<i32>,
    #[arg(long)]
    pub wip_limit: Option<u32>,
    #[arg(long)]
    pub clear_wip_limit: bool,
}

// Card commands
#[derive(Args)]
pub struct CardCommand {
    #[command(subcommand)]
    pub action: CardAction,
}

#[derive(Subcommand)]
pub enum CardAction {
    /// Create a new card
    Create(CardCreateArgs),
    /// List cards with optional filters
    List(CardListArgs),
    /// Get a specific card by UUID or identifier (e.g. KAN-5)
    Get {
        /// Card UUID or identifier like KAN-5 or 5
        card: String,
    },
    /// Update a card
    Update(CardUpdateArgs),
    /// Move a card to another column
    Move {
        /// Card UUID or identifier like KAN-5 or 5
        card: String,
        /// Column UUID or name
        #[arg(long)]
        column: String,
        #[arg(long)]
        position: Option<i32>,
    },
    /// Archive a card by UUID or identifier (e.g. KAN-5)
    Archive {
        /// Card UUID or identifier like KAN-5 or 5
        card: String,
    },
    /// Restore an archived card by UUID or identifier (e.g. KAN-5)
    Restore {
        /// Card UUID or identifier like KAN-5 or 5
        card: String,
        /// Column UUID or name
        #[arg(long)]
        column: Option<String>,
    },
    /// Permanently delete an archived card by UUID or identifier (e.g. KAN-5)
    Delete {
        /// Card UUID or identifier like KAN-5 or 5
        card: String,
    },
    /// Assign a card to a sprint
    AssignSprint {
        /// Card UUID or identifier like KAN-5 or 5
        card: String,
        /// Sprint UUID, name, or number
        #[arg(long)]
        sprint: String,
    },
    /// Unassign a card from its sprint
    UnassignSprint {
        /// Card UUID or identifier like KAN-5 or 5
        card: String,
    },
    /// Get the branch name for a card
    BranchName {
        /// Card UUID or identifier like KAN-5 or 5
        card: String,
    },
    /// Get the git checkout command for a card
    GitCheckout {
        /// Card UUID or identifier like KAN-5 or 5
        card: String,
    },
    /// Archive multiple cards
    #[command(name = "archive-cards")]
    ArchiveCards {
        /// Comma-separated card UUIDs or identifiers (e.g. KAN-1,KAN-2,42)
        #[arg(long, value_delimiter = ',')]
        cards: Vec<String>,
    },
    /// Move multiple cards to a column
    #[command(name = "move-cards")]
    MoveCards {
        /// Comma-separated card UUIDs or identifiers (e.g. KAN-1,KAN-2,42)
        #[arg(long, value_delimiter = ',')]
        cards: Vec<String>,
        /// Column UUID or name (must be on the same board as all selected cards)
        #[arg(long)]
        column: String,
    },
    /// Assign multiple cards to a sprint
    #[command(name = "assign-cards-to-sprint")]
    AssignCardsToSprint {
        /// Comma-separated card UUIDs or identifiers (e.g. KAN-1,KAN-2,42)
        #[arg(long, value_delimiter = ',')]
        cards: Vec<String>,
        /// Sprint UUID, name, or number (must be on the same board as all selected cards)
        #[arg(long)]
        sprint: String,
    },
}

// Relation commands

/// Sort key for `kanban relation parents` / `children` output.
#[derive(Copy, Clone, Debug, ValueEnum)]
pub enum SortKey {
    CardNumber,
    Priority,
    Points,
    CreatedAt,
    UpdatedAt,
    DueDate,
    Status,
    Position,
}

/// Sort direction.
#[derive(Copy, Clone, Debug, ValueEnum)]
pub enum SortDir {
    Asc,
    Desc,
}

impl SortKey {
    pub fn to_sort_by(self) -> kanban_domain::sort::SortBy {
        use kanban_domain::sort::SortBy;
        match self {
            SortKey::CardNumber => SortBy::CardNumber,
            SortKey::Priority => SortBy::Priority,
            SortKey::Points => SortBy::Points,
            SortKey::CreatedAt => SortBy::CreatedAt,
            SortKey::UpdatedAt => SortBy::UpdatedAt,
            SortKey::DueDate => SortBy::DueDate,
            SortKey::Status => SortBy::Status,
            SortKey::Position => SortBy::Position,
        }
    }

    /// Convert a CLI sort flag to the board-level `SortField`. `CardNumber`
    /// has no `SortField` counterpart, so it falls back to `SortField::Default`
    /// which `get_sorter_for_field` also resolves via `SortBy::CardNumber`.
    pub fn to_sort_field(self) -> kanban_domain::SortField {
        use kanban_domain::SortField;
        match self {
            SortKey::CardNumber => SortField::Default,
            SortKey::Priority => SortField::Priority,
            SortKey::Points => SortField::Points,
            SortKey::CreatedAt => SortField::CreatedAt,
            SortKey::UpdatedAt => SortField::UpdatedAt,
            SortKey::DueDate => SortField::DueDate,
            SortKey::Status => SortField::Status,
            SortKey::Position => SortField::Position,
        }
    }
}

impl SortDir {
    pub fn to_sort_order(self) -> kanban_domain::SortOrder {
        match self {
            SortDir::Asc => kanban_domain::SortOrder::Ascending,
            SortDir::Desc => kanban_domain::SortOrder::Descending,
        }
    }
}

#[cfg(test)]
mod sort_key_tests {
    use super::*;
    use kanban_domain::sort::SortBy;
    use kanban_domain::SortField;

    #[test]
    fn test_sort_key_due_date_maps_to_sort_by_due_date() {
        assert!(matches!(SortKey::DueDate.to_sort_by(), SortBy::DueDate));
    }

    #[test]
    fn test_sort_key_due_date_maps_to_sort_field_due_date() {
        assert_eq!(SortKey::DueDate.to_sort_field(), SortField::DueDate);
    }
}

#[derive(Args)]
pub struct RelationCommand {
    #[command(subcommand)]
    pub action: RelationAction,
}

#[derive(Subcommand)]
pub enum RelationAction {
    /// Add parent → child edges between one parent and one or more children
    Add {
        /// Parent card UUID or identifier (e.g. KAN-2)
        parent: String,
        /// One or more child cards (UUID or identifier)
        #[arg(required = true, num_args = 1..)]
        children: Vec<String>,
    },
    /// Remove parent → child edges between one parent and one or more children
    Remove {
        /// Parent card UUID or identifier (e.g. KAN-2)
        parent: String,
        /// One or more child cards (UUID or identifier)
        #[arg(required = true, num_args = 1..)]
        children: Vec<String>,
    },
    /// List direct parents of a card
    Parents {
        /// Card UUID or identifier
        card: String,
        /// Sort key for the returned list
        #[arg(long, value_enum, default_value_t = SortKey::CardNumber)]
        sort: SortKey,
        /// Sort direction
        #[arg(long, value_enum, default_value_t = SortDir::Asc)]
        order: SortDir,
    },
    /// List direct children of a card
    Children {
        /// Card UUID or identifier
        card: String,
        /// Sort key for the returned list
        #[arg(long, value_enum, default_value_t = SortKey::CardNumber)]
        sort: SortKey,
        /// Sort direction
        #[arg(long, value_enum, default_value_t = SortDir::Asc)]
        order: SortDir,
    },
}

#[derive(Args)]
pub struct CardCreateArgs {
    /// Board UUID or name
    #[arg(long)]
    pub board: String,
    /// Column UUID or name
    #[arg(long)]
    pub column: String,
    #[arg(long)]
    pub title: String,
    #[arg(long)]
    pub description: Option<String>,
    #[arg(long)]
    pub priority: Option<String>,
    #[arg(long)]
    pub points: Option<u8>,
    #[arg(long)]
    pub due_date: Option<String>,
    /// Assign the new card to a sprint. Pass a UUID, name, or number to pick a
    /// specific sprint; pass without a value to use the board's sole active
    /// sprint (errors if zero or more than one active sprint exists).
    #[arg(long = "assign", short = 'a', num_args = 0..=1, default_missing_value = "")]
    pub assign_sprint: Option<String>,
}

#[derive(Args)]
pub struct CardListArgs {
    /// Board UUID or name
    #[arg(long)]
    pub board: Option<String>,
    /// Column UUID or name (scoped to --board if given, else searched globally)
    #[arg(long)]
    pub column: Option<String>,
    /// Sprint UUID, name, or number (scoped to --board if given, else searched globally)
    #[arg(long)]
    pub sprint: Option<String>,
    #[arg(long)]
    pub status: Option<String>,
    /// Show only archived cards (mutually exclusive with --include-archived).
    #[arg(long, conflicts_with = "include_archived")]
    pub archived: bool,
    /// Include archived cards alongside live ones.
    #[arg(long)]
    pub include_archived: bool,
    /// Sort key. When omitted, falls back to the board's `task_sort_field`
    /// (requires --board).
    #[arg(long, value_enum)]
    pub sort: Option<SortKey>,
    /// Sort direction. When omitted, falls back to the board's
    /// `task_sort_order` (requires --board).
    #[arg(long, value_enum)]
    pub order: Option<SortDir>,
    #[arg(long)]
    pub page: Option<u32>,
    #[arg(long)]
    pub page_size: Option<u32>,
}

#[derive(Args)]
pub struct CardUpdateArgs {
    /// Card UUID or identifier like KAN-5 or 5
    pub card: String,
    #[arg(long)]
    pub title: Option<String>,
    #[arg(long)]
    pub description: Option<String>,
    #[arg(long)]
    pub priority: Option<String>,
    #[arg(long)]
    pub status: Option<String>,
    #[arg(long)]
    pub points: Option<u8>,
    #[arg(long)]
    pub due_date: Option<String>,
    #[arg(long)]
    pub clear_due_date: bool,
}

// Sprint commands
#[derive(Args)]
pub struct SprintCommand {
    #[command(subcommand)]
    pub action: SprintAction,
}

#[derive(Subcommand)]
pub enum SprintAction {
    /// Create a new sprint
    Create {
        /// Board UUID or name
        #[arg(long)]
        board: String,
        #[arg(long)]
        prefix: Option<String>,
        #[arg(long)]
        name: Option<String>,
    },
    /// List sprints for a board
    List {
        /// Board UUID or name
        #[arg(long)]
        board: String,
        #[arg(long)]
        page: Option<u32>,
        #[arg(long)]
        page_size: Option<u32>,
    },
    /// Get a specific sprint by UUID, name, or number
    Get {
        /// Sprint UUID, name, or number
        sprint: String,
    },
    /// Update a sprint
    Update(SprintUpdateArgs),
    /// Activate a sprint by UUID, name, or number
    Activate {
        /// Sprint UUID, name, or number
        sprint: String,
        #[arg(long)]
        duration_days: Option<i32>,
    },
    /// Complete a sprint by UUID, name, or number
    Complete {
        /// Sprint UUID, name, or number
        sprint: String,
    },
    /// Cancel a sprint by UUID, name, or number
    Cancel {
        /// Sprint UUID, name, or number
        sprint: String,
    },
    /// Delete a sprint by UUID, name, or number
    Delete {
        /// Sprint UUID, name, or number
        sprint: String,
    },
    /// Carry over uncompleted cards from a completed sprint to a planning sprint
    CarryOver {
        /// Source sprint UUID, name, or number (must be completed)
        #[arg(long)]
        from: String,
        /// Target sprint UUID, name, or number (must be in planning; on same board as source)
        #[arg(long)]
        to: String,
    },
}

#[derive(Args)]
pub struct SprintUpdateArgs {
    /// Sprint UUID, name, or number
    pub sprint: String,
    #[arg(long)]
    pub name: Option<String>,
    #[arg(long)]
    pub prefix: Option<String>,
    #[arg(long)]
    pub card_prefix: Option<String>,
    #[arg(long)]
    pub start_date: Option<String>,
    #[arg(long)]
    pub end_date: Option<String>,
    #[arg(long)]
    pub clear_start_date: bool,
    #[arg(long)]
    pub clear_end_date: bool,
}

// Migrate command
#[derive(Args)]
#[command(after_help = "EXAMPLES:
    kanban migrate boards.json sqlite
    kanban migrate boards.json sqlite -o /path/to/output.sqlite
    kanban migrate boards.sqlite json -o boards.json
    kanban migrate data.bin json --source-backend sqlite")]
pub struct MigrateArgs {
    /// Path to source file
    pub source: String,
    /// Target backend name
    pub backend: String,
    /// Output path (default: derived from source filename and target backend)
    #[arg(long, short)]
    pub output: Option<String>,
    /// Override source backend auto-detection
    #[arg(long)]
    pub source_backend: Option<String>,
}

// Export/Import commands
#[derive(Args)]
pub struct ExportArgs {
    /// Board UUID or name; if omitted, exports all boards
    #[arg(long)]
    pub board: Option<String>,
}

#[derive(Args)]
pub struct ImportArgs {
    #[arg(long)]
    pub file: String,
}
