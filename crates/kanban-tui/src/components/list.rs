use crate::theme::{active_item, normal_text, selected_item};
use ratatui::{
    style::Style,
    text::{Line, Span},
};

pub struct ListItemConfig {
    pub is_selected: bool,
    pub is_focused: bool,
    pub is_active: bool,
    pub is_multi_selected: bool,
}

impl ListItemConfig {
    pub fn new() -> Self {
        Self {
            is_selected: false,
            is_focused: false,
            is_active: false,
            is_multi_selected: false,
        }
    }

    pub fn selected(mut self, selected: bool) -> Self {
        self.is_selected = selected;
        self
    }

    pub fn focused(mut self, focused: bool) -> Self {
        self.is_focused = focused;
        self
    }

    pub fn active(mut self, active: bool) -> Self {
        self.is_active = active;
        self
    }

    pub fn multi_selected(mut self, multi_selected: bool) -> Self {
        self.is_multi_selected = multi_selected;
        self
    }

    pub fn item_style(&self) -> Style {
        let mut style = normal_text();

        if self.is_active {
            style = active_item();
        }

        if self.is_selected && self.is_focused {
            style = style.bg(selected_item(true).bg.unwrap());
        }

        style
    }

    pub fn item_prefix(&self) -> &'static str {
        if self.is_active {
            "● "
        } else if self.is_multi_selected {
            "► "
        } else {
            "  "
        }
    }
}

impl Default for ListItemConfig {
    fn default() -> Self {
        Self::new()
    }
}

pub fn styled_list_item(text: impl Into<String>, config: &ListItemConfig) -> Line<'static> {
    let prefix = config.item_prefix();
    let style = config.item_style();
    Line::from(Span::styled(format!("{}{}", prefix, text.into()), style))
}
