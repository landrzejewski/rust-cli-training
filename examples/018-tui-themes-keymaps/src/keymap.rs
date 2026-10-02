//! Keymaps: keys -> actions, with vim/emacs presets and user overrides.
//!
//! The UI handles ACTIONS (`MoveDown`), never raw keys. A keymap translates key presses
//! (including sequences like `g g`) into actions. Presets cover common muscle memory; a config
//! file can override single actions.

use std::collections::HashMap;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde::Deserialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Action {
    Up,
    Down,
    Top,
    Bottom,
    PageUp,
    PageDown,
    Search,
    ToggleTheme,
    Help,
    Quit,
}

impl Action {
    pub const ALL: [Action; 10] = [
        Action::Up,
        Action::Down,
        Action::Top,
        Action::Bottom,
        Action::PageUp,
        Action::PageDown,
        Action::Search,
        Action::ToggleTheme,
        Action::Help,
        Action::Quit,
    ];

    pub fn description(self) -> &'static str {
        match self {
            Self::Up => "previous item",
            Self::Down => "next item",
            Self::Top => "first item",
            Self::Bottom => "last item",
            Self::PageUp => "page up",
            Self::PageDown => "page down",
            Self::Search => "search",
            Self::ToggleTheme => "next theme",
            Self::Help => "toggle help",
            Self::Quit => "quit",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum Preset {
    /// Arrows, Home/End, PgUp/PgDn
    Default,
    /// j/k, gg/G, Ctrl+d/Ctrl+u, /
    Vim,
    /// Ctrl+n/Ctrl+p, Alt+</Alt+>, Ctrl+v/Alt+v, Ctrl+s
    Emacs,
}

/// One key press: code + modifiers, e.g. `ctrl-d`.
pub type Key = (KeyCode, KeyModifiers);

/// A binding: one or more keys pressed in sequence (`g g`).
pub type Sequence = Vec<Key>;

#[derive(Debug, Clone)]
pub struct Keymap {
    bindings: Vec<(Sequence, Action)>,
    /// Keys typed so far that are a PREFIX of some sequence (e.g. the first `g`).
    pending: Sequence,
}

impl Keymap {
    pub fn preset(preset: Preset) -> Self {
        // Arrows and Esc-style basics work in every preset – users expect them.
        let mut spec: Vec<(Action, &str)> = vec![
            (Action::Up, "up"),
            (Action::Down, "down"),
            (Action::PageUp, "pageup"),
            (Action::PageDown, "pagedown"),
            (Action::Help, "?"),
            (Action::ToggleTheme, "t"),
            (Action::Quit, "q"),
        ];
        match preset {
            Preset::Default => spec.extend([
                (Action::Top, "home"),
                (Action::Bottom, "end"),
                (Action::Search, "/"),
            ]),
            Preset::Vim => spec.extend([
                (Action::Down, "j"),
                (Action::Up, "k"),
                (Action::Top, "g g"),
                (Action::Bottom, "shift-g"),
                (Action::PageDown, "ctrl-d"),
                (Action::PageUp, "ctrl-u"),
                (Action::Search, "/"),
            ]),
            Preset::Emacs => spec.extend([
                (Action::Down, "ctrl-n"),
                (Action::Up, "ctrl-p"),
                (Action::Top, "alt-<"),
                (Action::Bottom, "alt->"),
                (Action::PageDown, "ctrl-v"),
                (Action::PageUp, "alt-v"),
                (Action::Search, "ctrl-s"),
                (Action::Quit, "ctrl-x ctrl-c"),
            ]),
        }
        let bindings = spec
            .into_iter()
            .map(|(action, text)| (parse_sequence(text).expect("valid preset key"), action))
            .collect();
        Self {
            bindings,
            pending: Vec::new(),
        }
    }

    /// User overrides REPLACE all bindings of the given action.
    pub fn with_overrides(
        mut self,
        overrides: &HashMap<Action, Vec<String>>,
    ) -> Result<Self, String> {
        for (action, keys) in overrides {
            self.bindings.retain(|(_, bound)| bound != action);
            for key in keys {
                self.bindings.push((parse_sequence(key)?, *action));
            }
        }
        Ok(self)
    }

    /// Feeds one key press. Returns an action when a full sequence matched.
    pub fn handle(&mut self, event: KeyEvent) -> Option<Action> {
        self.pending.push(normalize(event));
        loop {
            if let Some((_, action)) = self.bindings.iter().find(|(seq, _)| *seq == self.pending) {
                self.pending.clear();
                return Some(*action);
            }
            let is_prefix = self
                .bindings
                .iter()
                .any(|(seq, _)| seq.len() > self.pending.len() && seq.starts_with(&self.pending));
            if is_prefix {
                return None; // wait for the next key of the sequence
            }
            // Dead end (e.g. `g` then `j`): drop the oldest key and retry with the rest,
            // so `j` still works on its own.
            self.pending.remove(0);
            if self.pending.is_empty() {
                return None;
            }
        }
    }

    /// Human-readable bindings per action – for the help screen (discoverability).
    pub fn describe(&self, action: Action) -> String {
        self.bindings
            .iter()
            .filter(|(_, bound)| *bound == action)
            .map(|(seq, _)| seq.iter().map(format_key).collect::<Vec<_>>().join(" "))
            .collect::<Vec<_>>()
            .join(", ")
    }

    pub fn pending(&self) -> String {
        self.pending
            .iter()
            .map(format_key)
            .collect::<Vec<_>>()
            .join(" ")
    }
}

/// Terminals report modifiers inconsistently: `G` may come with or without SHIFT, `<` and `?`
/// usually with SHIFT. Normalize: uppercase letters always carry SHIFT, other characters never.
fn normalize(event: KeyEvent) -> Key {
    let mut modifiers =
        event.modifiers & (KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SHIFT);
    match event.code {
        KeyCode::Char(c) if c.is_ascii_uppercase() => modifiers.insert(KeyModifiers::SHIFT),
        KeyCode::Char(_) => modifiers.remove(KeyModifiers::SHIFT),
        _ => {}
    }
    (event.code, modifiers)
}

/// Parses `"ctrl-x ctrl-c"`, `"g g"`, `"shift-g"`, `"pagedown"`, `"/"`.
pub fn parse_sequence(text: &str) -> Result<Sequence, String> {
    text.split_whitespace().map(parse_key).collect()
}

fn parse_key(text: &str) -> Result<Key, String> {
    let mut modifiers = KeyModifiers::NONE;
    let mut rest = text;
    loop {
        if let Some(r) = rest.strip_prefix("ctrl-") {
            modifiers |= KeyModifiers::CONTROL;
            rest = r;
        } else if let Some(r) = rest.strip_prefix("alt-") {
            modifiers |= KeyModifiers::ALT;
            rest = r;
        } else if let Some(r) = rest.strip_prefix("shift-") {
            modifiers |= KeyModifiers::SHIFT;
            rest = r;
        } else {
            break;
        }
    }
    let code = match rest {
        "up" => KeyCode::Up,
        "down" => KeyCode::Down,
        "pageup" => KeyCode::PageUp,
        "pagedown" => KeyCode::PageDown,
        "home" => KeyCode::Home,
        "end" => KeyCode::End,
        "esc" => KeyCode::Esc,
        "enter" => KeyCode::Enter,
        single if single.chars().count() == 1 => {
            let c = single.chars().next().unwrap_or_default();
            if modifiers.contains(KeyModifiers::SHIFT) && c.is_ascii_alphabetic() {
                KeyCode::Char(c.to_ascii_uppercase())
            } else {
                KeyCode::Char(c)
            }
        }
        other => return Err(format!("unknown key `{other}` in `{text}`")),
    };
    // Same normalization as `normalize`: SHIFT only on uppercase letters among characters.
    if let KeyCode::Char(c) = code
        && !c.is_ascii_uppercase()
    {
        modifiers.remove(KeyModifiers::SHIFT);
    }
    Ok((code, modifiers))
}

fn format_key((code, modifiers): &Key) -> String {
    let mut text = String::new();
    if modifiers.contains(KeyModifiers::CONTROL) {
        text.push_str("C-");
    }
    if modifiers.contains(KeyModifiers::ALT) {
        text.push_str("M-");
    }
    match code {
        KeyCode::Char(c) => text.push(*c),
        KeyCode::Up => text.push('↑'),
        KeyCode::Down => text.push('↓'),
        KeyCode::PageUp => text.push_str("PgUp"),
        KeyCode::PageDown => text.push_str("PgDn"),
        KeyCode::Home => text.push_str("Home"),
        KeyCode::End => text.push_str("End"),
        other => text.push_str(&format!("{other:?}")),
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, modifiers)
    }

    fn ch(c: char) -> KeyEvent {
        key(KeyCode::Char(c), KeyModifiers::NONE)
    }

    #[test]
    fn vim_sequences_and_modifiers() {
        let mut keymap = Keymap::preset(Preset::Vim);

        assert_eq!(keymap.handle(ch('j')), Some(Action::Down));
        assert_eq!(keymap.handle(ch('g')), None, "prefix of `g g`");
        assert_eq!(keymap.pending(), "g");
        assert_eq!(keymap.handle(ch('g')), Some(Action::Top));
        assert_eq!(
            keymap.handle(key(KeyCode::Char('G'), KeyModifiers::SHIFT)),
            Some(Action::Bottom)
        );
        assert_eq!(
            keymap.handle(key(KeyCode::Char('d'), KeyModifiers::CONTROL)),
            Some(Action::PageDown)
        );
    }

    #[test]
    fn broken_sequence_restarts_with_last_key() {
        let mut keymap = Keymap::preset(Preset::Vim);

        keymap.handle(ch('g'));
        // `g` followed by `j`: not a sequence; `j` alone still works.
        assert_eq!(keymap.handle(ch('j')), Some(Action::Down));
    }

    #[test]
    fn emacs_multi_key_quit() {
        let mut keymap = Keymap::preset(Preset::Emacs);
        let ctrl = |c| key(KeyCode::Char(c), KeyModifiers::CONTROL);

        assert_eq!(keymap.handle(ctrl('n')), Some(Action::Down));
        assert_eq!(keymap.handle(ctrl('x')), None);
        assert_eq!(keymap.handle(ctrl('c')), Some(Action::Quit));
        assert_eq!(
            keymap.handle(key(KeyCode::Char('<'), KeyModifiers::ALT)),
            Some(Action::Top)
        );
    }

    #[test]
    fn overrides_replace_bindings_of_an_action() {
        let overrides = HashMap::from([(Action::Quit, vec!["ctrl-c".to_string()])]);
        let mut keymap = Keymap::preset(Preset::Default)
            .with_overrides(&overrides)
            .unwrap();

        assert_eq!(keymap.handle(ch('q')), None);
        assert_eq!(
            keymap.handle(key(KeyCode::Char('c'), KeyModifiers::CONTROL)),
            Some(Action::Quit)
        );
        assert_eq!(keymap.describe(Action::Quit), "C-c");
    }

    #[test]
    fn invalid_key_names_are_reported() {
        assert!(parse_sequence("ctrl-banana").is_err());
    }
}
