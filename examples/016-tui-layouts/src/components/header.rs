use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Flex, Layout, Rect},
    style::Stylize,
    text::Line,
    widgets::Widget,
};

pub struct Header<'a> {
    pub title: &'a str,
    pub environment: &'a str,
}

impl Widget for &Header<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // `Flex::SpaceBetween` – first item at the left edge, last at the right edge,
        // free space in between. Lengths are the exact text widths.
        let env = format!(" env: {} ", self.environment);
        let [title_area, env_area] = Layout::horizontal([
            Constraint::Length(self.title.len() as u16 + 2),
            Constraint::Length(env.len() as u16),
        ])
        .flex(Flex::SpaceBetween)
        .areas(area);

        Line::from(format!(" {} ", self.title))
            .bold()
            .reversed()
            .render(title_area, buf);
        Line::from(env).black().on_yellow().render(env_area, buf);
    }
}
