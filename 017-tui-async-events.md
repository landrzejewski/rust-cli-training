# 017 – Keyboard and mouse events; asynchronous event loop with tokio

## Goal

Keep a TUI responsive while data arrives in the background: merge terminal input, timers and
background tasks into one async event stream; handle mouse input; trigger async work from the UI.

Example: `examples/017-tui-async-events` – `platctl-live`: live log stream, deployments with
progress, mouse selection and scrolling.

## Key concepts

### Why async

A blocking `event::read()` loop (step 014) can't update the screen while waiting for a key.
Real tools need: streaming logs, polling APIs, progress of long operations, spinners.

### One channel, many sources

```text
crossterm EventStream ─┐
tick interval ─────────┼──► mpsc::Sender<AppEvent> ──► loop { draw; rx.recv().await; update }
background tasks ──────┘
```

```rust
enum AppEvent { Key(KeyEvent), Mouse(MouseEvent), Resize, Tick, Log(String),
                DeployProgress { service: String, percent: u16 }, DeployFinished { service: String } }

let mut events = EventStream::new();             // crossterm feature `event-stream`
tokio::select! {
    () = token.cancelled() => break,
    event = events.next() => ...,                // futures_util::StreamExt
}
```

- every source is a task sending into a **bounded** `mpsc` channel (backpressure),
- the main loop has a single await point – simple to reason about,
- `CancellationToken` (step 004) stops all tasks on quit; `child_token()` for per-operation tasks.

Alternative: `tokio::select!` directly in the main loop over all sources – fine for 2–3 sources,
the channel scales better.

### Update returns commands (TEA)

```rust
fn update(&mut self, event: AppEvent) -> Option<Command>     // pure, synchronous
enum Command { StartDeployment(String), Quit }

match app.update(event) {
    Some(Command::StartDeployment(s)) => events::spawn_deployment(s, tx.clone(), token.child_token()),
    ...
}
```

Effects (spawning, I/O) happen outside `update` → state transitions are unit-testable with
synthetic events. Results come back as new events (`DeployProgress`, `DeployFinished`).

### Keyboard

- `KeyEvent { code, modifiers, kind }`; check `kind == Press` (Windows reports releases),
- `KeyCode::Char('c')` + `KeyModifiers::CONTROL` – Ctrl+C arrives as a key in raw mode, not as SIGINT,
- keymaps and vim/emacs bindings: step 018.

### Mouse

```rust
execute!(io::stdout(), EnableMouseCapture)?;     // ... DisableMouseCapture on exit
MouseEventKind::Down(MouseButton::Left) | ScrollUp | ScrollDown | Drag(..) | Moved
```

- hit-testing needs the `Rect`s from the last render – store them in the state (`list_area`),
- mouse capture disables the terminal's native text selection (often Shift+drag still works) –
  consider making it optional,
- always offer keyboard equivalents.

### Ticks and redraws

A tick (100–250 ms) drives spinners and relative times. Redrawing on every event is fine with
ratatui's diffing; for heavy views redraw only when `update` changed something (dirty flag).

### Memory in long-running TUIs

Bounded buffers (`VecDeque` with a cap) for logs and history – a dashboard left open for a week
must not grow forever.

## Example walkthrough

- `src/events.rs` – `AppEvent`, input/tick sources, log streamer, deployment task.
- `src/app.rs` – state, `update` → `Option<Command>`, mouse hit-testing, bounded log buffer,
  scrolling, spinner; tests for commands, progress, click mapping, buffer bound.
- `src/main.rs` – tokio main, mouse capture on/off, channel + token wiring, command execution.

## Try it

```bash
cargo test -p ex017-tui-async-events
cargo run -p ex017-tui-async-events
# logs stream in; click a service; d = deploy with progress; mouse wheel / PgUp scroll; q quits
```

## Exercises

1. Add `c` to cancel a running deployment (keep its child token in the state).
2. Redraw only when `update` reports a change; measure CPU usage before and after (`top`).
3. Replace the simulated log streamer with lines read from a file being appended to (`tokio::fs` + polling).
