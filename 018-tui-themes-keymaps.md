# 018 – Color themes, accessibility, vim and emacs key bindings

## Goal

Make a TUI comfortable for different users: switchable color themes built from semantic roles,
accessible variants (high contrast, no color), keymap presets for vim/emacs muscle memory and
user-defined bindings – with a help screen that always matches the configuration.

Example: `examples/018-tui-themes-keymaps` – `platctl-browse --theme ... --keymap ... --keys FILE`.

## Key concepts

### Themes = semantic roles

```rust
pub struct Theme { base, border, title, selection, muted, ok, warn, error: Style }
let style = match state { "running" => theme.ok, "degraded" => theme.warn, _ => theme.error };
```

- widgets ask for a role, never for "red" → changing/adding a theme touches one file,
- paint the background explicitly (`Block::new().style(theme.base)`) – a light theme on a dark
  terminal background is unreadable otherwise,
- prefer the 16 named ANSI colors (users' terminal palettes remap them); truecolor (`Color::Rgb`)
  only when you also set the background,
- themes can be loaded from config (serde on a struct of color names) – the same layering as steps 005/011.

### Accessibility

| Need | Solution |
|------|----------|
| color blindness | never color-only: text/symbols next to colors; avoid red/green as the only distinction |
| low vision | high-contrast theme (white on black, bold, underline for errors) |
| `NO_COLOR`, monochrome terminals | `Mono` theme: meaning via bold / underline / reverse / dim |
| screen readers | TUIs are hard for them – offer a non-TUI mode (plain output, `-o json`) for every feature |
| keyboard-only | every action reachable by keyboard (mouse is optional, step 017) |
| discoverability | `?` help generated from the keymap, status line shows pending keys |

### Actions, not keys

```rust
enum Action { Up, Down, Top, Bottom, PageUp, PageDown, Search, ToggleTheme, Help, Quit }
if let Some(action) = app.keymap.handle(key) { app.apply(action); }
```

The app reacts to actions; the keymap is replaceable data.

### Presets

| Action | default | vim | emacs |
|--------|---------|-----|-------|
| down / up | ↓ ↑ | j k | C-n C-p |
| top / bottom | Home End | g g / G | M-< M-> |
| page down / up | PgDn PgUp | C-d C-u | C-v M-v |
| search | / | / | C-s |
| quit | q | q | C-x C-c |

### Key sequences

`g g`, `C-x C-c` – a pending buffer; on each key:
1. exact match → action, clear,
2. prefix of a longer binding → wait,
3. dead end → drop the oldest key and retry (`g j` still triggers `j`).

Show pending keys in the status line (`keys: g`), like vim's `showcmd`.

### Terminal key quirks

- uppercase letters may or may not carry `SHIFT` – normalize before matching,
- `<`, `?`, `/` often arrive with `SHIFT` – ignore it for symbols,
- `Ctrl+i` = Tab, `Ctrl+m` = Enter, `Ctrl+[` = Esc in legacy terminals; enable crossterm's
  keyboard enhancement flags (kitty protocol) for unambiguous keys where supported,
- Alt may arrive as an Esc prefix in some terminals.

### Configurable bindings

```toml
preset = "vim"
[bindings]
quit = ["q", "ctrl-c"]
search = ["/", "ctrl-f"]
```

Overrides replace all keys of an action; invalid key names are reported **before** entering raw mode
(errors print normally in the shell).

## Example walkthrough

- `src/theme.rs` – 4 themes, `next()`, `resolve()` with NO_COLOR; test: mono has no colors.
- `src/keymap.rs` – actions, presets, parser (`ctrl-x ctrl-c`, `shift-g`), normalization,
  sequence matching, overrides, `describe()` for help; tests for vim, emacs, broken sequences, overrides.
- `src/main.rs` – CLI options, key file loading, themed list, status line, generated help popup; tests.
- `keys.example.toml` – custom bindings.

## Try it

```bash
cargo test -p ex018-tui-themes-keymaps
cargo run -p ex018-tui-themes-keymaps -- --keymap vim          # j k, g g, G, C-d, t (theme), ? (help)
cargo run -p ex018-tui-themes-keymaps -- --keymap emacs        # C-n C-p, M-< M->, C-x C-c
cargo run -p ex018-tui-themes-keymaps -- --keys examples/018-tui-themes-keymaps/keys.example.toml
NO_COLOR=1 cargo run -p ex018-tui-themes-keymaps               # monochrome
cargo run -p ex018-tui-themes-keymaps -- --theme high-contrast
```

## Exercises

1. Load themes from TOML (`[themes.solarized] ok = "green" ...`) with serde and a color parser.
2. Implement `Search`: `/` opens an input line, typing filters the list, Esc cancels (input mode = a second keymap).
3. Support counts like vim: `5 j` moves five items down.
