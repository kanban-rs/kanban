use crate::theme::{focused_border, label_text, normal_text, unfocused_border};
use ratatui::{
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders},
};

pub struct FieldSectionConfig<'a> {
    pub title: &'a str,
    pub focused_title: &'a str,
    pub is_focused: bool,
}

impl<'a> FieldSectionConfig<'a> {
    pub fn new(title: &'a str) -> Self {
        Self {
            title,
            focused_title: title,
            is_focused: false,
        }
    }

    pub fn with_focus_indicator(mut self, focused_title: &'a str) -> Self {
        self.focused_title = focused_title;
        self
    }

    pub fn focused(mut self, focused: bool) -> Self {
        self.is_focused = focused;
        self
    }

    pub fn border_style(&self) -> Style {
        if self.is_focused {
            focused_border()
        } else {
            unfocused_border()
        }
    }

    pub fn title_text(&self) -> &str {
        if self.is_focused {
            self.focused_title
        } else {
            self.title
        }
    }

    pub fn block(&'a self) -> Block<'a> {
        Block::default()
            .title(self.title_text())
            .borders(Borders::ALL)
            .border_style(self.border_style())
    }
}

pub fn metadata_line<'a>(label: &'a str, value: impl Into<String>) -> Line<'a> {
    Line::from(vec![
        Span::styled(format!("{}: ", label), label_text()),
        Span::styled(value.into(), normal_text()),
    ])
}

pub fn metadata_line_styled<'a>(
    label: &'a str,
    value: impl Into<String>,
    style: Style,
) -> Line<'a> {
    Line::from(vec![
        Span::styled(format!("{}: ", label), label_text()),
        Span::styled(value.into(), style),
    ])
}

pub fn metadata_line_selectable<'a>(
    label: &'a str,
    value: impl Into<String>,
    selected: bool,
    highlight_bg: Color,
) -> Line<'a> {
    if selected {
        Line::from(vec![
            Span::styled(format!("{}: ", label), label_text().bg(highlight_bg)),
            Span::styled(value.into(), normal_text().bg(highlight_bg)),
        ])
    } else {
        metadata_line(label, value)
    }
}

pub fn metadata_line_multi<'a>(parts: Vec<(&'a str, String, Style)>) -> Line<'a> {
    let mut spans = Vec::new();
    for (i, (label, value, style)) in parts.into_iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw("  "));
        }
        spans.push(Span::styled(format!("{}: ", label), label_text()));
        spans.push(Span::styled(value, style));
    }
    Line::from(spans)
}
