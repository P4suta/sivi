//! Replays the traces of `specs/binder.qnt` through `binder::step` and compares the full state after every step.
//!
//! The model's master mirrors a figure master with a section, which this test builds slot for slot.

use anyhow::{Context, Result, bail, ensure};
use serde_json::Value;
use sivi_core::binder::{BindState, Input, Phase, Step, step};
use sivi_core::diagnostic::Code;
use sivi_core::ir::{BlockKind, HeadingLevel};
use sivi_core::slot_rules::{BlockFacts, MAX_SLOTS, SlotGrammar};
use sivi_core::theme::{Binding, BlockKindSet, Capacity};
use std::path::Path;

/// The directory of the committed traces, which `cargo xtask gen` writes and `cargo xtask proofs` compares.
const TRACES: &str = "../../specs/traces";

/// How many traces of the model the manifest commits.
const TRACE_COUNT: usize = 100;

/// The model's slots in order: the title, the subtitle, the text flow, the image that follows it, and the section.
const MODEL_SLOTS: usize = 5;

const fn slot(
    binds: Binding,
    accepts: BlockKindSet,
    follows: Option<usize>,
    blocks: Option<u32>,
) -> SlotGrammar {
    SlotGrammar {
        binds,
        accepts,
        required: false,
        follows,
        capacity: Capacity {
            blocks,
            items: None,
            depth: None,
            rows: None,
            columns: None,
            lines: None,
        },
    }
}

/// The master of `specs/binder.qnt`, as `binds`, `accepts`, `capacity`, and `follows` spell it.
const fn master() -> [SlotGrammar; MODEL_SLOTS] {
    let text = BlockKindSet::EMPTY
        .with(BlockKind::Paragraph)
        .with(BlockKind::List);
    [
        slot(Binding::Title, BlockKindSet::EMPTY, None, None),
        slot(
            Binding::Subtitle,
            BlockKindSet::EMPTY.with(BlockKind::Paragraph),
            None,
            None,
        ),
        slot(Binding::Flow, text, None, Some(2)),
        slot(
            Binding::Flow,
            BlockKindSet::EMPTY.with(BlockKind::Image),
            Some(2),
            Some(1),
        ),
        slot(Binding::Section(1), text, None, Some(2)),
    ]
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

fn integer(value: &Value) -> Result<i64> {
    value
        .get("#bigint")
        .and_then(Value::as_str)
        .context("an ITF integer")?
        .parse()
        .context("an ITF integer")
}

/// A slot index, or `None` where the model writes minus one.
fn slot_index(value: &Value) -> Result<Option<usize>> {
    let number = integer(value)?;
    Ok(if number < 0 {
        None
    } else {
        Some(usize::try_from(number)?)
    })
}

fn tag(value: &Value) -> Result<&str> {
    value
        .get("tag")
        .and_then(Value::as_str)
        .context("an ITF variant")
}

fn phase(value: &Value) -> Result<Phase> {
    Ok(match tag(value)? {
        "Start" => Phase::Start,
        "Titled" => Phase::Titled,
        "Flowing" => Phase::Flow,
        "InSection" => Phase::Section(u32::try_from(integer(
            value.get("value").context("a section number")?,
        )?)?),
        other => bail!("unknown phase {other}"),
    })
}

fn kind(value: &Value) -> Result<BlockKind> {
    Ok(match tag(value)? {
        "Paragraph" => BlockKind::Paragraph,
        "ListBlock" => BlockKind::List,
        "Image" => BlockKind::Image,
        "Code" => BlockKind::Code,
        other => bail!("unknown kind {other}"),
    })
}

fn input(value: &Value) -> Result<Input> {
    Ok(match tag(value)? {
        "TitleHeading" => Input::Heading(HeadingLevel::Title),
        "SectionHeading" => Input::Heading(HeadingLevel::Section),
        "Block" => Input::Block(BlockFacts {
            kind: kind(value.get("value").context("a block kind")?)?,
            items: 0,
            depth: 0,
            rows: 0,
            columns: 0,
            lines: 0,
        }),
        other => bail!("unknown input {other}"),
    })
}

/// The fill counts of the model's map from slot to count, each slot once and in order.
fn filled(value: &Value) -> Result<[u32; MAX_SLOTS]> {
    let pairs = value
        .get("#map")
        .and_then(Value::as_array)
        .context("a map over the slots")?;
    ensure!(pairs.len() == MODEL_SLOTS, "a map over every slot");
    let mut counts = [0; MAX_SLOTS];
    for (index, pair) in pairs.iter().enumerate() {
        let Some([key, count]) = pair.as_array().map(Vec::as_slice) else {
            bail!("a map entry is a pair");
        };
        ensure!(integer(key)? == i64::try_from(index)?, "slots in order");
        if let Some(slot) = counts.get_mut(index) {
            *slot = u32::try_from(integer(count)?)?;
        }
    }
    Ok(counts)
}

/// The binder state one state of a trace records.
fn decode(value: &Value) -> Result<BindState> {
    Ok(BindState {
        phase: phase(variable(value, "phase")?)?,
        filled: filled(variable(value, "filled")?)?,
        last: slot_index(variable(value, "last")?)?,
    })
}

fn code(name: &str) -> Result<Code> {
    Ok(match name {
        "E0303" => Code::NoSlotForBlock,
        "E0305" => Code::SectionCount,
        "E0306" => Code::CapacityExceeded,
        "E0307" => Code::TitlePosition,
        other => bail!("unknown refusal {other}"),
    })
}

/// Replays one trace: every step of the model agrees with `binder::step` on the slot or the refusal and on the state.
fn replay(text: &str) -> Result<usize> {
    let document: Value = serde_json::from_str(text)?;
    let states = document
        .get("states")
        .and_then(Value::as_array)
        .context("a trace holds states")?;
    let slots = master();
    let mut replayed: usize = 0;
    for (index, pair) in states.windows(2).enumerate() {
        let (Some(before), Some(after)) = (pair.first(), pair.get(1)) else {
            continue;
        };
        let read = input(variable(after, "input")?)?;
        let expected_bound = slot_index(variable(after, "bound")?)?;
        let refusal = variable(after, "refusal")?
            .as_str()
            .context("a refusal is text")?;
        let expected_state = decode(after)?;
        match step(&slots, decode(before)?, read) {
            Step::Bound { slot, state } => {
                ensure!(
                    expected_bound == Some(slot) && refusal.is_empty(),
                    "step {index}: the binder bound slot {slot} but the model wrote {expected_bound:?} {refusal}"
                );
                ensure!(
                    state == expected_state,
                    "step {index}: the states differ after binding"
                );
            }
            Step::Refused { code: found, state } => {
                ensure!(
                    expected_bound.is_none() && code(refusal)? == found,
                    "step {index}: the binder refused with {found:?} but the model wrote {expected_bound:?} {refusal}"
                );
                ensure!(
                    state == expected_state,
                    "step {index}: a refusal changed the state"
                );
            }
        }
        replayed = replayed.saturating_add(1);
    }
    Ok(replayed)
}

#[test]
fn every_committed_trace_replays_through_the_binder() -> Result<()> {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join(TRACES);
    let mut steps = 0;
    for index in 0..TRACE_COUNT {
        let path = directory.join(format!("binder.{index}.itf.json"));
        let text = sivi::io::read_to_string(&path).with_context(|| {
            format!(
                "read {}; run `cargo xtask gen` to write the traces",
                path.display()
            )
        })?;
        steps += replay(&text).with_context(|| path.display().to_string())?;
    }
    ensure!(steps > 0, "the traces hold steps");
    Ok(())
}

#[test]
fn the_probe_trace_ends_in_a_state_the_binder_never_reaches() -> Result<()> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(TRACES)
        .join("binder_probe.itf.json");
    let text = sivi::io::read_to_string(&path).with_context(|| {
        format!(
            "read {}; run `cargo xtask gen` to write the traces",
            path.display()
        )
    })?;
    ensure!(
        replay(&text).is_err(),
        "the probe's violation replays through the binder, so the binder keeps no capacity"
    );
    Ok(())
}
