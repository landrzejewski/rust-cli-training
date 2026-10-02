# 015 – Widgets: Block, List, Table, Paragraph, Chart, Gauge and more

## Goal

Know ratatui's built-in widgets, how they are configured, and how stateful widgets (List, Table)
keep selection and scrolling.

Example: `examples/015-tui-widgets` – `platctl-dash`, a service dashboard with two tabs.

## Key concepts

### Widget catalogue

| Widget | Typical use | Key API |
|--------|-------------|---------|
| `Block` | borders, titles (top/bottom, aligned), padding; wraps other widgets via `.block()` | `Block::bordered().title(..).title_bottom(..)` |
| `Paragraph` | text, styled spans, wrapping, scrolling | `.wrap(Wrap { trim: true })`, `.scroll((y, x))`, `.alignment(..)` |
| `List` | selectable items | `.highlight_style(..)`, `.highlight_symbol("▶ ")` + `ListState` |
| `Table` | rows × columns with header | `Table::new(rows, widths).header(..).row_highlight_style(..)` + `TableState` |
| `Tabs` | page switcher | `.select(i)`, `.highlight_style(..)` |
| `Gauge` / `LineGauge` | progress, quotas | `.ratio(0.63)` or `.percent(63)`, `.label(..)` |
| `Sparkline` | compact trend | `.data(&[u64])` |
| `Chart` | line/scatter plots with axes and legend | `Dataset` + `Axis::bounds/labels`, `Marker::Braille` |
| `BarChart` | categorical values | `.data(&[("label", 12)])`, `.bar_width(..)` |
| `Scrollbar`, `Canvas`, `Calendar`, `Clear` | scroll indicators, free drawing, dates, popups |

All implement the `Widget` trait (consumed by `render`); some also `StatefulWidget`.

### Stateless vs stateful

```rust
frame.render_widget(Paragraph::new(text), area);                       // stateless

let list = List::new(items).highlight_symbol("▶ ");
frame.render_stateful_widget(list, area, &mut app.services);           // ListState owned by the app
app.services.select_next();                                            // in the update function
```

The widget is recreated every frame; the `*State` (selected index, scroll offset) persists in your
app struct – consistent with immediate mode (step 014). `select_next`/`select_previous` are
clamped to the number of items at render time.

### Text model

`Span` (string + style) → `Line` (spans) → `Text` (lines). Build rich content without manual
escape codes: `Line::from(vec![Span::raw("Version: "), Span::from(v).cyan()])`.
`Stylize` adds `.bold()`, `.cyan()`, `.reversed()`, `.underlined()` to strings, spans, lines, cells, rows.

### Choosing widgets for CLI dashboards

- tables for exact values (status, versions), charts/sparklines for trends,
- gauges for bounded ratios; color thresholds plus the number (accessibility, step 018),
- text labels on bars (`12`, `3`) – a bar alone is hard to read in a terminal,
- terminals are small: 80×24 is still a common size; test with small windows.

Custom widgets: implement `Widget for &MyThing` (`fn render(self, area: Rect, buf: &mut Buffer)`) – step 016.
More widgets: `tui-tree-widget`, `tui-textarea`, `tui-logger`, `ratatui-image` (ecosystem crates).

## Example walkthrough

- `App` – tab index, `ListState`, `TableState`; key handling per tab.
- `render_overview` – List, Paragraph, Gauge (threshold colors), Sparkline, Chart (Braille line,
  axes), BarChart; `render_deployments` – Table with header, styled cells, highlighted row.
- `series` – deterministic fake metrics.
- tests – render into a 100×30 `TestBackend` and assert visible content.

## Try it

```bash
cargo test -p ex015-tui-widgets
cargo run -p ex015-tui-widgets        # ↑↓ selects a service (all panels follow), Tab = deployments table
```

## Exercises

1. Add a `Scrollbar` to the deployments table and generate 100 rows.
2. Add a second dataset ("errors/s") to the chart with a different color and marker.
3. Show a popup with service details on Enter (`Clear` widget + a centered `Rect`).
