//! The state machine of the terminal interface: which screen shows, whether a build runs, what each key asks for,
//! and which effect the shell carries out.
//!
//! The shell owns every effect; this module decides.
//! `specs/ui.qnt` models the same machine, and `crates/sivi/tests/ui_replay.rs` replays its traces through [`step`].

/// Whether the interface can own both streams.
#[must_use]
pub const fn terminal_available(input: bool, output: bool) -> bool {
    input && output
}

/// What the terminal shows.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash)]
#[cfg_attr(kani, derive(kani::Arbitrary))]
pub enum Screen {
    /// The deck at a glance: its theme, its slides, and the last verdict.
    #[default]
    Overview,
    /// The slides with the selected one previewed.
    Slides,
    /// The diagnostics of the selected slide.
    Diagnostics,
    /// A list to pick from.
    Picker(Picker),
    /// The keys.
    Help,
}

/// What a picker lists.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
#[cfg_attr(kani, derive(kani::Arbitrary))]
pub enum Picker {
    Theme,
    Font,
    Palette,
    CodeTheme,
}

/// Whether a build runs.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash)]
#[cfg_attr(kani, derive(kani::Arbitrary))]
pub enum Job {
    #[default]
    Idle,
    Building,
}

/// How the last build ended.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
#[cfg_attr(kani, derive(kani::Arbitrary))]
pub enum Verdict {
    Passed,
    Failed,
}

/// What a key asks for.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
#[cfg_attr(kani, derive(kani::Arbitrary))]
pub enum Action {
    Quit,
    Build,
    ToggleWatch,
    Next,
    Previous,
    ShowOverview,
    ShowSlides,
    ShowDiagnostics,
    ShowHelp,
    /// Opens a picker over `items` entries, which the shell counts.
    Pick {
        kind: Picker,
        items: usize,
    },
    Select,
    Back,
    OpenPdf,
}

/// What reaches the machine.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
#[cfg_attr(kani, derive(kani::Arbitrary))]
pub enum Event {
    Key(Action),
    BuildDone(Verdict),
    FileChanged,
    Tick,
}

/// A note the shell shows for a moment.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum Message {
    BuildRunning,
    NothingToOpen,
    WatchOn,
    WatchOff,
}

/// What the shell carries out after a step.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum Effect {
    /// Builds the deck, under the carried theme at the index when one is chosen.
    StartBuild {
        theme: Option<usize>,
    },
    OpenPdf,
    /// Installs the font family at the index of the catalog listing.
    Install {
        font: usize,
    },
    Quit,
    Note(Message),
}

/// The whole state of the interface.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash)]
#[cfg_attr(kani, derive(kani::Arbitrary))]
pub struct State {
    pub screen: Screen,
    pub job: Job,
    /// Whether a change of the deck starts a build.
    pub watch: bool,
    /// Whether a change arrived while a build ran, so another follows.
    pub pending: bool,
    pub quitting: bool,
    /// The selected slide and how many there are.
    pub slide: usize,
    pub slides: usize,
    /// The selected entry of the open picker and how many it lists.
    pub cursor: usize,
    pub items: usize,
    /// The carried theme chosen in the picker, or none for the deck's own.
    pub theme: Option<usize>,
    pub last: Option<Verdict>,
    /// The phase of the spinner while a build runs.
    pub tick: u8,
}

/// How many phases the spinner cycles through.
pub const SPINNER: u8 = 4;

/// What one step leaves: the state after it and the effect the shell carries out, if any.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct Step {
    pub state: State,
    pub effect: Option<Effect>,
}

/// The largest index a selection over `count` entries may take, which is zero for none.
const fn last_index(count: usize) -> usize {
    count.saturating_sub(1)
}

/// The state with its selections held within their counts.
const fn clamped(mut state: State) -> State {
    state.slide = lower(state.slide, last_index(state.slides));
    state.cursor = lower(state.cursor, last_index(state.items));
    state
}

/// The lower of two counts, written without a comparison so no mutant of one survives as its equal.
const fn lower(first: usize, second: usize) -> usize {
    first.saturating_sub(first.saturating_sub(second))
}

/// A build starts when none runs; otherwise the note says one runs.
const fn start(mut state: State) -> Step {
    match state.job {
        Job::Idle => {
            state.job = Job::Building;
            state.pending = false;
            state.tick = 0;
            Step {
                state,
                effect: Some(Effect::StartBuild { theme: state.theme }),
            }
        }
        Job::Building => Step {
            state,
            effect: Some(Effect::Note(Message::BuildRunning)),
        },
    }
}

const fn quiet(state: State) -> Step {
    Step {
        state,
        effect: None,
    }
}

/// Watching turns on with a build when none runs, and off with a note.
const fn toggled(mut state: State) -> Step {
    if state.watch {
        state.watch = false;
        state.pending = false;
        return Step {
            state,
            effect: Some(Effect::Note(Message::WatchOff)),
        };
    }
    state.watch = true;
    let started = start(state);
    let effect = match started.effect {
        Some(Effect::StartBuild { theme }) => Effect::StartBuild { theme },
        Some(Effect::Note(_) | Effect::OpenPdf | Effect::Install { .. } | Effect::Quit) | None => {
            Effect::Note(Message::WatchOn)
        }
    };
    Step {
        state: started.state,
        effect: Some(effect),
    }
}

/// The selection the screen moves: the picker's cursor, or the slide; the help moves nothing.
const fn moved(mut state: State, forward: bool) -> Step {
    match state.screen {
        Screen::Picker(_) => {
            state.cursor = if forward {
                lower(state.cursor.saturating_add(1), last_index(state.items))
            } else {
                state.cursor.saturating_sub(1)
            };
        }
        Screen::Overview | Screen::Slides | Screen::Diagnostics => {
            state.slide = if forward {
                lower(state.slide.saturating_add(1), last_index(state.slides))
            } else {
                state.slide.saturating_sub(1)
            };
        }
        Screen::Help => {}
    }
    quiet(state)
}

const fn key(mut state: State, action: Action) -> Step {
    match action {
        Action::Quit => {
            state.quitting = true;
            Step {
                state,
                effect: Some(Effect::Quit),
            }
        }
        Action::Build => start(state),
        Action::ToggleWatch => toggled(state),
        Action::Next => moved(state, true),
        Action::Previous => moved(state, false),
        Action::ShowOverview => {
            state.screen = Screen::Overview;
            quiet(state)
        }
        Action::ShowSlides => {
            state.screen = Screen::Slides;
            quiet(state)
        }
        Action::ShowDiagnostics => {
            state.screen = Screen::Diagnostics;
            quiet(state)
        }
        Action::ShowHelp => {
            state.screen = Screen::Help;
            quiet(state)
        }
        Action::Pick { kind, items } => {
            state.screen = Screen::Picker(kind);
            state.items = items;
            state.cursor = 0;
            quiet(state)
        }
        Action::Select => select(state),
        Action::Back => {
            if let Screen::Picker(_) | Screen::Help = state.screen {
                state.screen = Screen::Overview;
            }
            quiet(state)
        }
        Action::OpenPdf => match state.last {
            Some(Verdict::Passed) => Step {
                state,
                effect: Some(Effect::OpenPdf),
            },
            Some(Verdict::Failed) | None => Step {
                state,
                effect: Some(Effect::Note(Message::NothingToOpen)),
            },
        },
    }
}

/// What a pick means: a theme is chosen and built, a font is installed, and a palette or a scheme is read and left.
const fn select(mut state: State) -> Step {
    match state.screen {
        Screen::Picker(Picker::Theme) => {
            state.theme = Some(state.cursor);
            state.screen = Screen::Overview;
            start(state)
        }
        Screen::Picker(Picker::Font) => Step {
            state,
            effect: Some(Effect::Install { font: state.cursor }),
        },
        Screen::Picker(Picker::Palette | Picker::CodeTheme) => {
            state.screen = Screen::Overview;
            quiet(state)
        }
        Screen::Overview | Screen::Slides | Screen::Diagnostics | Screen::Help => quiet(state),
    }
}

/// One step of the machine: the state after `event` and the effect it asks of the shell.
#[must_use]
pub const fn step(state: State, event: Event) -> Step {
    let state = clamped(state);
    let stepped = match event {
        Event::Key(action) => key(state, action),
        Event::BuildDone(verdict) => {
            let mut after = state;
            after.job = Job::Idle;
            after.last = Some(verdict);
            after.tick = 0;
            if after.pending && after.watch {
                start(after)
            } else {
                after.pending = false;
                quiet(after)
            }
        }
        Event::FileChanged => {
            if state.watch {
                match state.job {
                    Job::Idle => start(state),
                    Job::Building => {
                        let mut after = state;
                        after.pending = true;
                        quiet(after)
                    }
                }
            } else {
                quiet(state)
            }
        }
        Event::Tick => {
            let mut after = state;
            after.tick = match state.job {
                Job::Building => (state.tick.wrapping_add(1)) % SPINNER,
                Job::Idle => 0,
            };
            quiet(after)
        }
    };
    Step {
        state: clamped(stepped.state),
        effect: stepped.effect,
    }
}

/// The properties Kani proves for every state and event, and the tests check over the inputs they list.
#[cfg(any(test, kani))]
mod proofs {
    use super::{Action, Effect, Event, Job, SPINNER, State, step};
    #[cfg(kani)]
    use super::{Picker, Screen, Verdict};

    #[cfg(kani)]
    #[kani::proof]
    fn both_streams_are_terminals_before_the_interface_opens() {
        let input: bool = kani::any();
        let output: bool = kani::any();
        assert_eq!(super::terminal_available(input, output), input && output);
        kani::cover!(super::terminal_available(input, output));
        kani::cover!(!super::terminal_available(input, output));
    }

    /// Quitting is one key away from every state.
    pub(super) fn spec_quit(state: State) {
        let quit = step(state, Event::Key(Action::Quit));
        assert!(quit.state.quitting);
        assert_eq!(quit.effect, Some(Effect::Quit));
    }

    /// A build starts only when none runs, or when the one that ran has just ended.
    pub(super) fn spec_single_build(state: State, event: Event) {
        let stepped = step(state, event);
        if let Some(Effect::StartBuild { .. }) = stepped.effect {
            assert!(state.job == Job::Idle || matches!(event, Event::BuildDone(_)));
            assert_eq!(stepped.state.job, Job::Building);
        }
        if stepped.state.job == Job::Building
            && !matches!(stepped.effect, Some(Effect::StartBuild { .. }))
        {
            assert_eq!(state.job, Job::Building);
        }
    }

    /// Every selection stays within its count after every event, and the spinner within its phases.
    pub(super) fn spec_ranges(state: State, event: Event) {
        let after = step(state, event).state;
        assert!(after.slide <= after.slides.saturating_sub(1));
        assert!(after.cursor <= after.items.saturating_sub(1));
        assert!(after.tick < SPINNER);
    }

    #[cfg(kani)]
    #[kani::proof]
    fn quitting_is_one_key_away_from_every_state() {
        let state: State = kani::any();
        spec_quit(state);
        kani::cover!(state.job == Job::Building, "while a build runs");
        kani::cover!(
            matches!(state.screen, Screen::Picker(Picker::Font)),
            "from a picker"
        );
    }

    #[cfg(kani)]
    #[kani::proof]
    fn a_build_starts_only_when_none_runs() {
        let state: State = kani::any();
        let event: Event = kani::any();
        spec_single_build(state, event);
        kani::cover!(
            step(state, event).effect == Some(Effect::StartBuild { theme: None }),
            "a build starts"
        );
        kani::cover!(
            matches!(event, Event::BuildDone(Verdict::Passed))
                && matches!(step(state, event).effect, Some(Effect::StartBuild { .. })),
            "a pending change starts the next build"
        );
    }

    #[cfg(kani)]
    #[kani::proof]
    fn every_selection_stays_within_its_count() {
        let state: State = kani::any();
        kani::assume(state.tick < SPINNER);
        let event: Event = kani::any();
        spec_ranges(state, event);
        kani::cover!(step(state, event).state.cursor > 0, "a cursor moves");
    }

    /// A key for a build while one runs must leave the build alone; this probe claims it starts another and must fail.
    #[cfg(kani)]
    #[kani::proof]
    fn rejects_a_second_build_while_one_runs() {
        let mut state = State::default();
        state.job = Job::Building;
        let stepped = step(state, Event::Key(Action::Build));
        kani::assert(
            matches!(stepped.effect, Some(Effect::StartBuild { .. })),
            "NEGATIVE_UI_CONTROL",
        );
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Action, Effect, Event, Job, Message, Picker, SPINNER, Screen, State, Verdict, proofs, step,
    };

    #[test]
    fn redirecting_either_stream_refuses_the_interface() {
        for (input, output, expected) in [
            (false, false, false),
            (false, true, false),
            (true, false, false),
            (true, true, true),
        ] {
            assert_eq!(super::terminal_available(input, output), expected);
        }
    }

    fn states() -> [State; 6] {
        [
            State::default(),
            State {
                job: Job::Building,
                slides: 3,
                slide: 1,
                ..State::default()
            },
            State {
                screen: Screen::Picker(Picker::Theme),
                items: 4,
                cursor: 3,
                slides: 2,
                ..State::default()
            },
            State {
                screen: Screen::Picker(Picker::Font),
                items: 170,
                cursor: 12,
                watch: true,
                last: Some(Verdict::Passed),
                ..State::default()
            },
            State {
                screen: Screen::Help,
                watch: true,
                job: Job::Building,
                pending: true,
                last: Some(Verdict::Failed),
                tick: SPINNER - 1,
                ..State::default()
            },
            State {
                screen: Screen::Diagnostics,
                slides: 9,
                slide: 9,
                items: 0,
                cursor: 5,
                ..State::default()
            },
        ]
    }

    fn events() -> [Event; 18] {
        [
            Event::Key(Action::Quit),
            Event::Key(Action::Build),
            Event::Key(Action::ToggleWatch),
            Event::Key(Action::Next),
            Event::Key(Action::Previous),
            Event::Key(Action::ShowOverview),
            Event::Key(Action::ShowSlides),
            Event::Key(Action::ShowDiagnostics),
            Event::Key(Action::ShowHelp),
            Event::Key(Action::Pick {
                kind: Picker::Palette,
                items: 12,
            }),
            Event::Key(Action::Pick {
                kind: Picker::CodeTheme,
                items: 0,
            }),
            Event::Key(Action::Select),
            Event::Key(Action::Back),
            Event::Key(Action::OpenPdf),
            Event::BuildDone(Verdict::Passed),
            Event::BuildDone(Verdict::Failed),
            Event::FileChanged,
            Event::Tick,
        ]
    }

    #[test]
    fn every_listed_state_and_event_meets_the_specification() {
        for state in states() {
            proofs::spec_quit(state);
            for event in events() {
                proofs::spec_single_build(state, event);
                proofs::spec_ranges(state, event);
            }
        }
    }

    #[test]
    fn a_change_while_building_starts_the_next_build_when_watching() {
        let watching = State {
            watch: true,
            ..State::default()
        };
        let first = step(watching, Event::FileChanged);
        assert_eq!(first.effect, Some(Effect::StartBuild { theme: None }));
        assert_eq!(first.state.job, Job::Building);
        let second = step(first.state, Event::FileChanged);
        assert_eq!(second.effect, None);
        assert!(second.state.pending);
        let done = step(second.state, Event::BuildDone(Verdict::Failed));
        assert_eq!(done.effect, Some(Effect::StartBuild { theme: None }));
        assert_eq!(done.state.last, Some(Verdict::Failed));
        assert!(!done.state.pending);
        let idle = step(done.state, Event::BuildDone(Verdict::Passed));
        assert_eq!(idle.effect, None);
        assert_eq!(idle.state.job, Job::Idle);
        let unwatched = step(State::default(), Event::FileChanged);
        assert_eq!(unwatched.effect, None);
        assert_eq!(
            step(first.state, Event::Key(Action::Build)).effect,
            Some(Effect::Note(Message::BuildRunning))
        );
    }

    #[test]
    fn the_keys_move_between_screens_and_selections() {
        let three = State {
            slides: 3,
            ..State::default()
        };
        let next = step(three, Event::Key(Action::Next)).state;
        assert_eq!(next.slide, 1);
        let end = step(
            step(next, Event::Key(Action::Next)).state,
            Event::Key(Action::Next),
        )
        .state;
        assert_eq!(end.slide, 2);
        assert_eq!(step(end, Event::Key(Action::Previous)).state.slide, 1);
        assert_eq!(
            step(State::default(), Event::Key(Action::Previous))
                .state
                .slide,
            0
        );
        let picker = step(
            three,
            Event::Key(Action::Pick {
                kind: Picker::Theme,
                items: 4,
            }),
        )
        .state;
        assert_eq!(picker.screen, Screen::Picker(Picker::Theme));
        assert_eq!(picker.items, 4);
        let moved = step(picker, Event::Key(Action::Next)).state;
        assert_eq!(moved.cursor, 1);
        assert_eq!(moved.slide, 0);
        let chosen = step(moved, Event::Key(Action::Select));
        assert_eq!(chosen.state.theme, Some(1));
        assert_eq!(chosen.state.screen, Screen::Overview);
        assert_eq!(chosen.effect, Some(Effect::StartBuild { theme: Some(1) }));
        let help = step(three, Event::Key(Action::ShowHelp)).state;
        assert_eq!(help.screen, Screen::Help);
        assert_eq!(step(help, Event::Key(Action::Next)).state, help);
        assert_eq!(
            step(help, Event::Key(Action::Back)).state.screen,
            Screen::Overview
        );
        let slides = step(three, Event::Key(Action::ShowSlides)).state;
        assert_eq!(slides.screen, Screen::Slides);
        assert_eq!(step(slides, Event::Key(Action::Back)).state, slides);
        assert_eq!(
            step(three, Event::Key(Action::ShowDiagnostics))
                .state
                .screen,
            Screen::Diagnostics
        );
        assert_eq!(
            step(slides, Event::Key(Action::ShowOverview)).state.screen,
            Screen::Overview
        );
    }

    #[test]
    fn a_font_pick_installs_and_a_palette_pick_leaves_and_the_pdf_opens_after_a_pass() {
        let font = State {
            screen: Screen::Picker(Picker::Font),
            items: 10,
            cursor: 7,
            ..State::default()
        };
        assert_eq!(
            step(font, Event::Key(Action::Select)).effect,
            Some(Effect::Install { font: 7 })
        );
        let palette = State {
            screen: Screen::Picker(Picker::Palette),
            items: 12,
            ..State::default()
        };
        let left = step(palette, Event::Key(Action::Select));
        assert_eq!(left.state.screen, Screen::Overview);
        assert_eq!(left.effect, None);
        assert_eq!(
            step(State::default(), Event::Key(Action::OpenPdf)).effect,
            Some(Effect::Note(Message::NothingToOpen))
        );
        let passed = State {
            last: Some(Verdict::Passed),
            ..State::default()
        };
        assert_eq!(
            step(passed, Event::Key(Action::OpenPdf)).effect,
            Some(Effect::OpenPdf)
        );
        let failed = State {
            last: Some(Verdict::Failed),
            ..State::default()
        };
        assert_eq!(
            step(failed, Event::Key(Action::OpenPdf)).effect,
            Some(Effect::Note(Message::NothingToOpen))
        );
    }

    #[test]
    fn watching_turns_on_with_a_build_and_off_with_a_note_and_the_spinner_turns_while_building() {
        let on = step(State::default(), Event::Key(Action::ToggleWatch));
        assert!(on.state.watch);
        assert_eq!(on.effect, Some(Effect::StartBuild { theme: None }));
        let off = step(on.state, Event::Key(Action::ToggleWatch));
        assert!(!off.state.watch);
        assert_eq!(off.effect, Some(Effect::Note(Message::WatchOff)));
        let busy = State {
            job: Job::Building,
            ..State::default()
        };
        let on_busy = step(busy, Event::Key(Action::ToggleWatch));
        assert!(on_busy.state.watch);
        assert_eq!(on_busy.effect, Some(Effect::Note(Message::WatchOn)));
        let mut turning = busy;
        for expected in [1, 2, 3, 0, 1] {
            turning = step(turning, Event::Tick).state;
            assert_eq!(turning.tick, expected);
        }
        assert_eq!(step(State::default(), Event::Tick).state.tick, 0);
        assert_eq!(
            step(turning, Event::BuildDone(Verdict::Passed)).state.tick,
            0
        );
    }
}
