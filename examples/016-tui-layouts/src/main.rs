//! `platctl-console` – step 016: layouts and composing a screen from components.
//!
//! ```text
//! ┌ header (Length 1) ─────────────────────────────────────┐
//! ├ sidebar (Length 22) ┬ main: logs (Fill)                │
//! │                     │                                  │
//! ├ status bar (Length 1) ─────────────────────────────────┤
//! └────────────────────────────────────────────────────────┘
//! ```
//!
//! The layout is computed by a pure function (`layout`) – responsive rules (hide the sidebar on
//! narrow terminals, zoom mode) are testable without a terminal.

mod components;

use std::io;

use components::{Header, HelpPopup, LogPanel, Sidebar, StatusBar};
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::{
    DefaultTerminal, Frame,
    layout::{Constraint, Layout, Rect},
};

const ENVIRONMENTS: [&str; 3] = ["dev", "staging", "prod"];
const LOGS: [&str; 6] = [
    "10:00:00 level=info  service=billing msg=\"invoice created\"",
    "10:00:02 level=error service=billing msg=\"payment declined\"",
    "10:00:03 level=warn  service=search  msg=\"slow query\" ms=1840",
    "10:00:04 level=error service=gateway msg=\"upstream timeout\"",
    "10:00:05 level=info  service=billing msg=\"invoice created\"",
    "10:00:07 level=info  service=reports msg=\"export finished\"",
];

/// Below this width the sidebar is hidden automatically.
const MIN_WIDTH_FOR_SIDEBAR: u16 = 70;

/// Result of the layout computation: one `Rect` per region; `None` = region hidden.
#[derive(Debug, PartialEq, Eq)]
struct AppLayout {
    header: Option<Rect>,
    sidebar: Option<Rect>,
    main: Rect,
    status: Option<Rect>,
}

fn layout(area: Rect, sidebar_wanted: bool, zoom: bool) -> AppLayout {
    if zoom {
        // Zoom: the main panel takes everything.
        return AppLayout {
            header: None,
            sidebar: None,
            main: area,
            status: None,
        };
    }

    // Vertical split. Constraint kinds:
    //   Length(n)     exactly n cells          Min(n) / Max(n)   at least / at most n
    //   Percentage(p) p % of the space         Ratio(a, b)       a/b of the space
    //   Fill(w)       share of the REMAINING space, weighted (Fill(1), Fill(2) = 1:2)
    let [header, body, status] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Fill(1),
        Constraint::Length(1),
    ])
    .areas(area);

    // Responsive rule: user preference AND enough room.
    let show_sidebar = sidebar_wanted && area.width >= MIN_WIDTH_FOR_SIDEBAR;
    let (sidebar, main) = if show_sidebar {
        // Nested layout: split the body horizontally. `spacing(1)` – 1 cell gap between parts.
        let [sidebar, main] = Layout::horizontal([Constraint::Length(22), Constraint::Fill(1)])
            .spacing(1)
            .areas(body);
        (Some(sidebar), main)
    } else {
        (None, body)
    };

    AppLayout {
        header: Some(header),
        sidebar,
        main,
        status: Some(status),
    }
}

struct App {
    environment: usize,
    sidebar: bool,
    zoom: bool,
    help: bool,
    should_quit: bool,
}

impl App {
    fn handle_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,
            KeyCode::Char('j') | KeyCode::Down => {
                self.environment = (self.environment + 1).min(ENVIRONMENTS.len() - 1);
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.environment = self.environment.saturating_sub(1)
            }
            KeyCode::Char('s') => self.sidebar = !self.sidebar,
            KeyCode::Char('z') => self.zoom = !self.zoom,
            KeyCode::Char('?') => self.help = !self.help,
            _ => {}
        }
    }

    /// The screen is assembled from components; each gets its `Rect` from `layout`.
    fn render(&self, frame: &mut Frame) {
        let area = frame.area();
        let regions = layout(area, self.sidebar, self.zoom);
        let environment = ENVIRONMENTS[self.environment];

        if let Some(header) = regions.header {
            frame.render_widget(
                &Header {
                    title: "platctl console",
                    environment,
                },
                header,
            );
        }
        if let Some(sidebar) = regions.sidebar {
            frame.render_widget(
                &Sidebar {
                    environments: &ENVIRONMENTS,
                    selected: self.environment,
                },
                sidebar,
            );
        }
        frame.render_widget(
            &LogPanel {
                environment,
                lines: &LOGS,
            },
            regions.main,
        );
        if let Some(status) = regions.status {
            frame.render_widget(
                &StatusBar {
                    width: area.width,
                    height: area.height,
                    sidebar_visible: regions.sidebar.is_some(),
                },
                status,
            );
        }
        // Rendered last = drawn on top.
        if self.help {
            frame.render_widget(&HelpPopup, area);
        }
    }
}

fn run(terminal: &mut DefaultTerminal) -> io::Result<()> {
    let mut app = App {
        environment: 0,
        sidebar: true,
        zoom: false,
        help: false,
        should_quit: false,
    };
    while !app.should_quit {
        terminal.draw(|frame| app.render(frame))?;
        if let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
        {
            app.handle_key(key.code);
        }
    }
    Ok(())
}

fn main() -> io::Result<()> {
    ratatui::run(run)
}

#[cfg(test)]
mod tests {
    use ratatui::{Terminal, backend::TestBackend};

    use super::*;

    #[test]
    fn wide_terminal_has_all_regions() {
        let regions = layout(Rect::new(0, 0, 100, 30), true, false);

        assert_eq!(regions.header, Some(Rect::new(0, 0, 100, 1)));
        assert_eq!(regions.sidebar, Some(Rect::new(0, 1, 22, 28)));
        // 100 - 22 (sidebar) - 1 (spacing) = 77
        assert_eq!(regions.main, Rect::new(23, 1, 77, 28));
        assert_eq!(regions.status, Some(Rect::new(0, 29, 100, 1)));
    }

    #[test]
    fn narrow_terminal_hides_sidebar() {
        let regions = layout(Rect::new(0, 0, 60, 20), true, false);

        assert_eq!(regions.sidebar, None);
        assert_eq!(regions.main.width, 60);
    }

    #[test]
    fn zoom_gives_everything_to_main() {
        let area = Rect::new(0, 0, 100, 30);

        assert_eq!(layout(area, true, true).main, area);
    }

    #[test]
    fn help_popup_is_drawn_on_top() {
        let mut terminal = Terminal::new(TestBackend::new(80, 20)).unwrap();
        let app = App {
            environment: 2,
            sidebar: true,
            zoom: false,
            help: true,
            should_quit: false,
        };

        terminal.draw(|frame| app.render(frame)).unwrap();

        let screen: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect();
        assert!(screen.contains("env: prod"));
        assert!(screen.contains("toggle this help"));
    }
}
