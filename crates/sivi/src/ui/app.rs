//! The shell of the interface: it feeds keys, build results, and file changes to the machine and carries out what the machine asks, while the view draws the state.

use super::pickers::Pickers;
use super::preview::{self, Protocol};
use super::terminal::{self, Key, Session};
use super::view;
use super::{opener, watch};
use crate::args::{Environment, FontsCommand, Rendering};
use crate::build::{self, Judged, Outcome, Request, Sink};
use crate::cli::{Run, SUCCESS_EXIT_CODE, USAGE_EXIT_CODE, default_output};
use crate::library;
use sivi_core::diagnostic::{Diagnostic, Severity, has_error};
use sivi_core::ir::{BlockBody, Deck, plain_text};
use sivi_core::mosaic::Raster;
use sivi_core::ui_rules::{Action, Effect, Event, Message, Picker, Screen, State, Verdict, step};
use sivi_typst::render;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};

/// The pixels per point a preview renders at: the slide's own size, enough for a terminal.
const PREVIEW_PIXELS_PER_POINT: f64 = 1.0;
/// How many ticks a note stays on screen.
const NOTE_TICKS: u8 = 30;
/// The extension of a deck, which `sivi ui` looks for in the working directory when no deck is named.
const DECK_EXTENSION: &str = "md";

/// One slide as the list shows it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct SlideRow {
    pub number: usize,
    pub master: String,
    pub title: String,
    pub errors: usize,
    pub warnings: usize,
}

/// What a build thread hands back.
pub(super) struct Done {
    pub outcome: Outcome,
    pub judged: Option<Judged>,
    pub slides: Vec<SlideRow>,
}

/// What reaches the shell from its threads and its watcher.
pub(super) enum Signal {
    Changed,
    Done(Box<Done>),
    Installed(String),
}

/// A page rendered for the preview.
pub(super) struct Rendered {
    pub raster: Raster,
    pub png: Vec<u8>,
}

/// Which image the terminal shows, so it is sent again only on a change.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Shown {
    page: usize,
    area: ratatui::layout::Rect,
}

/// The whole shell.
pub(super) struct App {
    pub machine: State,
    pub deck: PathBuf,
    pub protocol: Protocol,
    pub slides: Vec<SlideRow>,
    pub outcome: Option<Outcome>,
    pub judged: Option<Judged>,
    pub pages: BTreeMap<usize, Rendered>,
    pub note: Option<(String, u8)>,
    pub pickers: Pickers,
    pub theme_names: Vec<&'static str>,
    environment: Environment,
    sender: Sender<Signal>,
    receiver: Receiver<Signal>,
    /// The watcher, kept so it runs as long as the shell does.
    _watcher: Option<watch::Watch>,
    shown: Option<Shown>,
}

/// The title of a slide: the text of its first heading, or nothing.
fn title_of(slide: &sivi_core::ir::Slide) -> String {
    slide
        .blocks
        .iter()
        .find_map(|block| match &block.body {
            BlockBody::Heading(heading) => {
                Some(String::from(plain_text(&heading.inlines).as_str()))
            }
            BlockBody::Paragraph(_)
            | BlockBody::List(_)
            | BlockBody::Code(_)
            | BlockBody::Quote(_)
            | BlockBody::Image(_)
            | BlockBody::Table(_) => None,
        })
        .unwrap_or_default()
}

/// The slides of the deck text with their masters and their diagnostics counted.
fn slide_rows(deck: Option<&Deck>, diagnostics: &[Diagnostic]) -> Vec<SlideRow> {
    let Some(deck) = deck else {
        return Vec::new();
    };
    deck.slides
        .iter()
        .map(|slide| {
            let master = slide
                .master
                .as_ref()
                .or(deck.head.default_master.as_ref())
                .map_or_else(String::new, |name| String::from(name.as_str()));
            let on_slide = diagnostics.iter().filter(|diagnostic| {
                diagnostic
                    .slide
                    .as_ref()
                    .is_some_and(|at| at.number == slide.number)
            });
            let (mut errors, mut warnings): (usize, usize) = (0, 0);
            for diagnostic in on_slide {
                match diagnostic.severity() {
                    Severity::Error => errors = errors.saturating_add(1),
                    Severity::Warning => warnings = warnings.saturating_add(1),
                }
            }
            SlideRow {
                number: slide.number,
                master,
                title: title_of(slide),
                errors,
                warnings,
            }
        })
        .collect()
}

/// Builds the deck on its own thread and sends the result.
fn build_in_thread(
    deck: PathBuf,
    theme: Option<String>,
    fonts_dir: Option<PathBuf>,
    sender: Sender<Signal>,
) {
    std::thread::spawn(move || {
        let request = Request {
            deck: &deck,
            theme: theme.as_deref(),
            sink: Sink::File(default_output(&deck)),
            timestamp: None,
            fonts_dir,
        };
        let (outcome, judged) = build::judge(&request);
        let parsed = sivi_markdown::parse(outcome.deck_text.as_bytes());
        let slides = slide_rows(parsed.deck.as_ref(), &outcome.diagnostics);
        match sender.send(Signal::Done(Box::new(Done {
            outcome,
            judged,
            slides,
        }))) {
            Ok(()) | Err(_) => {}
        }
    });
}

/// Installs a family on its own thread and sends the first line the command printed.
fn install_in_thread(family: String, environment: Environment, sender: Sender<Signal>) {
    std::thread::spawn(move || {
        let run = library::run(
            FontsCommand::Install {
                families: vec![family],
                rendering: Rendering::default(),
            },
            &environment,
        );
        let text = if run.stderr.is_empty() {
            String::from_utf8_lossy(&run.stdout).into_owned()
        } else {
            run.stderr
        };
        let first = text.lines().next().unwrap_or("installed").to_owned();
        match sender.send(Signal::Installed(first)) {
            Ok(()) | Err(_) => {}
        }
    });
}

/// The one deck of the working directory, when exactly one `.md` file lies there.
fn only_deck(dir: &Path) -> Option<PathBuf> {
    let mut found = Vec::new();
    let Ok(entries) = crate::io::entries(dir) else {
        return None;
    };
    for entry in entries {
        if entry.extension().and_then(|extension| extension.to_str()) == Some(DECK_EXTENSION) {
            found.push(entry);
        }
    }
    match found.as_slice() {
        [one] => Some(one.clone()),
        _ => None,
    }
}

impl App {
    fn new(deck: PathBuf, environment: &Environment) -> Self {
        let (sender, receiver) = mpsc::channel();
        let dir = deck.parent().unwrap_or_else(|| Path::new("."));
        let notifier = sender.clone();
        let (watcher, unwatched) =
            match watch::start(dir, move || match notifier.send(Signal::Changed) {
                Ok(()) | Err(_) => {}
            }) {
                Ok(watcher) => (Some(watcher), None),
                Err(error) => (
                    None,
                    Some(format!("the directory cannot be watched: {error}")),
                ),
            };
        Self {
            machine: State::default(),
            protocol: preview::protocol(environment),
            slides: Vec::new(),
            outcome: None,
            judged: None,
            pages: BTreeMap::new(),
            note: unwatched.map(|text| (text, NOTE_TICKS)),
            pickers: Pickers::read(environment),
            theme_names: sivi_theme::BUILTINS
                .iter()
                .map(|theme| theme.name)
                .collect(),
            environment: environment.clone(),
            sender,
            receiver,
            _watcher: watcher,
            shown: None,
            deck,
        }
    }

    /// Feeds one event to the machine and carries out its effect.
    pub(super) fn event(&mut self, event: Event) {
        let stepped = step(self.machine, event);
        self.machine = stepped.state;
        if let Some(effect) = stepped.effect {
            self.effect(effect);
        }
    }

    fn note(&mut self, text: String) {
        self.note = Some((text, NOTE_TICKS));
    }

    fn effect(&mut self, effect: Effect) {
        match effect {
            Effect::StartBuild { theme } => {
                let name = theme
                    .and_then(|index| self.theme_names.get(index))
                    .map(|name| String::from(*name));
                build_in_thread(
                    self.deck.clone(),
                    name,
                    library::fonts_dir(&self.environment),
                    self.sender.clone(),
                );
            }
            Effect::OpenPdf => {
                if let Err(error) = opener::open(&default_output(&self.deck)) {
                    self.note(format!("the viewer could not start: {error}"));
                }
            }
            Effect::Install { font } => {
                let name = self.pickers.fonts.get(font).map(|entry| entry.name.clone());
                if let Some(family) = name {
                    self.note(format!("installing {family}…"));
                    install_in_thread(family, self.environment.clone(), self.sender.clone());
                }
            }
            Effect::Quit => {}
            Effect::Note(message) => self.note(String::from(match message {
                Message::BuildRunning => "a build is running",
                Message::NothingToOpen => "no PDF to open yet; build first",
                Message::WatchOn => "watching the deck's directory",
                Message::WatchOff => "watching stopped",
            })),
        }
    }

    fn signal(&mut self, signal: Signal) {
        match signal {
            Signal::Changed => self.event(Event::FileChanged),
            Signal::Done(done) => {
                let verdict = if has_error(&done.outcome.diagnostics) {
                    Verdict::Failed
                } else {
                    Verdict::Passed
                };
                self.slides = done.slides;
                self.machine.slides = self.slides.len();
                self.outcome = Some(done.outcome);
                self.judged = done.judged;
                self.pages.clear();
                self.shown = None;
                self.event(Event::BuildDone(verdict));
            }
            Signal::Installed(text) => self.note(text),
        }
    }

    /// The page of the selected slide, rendered on first use.
    pub(super) fn page(&mut self, index: usize) -> Option<&Rendered> {
        if !self.pages.contains_key(&index) {
            let judged = self.judged.as_ref()?;
            let raster = render::raster(&judged.compiled, index, PREVIEW_PIXELS_PER_POINT)?;
            let png = match render::png(&judged.compiled, index, PREVIEW_PIXELS_PER_POINT) {
                Ok(Some(png)) => png,
                Ok(None) | Err(_) => Vec::new(),
            };
            self.pages.insert(index, Rendered { raster, png });
        }
        self.pages.get(&index)
    }

    fn key(&mut self, key: Key) {
        let action = match key {
            Key::Char('q') | Key::Ctrl('c') => Some(Action::Quit),
            Key::Char('b') => Some(Action::Build),
            Key::Char('w') => Some(Action::ToggleWatch),
            Key::Char('j') | Key::Down => Some(Action::Next),
            Key::Char('k') | Key::Up => Some(Action::Previous),
            Key::Char('1') => Some(Action::ShowOverview),
            Key::Char('2') => Some(Action::ShowSlides),
            Key::Char('3') => Some(Action::ShowDiagnostics),
            Key::Char('?') => Some(Action::ShowHelp),
            Key::Char('t') => Some(self.pick(Picker::Theme)),
            Key::Char('f') => Some(self.pick(Picker::Font)),
            Key::Char('p') => Some(self.pick(Picker::Palette)),
            Key::Char('c') => Some(self.pick(Picker::CodeTheme)),
            Key::Enter => Some(Action::Select),
            Key::Esc => Some(Action::Back),
            Key::Char('o') => Some(Action::OpenPdf),
            Key::Char(_) | Key::Ctrl(_) | Key::Resize | Key::Other => None,
        };
        if key == Key::Resize {
            self.shown = None;
        }
        if let Some(action) = action {
            self.event(Event::Key(action));
        }
    }

    fn pick(&self, kind: Picker) -> Action {
        Action::Pick {
            kind,
            items: self.pickers.count(kind),
        }
    }

    fn tick(&mut self) {
        self.event(Event::Tick);
        if let Some((_, left)) = &mut self.note {
            *left = left.saturating_sub(1);
            if *left == 0 {
                self.note = None;
            }
        }
    }

    /// Sends the selected page to the terminal through its image protocol, when the frame shows a preview and the page or its place changed.
    fn place_image(&mut self, session: &mut Session) -> std::io::Result<()> {
        let Some(area) = view::preview_area(self, session.terminal.get_frame().area()) else {
            if self.shown.take().is_some() && self.protocol == Protocol::Kitty {
                session.write(&preview::kitty_clear())?;
            }
            return Ok(());
        };
        let page = self.machine.slide;
        if self.shown == Some(Shown { page, area }) {
            return Ok(());
        }
        let protocol = self.protocol;
        let Some(rendered) = self.page(page) else {
            return Ok(());
        };
        let Some((columns, rows)) =
            preview::fitted_cells(rendered.raster.width, rendered.raster.height, area)
        else {
            return Ok(());
        };
        let mut bytes = Vec::new();
        match protocol {
            Protocol::Kitty => {
                bytes.extend(preview::kitty_clear());
                bytes.extend(preview::move_to(area.x, area.y));
                bytes.extend(preview::kitty_show(&rendered.png, columns, rows));
            }
            Protocol::Iterm2 => {
                bytes.extend(preview::move_to(area.x, area.y));
                bytes.extend(preview::iterm2_show(&rendered.png, columns, rows));
            }
            Protocol::Mosaic => return Ok(()),
        }
        session.write(&bytes)?;
        self.shown = Some(Shown { page, area });
        Ok(())
    }

    fn drain(&mut self) {
        while let Ok(signal) = self.receiver.try_recv() {
            self.signal(signal);
        }
    }

    const fn running(&self) -> bool {
        !self.machine.quitting
    }

    /// The screen the machine shows.
    #[must_use]
    pub(super) const fn screen(&self) -> Screen {
        self.machine.screen
    }
}

fn session_loop(app: &mut App, session: &mut Session) -> std::io::Result<()> {
    app.event(Event::Key(Action::Build));
    while app.running() {
        session.terminal.draw(|frame| view::draw(frame, app))?;
        app.place_image(session)?;
        app.drain();
        match terminal::poll_key()? {
            Some(key) => app.key(key),
            None => app.tick(),
        }
    }
    Ok(())
}

/// Runs the interface over `deck`, or over the one deck of the working directory, until the person quits.
#[must_use]
pub fn run(deck: Option<&Path>, environment: &Environment) -> Run {
    let deck = match deck {
        Some(path) => path.to_path_buf(),
        None => match only_deck(Path::new(".")) {
            Some(found) => found,
            None => {
                return Run {
                    stdout: Vec::new(),
                    stderr: String::from(
                        "error: name the deck, since the working directory does not hold exactly one `.md` file\n",
                    ),
                    code: USAGE_EXIT_CODE,
                };
            }
        },
    };
    let mut app = App::new(deck, environment);
    let mut session = match Session::open() {
        Ok(session) => session,
        Err(error) => {
            return Run {
                stdout: Vec::new(),
                stderr: format!("error: the terminal cannot be taken: {error}\n"),
                code: USAGE_EXIT_CODE,
            };
        }
    };
    let ended = session_loop(&mut app, &mut session);
    drop(session);
    match ended {
        Ok(()) => Run {
            stdout: Vec::new(),
            stderr: String::new(),
            code: SUCCESS_EXIT_CODE,
        },
        Err(error) => Run {
            stdout: Vec::new(),
            stderr: format!("error: the terminal failed: {error}\n"),
            code: USAGE_EXIT_CODE,
        },
    }
}
