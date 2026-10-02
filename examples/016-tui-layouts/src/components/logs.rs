use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Stylize,
    text::Line,
    widgets::{Block, Paragraph, Widget, Wrap},
};

pub struct LogPanel<'a> {
    pub environment: &'a str,
    pub lines: &'a [&'a str],
}

impl Widget for &LogPanel<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let lines: Vec<Line> = self
            .lines
            .iter()
            .map(|line| {
                if line.contains("level=error") {
                    Line::from(*line).red()
                } else {
                    Line::from(*line)
                }
            })
            .collect();
        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .block(Block::bordered().title(format!(" Logs – {} ", self.environment)))
            .render(area, buf);
    }
}
