//! `platctl-incidents` – step 020: code structured for testing.
//!
//! - `App::handle_key` – state transitions, tested by feeding key sequences,
//! - `App::render` – drawing, tested by rendering into ratatui's `TestBackend` and comparing
//!   the result with a stored snapshot (`insta`),
//! - the binary's non-interactive commands (`list`, `ack`) – tested end-to-end with `assert_cmd`
//!   and `trycmd`.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    Frame,
    layout::{Constraint, Layout},
    style::{Color, Style, Stylize},
    text::Line,
    widgets::{Block, List, ListState, Paragraph, Wrap},
};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Incident {
    pub id: u32,
    pub service: &'static str,
    pub severity: Severity,
    pub title: &'static str,
    pub acknowledged: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Critical,
    Warning,
}

pub fn sample_incidents() -> Vec<Incident> {
    vec![
        Incident {
            id: 101,
            service: "billing",
            severity: Severity::Critical,
            title: "Payment provider returns 503",
            acknowledged: false,
        },
        Incident {
            id: 102,
            service: "search",
            severity: Severity::Warning,
            title: "p95 latency above 1s",
            acknowledged: false,
        },
        Incident {
            id: 103,
            service: "notifications",
            severity: Severity::Warning,
            title: "E-mail queue growing",
            acknowledged: true,
        },
    ]
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    List,
    Details,
}

pub struct App {
    pub incidents: Vec<Incident>,
    pub list: ListState,
    pub view: View,
    pub should_quit: bool,
}

impl App {
    pub fn new(incidents: Vec<Incident>) -> Self {
        Self {
            incidents,
            list: ListState::default().with_selected(Some(0)),
            view: View::List,
            should_quit: false,
        }
    }

    pub fn selected(&self) -> Option<&Incident> {
        self.list.selected().and_then(|i| self.incidents.get(i))
    }

    pub fn handle_key(&mut self, key: KeyEvent) {
        match (self.view, key.code) {
            (_, KeyCode::Char('q')) => self.should_quit = true,
            (_, KeyCode::Char('c')) if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.should_quit = true;
            }
            (View::List, KeyCode::Char('j') | KeyCode::Down) => self.list.select_next(),
            (View::List, KeyCode::Char('k') | KeyCode::Up) => self.list.select_previous(),
            (View::List, KeyCode::Enter) => self.view = View::Details,
            (View::Details, KeyCode::Esc) => self.view = View::List,
            (_, KeyCode::Char('a')) => {
                // Clamp: `select_next` may store an index past the end until the next render.
                let index = self
                    .list
                    .selected()
                    .unwrap_or(0)
                    .min(self.incidents.len() - 1);
                self.incidents[index].acknowledged = true;
            }
            _ => {}
        }
    }

    pub fn render(&mut self, frame: &mut Frame) {
        let [main, footer] =
            Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).areas(frame.area());
        match self.view {
            View::List => {
                let items = self.incidents.iter().map(|incident| {
                    let marker = if incident.acknowledged { "✔" } else { "!" };
                    let line = Line::from(format!(
                        "{marker} #{} {:<14} {}",
                        incident.id, incident.service, incident.title
                    ));
                    match (incident.acknowledged, incident.severity) {
                        (true, _) => line.dim(),
                        (false, Severity::Critical) => line.fg(Color::Red),
                        (false, Severity::Warning) => line.fg(Color::Yellow),
                    }
                });
                let list = List::new(items)
                    .block(Block::bordered().title(" Incidents "))
                    .highlight_style(Style::new().reversed())
                    .highlight_symbol("> ");
                frame.render_stateful_widget(list, main, &mut self.list);
            }
            View::Details => {
                let text = match self.selected() {
                    Some(incident) => format!(
                        "#{} {}\nservice:  {}\nseverity: {:?}\nstatus:   {}",
                        incident.id,
                        incident.title,
                        incident.service,
                        incident.severity,
                        if incident.acknowledged {
                            "acknowledged"
                        } else {
                            "open"
                        }
                    ),
                    None => "no incident selected".into(),
                };
                frame.render_widget(
                    Paragraph::new(text)
                        .wrap(Wrap { trim: true })
                        .block(Block::bordered().title(" Details ")),
                    main,
                );
            }
        }
        let help = match self.view {
            View::List => " j/k move  Enter details  a acknowledge  q quit ",
            View::Details => " Esc back  a acknowledge  q quit ",
        };
        frame.render_widget(Line::from(help).dim(), footer);
    }
}

/// Test helper: parses a compact key script like `"jj<enter>a<esc>"` into key events.
/// Lives in the library (not under `cfg(test)`) so integration tests can use it.
pub fn keys(script: &str) -> Vec<KeyEvent> {
    let mut events = Vec::new();
    let mut rest = script;
    while let Some(c) = rest.chars().next() {
        let (code, len) = if c == '<' {
            let end = rest.find('>').unwrap_or(rest.len() - 1);
            let code = match &rest[1..end] {
                "enter" => KeyCode::Enter,
                "esc" => KeyCode::Esc,
                "up" => KeyCode::Up,
                "down" => KeyCode::Down,
                other => panic!("unknown key <{other}>"),
            };
            (code, end + 1)
        } else {
            (KeyCode::Char(c), c.len_utf8())
        };
        events.push(KeyEvent::new(code, KeyModifiers::NONE));
        rest = &rest[len..];
    }
    events
}
