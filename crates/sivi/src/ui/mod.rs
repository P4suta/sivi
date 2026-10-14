//! The terminal interface: the shell around `ui_rules`, which owns the terminal, the watcher, the build thread,
//! the viewer, and the pickers, and carries out what the machine decides.

mod app;
mod opener;
mod pickers;
mod preview;
mod terminal;
mod view;
mod watch;

pub use app::run;
