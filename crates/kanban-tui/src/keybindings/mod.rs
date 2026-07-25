pub mod board_detail;
pub mod card_detail;
pub mod card_list;
pub mod dialog_modes;
pub mod normal_mode;
pub mod registry;
pub mod settings;
pub mod sprint_detail;

pub use registry::KeybindingRegistry;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeybindingAction {
    NavigateDown,
    NavigateUp,
    NavigateLeft,
    NavigateRight,
    SelectItem,
    CreateCard,
    CreateBoard,
    CreateSprint,
    CreateColumn,
    RenameBoard,
    RenameColumn,
    EditCard,
    EditBoard,
    ToggleCompletion,
    AssignToSprint,
    ArchiveCard,
    RestoreCard,
    DeleteCard,
    MoveCardLeft,
    MoveCardRight,
    MoveColumnUp,
    MoveColumnDown,
    DeleteColumn,
    DeleteBoard,
    ExportBoard,
    ExportAll,
    ImportBoard,
    OrderCards,
    OrderBoards,
    ToggleSortOrder,
    ToggleFilter,
    ToggleHideAssigned,
    ToggleArchivedView,
    ToggleArchivedBoardsView,
    RestoreBoard,
    DeleteArchivedBoard,
    ToggleBoardsSortOrder,
    ToggleTaskListView,
    ToggleCardSelection,
    ClearCardSelection,
    SelectAllCards,
    SetCardPriority,
    SetSelectedCardsPriority,
    Search,
    ShowHelp,
    Escape,
    FocusPanel(usize),
    JumpToTop,
    JumpToBottom,
    JumpHalfViewportUp,
    JumpHalfViewportDown,
    ManageParents,
    ManageChildren,
    CarryOver,
    Undo,
    Redo,
    OpenSettings,
    ExportBoards,
    ConfirmPrefixCollision,
    RejectPrefixCollision,
    CancelPrefixCollision,
    ForceOverwriteConflict,
    TakeTheirsConflict,
    CancelConflictResolution,
    ReloadDiscardLocal,
    KeepLocalChanges,
    DismissExternalChange,
}

#[derive(Debug, Clone)]
pub struct Keybinding {
    pub key: String,
    pub short_description: String,
    pub description: String,
    pub action: KeybindingAction,
}

impl Keybinding {
    pub fn new(
        key: impl Into<String>,
        short_description: impl Into<String>,
        description: impl Into<String>,
        action: KeybindingAction,
    ) -> Self {
        Self {
            key: key.into(),
            short_description: short_description.into(),
            description: description.into(),
            action,
        }
    }
}

#[derive(Debug, Clone)]
pub struct KeybindingContext {
    pub name: String,
    pub bindings: Vec<Keybinding>,
}

impl KeybindingContext {
    pub fn new(name: impl Into<String>, bindings: Vec<Keybinding>) -> Self {
        Self {
            name: name.into(),
            bindings,
        }
    }
}

pub trait KeybindingProvider {
    fn get_context(&self) -> KeybindingContext;
}
