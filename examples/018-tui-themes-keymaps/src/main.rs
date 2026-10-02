//! `platctl-browse` – step 018: themes, accessibility and configurable key bindings.
//!
//! ```text
//! platctl-browse [--theme dark|light|high-contrast|mono] [--keymap default|vim|emacs] [--keys FILE]
//! ```
//!
//! - all colors come from a `Theme` (semantic roles); NO_COLOR forces the monochrome theme,
//! - keys are translated to `Action`s by a `Keymap` (presets + overrides from a TOML file),
//! - the help screen is generated from the active keymap – it can never be out of date.

mod keymap;
mod theme;

use std::{collections::HashMap, io, path::PathBuf};

use anyhow::Context;
use clap::Parser;
use crossterm::event::{self, Event, KeyEventKind};
use keymap::{Action, Keymap, Preset};
use ratatui::{
    DefaultTerminal, Frame,
    layout::{Constraint, Layout},
    text::{Line, Span},
    widgets::{Block, Clear, List, ListState, Row, Table},
};
use serde::Deserialize;
use theme::{Theme, ThemeName};

/// Browse platform services with configurable theme and key bindings.
#[derive(Debug, Parser)]
#[command(name = "platctl-browse", version)]
struct Cli {
    /// Color theme
    #[arg(long, value_enum, default_value_t = ThemeName::Dark)]
    theme: ThemeName,

    /// Key binding preset
    #[arg(long, value_enum, default_value_t = Preset::Default)]
    keymap: Preset,

    /// TOML file with a preset and per-action key overrides (see keys.example.toml)
    #[arg(long)]
    keys: Option<PathBuf>,
}

/// Format of the key bindings file.
#[derive(Debug, Deserialize)]
struct KeysFile {
    preset: Option<Preset>,
    #[serde(default)]
    bindings: HashMap<Action, Vec<String>>,
}

struct App {
    theme: Theme,
    keymap: Keymap,
    keymap_name: String,
    services: Vec<(String, &'static str)>,
    list: ListState,
    help: bool,
    should_quit: bool,
}

impl App {
    fn apply(&mut self, action: Action) {
        let last = self.services.len() - 1;
        let current = self.list.selected().unwrap_or(0);
        let page = 10;
        let target = match action {
            Action::Up => current.saturating_sub(1),
            Action::Down => (current + 1).min(last),
            Action::Top => 0,
            Action::Bottom => last,
            Action::PageUp => current.saturating_sub(page),
            Action::PageDown => (current + page).min(last),
            Action::ToggleTheme => {
                self.theme = Theme::get(self.theme.name.next());
                current
            }
            Action::Help => {
                self.help = !self.help;
                current
            }
            Action::Search => current, // left as an exercise
            Action::Quit => {
                self.should_quit = true;
                current
            }
        };
        self.list.select(Some(target));
    }

    fn render(&mut self, frame: &mut Frame) {
        let theme = self.theme;
        // Paint the whole background first – light/dark themes must not depend on the
        // terminal's own background color.
        frame.render_widget(Block::new().style(theme.base), frame.area());

        let [main, status] =
            Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).areas(frame.area());

        let items = self.services.iter().map(|(name, state)| {
            // Status: color AND text (step 007 accessibility rule).
            let style = match *state {
                "running" => theme.ok,
                "degraded" => theme.warn,
                _ => theme.error,
            };
            Line::from(vec![
                Span::raw(format!("{name:<20}")),
                Span::styled(format!("{state:<10}"), style),
            ])
        });
        let list = List::new(items)
            .block(
                Block::bordered()
                    .border_style(theme.border)
                    .title(Span::styled(" Services ", theme.title)),
            )
            .highlight_style(theme.selection)
            .highlight_symbol("> ");
        frame.render_stateful_widget(list, main, &mut self.list);

        let pending = self.keymap.pending();
        let status_line = format!(
            " theme: {:?}  keymap: {}  {}| ? help ",
            self.theme.name,
            self.keymap_name,
            if pending.is_empty() {
                String::new()
            } else {
                format!("keys: {pending} ")
            }
        );
        frame.render_widget(Line::styled(status_line, theme.muted), status);

        if self.help {
            self.render_help(frame);
        }
    }

    /// Generated from the active keymap – custom bindings show up automatically.
    fn render_help(&self, frame: &mut Frame) {
        let theme = self.theme;
        let area = frame
            .area()
            .centered(Constraint::Length(50), Constraint::Length(14));
        let rows = Action::ALL.iter().map(|action| {
            Row::new([
                self.keymap.describe(*action),
                action.description().to_string(),
            ])
        });
        let table = Table::new(rows, [Constraint::Length(22), Constraint::Fill(1)])
            .style(theme.base)
            .block(
                Block::bordered()
                    .border_style(theme.title)
                    .title(Span::styled(" Key bindings ", theme.title)),
            );
        frame.render_widget(Clear, area);
        frame.render_widget(table, area);
    }
}

fn load_keymap(cli: &Cli) -> anyhow::Result<(Keymap, String)> {
    let Some(path) = &cli.keys else {
        return Ok((
            Keymap::preset(cli.keymap),
            format!("{:?}", cli.keymap).to_lowercase(),
        ));
    };
    let text =
        std::fs::read_to_string(path).with_context(|| format!("cannot read {}", path.display()))?;
    let file: KeysFile =
        toml::from_str(&text).with_context(|| format!("invalid key file {}", path.display()))?;
    let preset = file.preset.unwrap_or(cli.keymap);
    let keymap = Keymap::preset(preset)
        .with_overrides(&file.bindings)
        .map_err(anyhow::Error::msg)?;
    Ok((keymap, format!("{preset:?}+custom").to_lowercase()))
}

fn run(terminal: &mut DefaultTerminal, mut app: App) -> io::Result<()> {
    while !app.should_quit {
        terminal.draw(|frame| app.render(frame))?;
        if let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
            && let Some(action) = app.keymap.handle(key)
        {
            app.apply(action);
        }
    }
    Ok(())
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    // Load configuration BEFORE entering raw mode: errors print normally on the shell.
    let (keymap, keymap_name) = load_keymap(&cli)?;
    let no_color = std::env::var("NO_COLOR").is_ok_and(|v| !v.is_empty());

    let app = App {
        theme: Theme::resolve(cli.theme, no_color),
        keymap,
        keymap_name,
        services: (1..=40)
            .map(|i| {
                let state = match i % 7 {
                    0 => "stopped",
                    3 => "degraded",
                    _ => "running",
                };
                (format!("service-{i:02}"), state)
            })
            .collect(),
        list: ListState::default().with_selected(Some(0)),
        help: false,
        should_quit: false,
    };
    ratatui::run(|terminal| run(terminal, app))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use clap::CommandFactory;

    use super::*;

    #[test]
    fn cli_definition_is_valid() {
        Cli::command().debug_assert();
    }

    #[test]
    fn example_key_file_is_valid() {
        let text = include_str!("../keys.example.toml");
        let file: KeysFile = toml::from_str(text).unwrap();

        assert_eq!(file.preset, Some(Preset::Vim));
        let keymap = Keymap::preset(Preset::Vim)
            .with_overrides(&file.bindings)
            .unwrap();
        assert_eq!(keymap.describe(Action::Search), "/, C-f");
    }

    #[test]
    fn help_lists_active_bindings() {
        use ratatui::{Terminal, backend::TestBackend};

        let mut app = App {
            theme: Theme::get(ThemeName::Mono),
            keymap: Keymap::preset(Preset::Emacs),
            keymap_name: "emacs".into(),
            services: vec![("billing".into(), "running")],
            list: ListState::default().with_selected(Some(0)),
            help: true,
            should_quit: false,
        };
        let mut terminal = Terminal::new(TestBackend::new(60, 20)).unwrap();

        terminal.draw(|frame| app.render(frame)).unwrap();

        let screen: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect();
        assert!(screen.contains("C-x C-c"), "{screen}");
        assert!(screen.contains("M-<"));
    }
}
