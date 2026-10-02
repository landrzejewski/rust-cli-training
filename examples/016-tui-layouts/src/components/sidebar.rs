use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Style, Stylize},
    widgets::{Block, List, Widget},
};

pub struct Sidebar<'a> {
    pub environments: &'a [&'a str],
    pub selected: usize,
}

impl Widget for &Sidebar<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let items = self.environments.iter().enumerate().map(|(index, env)| {
            if index == self.selected {
                format!("▶ {env}")
            } else {
                format!("  {env}")
            }
        });
        // Built-in widgets compose inside custom ones: render them into the given area/buffer.
        List::new(items)
            .style(Style::new())
            .block(Block::bordered().title(" Environments ".bold()))
            .render(area, buf);
    }
}
