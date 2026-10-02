use ratatui::{buffer::Buffer, layout::Rect, style::Stylize, text::Line, widgets::Widget};

pub struct StatusBar {
    pub width: u16,
    pub height: u16,
    pub sidebar_visible: bool,
}

impl Widget for &StatusBar {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let sidebar = if self.sidebar_visible { "on" } else { "off" };
        Line::from(format!(
            " {}x{}  sidebar: {sidebar}  |  j/k env  s sidebar  z zoom  ? help  q quit ",
            self.width, self.height
        ))
        .dim()
        .render(area, buf);
    }
}
