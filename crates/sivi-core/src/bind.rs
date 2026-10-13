//! Binding every slide of a deck to the slots of its master, block by block through the binder.
//!
//! The result names, for each slide, its master and the blocks each slot holds, which the emitter places and the report reads.

use crate::binder::{self, BindState, Input, Phase, Step};
use crate::diagnostic::{Code, Diagnostic, SlideRef};
use crate::ir::{Block, BlockBody, BlockKind, Deck, List, Slide};
use crate::master_rules::{self, Resolution};
use crate::protocol::SlideIndex;
use crate::slot_rules::{BlockFacts, SlotGrammar};
use crate::span::Span;
use crate::text::Identifier;
use crate::theme::{Binding, Capacity, Master, Slot, Theme};
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

/// A deck whose every slide is bound to a master of the theme.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct BoundDeck {
    pub slides: Vec<BoundSlide>,
}

/// One slide bound to its master.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BoundSlide {
    pub index: SlideIndex,
    /// The index of the master in the theme's list.
    pub master: usize,
    /// The regions in the order their first block appears.
    pub regions: Vec<BoundRegion>,
}

/// The blocks one slot holds, as indices into the slide's blocks in order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BoundRegion {
    /// The index of the slot in the master's list.
    pub slot: usize,
    pub blocks: Vec<usize>,
}

/// A measure of a block beyond what the binder counts.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct Overflow;

/// The slot grammar of a master, with `follows` as the index of the slot it names.
#[must_use]
pub fn grammar(master: &Master) -> Vec<SlotGrammar> {
    master
        .slots
        .iter()
        .map(|slot| SlotGrammar {
            binds: slot.binds,
            accepts: slot.accepts,
            required: slot.required,
            follows: slot
                .follows
                .as_ref()
                .and_then(|name| master.slots.iter().position(|other| other.name == *name)),
            capacity: slot.capacity,
        })
        .collect()
}

/// How many items a list holds at every depth.
fn items(list: &List) -> u32 {
    list.items.iter().fold(0, |total, item| {
        let below = item.nested.as_ref().map_or(0, items);
        total.saturating_add(1).saturating_add(below)
    })
}

fn count(measure: usize) -> Result<u32, Overflow> {
    u32::try_from(measure).map_err(|_beyond| Overflow)
}

/// What the binder reads of a block.
///
/// # Errors
///
/// Returns `Overflow` when a measure of the block exceeds what the binder counts.
pub fn input(block: &Block) -> Result<Input, Overflow> {
    let facts = |kind: BlockKind| BlockFacts {
        kind,
        items: 0,
        depth: 0,
        rows: 0,
        columns: 0,
        lines: 0,
    };
    Ok(match &block.body {
        BlockBody::Heading(heading) => Input::Heading(heading.level),
        BlockBody::Paragraph(_) => Input::Block(facts(BlockKind::Paragraph)),
        BlockBody::Quote(_) => Input::Block(facts(BlockKind::Quote)),
        BlockBody::Image(_) => Input::Block(facts(BlockKind::Image)),
        BlockBody::List(list) => Input::Block(BlockFacts {
            items: items(list),
            depth: count(list.depth())?,
            ..facts(BlockKind::List)
        }),
        BlockBody::Code(code) => Input::Block(BlockFacts {
            lines: count(code.text.lines().count())?,
            ..facts(BlockKind::Code)
        }),
        BlockBody::Table(table) => Input::Block(BlockFacts {
            rows: count(table.rows.len())?,
            columns: count(table.header.cells.len())?,
            ..facts(BlockKind::Table)
        }),
    })
}

/// Whether a master has a flow or section slot that accepts `kind`.
fn accepts(master: &Master, kind: BlockKind) -> bool {
    master.slots.iter().any(|slot| {
        matches!(slot.binds, Binding::Flow | Binding::Section(_)) && slot.accepts.contains(kind)
    })
}

/// The names in backticks, separated by commas, or `none`.
fn names<'a>(names: impl Iterator<Item = &'a Identifier>) -> String {
    let mut listed = String::new();
    for name in names {
        if !listed.is_empty() {
            listed.push_str(", ");
        }
        listed.push('`');
        listed.push_str(name.as_str());
        listed.push('`');
    }
    if listed.is_empty() {
        String::from("none")
    } else {
        listed
    }
}

/// `count` of `noun`, with the plural `nouns` beyond one and `no` for zero.
fn counted(count: usize, noun: &str) -> String {
    match count {
        0 => format!("no {noun}s"),
        1 => format!("1 {noun}"),
        many => format!("{many} {noun}s"),
    }
}

/// What a slot cannot hold of a block, one line per exceeded limit.
fn room(slot: &Slot, filled: u32, facts: BlockFacts) -> Vec<String> {
    let name = &slot.name;
    let Capacity {
        blocks,
        items,
        depth,
        rows,
        columns,
        lines,
    } = slot.capacity;
    let mut found = Vec::new();
    if let Some(most) = blocks
        && filled >= most
    {
        found.push(format!(
            "slot `{name}` already holds {filled} blocks, its `max_blocks`"
        ));
    }
    let measures = [
        (items, facts.items, "max_items", "items"),
        (depth, facts.depth, "max_depth", "levels of nesting"),
        (rows, facts.rows, "max_rows", "rows"),
        (columns, facts.columns, "max_columns", "columns"),
        (lines, facts.lines, "max_lines", "lines"),
    ];
    for (limit, measure, key, noun) in measures {
        if let Some(most) = limit
            && measure > most
        {
            found.push(format!(
                "slot `{name}` takes at most {most} {noun} (`{key}`); this block has {measure}"
            ));
        }
    }
    found
}

/// A block as the binder read it.
#[derive(Clone, Copy)]
struct Read<'a> {
    block: &'a Block,
    input: Input,
}

impl Read<'_> {
    const fn kind(self) -> BlockKind {
        match self.input {
            Input::Heading(_) => BlockKind::Heading,
            Input::Block(facts) => facts.kind,
        }
    }
}

/// One slide against its master, which every diagnostic of the slide names.
struct Scene<'a> {
    theme: &'a Theme,
    master: &'a Master,
    slots: Vec<SlotGrammar>,
    slide: &'a Slide,
}

impl Scene<'_> {
    fn diagnostic(&self, code: Code, message: String) -> Diagnostic {
        Diagnostic::new(code, message).on_slide(SlideRef {
            number: self.slide.number,
            id: self.slide.id.clone(),
            master: Some(self.master.name.clone()),
        })
    }

    fn too_large(&self, block: &Block) -> Diagnostic {
        self.diagnostic(
            Code::LimitExceeded,
            String::from("a measure of this block exceeds what the binder counts"),
        )
        .at(block.span, String::from("this block"))
        .help(String::from("split the block"))
    }

    fn title_position(&self, span: Span) -> Diagnostic {
        if self
            .master
            .slots
            .iter()
            .any(|slot| slot.binds == Binding::Title)
        {
            return self
                .diagnostic(
                    Code::TitlePosition,
                    String::from("a `#` heading may only open the slide"),
                )
                .at(span, String::from("this heading comes after other content"))
                .help(String::from(
                    "move the heading to the top of the slide, or make it a `##` section heading",
                ));
        }
        let titled = self
            .theme
            .masters
            .iter()
            .filter(|master| master.slots.iter().any(|slot| slot.binds == Binding::Title))
            .map(|master| &master.name);
        self.diagnostic(
            Code::TitlePosition,
            format!(
                "master `{}` has no title slot for a `#` heading",
                self.master.name
            ),
        )
        .at(span, String::from("this heading has no slot"))
        .help(format!("use a master with a title slot: {}", names(titled)))
    }

    fn section_count(&self, state: BindState, span: Span) -> Diagnostic {
        let opened = match state.phase {
            Phase::Section(number) => number.saturating_add(1),
            Phase::Start | Phase::Titled | Phase::Flow => 1,
        };
        let available = self
            .master
            .slots
            .iter()
            .filter(|slot| matches!(slot.binds, Binding::Section(_)))
            .count();
        self.diagnostic(
            Code::SectionCount,
            format!(
                "this `##` opens section {opened}, but master `{}` has {}",
                self.master.name,
                counted(available, "section slot")
            ),
        )
        .at(
            span,
            String::from("this heading opens one section too many"),
        )
        .help(format!(
            "use a master with {opened} section slots, or merge this section into the one before"
        ))
    }

    fn no_slot(&self, kind: BlockKind, span: Span) -> Diagnostic {
        let accepting: Vec<&Identifier> = self
            .theme
            .masters
            .iter()
            .filter(|master| accepts(master, kind))
            .map(|master| &master.name)
            .collect();
        let diagnostic = self
            .diagnostic(
                Code::NoSlotForBlock,
                format!(
                    "no slot of master `{}` accepts {} blocks",
                    self.master.name,
                    kind.name()
                ),
            )
            .at(span, format!("this {} block has no slot", kind.name()));
        if accepting.is_empty() {
            diagnostic.help(format!(
                "no master of theme `{}` accepts {} blocks; remove the block",
                self.theme.name,
                kind.name()
            ))
        } else {
            diagnostic.help(format!(
                "use a master whose slots accept {} blocks: {}",
                kind.name(),
                names(accepting.into_iter())
            ))
        }
    }

    fn capacity(&self, state: BindState, read: Read<'_>) -> Diagnostic {
        let Input::Block(facts) = read.input else {
            return self.no_slot(read.kind(), read.block.span);
        };
        let mut diagnostic = self
            .diagnostic(
                Code::CapacityExceeded,
                format!(
                    "no slot of master `{}` has room for this {} block",
                    self.master.name,
                    facts.kind.name()
                ),
            )
            .at(
                read.block.span,
                String::from("this block exceeds every slot that accepts it"),
            );
        for (index, slot) in self.master.slots.iter().enumerate() {
            let candidate = match state.phase {
                Phase::Section(number) => slot.binds == Binding::Section(number),
                Phase::Start | Phase::Titled | Phase::Flow => slot.binds == Binding::Flow,
            };
            if !candidate || !slot.accepts.contains(facts.kind) {
                continue;
            }
            let filled = state.filled.get(index).copied().unwrap_or(0);
            for line in room(slot, filled, facts) {
                diagnostic = diagnostic.note(line);
            }
        }
        diagnostic.help(String::from(
            "shorten the block, split the slide, or use a master with a larger slot",
        ))
    }

    fn refusal(&self, state: BindState, read: Read<'_>, code: Code) -> Diagnostic {
        let span = read.block.span;
        if code == Code::TitlePosition {
            self.title_position(span)
        } else if code == Code::SectionCount {
            self.section_count(state, span)
        } else if code == Code::NoSlotForBlock {
            self.no_slot(read.kind(), span)
        } else if code == Code::CapacityExceeded {
            self.capacity(state, read)
        } else {
            self.diagnostic(
                code,
                format!("the binder refused this {} block", read.kind().name()),
            )
            .at(span, String::from("this block"))
        }
    }

    fn empty(&self, index: usize) -> Diagnostic {
        let Some(slot) = self.master.slots.get(index) else {
            return self.diagnostic(
                Code::Internal,
                format!("the binder named slot {index}, which the master lacks"),
            );
        };
        let help = match slot.binds {
            Binding::Title => String::from("open the slide with a `#` heading"),
            Binding::Subtitle => String::from("write a paragraph right after the `#` heading"),
            Binding::Section(number) => format!("add a `##` heading that opens section {number}"),
            Binding::Flow => format!(
                "add a block the slot accepts: {}",
                names_of_kinds(slot.accepts)
            ),
        };
        self.diagnostic(
            Code::RequiredSlotEmpty,
            format!(
                "slot `{}` of master `{}` is required but holds nothing",
                slot.name, self.master.name
            ),
        )
        .at(self.slide.span, String::from("this slide leaves it empty"))
        .help(help)
    }
}

/// The kinds a set holds, by name and separated by commas.
fn names_of_kinds(set: crate::theme::BlockKindSet) -> String {
    let mut listed = String::new();
    for kind in BlockKind::ALL {
        if set.contains(kind) {
            if !listed.is_empty() {
                listed.push_str(", ");
            }
            listed.push_str(kind.name());
        }
    }
    listed
}

fn missing_master(theme: &Theme, slide: &Slide) -> Diagnostic {
    Diagnostic::new(
        Code::MasterMissing,
        format!(
            "slide {} names no master, and the deck sets no `default_master`",
            slide.number
        ),
    )
    .at(slide.span, String::from("this slide"))
    .help(String::from(
        "write `master: <name>` in the slide's front matter, or `default_master: <name>` in the head of the deck",
    ))
    .note(format!(
        "theme `{}` has the masters {}",
        theme.name,
        names(theme.masters.iter().map(|master| &master.name))
    ))
    .on_slide(SlideRef {
        number: slide.number,
        id: slide.id.clone(),
        master: None,
    })
}

fn unknown_master(theme: &Theme, slide: &Slide, named: (&Identifier, bool)) -> Diagnostic {
    let (name, defaulted) = named;
    let known: Vec<Identifier> = theme
        .masters
        .iter()
        .map(|master| master.name.clone())
        .collect();
    let label = if defaulted {
        "the deck's `default_master` names it for this slide"
    } else {
        "this slide names it"
    };
    let mut diagnostic = Diagnostic::new(
        Code::UnknownMaster,
        format!("theme `{}` has no master `{name}`", theme.name),
    )
    .at(slide.span, String::from(label));
    if let Some(near) = master_rules::closest(name, &known) {
        diagnostic = diagnostic.help(format!("did you mean `{near}`?"));
    }
    diagnostic
        .note(format!(
            "theme `{}` has the masters {}",
            theme.name,
            names(known.iter())
        ))
        .on_slide(SlideRef {
            number: slide.number,
            id: slide.id.clone(),
            master: Some(name.clone()),
        })
}

/// The master of a slide with its index in the theme, or `None` after reporting why it has none.
fn master_of<'a>(
    theme: &'a Theme,
    deck: &Deck,
    slide: &Slide,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<(usize, &'a Master)> {
    let (name, defaulted) =
        match master_rules::resolve(slide.master.as_ref(), deck.head.default_master.as_ref()) {
            Resolution::Named(name) => (name, false),
            Resolution::Defaulted(name) => (name, true),
            Resolution::Missing => {
                diagnostics.push(missing_master(theme, slide));
                return None;
            }
        };
    let found = theme
        .masters
        .iter()
        .enumerate()
        .find(|(_, master)| master.name == *name);
    if found.is_none() {
        diagnostics.push(unknown_master(theme, slide, (name, defaulted)));
    }
    found
}

/// Records that the slot at `slot` took the block at `position`.
fn hold(regions: &mut Vec<BoundRegion>, slot: usize, position: usize) {
    match regions.iter_mut().find(|region| region.slot == slot) {
        Some(region) => region.blocks.push(position),
        None => regions.push(BoundRegion {
            slot,
            blocks: alloc::vec![position],
        }),
    }
}

fn bind_slide(
    scene: &Scene<'_>,
    index: SlideIndex,
    master: usize,
    diagnostics: &mut Vec<Diagnostic>,
) -> BoundSlide {
    let mut state = BindState::default();
    let mut regions = Vec::new();
    for (position, block) in scene.slide.blocks.iter().enumerate() {
        let Ok(input) = input(block) else {
            diagnostics.push(scene.too_large(block));
            continue;
        };
        let read = Read { block, input };
        match binder::step(&scene.slots, state, read.input) {
            Step::Bound { slot, state: next } => {
                state = next;
                hold(&mut regions, slot, position);
            }
            Step::Refused { code, state: same } => {
                state = same;
                diagnostics.push(scene.refusal(state, read, code));
            }
        }
    }
    if let Some(slot) = binder::unfilled(&scene.slots, state) {
        diagnostics.push(scene.empty(slot));
    }
    BoundSlide {
        index,
        master,
        regions,
    }
}

/// Binds every slide of `deck` to a master of `theme`.
///
/// # Errors
///
/// Returns every binding diagnostic when any slide cannot be bound.
pub fn bind(deck: &Deck, theme: &Theme) -> Result<BoundDeck, Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    let mut slides = Vec::new();
    for (position, slide) in deck.slides.iter().enumerate() {
        let Some((master, found)) = master_of(theme, deck, slide, &mut diagnostics) else {
            continue;
        };
        let Ok(index) = u32::try_from(position) else {
            diagnostics.push(
                Diagnostic::new(
                    Code::LimitExceeded,
                    format!(
                        "slide {} is beyond the slides the binder counts",
                        slide.number
                    ),
                )
                .at(slide.span, String::from("this slide")),
            );
            continue;
        };
        let scene = Scene {
            theme,
            master: found,
            slots: grammar(found),
            slide,
        };
        slides.push(bind_slide(
            &scene,
            SlideIndex(index),
            master,
            &mut diagnostics,
        ));
    }
    if diagnostics.is_empty() {
        Ok(BoundDeck { slides })
    } else {
        Err(diagnostics)
    }
}

#[cfg(test)]
mod tests {
    use super::{BoundRegion, bind, grammar, input};
    use crate::binder::Input;
    use crate::diagnostic::Code;
    use crate::ir::{
        Alignment, AssetRef, Block, BlockBody, BlockKind, Deck, Head, Heading, HeadingLevel, Image,
        Inline, List, ListItem, ListKind, Paragraph, PlainText, Slide, Table, TableRow,
    };
    use crate::span::Span;
    use crate::text::Identifier;
    use crate::theme::fixture::{id, master, slot, tiny};
    use crate::theme::{Binding, BlockKindSet, Capacity, Slot, Theme};
    use alloc::string::String;
    use alloc::vec;
    use alloc::vec::Vec;

    const MAX_ITEMS: u32 = 2;

    /// The master `page`: a required title, a body of paragraphs and lists of up to two items, a caption after the body, and one section.
    fn page() -> Theme {
        let mut theme = tiny();
        let mut page = master("page", &["title", "body", "caption", "left"]);
        page.slots = vec![
            Slot {
                binds: Binding::Title,
                accepts: BlockKindSet::EMPTY,
                required: true,
                ..slot("title")
            },
            Slot {
                accepts: BlockKindSet::EMPTY
                    .with(BlockKind::Paragraph)
                    .with(BlockKind::List),
                capacity: Capacity {
                    items: Some(MAX_ITEMS),
                    depth: Some(1),
                    ..Capacity::default()
                },
                ..slot("body")
            },
            Slot {
                follows: Some(id("body")),
                capacity: Capacity {
                    blocks: Some(1),
                    ..Capacity::default()
                },
                ..slot("caption")
            },
            Slot {
                binds: Binding::Section(1),
                ..slot("left")
            },
        ];
        theme.masters.push(page);
        theme
    }

    fn block(line: usize, body: BlockBody) -> Block {
        Block {
            body,
            span: Span::line(line, 1),
        }
    }

    fn heading(line: usize, level: HeadingLevel) -> Block {
        block(
            line,
            BlockBody::Heading(Heading {
                level,
                inlines: vec![Inline::Text(String::from("t"))],
            }),
        )
    }

    fn paragraph(line: usize) -> Block {
        block(
            line,
            BlockBody::Paragraph(Paragraph {
                inlines: vec![Inline::Text(String::from("p"))],
            }),
        )
    }

    fn list(line: usize, items: usize) -> Block {
        block(
            line,
            BlockBody::List(List {
                kind: ListKind::Bullet,
                items: (0..items)
                    .map(|_| ListItem {
                        span: Span::line(line, 1),
                        inlines: Vec::new(),
                        nested: None,
                    })
                    .collect(),
            }),
        )
    }

    fn deck(master: Option<&str>, blocks: Vec<Block>) -> Deck {
        Deck {
            head: Head::default(),
            slides: vec![Slide {
                number: 1,
                span: Span::line(1, 1),
                master: master.map(|name| Identifier::new(name).unwrap()),
                id: None,
                blocks,
                notes: None,
            }],
        }
    }

    fn codes(deck: &Deck, theme: &Theme) -> Vec<Code> {
        match bind(deck, theme) {
            Ok(_) => Vec::new(),
            Err(diagnostics) => diagnostics.iter().map(|d| d.code).collect(),
        }
    }

    #[test]
    fn a_slide_binds_its_blocks_to_regions_in_order() {
        let theme = page();
        let deck = deck(
            Some("page"),
            vec![
                heading(1, HeadingLevel::Title),
                paragraph(3),
                list(5, 2),
                paragraph(8),
                heading(10, HeadingLevel::Section),
                paragraph(12),
            ],
        );
        let bound = bind(&deck, &theme).unwrap();
        let [slide] = bound.slides.as_slice() else {
            panic!("one slide");
        };
        assert_eq!(slide.master, 2);
        assert_eq!(
            slide.regions,
            vec![
                BoundRegion {
                    slot: 0,
                    blocks: vec![0]
                },
                BoundRegion {
                    slot: 1,
                    blocks: vec![1, 2]
                },
                BoundRegion {
                    slot: 2,
                    blocks: vec![3]
                },
                BoundRegion {
                    slot: 3,
                    blocks: vec![4, 5]
                },
            ]
        );
    }

    #[test]
    fn a_missing_or_unknown_master_is_reported_with_the_known_names() {
        let theme = page();
        assert_eq!(
            codes(&deck(None, vec![paragraph(1)]), &theme),
            vec![Code::MasterMissing]
        );
        let unknown = bind(&deck(Some("pag"), vec![paragraph(1)]), &theme).unwrap_err();
        assert_eq!(unknown[0].code, Code::UnknownMaster);
        assert_eq!(unknown[0].helps, vec!["did you mean `page`?"]);
        assert_eq!(
            unknown[0].notes,
            vec!["theme `tiny` has the masters `a`, `b`, `page`"]
        );
        let mut defaulted = deck(None, vec![paragraph(1)]);
        defaulted.head.default_master = Some(Identifier::new("zz").unwrap());
        let found = bind(&defaulted, &theme).unwrap_err();
        assert!(
            found[0]
                .primary
                .as_ref()
                .unwrap()
                .text
                .contains("default_master")
        );
        assert_eq!(found[0].helps, Vec::<String>::new());
    }

    #[test]
    fn every_refusal_of_the_binder_names_what_the_master_lacks() {
        let theme = page();
        let title = heading(1, HeadingLevel::Title);
        let full = bind(&deck(Some("page"), vec![title.clone(), list(3, 3)]), &theme).unwrap_err();
        assert_eq!(full[0].code, Code::CapacityExceeded);
        assert_eq!(
            full[0].notes,
            vec!["slot `body` takes at most 2 items (`max_items`); this block has 3"]
        );
        let image = block(
            3,
            BlockBody::Image(Image {
                path: AssetRef::new("a.png").unwrap(),
                alt: PlainText::default(),
            }),
        );
        let homeless = bind(&deck(Some("page"), vec![title.clone(), image]), &theme).unwrap_err();
        assert_eq!(homeless[0].code, Code::NoSlotForBlock);
        assert!(homeless[0].helps[0].contains("remove the block"));
        let sections = bind(
            &deck(
                Some("page"),
                vec![
                    title.clone(),
                    heading(3, HeadingLevel::Section),
                    heading(5, HeadingLevel::Section),
                ],
            ),
            &theme,
        )
        .unwrap_err();
        assert_eq!(sections[0].code, Code::SectionCount);
        assert!(sections[0].message.contains("1 section slot"));
        let late = bind(
            &deck(Some("page"), vec![title, heading(3, HeadingLevel::Title)]),
            &theme,
        )
        .unwrap_err();
        assert_eq!(late[0].code, Code::TitlePosition);
        assert_eq!(
            codes(&deck(Some("page"), vec![paragraph(1)]), &theme),
            vec![Code::RequiredSlotEmpty]
        );
        let titleless = bind(
            &deck(Some("a"), vec![heading(1, HeadingLevel::Title)]),
            &theme,
        )
        .unwrap_err();
        assert_eq!(titleless[0].code, Code::TitlePosition);
        assert!(titleless[0].message.contains("has no title slot"));
        assert_eq!(
            titleless[0].helps,
            vec!["use a master with a title slot: `page`"]
        );
    }

    /// A master with one flow slot of paragraphs that holds one block.
    fn narrow() -> Theme {
        let mut theme = tiny();
        let mut narrow = master("narrow", &[]);
        narrow.slots = vec![Slot {
            capacity: Capacity {
                blocks: Some(1),
                ..Capacity::default()
            },
            ..slot("only")
        }];
        theme.masters.push(narrow);
        let mut sectioned = master("sectioned", &[]);
        sectioned.slots = vec![
            Slot {
                binds: Binding::Section(1),
                accepts: BlockKindSet::EMPTY
                    .with(BlockKind::Paragraph)
                    .with(BlockKind::List),
                capacity: Capacity {
                    items: Some(1),
                    ..Capacity::default()
                },
                ..slot("left")
            },
            Slot {
                accepts: BlockKindSet::EMPTY.with(BlockKind::List),
                capacity: Capacity {
                    items: Some(MAX_ITEMS),
                    ..Capacity::default()
                },
                ..slot("body")
            },
        ];
        theme.masters.push(sectioned);
        let mut flow_only = master("flow-only", &[]);
        flow_only.slots = vec![Slot {
            accepts: BlockKindSet::EMPTY
                .with(BlockKind::Paragraph)
                .with(BlockKind::List),
            required: true,
            ..slot("body")
        }];
        theme.masters.push(flow_only);
        theme
    }

    #[test]
    fn capacity_notes_name_only_the_slots_in_play() {
        let theme = narrow();
        let blocks = bind(
            &deck(Some("narrow"), vec![paragraph(1), paragraph(3)]),
            &theme,
        )
        .unwrap_err();
        assert_eq!(blocks[0].code, Code::CapacityExceeded);
        assert_eq!(
            blocks[0].notes,
            vec!["slot `only` already holds 1 blocks, its `max_blocks`"]
        );
        let inside = bind(
            &deck(
                Some("sectioned"),
                vec![heading(1, HeadingLevel::Section), list(3, 3)],
            ),
            &theme,
        )
        .unwrap_err();
        assert_eq!(inside[0].code, Code::CapacityExceeded);
        assert_eq!(
            inside[0].notes,
            vec!["slot `left` takes at most 1 items (`max_items`); this block has 3"]
        );
    }

    #[test]
    fn an_empty_required_flow_slot_names_the_kinds_it_accepts() {
        let theme = narrow();
        let empty = bind(&deck(Some("flow-only"), Vec::new()), &theme).unwrap_err();
        assert_eq!(empty[0].code, Code::RequiredSlotEmpty);
        assert_eq!(
            empty[0].helps,
            vec!["add a block the slot accepts: paragraph, list"]
        );
    }

    #[test]
    fn every_block_kind_has_its_name() {
        let names: Vec<&str> = BlockKind::ALL.iter().map(|kind| kind.name()).collect();
        assert_eq!(
            names,
            [
                "heading",
                "paragraph",
                "list",
                "code",
                "quote",
                "image",
                "table"
            ]
        );
    }

    #[test]
    fn the_grammar_and_the_input_mirror_the_master_and_the_block() {
        let theme = page();
        let slots = grammar(&theme.masters[2]);
        assert_eq!(slots[2].follows, Some(1));
        assert_eq!(slots[0].binds, Binding::Title);
        assert!(slots[0].required);
        let Input::Block(facts) = input(&list(1, 2)).unwrap() else {
            panic!("a block");
        };
        assert_eq!(
            (facts.kind, facts.items, facts.depth),
            (BlockKind::List, 2, 1)
        );
        assert_eq!(
            input(&heading(1, HeadingLevel::Section)).unwrap(),
            Input::Heading(HeadingLevel::Section)
        );
        let row = |line: usize| TableRow {
            span: Span::line(line, 1),
            cells: vec![Vec::new(), Vec::new()],
        };
        let table = block(
            1,
            BlockBody::Table(Table {
                alignments: vec![Alignment::Unset, Alignment::Left],
                header: row(1),
                rows: vec![row(3)],
            }),
        );
        let Input::Block(cells) = input(&table).unwrap() else {
            panic!("a block");
        };
        assert_eq!(
            (cells.kind, cells.rows, cells.columns),
            (BlockKind::Table, 1, 2)
        );
    }
}
