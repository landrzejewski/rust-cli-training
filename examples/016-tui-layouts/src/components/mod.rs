//! Components: small, reusable pieces of UI.
//!
//! Each component is a struct holding the data it shows and implements ratatui's `Widget`
//! trait for a reference (`impl Widget for &Header`), so it can be rendered like any built-in
//! widget: `frame.render_widget(&header, area)`. Components know nothing about WHERE they are
//! placed – the parent layout passes them a `Rect`.

mod header;
mod help;
mod logs;
mod sidebar;
mod status_bar;

pub use header::Header;
pub use help::HelpPopup;
pub use logs::LogPanel;
pub use sidebar::Sidebar;
pub use status_bar::StatusBar;
