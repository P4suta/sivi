//! The one owner of the terminal: raw mode, the alternate screen, the keys, and the bytes an image protocol writes.

use ratatui::DefaultTerminal;
use ratatui::crossterm::event::{self, Event as TermEvent, KeyCode, KeyEventKind, KeyModifiers};
use std::io::{self, Write as _};
use std::time::Duration;

/// How long the loop waits for a key before a tick passes.
const TICK: Duration = Duration::from_millis(100);

mod raw {
    #![expect(
        clippy::disallowed_methods,
        reason = "the interface's one terminal owner takes standard output in this statement alone"
    )]

    use ratatui::crossterm::execute;
    use ratatui::crossterm::terminal::{
        EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
    };
    use ratatui::{DefaultTerminal, Terminal, backend::CrosstermBackend};
    use std::io;

    pub(super) fn enter() -> io::Result<DefaultTerminal> {
        enable_raw_mode()?;
        let mut out = io::stdout();
        execute!(out, EnterAlternateScreen)?;
        Terminal::new(CrosstermBackend::new(out))
    }

    pub(super) fn leave(terminal: &mut DefaultTerminal) -> io::Result<()> {
        disable_raw_mode()?;
        execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
        terminal.show_cursor()
    }
}

/// A key the interface reads.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Key {
    Char(char),
    Ctrl(char),
    Enter,
    Esc,
    Up,
    Down,
    /// The terminal changed size, so the frame is drawn again.
    Resize,
    Other,
}

/// A terminal taken for the interface, which goes back to the shell when dropped.
pub(super) struct Session {
    pub terminal: DefaultTerminal,
}

impl Session {
    /// Takes the terminal: raw mode on the alternate screen.
    ///
    /// # Errors
    ///
    /// Returns the error when the terminal refuses raw mode or the alternate screen.
    pub(super) fn open() -> io::Result<Self> {
        Ok(Self {
            terminal: raw::enter()?,
        })
    }

    /// Writes `bytes` straight to the terminal, as an image protocol asks, and flushes them.
    ///
    /// # Errors
    ///
    /// Returns the error when the terminal is gone.
    pub(super) fn write(&mut self, bytes: &[u8]) -> io::Result<()> {
        let backend = self.terminal.backend_mut();
        backend.write_all(bytes)?;
        backend.flush()
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        match raw::leave(&mut self.terminal) {
            Ok(()) | Err(_) => {}
        }
    }
}

/// The next key within one tick, or none when the tick passes.
///
/// # Errors
///
/// Returns the error when the terminal's events cannot be read.
pub(super) fn poll_key() -> io::Result<Option<Key>> {
    if !event::poll(TICK)? {
        return Ok(None);
    }
    Ok(match event::read()? {
        TermEvent::Key(key) if key.kind == KeyEventKind::Press => Some(match key.code {
            KeyCode::Char(character) if key.modifiers.contains(KeyModifiers::CONTROL) => {
                Key::Ctrl(character)
            }
            KeyCode::Char(character) => Key::Char(character),
            KeyCode::Enter => Key::Enter,
            KeyCode::Esc => Key::Esc,
            KeyCode::Up => Key::Up,
            KeyCode::Down => Key::Down,
            KeyCode::Backspace
            | KeyCode::Left
            | KeyCode::Right
            | KeyCode::Home
            | KeyCode::End
            | KeyCode::PageUp
            | KeyCode::PageDown
            | KeyCode::Tab
            | KeyCode::BackTab
            | KeyCode::Delete
            | KeyCode::Insert
            | KeyCode::F(_)
            | KeyCode::Null
            | KeyCode::CapsLock
            | KeyCode::ScrollLock
            | KeyCode::NumLock
            | KeyCode::PrintScreen
            | KeyCode::Pause
            | KeyCode::Menu
            | KeyCode::KeypadBegin
            | KeyCode::Media(_)
            | KeyCode::Modifier(_) => Key::Other,
        }),
        TermEvent::Resize(_, _) => Some(Key::Resize),
        TermEvent::Key(_)
        | TermEvent::FocusGained
        | TermEvent::FocusLost
        | TermEvent::Mouse(_)
        | TermEvent::Paste(_) => Some(Key::Other),
    })
}
