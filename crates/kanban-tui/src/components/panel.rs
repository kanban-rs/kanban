use crate::theme::{focused_border, unfocused_border};
use ratatui::{
    layout::Rect,
    style::Style,
    text::Line,
    widgets::{Block, Borders, Paragraph},
    Frame,
};

pub struct PanelConfig<'a> {
    pub title: Line<'a>,
    pub focused_title: Line<'a>,
    pub is_focused: bool,
    pub custom_border_style: Option<Style>,
}

impl<'a> PanelConfig<'a> {
    pub fn new(title: impl Into<Line<'a>>) -> Self {
        let title = title.into();
        Self {
            focused_title: title.clone(),
            title,
            is_focused: false,
            custom_border_style: None,
        }
    }

    pub fn with_focus_indicator(mut self, focused_title: impl Into<Line<'a>>) -> Self {
        self.focused_title = focused_title.into();
        self
    }

    pub fn focused(mut self, focused: bool) -> Self {
        self.is_focused = focused;
        self
    }

    pub fn with_custom_border_style(mut self, style: Style) -> Self {
        self.custom_border_style = Some(style);
        self
    }

    pub fn border_style(&self) -> ratatui::style::Style {
        if let Some(style) = self.custom_border_style {
            style
        } else if self.is_focused {
            focused_border()
        } else {
            unfocused_border()
        }
    }

    pub fn title_line(&self) -> &Line<'a> {
        if self.is_focused {
            &self.focused_title
        } else {
            &self.title
        }
    }

    pub fn block(&self) -> Block<'a> {
        Block::default()
            .borders(Borders::ALL)
            .border_style(self.border_style())
            .title(self.title_line().clone())
    }
}

pub fn render_panel<'a>(
    frame: &mut Frame,
    area: Rect,
    config: &PanelConfig<'a>,
    content: Paragraph<'a>,
) {
    let widget = content.block(config.block());
    frame.render_widget(widget, area);
}
