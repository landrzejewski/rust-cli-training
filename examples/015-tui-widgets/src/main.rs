//! `platctl-dash` – step 015: a tour of ratatui's built-in widgets on a service dashboard.
//!
//! | Widget | Used for |
//! |--------|----------|
//! | `Block` | borders, titles, padding around other widgets |
//! | `Tabs` | page switcher (Overview / Deployments) |
//! | `List` (stateful) | service selector with highlight |
//! | `Table` (stateful) | deployments with header, column widths, selected row |
//! | `Paragraph` | wrapped text with styled spans |
//! | `Gauge` | CPU quota usage |
//! | `Sparkline` | compact latency history |
//! | `Chart` | request rate over time (line, axes, legend) |
//! | `BarChart` | errors per service |
//!
//! Keys: Tab switch page, ↑/↓ (j/k) select, q quit. Layout details: step 016.

use std::io;

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::{
    DefaultTerminal, Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    symbols,
    text::{Line, Span},
    widgets::{
        Axis, BarChart, Block, Cell, Chart, Dataset, Gauge, GraphType, List, ListState, Paragraph,
        Row, Sparkline, Table, TableState, Tabs, Wrap,
    },
};

struct Service {
    name: &'static str,
    version: &'static str,
    cpu: f64,
    errors: u64,
    owner: &'static str,
}

const SERVICES: [Service; 4] = [
    Service {
        name: "billing",
        version: "1.4.2",
        cpu: 0.63,
        errors: 12,
        owner: "payments",
    },
    Service {
        name: "search",
        version: "2.0.0",
        cpu: 0.81,
        errors: 3,
        owner: "discovery",
    },
    Service {
        name: "notifications",
        version: "0.9.3",
        cpu: 0.27,
        errors: 27,
        owner: "growth",
    },
    Service {
        name: "reports",
        version: "0.4.0",
        cpu: 0.05,
        errors: 0,
        owner: "analytics",
    },
];

const DEPLOYMENTS: [(&str, &str, &str, &str); 5] = [
    ("2026-10-01 17:45", "notifications", "0.9.3", "rolled back"),
    ("2026-09-30 12:00", "billing", "1.4.2", "succeeded"),
    ("2026-09-29 08:30", "search", "2.0.0", "succeeded"),
    ("2026-09-28 16:10", "billing", "1.4.1", "succeeded"),
    ("2026-08-14 09:00", "reports", "0.4.0", "succeeded"),
];

struct App {
    tab: usize,
    /// Stateful widgets keep their selection/scroll offset in a separate `*State` owned by YOU –
    /// the widget itself is still recreated every frame (immediate mode).
    services: ListState,
    deployments: TableState,
    should_quit: bool,
}

impl App {
    fn new() -> Self {
        Self {
            tab: 0,
            services: ListState::default().with_selected(Some(0)),
            deployments: TableState::default().with_selected(Some(0)),
            should_quit: false,
        }
    }

    fn selected(&self) -> &'static Service {
        &SERVICES[self.services.selected().unwrap_or(0)]
    }

    fn handle_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,
            KeyCode::Tab => self.tab = (self.tab + 1) % 2,
            // `select_next`/`select_previous` clamp to the item count during rendering.
            KeyCode::Down | KeyCode::Char('j') if self.tab == 0 => self.services.select_next(),
            KeyCode::Up | KeyCode::Char('k') if self.tab == 0 => self.services.select_previous(),
            KeyCode::Down | KeyCode::Char('j') => self.deployments.select_next(),
            KeyCode::Up | KeyCode::Char('k') => self.deployments.select_previous(),
            _ => {}
        }
    }

    fn render(&mut self, frame: &mut Frame) {
        let [tabs_area, body] =
            Layout::vertical([Constraint::Length(3), Constraint::Min(0)]).areas(frame.area());

        let tabs = Tabs::new(["Overview", "Deployments"])
            .select(self.tab)
            .highlight_style(Style::new().bold().yellow())
            .block(Block::bordered().title(" platctl dashboard (Tab, ↑↓, q) "));
        frame.render_widget(tabs, tabs_area);

        if self.tab == 0 {
            self.render_overview(frame, body);
        } else {
            self.render_deployments(frame, body);
        }
    }

    fn render_overview(&mut self, frame: &mut Frame, area: Rect) {
        let [left, right] =
            Layout::horizontal([Constraint::Length(24), Constraint::Min(0)]).areas(area);
        let [details, gauge, sparkline, charts] = Layout::vertical([
            Constraint::Length(5),
            Constraint::Length(3),
            Constraint::Length(4),
            Constraint::Min(8),
        ])
        .areas(right);
        let [chart_area, bars_area] =
            Layout::horizontal([Constraint::Percentage(60), Constraint::Percentage(40)])
                .areas(charts);

        // List – items + highlight; selection lives in `self.services`.
        let list = List::new(SERVICES.iter().map(|s| s.name))
            .block(Block::bordered().title(" Services "))
            .highlight_style(Style::new().add_modifier(Modifier::REVERSED))
            .highlight_symbol("▶ ");
        frame.render_stateful_widget(list, left, &mut self.services);

        let service = self.selected();

        // Paragraph – rich text: lines made of styled spans, wrapped to the width.
        let text = vec![
            Line::from(vec![
                Span::raw("Service: "),
                Span::from(service.name).bold(),
            ]),
            Line::from(vec![
                Span::raw("Version: "),
                Span::from(service.version).cyan(),
            ]),
            Line::from(format!(
                "Owner:   team {} – escalation via #{}-oncall",
                service.owner, service.owner
            )),
        ];
        frame.render_widget(
            Paragraph::new(text)
                .wrap(Wrap { trim: true })
                .block(Block::bordered().title(" Details ")),
            details,
        );

        // Gauge – a ratio 0.0..=1.0 with a label; color reflects the level.
        let color = match service.cpu {
            c if c > 0.75 => Color::Red,
            c if c > 0.5 => Color::Yellow,
            _ => Color::Green,
        };
        frame.render_widget(
            Gauge::default()
                .block(Block::bordered().title(" CPU quota "))
                .gauge_style(Style::new().fg(color))
                .ratio(service.cpu)
                .label(format!("{:.0}%", service.cpu * 100.0)),
            gauge,
        );

        // Sparkline – tiny bar history without axes; good for trends in little space.
        let latency = series(service.name, 60, 40.0, 25.0);
        let latency: Vec<u64> = latency.iter().map(|(_, y)| *y as u64).collect();
        frame.render_widget(
            Sparkline::default()
                .block(Block::bordered().title(" p95 latency (ms), last 60 min "))
                .data(&latency)
                .style(Style::new().magenta()),
            sparkline,
        );

        // Chart – datasets of (x, y) points, axes with bounds and labels, optional legend.
        let requests = series(service.name, 60, 120.0, 60.0);
        let datasets = vec![
            Dataset::default()
                .name("req/s")
                .marker(symbols::Marker::Braille)
                .graph_type(GraphType::Line)
                .style(Style::new().cyan())
                .data(&requests),
        ];
        let chart = Chart::new(datasets)
            .block(Block::bordered().title(" Requests "))
            .x_axis(
                Axis::default()
                    .title("min")
                    .bounds([0.0, 60.0])
                    .labels(["-60", "-30", "now"]),
            )
            .y_axis(
                Axis::default()
                    .bounds([0.0, 200.0])
                    .labels(["0", "100", "200"]),
            );
        frame.render_widget(chart, chart_area);

        // BarChart – labelled values; here errors per service.
        let bars: Vec<(&str, u64)> = SERVICES.iter().map(|s| (&s.name[..4], s.errors)).collect();
        frame.render_widget(
            BarChart::default()
                .block(Block::bordered().title(" Errors (24h) "))
                .bar_width(5)
                .bar_gap(1)
                .bar_style(Style::new().red())
                .data(bars.as_slice()),
            bars_area,
        );
    }

    fn render_deployments(&mut self, frame: &mut Frame, area: Rect) {
        let rows = DEPLOYMENTS.iter().map(|(time, service, version, result)| {
            let result_cell = if *result == "succeeded" {
                Cell::from(*result).green()
            } else {
                Cell::from(*result).red().bold()
            };
            Row::new([
                Cell::from(*time),
                Cell::from(*service),
                Cell::from(*version),
                result_cell,
            ])
        });
        // Column widths are constraints too (step 016).
        let widths = [
            Constraint::Length(17),
            Constraint::Length(15),
            Constraint::Length(8),
            Constraint::Min(10),
        ];
        let table = Table::new(rows, widths)
            .header(
                Row::new(["TIME", "SERVICE", "VERSION", "RESULT"])
                    .bold()
                    .underlined(),
            )
            .row_highlight_style(Style::new().reversed())
            .highlight_symbol("▶ ")
            .block(Block::bordered().title(" Recent deployments "));
        frame.render_stateful_widget(table, area, &mut self.deployments);
    }
}

/// Deterministic fake time series: smooth wave per service.
fn series(seed: &str, points: usize, base: f64, amplitude: f64) -> Vec<(f64, f64)> {
    let phase = seed.len() as f64;
    (0..points)
        .map(|i| {
            let x = i as f64;
            (x, base + amplitude * ((x / 7.0) + phase).sin())
        })
        .collect()
}

fn run(terminal: &mut DefaultTerminal) -> io::Result<()> {
    let mut app = App::new();
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

    fn screen(app: &mut App) -> String {
        let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
        terminal.draw(|frame| app.render(frame)).unwrap();
        let buffer = terminal.backend().buffer();
        (0..buffer.area.height)
            .map(|y| {
                (0..buffer.area.width)
                    .map(|x| buffer[(x, y)].symbol())
                    .collect::<String>()
                    + "\n"
            })
            .collect()
    }

    #[test]
    fn overview_shows_selected_service() {
        let mut app = App::new();
        app.handle_key(KeyCode::Down);

        let screen = screen(&mut app);

        assert!(screen.contains("▶ search"), "{screen}");
        assert!(screen.contains("Version: 2.0.0"));
        assert!(screen.contains("81%"));
    }

    #[test]
    fn tab_switches_to_deployments_table() {
        let mut app = App::new();
        app.handle_key(KeyCode::Tab);

        let screen = screen(&mut app);

        assert!(screen.contains("Recent deployments"));
        assert!(screen.contains("rolled back"));
    }
}
