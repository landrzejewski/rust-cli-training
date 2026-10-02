# 014 – TUI basics: full redraw every frame (immediate mode)

## Goal

Understand how ratatui applications work: terminal setup and restore, the
state → render → event → update loop, and why redrawing everything each frame is cheap.

Example: `examples/014-tui-immediate-mode` – `platctl-scale`, change service replicas with the keyboard.

## Key concepts

### Terminal modes

| Mode | Normal shell | TUI |
|------|--------------|-----|
| cooked vs **raw** | line editing, echo, Ctrl+C = SIGINT | every key delivered immediately, no echo, Ctrl+C is just a key |
| main vs **alternate screen** | scrollback | separate full-screen buffer; shell content restored on exit |
| cursor | visible | usually hidden |

Leaving the terminal in raw mode after a crash breaks the shell (no echo; fix with `reset`).
`ratatui::init()` installs a **panic hook** that restores the terminal before the panic message.

```rust
fn main() -> io::Result<()> {
    ratatui::run(|terminal| app.run(terminal))   // init() + closure + restore()
}
```

### Immediate mode

```rust
while !app.should_quit {
    terminal.draw(|frame| app.render(frame))?;       // describe the WHOLE screen from state
    if let Event::Key(key) = event::read()?          // block until input
        && key.kind == KeyEventKind::Press { app.handle_key(key); }   // update state
}
```

- no retained widget tree: widgets are temporary values created in `render` and consumed,
- the screen is a pure function of the state → no "sync UI with model" bugs,
- ratatui double-buffers: compares the new frame with the previous one and writes only changed cells
  (in the example a replica change sends a few bytes, not the whole screen),
- resize = next `draw` with a new `frame.area()`.

Retained-mode alternatives (cursive) keep widget objects with their own state; immediate mode
(ratatui, egui, Dear ImGui) keeps state in your structs.

### Structure: state, update, view

```rust
struct App { services: Vec<(&str, u32)>, selected: usize, should_quit: bool }
impl App {
    fn handle_key(&mut self, key: KeyEvent) { ... }   // update – no I/O, testable
    fn render(&mut self, frame: &mut Frame) { ... }    // view – widgets from state
}
```

This is The Elm Architecture (TEA) in its simplest form; step 017 adds messages and async events.

### Widgets and frames

```rust
let block = Block::bordered().title(" platctl ".bold()).title_bottom(Line::from("q quit").dim());
frame.render_widget(Paragraph::new(lines).block(block), frame.area());
```

- `Frame::area()` – the full terminal `Rect`; layouts split it (step 016),
- text model: `Span` (styled string) → `Line` → `Text`,
- `Stylize` shortcuts: `.bold()`, `.dim()`, `.red()`, `.reversed()`.

### crossterm

Backend used by ratatui (default): raw mode, alternate screen, cursor, colors, and **events**
(`event::read()`, `event::poll(timeout)`); works on Unix and Windows.
On Windows key releases are reported too – filter `KeyEventKind::Press`.

## Example walkthrough

- `App` – state, `handle_key` (j/k/arrows, +/-, r, q), `render` (bordered block, highlighted
  selection, frame counter in the bottom border).
- `run` – the event loop; `main` – `ratatui::run`.
- tests – key handling; rendering into `TestBackend` (step 020 goes deeper).

## Try it

```bash
cargo test -p ex014-tui-immediate-mode
cargo run -p ex014-tui-immediate-mode       # j/k, +/-, r, q; resize the window while running
```

Observe: the frame counter grows with every key and resize; after quitting, the shell looks as before.

## Exercises

1. Add `p` that panics – check that the terminal is restored and the panic message is readable.
2. Use `event::poll(Duration::from_millis(250))` and increase replicas automatically every tick while `a` is toggled on.
3. Use `TerminalOptions { viewport: Viewport::Inline(8) }` (`ratatui::init_with_options`) to render inline below the prompt instead of the alternate screen.
