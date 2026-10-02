//! Application state, update and view (The Elm Architecture).
//!
//! `update(event)` is synchronous and returns an optional `Command` – a side effect to run
//! (spawn a task). Keeping effects out of `update` makes it a pure state machine: unit tests feed
//! events and inspect state and commands, no runtime or terminal needed.

use std::collections::VecDeque;

use crossterm::event::{KeyCode, KeyEventKind, MouseButton, MouseEventKind};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Style, Stylize},
    text::Line,
    widgets::{Block, Gauge, List, ListState, Paragraph},
};

use crate::events::AppEvent;

pub const SERVICES: [&str; 4] = ["billing", "search", "notifications", "reports"];
const MAX_LOG_LINES: usize = 200;

/// Side effects requested by `update`, executed by the main loop.
#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    StartDeployment(String),
    Quit,
}

pub struct App {
    pub services: ListState,
    pub logs: VecDeque<String>,
    /// Lines scrolled up from the bottom of the log panel (mouse wheel / PgUp).
    pub log_scroll: u16,
    pub deployment: Option<(String, u16)>,
    pub ticks: u64,
    /// Where the service list was drawn in the last frame – needed to map mouse clicks to rows.
    pub list_area: Rect,
}

impl App {
    pub fn new() -> Self {
        Self {
            services: ListState::default().with_selected(Some(0)),
            logs: VecDeque::new(),
            log_scroll: 0,
            deployment: None,
            ticks: 0,
            list_area: Rect::default(),
        }
    }

    pub fn update(&mut self, event: AppEvent) -> Option<Command> {
        match event {
            AppEvent::Key(key) if key.kind == KeyEventKind::Press => match key.code {
                KeyCode::Char('q') | KeyCode::Esc => return Some(Command::Quit),
                KeyCode::Down | KeyCode::Char('j') => self.services.select_next(),
                KeyCode::Up | KeyCode::Char('k') => self.services.select_previous(),
                KeyCode::PageUp => self.log_scroll = self.log_scroll.saturating_add(5),
                KeyCode::PageDown => self.log_scroll = self.log_scroll.saturating_sub(5),
                KeyCode::Char('d') if self.deployment.is_none() => {
                    let service = SERVICES[self.services.selected().unwrap_or(0).min(3)];
                    self.deployment = Some((service.to_string(), 0));
                    return Some(Command::StartDeployment(service.to_string()));
                }
                _ => {}
            },
            AppEvent::Key(_) => {}
            AppEvent::Mouse(mouse) => match mouse.kind {
                MouseEventKind::Down(MouseButton::Left) => {
                    // Row inside the list = screen row - list top - 1 (border).
                    let area = self.list_area;
                    let inside = mouse.column > area.x
                        && mouse.column < area.right().saturating_sub(1)
                        && mouse.row > area.y
                        && mouse.row < area.bottom().saturating_sub(1);
                    if inside {
                        let index = usize::from(mouse.row - area.y - 1);
                        if index < SERVICES.len() {
                            self.services.select(Some(index));
                        }
                    }
                }
                MouseEventKind::ScrollUp => self.log_scroll = self.log_scroll.saturating_add(1),
                MouseEventKind::ScrollDown => self.log_scroll = self.log_scroll.saturating_sub(1),
                _ => {}
            },
            AppEvent::Resize => {}
            AppEvent::Tick => self.ticks += 1,
            AppEvent::Log(line) => {
                // Bounded buffer – a long-running TUI must not grow memory forever.
                if self.logs.len() == MAX_LOG_LINES {
                    self.logs.pop_front();
                }
                self.logs.push_back(line);
            }
            AppEvent::DeployProgress { service, percent } => {
                self.deployment = Some((service, percent));
            }
            AppEvent::DeployFinished { service } => {
                self.deployment = None;
                self.logs
                    .push_back(format!("deployment of {service} finished"));
            }
        }
        None
    }

    pub fn render(&mut self, frame: &mut Frame) {
        let [top, logs_area, footer] = Layout::vertical([
            Constraint::Length(8),
            Constraint::Fill(1),
            Constraint::Length(1),
        ])
        .areas(frame.area());
        let [list_area, deploy_area] =
            Layout::horizontal([Constraint::Length(24), Constraint::Fill(1)]).areas(top);

        self.list_area = list_area;
        let list = List::new(SERVICES)
            .block(Block::bordered().title(" Services (click / j k) "))
            .highlight_style(Style::new().reversed())
            .highlight_symbol("▶ ");
        frame.render_stateful_widget(list, list_area, &mut self.services);

        let deploy_block = Block::bordered().title(" Deployment (d) ");
        match &self.deployment {
            Some((service, percent)) => frame.render_widget(
                Gauge::default()
                    .block(deploy_block)
                    .percent(*percent)
                    .label(format!("{service}: {percent}%"))
                    .green(),
                deploy_area,
            ),
            None => frame.render_widget(
                Paragraph::new("idle – press d to deploy the selected service").block(deploy_block),
                deploy_area,
            ),
        }

        // Show the tail of the log buffer, shifted by the scroll offset.
        let visible = usize::from(logs_area.height.saturating_sub(2));
        let end = self.logs.len().saturating_sub(usize::from(self.log_scroll));
        let start = end.saturating_sub(visible);
        let lines: Vec<Line> = self
            .logs
            .range(start..end)
            .map(|l| Line::from(l.as_str()))
            .collect();
        frame.render_widget(
            Paragraph::new(lines).block(Block::bordered().title(format!(
                " Live logs ({} lines, scroll {}) – wheel / PgUp PgDn ",
                self.logs.len(),
                self.log_scroll
            ))),
            logs_area,
        );

        // A spinner driven by ticks – proves the UI is alive while nothing else happens.
        let spinner = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"][self.ticks as usize % 10];
        frame.render_widget(
            Line::from(format!(" {spinner} live  |  q quit")).dim(),
            footer,
        );
    }
}

#[cfg(test)]
mod tests {
    use crossterm::event::{KeyEvent, KeyModifiers, MouseEvent};

    use super::*;

    fn key(code: KeyCode) -> AppEvent {
        AppEvent::Key(KeyEvent::new(code, KeyModifiers::NONE))
    }

    #[test]
    fn deploy_key_returns_a_command_once() {
        let mut app = App::new();
        app.update(key(KeyCode::Down));

        assert_eq!(
            app.update(key(KeyCode::Char('d'))),
            Some(Command::StartDeployment("search".into()))
        );
        // While running, `d` is ignored.
        assert_eq!(app.update(key(KeyCode::Char('d'))), None);
    }

    #[test]
    fn progress_events_update_state() {
        let mut app = App::new();

        app.update(AppEvent::DeployProgress {
            service: "billing".into(),
            percent: 40,
        });
        assert_eq!(app.deployment, Some(("billing".into(), 40)));

        app.update(AppEvent::DeployFinished {
            service: "billing".into(),
        });
        assert_eq!(app.deployment, None);
    }

    #[test]
    fn click_selects_list_row() {
        let mut app = App::new();
        app.list_area = Rect::new(0, 0, 24, 8);

        app.update(AppEvent::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 5,
            row: 3, // border at row 0 -> third item
            modifiers: KeyModifiers::NONE,
        }));

        assert_eq!(app.services.selected(), Some(2));
    }

    #[test]
    fn log_buffer_is_bounded() {
        let mut app = App::new();
        for i in 0..(MAX_LOG_LINES + 50) {
            app.update(AppEvent::Log(i.to_string()));
        }

        assert_eq!(app.logs.len(), MAX_LOG_LINES);
        assert_eq!(app.logs.front().map(String::as_str), Some("50"));
    }
}
