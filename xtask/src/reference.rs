//! The Prolog reference of the slot binding: `specs/binding.pl` binds the same slides as `binder::step`, and any difference fails.
//!
//! The slides come from the deck fixtures bound to the plain theme and from a seeded generator over the plain theme's masters.

use crate::fs;
use crate::tool::{self, Tool};
use anyhow::{Context, Result, anyhow, ensure};
use serde_json::{Value, json};
use sivi_core::bind::{self, Overflow, grammar};
use sivi_core::binder::{BindState, Input, Step, step, unfilled};
use sivi_core::diagnostic::Code;
use sivi_core::ir::{Block, BlockKind, Deck, HeadingLevel};
use sivi_core::slot_rules::{BlockFacts, SlotGrammar};
use sivi_core::theme::{Binding, Theme};
use std::path::Path;

/// The reference program.
pub const SPEC: &str = "specs/binding.pl";

/// The SWI-Prolog release the reference runs under.
pub const SWIPL_VERSION: &str = "10.0.2";

/// Where the deck fixtures live.
const DECKS: &str = "fixtures/decks";

/// The most blocks a generated slide holds.
const MAX_BLOCKS: u64 = 8;

/// The most of any measure a generated block carries, which crosses the plain theme's capacities.
const MAX_MEASURE: u64 = 10;

/// One slide to bind: the slots of its master and the blocks it holds.
#[derive(Clone, Debug)]
pub struct Case {
    pub name: String,
    pub slots: Vec<SlotGrammar>,
    pub inputs: Vec<Input>,
}

/// What a binding yields: the slot or the refusal of each block, and the first required slot left empty.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Outcome {
    pub steps: Vec<Result<usize, Code>>,
    pub unfilled: Option<usize>,
}

/// What the binder reads of a block, as the core's binding reads it.
///
/// # Errors
///
/// Returns an error when a measure of the block exceeds what the binder counts.
pub fn input(block: &Block) -> Result<Input> {
    bind::input(block)
        .map_err(|Overflow| anyhow!("a measure of the block exceeds what the binder counts"))
}

/// The binding `binder::step` yields for a case.
#[must_use]
pub fn bind(case: &Case) -> Outcome {
    let mut state = BindState::default();
    let mut steps = Vec::new();
    for read in &case.inputs {
        match step(&case.slots, state, *read) {
            Step::Bound { slot, state: next } => {
                state = next;
                steps.push(Ok(slot));
            }
            Step::Refused { code, .. } => steps.push(Err(code)),
        }
    }
    Outcome {
        steps,
        unfilled: unfilled(&case.slots, state),
    }
}

/// The slides of the deck fixtures that parse, each with the master the plain theme resolves for it.
fn fixture_cases(root: &Path, theme: &Theme) -> Result<Vec<Case>> {
    let mut cases = Vec::new();
    for path in fs::files_with_suffix(&root.join(DECKS), "deck.md")? {
        let parsed = sivi_markdown::parse(&fs::read(&path)?);
        let Some(deck) = parsed.deck else {
            continue;
        };
        cases.extend(deck_cases(&path.display().to_string(), &deck, theme)?);
    }
    Ok(cases)
}

fn deck_cases(name: &str, deck: &Deck, theme: &Theme) -> Result<Vec<Case>> {
    deck.slides
        .iter()
        .filter_map(|slide| {
            let master = slide
                .master
                .as_ref()
                .or(deck.head.default_master.as_ref())?;
            let master = theme.master(master)?;
            Some(
                slide
                    .blocks
                    .iter()
                    .map(input)
                    .collect::<Result<Vec<Input>>>()
                    .map(|inputs| Case {
                        name: format!("{name} slide {}", slide.number),
                        slots: grammar(master),
                        inputs,
                    }),
            )
        })
        .collect()
}

/// The shifts of the xorshift generator, as Marsaglia lists them for 64 bits.
const SHIFT_LEFT: u32 = 13;
const SHIFT_RIGHT: u32 = 7;
const SHIFT_LAST: u32 = 17;
/// The shapes a generated input takes: a title heading, a section heading, or a block of a kind.
const INPUT_SHAPES: u64 = 6;
const TITLE_SHAPE: u64 = 0;
const SECTION_SHAPE: u64 = 1;

/// A small deterministic generator, so the generated slides are the same on every host.
struct Generator(u64);

impl Generator {
    const fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << SHIFT_LEFT;
        x ^= x >> SHIFT_RIGHT;
        x ^= x << SHIFT_LAST;
        self.0 = x;
        x
    }

    fn below(&mut self, limit: u64) -> u64 {
        self.next().checked_rem(limit.max(1)).unwrap_or(0)
    }

    /// An index below `len`.
    fn index(&mut self, len: usize) -> Result<usize> {
        let limit = u64::try_from(len).context("a count the generator bounds")?;
        usize::try_from(self.below(limit)).context("an index the generator picks")
    }

    fn measure(&mut self) -> Result<u32> {
        u32::try_from(self.below(MAX_MEASURE)).context("a measure the generator picks")
    }

    fn block(&mut self) -> Result<Input> {
        const KINDS: [BlockKind; 7] = [
            BlockKind::Heading,
            BlockKind::Paragraph,
            BlockKind::List,
            BlockKind::Code,
            BlockKind::Quote,
            BlockKind::Image,
            BlockKind::Table,
        ];
        Ok(match self.below(INPUT_SHAPES) {
            TITLE_SHAPE => Input::Heading(HeadingLevel::Title),
            SECTION_SHAPE => Input::Heading(HeadingLevel::Section),
            _ => {
                let kind = KINDS
                    .get(self.index(KINDS.len())?)
                    .copied()
                    .context("a block kind")?;
                Input::Block(BlockFacts {
                    kind,
                    items: self.measure()?,
                    depth: self.measure()?,
                    rows: self.measure()?,
                    columns: self.measure()?,
                    lines: self.measure()?,
                })
            }
        })
    }
}

/// `count` generated slides over the masters of `theme`, from `seed`.
///
/// # Errors
///
/// Returns an error when the theme has no master or a pick leaves the machine's range.
pub fn generated_cases(theme: &Theme, count: u32, seed: u64) -> Result<Vec<Case>> {
    let mut generator = Generator(seed.max(1));
    let mut cases = Vec::new();
    for index in 0..count {
        let master = theme
            .masters
            .get(generator.index(theme.masters.len())?)
            .context("a master of the theme")?;
        let blocks = generator.below(MAX_BLOCKS).saturating_add(1);
        let mut inputs = Vec::new();
        for _ in 0..blocks {
            inputs.push(generator.block()?);
        }
        cases.push(Case {
            name: format!("generated {index} on {}", master.name),
            slots: grammar(master),
            inputs,
        });
    }
    Ok(cases)
}

const fn kind_name(kind: BlockKind) -> &'static str {
    match kind {
        BlockKind::Heading => "heading",
        BlockKind::Paragraph => "paragraph",
        BlockKind::List => "list",
        BlockKind::Code => "code",
        BlockKind::Quote => "quote",
        BlockKind::Image => "image",
        BlockKind::Table => "table",
    }
}

fn binding_json(binding: Binding) -> Value {
    match binding {
        Binding::Title => json!("title"),
        Binding::Subtitle => json!("subtitle"),
        Binding::Flow => json!("flow"),
        Binding::Section(number) => json!({ "section": number }),
    }
}

/// The case as the reference reads it.
#[must_use]
pub fn input_json(case: &Case) -> Value {
    const KINDS: [BlockKind; 7] = [
        BlockKind::Heading,
        BlockKind::Paragraph,
        BlockKind::List,
        BlockKind::Code,
        BlockKind::Quote,
        BlockKind::Image,
        BlockKind::Table,
    ];
    let slots: Vec<Value> = case
        .slots
        .iter()
        .map(|slot| {
            let accepts: Vec<&str> = KINDS
                .iter()
                .filter(|kind| slot.accepts.contains(**kind))
                .map(|kind| kind_name(*kind))
                .collect();
            json!({
                "binds": binding_json(slot.binds),
                "accepts": accepts,
                "required": slot.required,
                "follows": slot.follows,
                "capacity": {
                    "blocks": slot.capacity.blocks,
                    "items": slot.capacity.items,
                    "depth": slot.capacity.depth,
                    "rows": slot.capacity.rows,
                    "columns": slot.capacity.columns,
                    "lines": slot.capacity.lines,
                },
            })
        })
        .collect();
    let blocks: Vec<Value> = case
        .inputs
        .iter()
        .map(|read| match read {
            Input::Heading(HeadingLevel::Title) => json!({ "heading": "title" }),
            Input::Heading(HeadingLevel::Section) => json!({ "heading": "section" }),
            Input::Block(facts) => json!({
                "kind": kind_name(facts.kind),
                "items": facts.items,
                "depth": facts.depth,
                "rows": facts.rows,
                "columns": facts.columns,
                "lines": facts.lines,
            }),
        })
        .collect();
    json!({ "slots": slots, "blocks": blocks })
}

fn code_named(name: &str) -> Result<Code> {
    Ok(match name {
        "E0303" => Code::NoSlotForBlock,
        "E0305" => Code::SectionCount,
        "E0306" => Code::CapacityExceeded,
        "E0307" => Code::TitlePosition,
        other => anyhow::bail!("the reference named an unknown refusal {other}"),
    })
}

/// The outcome the reference wrote.
///
/// # Errors
///
/// Returns an error when the document is not an outcome.
pub fn outcome_json(text: &str) -> Result<Outcome> {
    let document: Value = serde_json::from_str(text).context("parse the reference's outcome")?;
    let mut steps = Vec::new();
    for entry in document
        .get("steps")
        .and_then(Value::as_array)
        .context("the outcome lists steps")?
    {
        if let Some(slot) = entry.get("slot").and_then(Value::as_u64) {
            steps.push(Ok(usize::try_from(slot)?));
        } else if let Some(refused) = entry.get("refused").and_then(Value::as_str) {
            steps.push(Err(code_named(refused)?));
        } else {
            anyhow::bail!("a step names a slot or a refusal: {entry}");
        }
    }
    let unfilled = match document.get("unfilled") {
        Some(Value::Number(number)) => {
            Some(usize::try_from(number.as_u64().context("a slot index")?)?)
        }
        Some(Value::Null) | None => None,
        Some(other) => anyhow::bail!("unfilled is a slot index or null: {other}"),
    };
    Ok(Outcome { steps, unfilled })
}

/// Runs the reference over a case.
fn reference(root: &Path, scratch: &Path, case: &Case) -> Result<Outcome> {
    let input = scratch.join("input.json");
    fs::write(&input, serde_json::to_string(&input_json(case))?.as_bytes())?;
    let mut swipl = Tool::Swipl.command(root);
    swipl
        .args([
            "--no-packs",
            "--on-error=status",
            "--on-warning=status",
            "-g",
            "main",
            "-t",
            "halt",
            SPEC,
        ])
        .arg(&input);
    let text = tool::capture(&mut swipl, "swipl")?;
    outcome_json(&text).with_context(|| format!("the reference's outcome for {}", case.name))
}

/// Binds every fixture slide and `generated` generated slides with both implementations and refuses any difference.
///
/// # Errors
///
/// Returns an error when the reference is unavailable, a case differs, or the tampered control agrees.
pub fn verify(root: &Path, generated: u32, seed: u64) -> Result<()> {
    tool::require_version(&mut Tool::Swipl.command(root), "swipl", SWIPL_VERSION)?;
    ensure!(
        fs::is_file(&root.join(SPEC))?,
        "the reference {SPEC} is missing"
    );
    let theme = sivi_theme::parse(sivi_theme::PLAIN)
        .map_err(|diagnostics| anyhow::anyhow!("the plain theme does not load: {diagnostics:?}"))?;
    let scratch = tempfile::tempdir().context("create the reference scratch directory")?;
    let mut cases = fixture_cases(root, &theme)?;
    cases.extend(generated_cases(&theme, generated, seed)?);
    ensure!(!cases.is_empty(), "no slide to bind");
    for case in &cases {
        let ours = bind(case);
        let theirs = reference(root, scratch.path(), case)?;
        ensure!(
            ours == theirs,
            "{}: the binder and the reference differ:\n  binder:    {ours:?}\n  reference: {theirs:?}",
            case.name
        );
    }
    let tampered = cases
        .iter()
        .find(|case| bind(case).steps.iter().any(Result::is_ok))
        .context("a case with a bound block, for the control")?;
    let mut control = bind(tampered);
    if let Some(first) = control.steps.iter_mut().find(|outcome| outcome.is_ok()) {
        *first = Err(Code::NoSlotForBlock);
    }
    ensure!(
        control != reference(root, scratch.path(), tampered)?,
        "the reference agreed with a tampered binding, so the comparison cannot fail"
    );
    eprintln!(
        "prolog: {} slides bind the same way under the binder and the reference, and a tampered binding differs",
        cases.len()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{Case, bind, input_json, outcome_json};
    use sivi_core::binder::Input;
    use sivi_core::diagnostic::Code;
    use sivi_core::ir::{BlockKind, HeadingLevel};
    use sivi_core::slot_rules::{BlockFacts, SlotGrammar};
    use sivi_core::theme::{Binding, BlockKindSet, Capacity};

    #[test]
    fn a_case_binds_and_its_json_names_every_slot_and_block() {
        let case = Case {
            name: String::from("tiny"),
            slots: vec![
                SlotGrammar {
                    binds: Binding::Title,
                    accepts: BlockKindSet::EMPTY,
                    required: true,
                    follows: None,
                    capacity: Capacity::default(),
                },
                SlotGrammar {
                    binds: Binding::Flow,
                    accepts: BlockKindSet::EMPTY.with(BlockKind::Paragraph),
                    required: false,
                    follows: None,
                    capacity: Capacity {
                        blocks: Some(1),
                        ..Capacity::default()
                    },
                },
            ],
            inputs: vec![
                Input::Heading(HeadingLevel::Title),
                Input::Block(BlockFacts {
                    kind: BlockKind::Paragraph,
                    items: 0,
                    depth: 0,
                    rows: 0,
                    columns: 0,
                    lines: 0,
                }),
                Input::Block(BlockFacts {
                    kind: BlockKind::Paragraph,
                    items: 0,
                    depth: 0,
                    rows: 0,
                    columns: 0,
                    lines: 0,
                }),
            ],
        };
        let outcome = bind(&case);
        assert_eq!(
            outcome.steps,
            vec![Ok(0), Ok(1), Err(Code::CapacityExceeded)]
        );
        assert_eq!(outcome.unfilled, None);
        let json = input_json(&case);
        assert_eq!(json["slots"][1]["capacity"]["blocks"], 1);
        assert_eq!(json["slots"][1]["accepts"][0], "paragraph");
        assert_eq!(json["blocks"][0]["heading"], "title");
        let parsed = outcome_json(
            r#"{"steps":[{"slot":0},{"slot":1},{"refused":"E0306"}],"unfilled":null}"#,
        )
        .unwrap();
        assert_eq!(parsed, outcome);
        outcome_json(r#"{"steps":[{"refused":"E9999"}],"unfilled":null}"#).unwrap_err();
    }
}
