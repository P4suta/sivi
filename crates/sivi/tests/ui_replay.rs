//! Replays the traces of `specs/ui.qnt` through `ui_rules::step` and compares the state and the effect after every step.

use anyhow::{Context, Result, bail, ensure};
use serde_json::Value;
use sivi_core::ui_rules::{
    Action, Effect, Event, Job, Message, Picker, Screen, State, Verdict, step,
};
use std::path::Path;

/// The directory of the committed traces, which `cargo xtask gen` writes and `cargo xtask proofs` compares.
const TRACES: &str = "../../specs/traces";

/// How many traces of the model the manifest commits.
const TRACE_COUNT: usize = 100;

/// How many slides the model's deck has.
const SLIDES: usize = 2;

/// How many entries each picker lists in the model.
const fn listed(kind: Picker) -> usize {
    match kind {
        Picker::Theme | Picker::Font => 2,
        Picker::Palette => 1,
        Picker::CodeTheme => 0,
    }
}

/// The model's variables carry the module path as a prefix, which the replay drops.
fn variable<'a>(state: &'a Value, name: &str) -> Result<&'a Value> {
    state
        .as_object()
        .context("an ITF state is an object")?
        .iter()
        .find_map(|(key, value)| (key.rsplit("::").next() == Some(name)).then_some(value))
        .with_context(|| format!("the trace has no variable {name}"))
}

fn field<'a>(record: &'a Value, name: &str) -> Result<&'a Value> {
    record
        .get(name)
        .with_context(|| format!("the record has no field {name}"))
}

fn integer(value: &Value) -> Result<i64> {
    value
        .get("#bigint")
        .and_then(Value::as_str)
        .context("an ITF integer")?
        .parse()
        .context("an ITF integer")
}

fn count(value: &Value) -> Result<usize> {
    usize::try_from(integer(value)?).context("a count")
}

fn tag(value: &Value) -> Result<&str> {
    value
        .get("tag")
        .and_then(Value::as_str)
        .context("an ITF variant")
}

fn inner(value: &Value) -> Result<&Value> {
    value.get("value").context("a variant with a value")
}

fn picker(value: &Value) -> Result<Picker> {
    Ok(match tag(value)? {
        "Theme" => Picker::Theme,
        "Font" => Picker::Font,
        "Palette" => Picker::Palette,
        "CodeTheme" => Picker::CodeTheme,
        other => bail!("unknown picker {other}"),
    })
}

fn screen(value: &Value) -> Result<Screen> {
    Ok(match tag(value)? {
        "Overview" => Screen::Overview,
        "Slides" => Screen::Slides,
        "Diagnostics" => Screen::Diagnostics,
        "Help" => Screen::Help,
        "PickerScreen" => Screen::Picker(picker(inner(value)?)?),
        other => bail!("unknown screen {other}"),
    })
}

fn verdict(value: &Value) -> Result<Verdict> {
    Ok(match tag(value)? {
        "Passed" => Verdict::Passed,
        "Failed" => Verdict::Failed,
        other => bail!("unknown verdict {other}"),
    })
}

fn action(value: &Value) -> Result<Action> {
    Ok(match tag(value)? {
        "Quit" => Action::Quit,
        "Build" => Action::Build,
        "ToggleWatch" => Action::ToggleWatch,
        "Next" => Action::Next,
        "Previous" => Action::Previous,
        "ShowOverview" => Action::ShowOverview,
        "ShowSlides" => Action::ShowSlides,
        "ShowDiagnostics" => Action::ShowDiagnostics,
        "ShowHelp" => Action::ShowHelp,
        "Pick" => {
            let kind = picker(inner(value)?)?;
            Action::Pick {
                kind,
                items: listed(kind),
            }
        }
        "Select" => Action::Select,
        "Back" => Action::Back,
        "OpenPdf" => Action::OpenPdf,
        other => bail!("unknown action {other}"),
    })
}

fn event(value: &Value) -> Result<Event> {
    Ok(match tag(value)? {
        "Key" => Event::Key(action(inner(value)?)?),
        "BuildDone" => Event::BuildDone(verdict(inner(value)?)?),
        "FileChanged" => Event::FileChanged,
        "Tick" => Event::Tick,
        other => bail!("unknown event {other}"),
    })
}

fn message(value: &Value) -> Result<Message> {
    Ok(match tag(value)? {
        "BuildRunning" => Message::BuildRunning,
        "NothingToOpen" => Message::NothingToOpen,
        "WatchOn" => Message::WatchOn,
        "WatchOff" => Message::WatchOff,
        other => bail!("unknown message {other}"),
    })
}

fn chosen(value: &Value) -> Result<Option<usize>> {
    Ok(match tag(value)? {
        "DeckTheme" => None,
        "Carried" => Some(count(inner(value)?)?),
        other => bail!("unknown choice {other}"),
    })
}

fn effect(value: &Value) -> Result<Option<Effect>> {
    Ok(match tag(value)? {
        "Nothing" => None,
        "StartBuild" => Some(Effect::StartBuild {
            theme: chosen(inner(value)?)?,
        }),
        "OpenPdfEffect" => Some(Effect::OpenPdf),
        "Install" => Some(Effect::Install {
            font: count(inner(value)?)?,
        }),
        "QuitEffect" => Some(Effect::Quit),
        "Note" => Some(Effect::Note(message(inner(value)?)?)),
        other => bail!("unknown effect {other}"),
    })
}

fn last(value: &Value) -> Result<Option<Verdict>> {
    Ok(match tag(value)? {
        "NoBuild" => None,
        "Ended" => Some(verdict(inner(value)?)?),
        other => bail!("unknown last {other}"),
    })
}

fn boolean(value: &Value) -> Result<bool> {
    value.as_bool().context("an ITF boolean")
}

/// The interface state one state of a trace records.
fn decode(value: &Value) -> Result<State> {
    let record = variable(value, "st")?;
    Ok(State {
        screen: screen(field(record, "screen")?)?,
        job: match tag(field(record, "job")?)? {
            "Idle" => Job::Idle,
            "Building" => Job::Building,
            other => bail!("unknown job {other}"),
        },
        watch: boolean(field(record, "watch")?)?,
        pending: boolean(field(record, "pending")?)?,
        quitting: boolean(field(record, "quitting")?)?,
        slide: count(field(record, "slide")?)?,
        slides: SLIDES,
        cursor: count(field(record, "cursor")?)?,
        items: count(field(record, "items")?)?,
        theme: chosen(field(record, "theme")?)?,
        last: last(field(record, "last")?)?,
        tick: u8::try_from(integer(field(record, "tick")?)?)?,
    })
}

/// Replays one trace: every step of the model agrees with `ui_rules::step` on the state and the effect.
fn replay(text: &str) -> Result<usize> {
    let document: Value = serde_json::from_str(text)?;
    let states = document
        .get("states")
        .and_then(Value::as_array)
        .context("a trace holds states")?;
    let mut replayed: usize = 0;
    for (index, pair) in states.windows(2).enumerate() {
        let (Some(before), Some(after)) = (pair.first(), pair.get(1)) else {
            continue;
        };
        let read = event(variable(after, "event")?)?;
        let expected_state = decode(after)?;
        let expected_effect = effect(variable(after, "effect")?)?;
        let stepped = step(decode(before)?, read);
        ensure!(
            stepped.state == expected_state,
            "step {index}: after {read:?} the machine left {:?} but the model wrote {expected_state:?}",
            stepped.state
        );
        ensure!(
            stepped.effect == expected_effect,
            "step {index}: after {read:?} the machine asked {:?} but the model wrote {expected_effect:?}",
            stepped.effect
        );
        replayed = replayed.saturating_add(1);
    }
    Ok(replayed)
}

#[test]
fn every_committed_trace_replays_through_the_interface_machine() {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join(TRACES);
    let mut total: usize = 0;
    for index in 0..TRACE_COUNT {
        let path = directory.join(format!("ui.{index}.itf.json"));
        let text = sivi::io::read_to_string(&path).unwrap();
        total = total.saturating_add(
            replay(&text).unwrap_or_else(|error| panic!("{}: {error:#}", path.display())),
        );
    }
    assert!(total > 0, "the traces hold steps");
}

#[test]
fn the_probe_trace_breaks_the_machine_as_the_model_says() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(TRACES)
        .join("ui_probe.itf.json");
    let text = sivi::io::read_to_string(&path).unwrap();
    assert!(
        replay(&text).is_err(),
        "the probe trace must disagree with the guarded machine"
    );
}
