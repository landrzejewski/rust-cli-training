//! TUI tests: state via key scripts, rendering via `TestBackend` + insta snapshots.
//!
//! Snapshots live in `tests/snapshots/*.snap` and are committed. When rendering changes:
//!   cargo insta test -p ex020-tui-testing     (or: INSTA_UPDATE=always cargo test)
//!   cargo insta review                        (accept/reject each diff)

use ex020_tui_testing::{App, View, keys, sample_incidents};
use ratatui::{Terminal, backend::TestBackend, style::Color};

fn app_after(script: &str) -> App {
    let mut app = App::new(sample_incidents());
    for key in keys(script) {
        app.handle_key(key);
    }
    app
}

/// Renders into an in-memory terminal of a fixed size – deterministic, no TTY needed.
fn render(app: &mut App, width: u16, height: u16) -> Terminal<TestBackend> {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal.draw(|frame| app.render(frame)).unwrap();
    terminal
}

// --- state: keys in, state out --------------------------------------------------------------

#[test]
fn navigation_and_acknowledge() {
    let app = app_after("ja");

    assert_eq!(app.list.selected(), Some(1));
    assert!(app.incidents[1].acknowledged);
    assert!(!app.incidents[0].acknowledged);
}

#[test]
fn details_view_and_back() {
    assert_eq!(app_after("j<enter>").view, View::Details);
    assert_eq!(app_after("j<enter><esc>").view, View::List);
}

#[test]
fn navigation_keys_are_ignored_in_details_view() {
    let app = app_after("<enter>jjj");

    assert_eq!(app.list.selected(), Some(0));
}

// --- rendering: snapshots ------------------------------------------------------------------

#[test]
fn list_view_snapshot() {
    let mut app = app_after("j");
    let terminal = render(&mut app, 60, 7);

    // `TestBackend` implements `Display`: the screen as text, one line per row.
    // First run creates `tests/snapshots/tui__list_view_snapshot.snap`; later runs compare.
    insta::assert_snapshot!(terminal.backend());
}

#[test]
fn details_view_snapshot() {
    let mut app = app_after("<enter>a");
    let terminal = render(&mut app, 60, 8);

    insta::assert_snapshot!(terminal.backend());
}

// --- rendering: targeted assertions (styles are not part of the text snapshot) ------------

#[test]
fn critical_open_incident_is_red() {
    let mut app = app_after("j"); // move the highlight away from the first row
    let terminal = render(&mut app, 60, 7);

    // Row 1 = first list item (row 0 is the border). Cell (column, row).
    let cell = &terminal.backend().buffer()[(5, 1)];
    assert_eq!(cell.fg, Color::Red);
}
