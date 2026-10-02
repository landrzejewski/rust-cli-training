//! `platctl-scale` – step 014: the immediate-mode model of ratatui.
//!
//! Immediate mode: there are no persistent widget objects. Every frame, the WHOLE screen is
//! described again from the application state:
//!
//! ```text
//! loop {
//!     terminal.draw(|frame| render(&state, frame));   // 1. state -> widgets -> buffer
//!     let event = event::read()?;                     // 2. wait for input
//!     state.update(event);                            // 3. input -> new state
//! }
//! ```
//!
//! ratatui keeps the previous frame's buffer and writes only the cells that changed to the
//! terminal – redrawing "everything" is cheap. State lives in YOUR structs, not in widgets,
//! which makes the UI a pure function of the state: easy to reason about and to test.
//!
//! Keys: ↑/↓ or k/j select, +/- change replicas, r reset, q/Esc quit.

use std::io;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use ratatui::{
    DefaultTerminal, Frame,
    style::{Modifier, Style, Stylize},
    text::Line,
    widgets::{Block, Paragraph},
};

/// The whole application state.
#[derive(Debug)]
struct App {
    services: Vec<(&'static str, u32)>,
    selected: usize,
    /// Counts `draw` calls – shows that every input produces a complete new frame.
    frames: u64,
    should_quit: bool,
}

impl App {
    fn new() -> Self {
        Self {
            services: vec![("billing", 3), ("search", 5), ("notifications", 1)],
            selected: 0,
            frames: 0,
            should_quit: false,
        }
    }

    /// Update: input -> state. No drawing here, no I/O – unit-testable.
    fn handle_key(&mut self, key: KeyEvent) {
        let last = self.services.len() - 1;
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,
            KeyCode::Down | KeyCode::Char('j') => self.selected = (self.selected + 1).min(last),
            KeyCode::Up | KeyCode::Char('k') => self.selected = self.selected.saturating_sub(1),
            KeyCode::Char('+') => self.services[self.selected].1 += 1,
            KeyCode::Char('-') => {
                let replicas = &mut self.services[self.selected].1;
                *replicas = replicas.saturating_sub(1);
            }
            KeyCode::Char('r') => {
                *self = Self {
                    frames: self.frames,
                    ..Self::new()
                }
            }
            _ => {}
        }
    }

    /// View: state -> widgets. Called for EVERY frame; builds throw-away widget values.
    fn render(&mut self, frame: &mut Frame) {
        self.frames += 1;

        let lines: Vec<Line> = self
            .services
            .iter()
            .enumerate()
            .map(|(index, (name, replicas))| {
                let bar = "■".repeat(*replicas as usize);
                let line = Line::from(format!(" {name:<15} {replicas:>3}  {bar}"));
                if index == self.selected {
                    // Highlight is just a style decision made while rendering.
                    line.style(Style::new().add_modifier(Modifier::REVERSED))
                } else {
                    line
                }
            })
            .collect();

        // `Stylize` adds shortcuts like `.bold()`, `.dim()` on strings/spans/widgets.
        let block = Block::bordered()
            .title(" platctl – replicas ".bold())
            .title_bottom(Line::from(" ↑↓ select  +/- scale  r reset  q quit ").dim())
            .title_bottom(Line::from(format!(" frame #{} ", self.frames)).right_aligned());

        // A widget is rendered into a rectangle of the frame; `frame.area()` = whole terminal.
        frame.render_widget(Paragraph::new(lines).block(block), frame.area());
    }
}

/// The event loop.
fn run(terminal: &mut DefaultTerminal) -> io::Result<()> {
    let mut app = App::new();
    while !app.should_quit {
        terminal.draw(|frame| app.render(frame))?;
        // Blocking read: the loop sleeps until the next event (key, mouse, resize).
        // A resize just triggers the next `draw` with the new `frame.area()`.
        if let Event::Key(key) = event::read()?
            // On Windows, releases are reported too – react to presses only.
            && key.kind == KeyEventKind::Press
        {
            app.handle_key(key);
        }
    }
    Ok(())
}

fn main() -> io::Result<()> {
    // `ratatui::run`:
    //   1. `init()` – raw mode (keys arrive immediately, no echo, no line editing),
    //      alternate screen (the shell's content is restored afterwards),
    //      and a panic hook that restores the terminal BEFORE the panic message is printed,
    //   2. runs the closure with the terminal,
    //   3. `restore()` – leaves raw mode and the alternate screen.
    // Without restoring, a crashed TUI leaves the shell unusable (no echo -> `reset`).
    ratatui::run(run)
}

#[cfg(test)]
mod tests {
    use crossterm::event::KeyModifiers;
    use ratatui::{Terminal, backend::TestBackend};

    use super::*;

    fn press(app: &mut App, code: KeyCode) {
        app.handle_key(KeyEvent::new(code, KeyModifiers::NONE));
    }

    #[test]
    fn keys_change_state() {
        let mut app = App::new();

        press(&mut app, KeyCode::Down);
        press(&mut app, KeyCode::Char('+'));
        press(&mut app, KeyCode::Char('q'));

        assert_eq!(app.services[1], ("search", 6));
        assert!(app.should_quit);
    }

    #[test]
    fn view_is_a_function_of_state() {
        // TestBackend – an in-memory terminal; more in step 020.
        let mut terminal = Terminal::new(TestBackend::new(40, 6)).unwrap();
        let mut app = App::new();
        press(&mut app, KeyCode::Char('-'));

        terminal.draw(|frame| app.render(frame)).unwrap();

        let screen: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(screen.contains("billing           2  ■■"), "{screen}");
        assert!(screen.contains("frame #1"));
    }
}
