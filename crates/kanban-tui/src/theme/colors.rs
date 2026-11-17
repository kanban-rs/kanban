use ratatui::style::Color;

pub const FOCUSED_BORDER: Color = Color::Cyan;
pub const INSERT_BORDER: Color = Color::Green;
pub const UNFOCUSED_BORDER: Color = Color::White;
pub const SELECTED_BG: Color = Color::Blue;

pub const ACTIVE_ITEM: Color = Color::Green;
pub const DONE_TEXT: Color = Color::DarkGray;
pub const NORMAL_TEXT: Color = Color::White;
pub const LABEL_TEXT: Color = Color::DarkGray;
pub const HIGHLIGHT_TEXT: Color = Color::Yellow;

pub const PRIORITY_CRITICAL: Color = Color::Red;
pub const PRIORITY_HIGH: Color = Color::LightRed;
pub const PRIORITY_MEDIUM: Color = Color::Yellow;
pub const PRIORITY_LOW: Color = Color::White;

pub const POINTS_1: Color = Color::Cyan;
pub const POINTS_2: Color = Color::Green;
pub const POINTS_3: Color = Color::Yellow;
pub const POINTS_4: Color = Color::LightMagenta;
pub const POINTS_5: Color = Color::Red;

pub const STATUS_ACTIVE: Color = Color::Green;
pub const STATUS_PLANNING: Color = Color::Yellow;
pub const STATUS_COMPLETED: Color = Color::Gray;
pub const STATUS_CANCELLED: Color = Color::Red;

pub const POPUP_BG: Color = Color::Black;
pub const ERROR_COLOR: Color = Color::Red;

pub const FLASH_DELETE: Color = Color::Red;
pub const FLASH_RESTORE: Color = Color::Blue;
