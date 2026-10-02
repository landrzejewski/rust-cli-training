# 016 – Layouts with Constraint and Direction; composing views from components

## Goal

Split the terminal into regions with constraints, build responsive layouts, and structure a TUI
as small components instead of one giant `render` function.

Example: `examples/016-tui-layouts` – `platctl-console` with header, sidebar, log panel, status bar
and a help popup.

## Key concepts

### Layout

```rust
let [header, body, status] = Layout::vertical([          // Direction::Vertical
    Constraint::Length(1),
    Constraint::Fill(1),
    Constraint::Length(1),
]).areas(frame.area());                                    // [Rect; 3], checked at compile time

let [sidebar, main] = Layout::horizontal([Constraint::Length(22), Constraint::Fill(1)])
    .spacing(1)                                            // gap between parts
    .areas(body);                                          // nested layout
```

| Constraint | Meaning |
|------------|---------|
| `Length(n)` | exactly n cells (when possible) |
| `Min(n)` / `Max(n)` | at least / at most n |
| `Percentage(p)` | p % of the available space |
| `Ratio(a, b)` | a/b of the available space |
| `Fill(w)` | share of the remaining space by weight (`Fill(1)`, `Fill(2)` → 1 : 2) |

Constraints are solved with priorities (cassowary solver, results cached): fixed sizes first, then
fills. Other knobs: `.margin(n)`, `.spacing(n)`, `.flex(Flex::Start | Center | End | SpaceBetween | SpaceAround | SpaceEvenly)`
for distributing leftover space (header: title left, env badge right with `SpaceBetween`).

`Rect` helpers: `area.centered(Constraint::Length(40), Constraint::Length(9))` (popups),
`area.inner(Margin::new(1, 1))`, `Block::inner(area)` (content area inside borders).

### Responsive layout

Compute regions in a **pure function** of the terminal size and UI state:

```rust
fn layout(area: Rect, sidebar_wanted: bool, zoom: bool) -> AppLayout { ... }
let show_sidebar = sidebar_wanted && area.width >= 70;
```

- unit-testable with plain `Rect`s,
- design for 80×24 first; degrade gracefully (hide secondary panels, shorten labels),
- recomputed on every frame – a resize "just works".

### Components

```rust
pub struct Header<'a> { pub title: &'a str, pub environment: &'a str }

impl Widget for &Header<'_> {                     // implement for a reference → reusable, no move
    fn render(self, area: Rect, buf: &mut Buffer) {
        // compose built-in widgets / sub-layouts inside `area`
    }
}

frame.render_widget(&Header { title, environment }, regions.header);
```

- a component = data it displays + rendering; it gets its `Rect` from the parent,
- the parent owns the layout and the state; components are dumb and testable,
- overlays: render later = on top; `Clear` wipes the area under a popup,
- larger apps add a trait like `Component { handle_event, update, render }` per screen
  (see the ratatui "component" template: `cargo generate ratatui/templates`).

## Example walkthrough

- `src/main.rs` – `layout()` with vertical + nested horizontal split, responsive sidebar, zoom;
  `App` toggles (s, z, ?); render assembles components; tests for layout math and the popup.
- `src/components/` – `Header` (Flex::SpaceBetween), `Sidebar` (List inside), `LogPanel`
  (Paragraph, error lines red), `StatusBar` (size info), `HelpPopup` (centered + Clear).

## Try it

```bash
cargo test -p ex016-tui-layouts
cargo run -p ex016-tui-layouts      # resize below 70 columns: sidebar disappears; s, z, ?, j/k, q
```

## Exercises

1. Add a right-hand "details" panel with `Constraint::Percentage(30)` shown only above 120 columns.
2. Make the log panel use `Fill(2)` and a new metrics panel `Fill(1)` stacked vertically.
3. Turn `HelpPopup` into a generic `Popup { title, body }` component and use it for a confirmation dialog.
