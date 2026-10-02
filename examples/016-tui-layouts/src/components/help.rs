use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Rect},
    style::Stylize,
    widgets::{Block, Clear, Paragraph, Widget},
};

pub struct HelpPopup;

impl Widget for &HelpPopup {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Centered rectangle: fixed size, clamped to the available area.
        let popup = area.centered(Constraint::Length(40), Constraint::Length(9));
        // `Clear` erases what was drawn underneath – otherwise the popup would be "transparent".
        Clear.render(popup, buf);
        Paragraph::new(
            "j / k   previous / next environment\n\
             s       toggle sidebar\n\
             z       zoom logs (hide everything else)\n\
             ?       toggle this help\n\
             q       quit",
        )
        .block(Block::bordered().title(" Help ".bold()).yellow())
        .render(popup, buf);
    }
}
