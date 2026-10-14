//! The one owner of the terminal: raw mode, the alternate screen, the keys, the query about an image protocol, and the bytes one writes.

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

    /// How long the query waits for the next bytes of an answer, in nanoseconds, which a terminal that answers nothing costs once.
    #[cfg(unix)]
    const PROBE_WAIT_NANOS: rustix::event::Nsecs = 300_000_000;
    /// How many bytes of an answer one read takes.
    #[cfg(unix)]
    const PROBE_CHUNK: usize = 256;

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

    /// Writes `query` and collects what the terminal answers until `complete` says the answer ended or a wait passes with nothing more, before any key is read.
    #[cfg(unix)]
    pub(super) fn probe(
        terminal: &mut DefaultTerminal,
        query: &[u8],
        complete: fn(&[u8]) -> bool,
    ) -> io::Result<Option<Vec<u8>>> {
        use rustix::event::{PollFd, PollFlags, Timespec, poll};
        use std::io::Write as _;

        let backend = terminal.backend_mut();
        backend.write_all(query)?;
        backend.flush()?;
        let input = rustix::stdio::stdin();
        let wait = Timespec {
            tv_sec: 0,
            tv_nsec: PROBE_WAIT_NANOS,
        };
        let mut answer = Vec::new();
        let mut chunk = [0; PROBE_CHUNK];
        while !complete(&answer) {
            let mut ready = [PollFd::from_borrowed_fd(input, PollFlags::IN)];
            if poll(&mut ready, Some(&wait))? == 0 {
                break;
            }
            let read = rustix::io::read(input, &mut chunk)?;
            if read == 0 {
                break;
            }
            answer.extend_from_slice(chunk.get(..read).unwrap_or_default());
        }
        Ok(Some(answer))
    }

    /// No query reaches a terminal outside Unix, where the variables alone name the protocol.
    #[cfg(not(unix))]
    pub(super) fn probe(
        _terminal: &mut DefaultTerminal,
        _query: &[u8],
        _complete: fn(&[u8]) -> bool,
    ) -> io::Result<Option<Vec<u8>>> {
        Ok(None)
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
    /// What the terminal answered to `query`, or none where no query could reach it.
    pub answer: Option<Vec<u8>>,
}

impl Session {
    /// Takes the terminal, raw mode on the alternate screen, and asks it `query`, whose answer `complete` recognizes as whole.
    ///
    /// # Errors
    ///
    /// Returns the error when the terminal refuses raw mode or the alternate screen, or cannot be asked.
    pub(super) fn open(query: &[u8], complete: fn(&[u8]) -> bool) -> io::Result<Self> {
        let mut terminal = raw::enter()?;
        let answer = raw::probe(&mut terminal, query, complete)?;
        Ok(Self { terminal, answer })
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
