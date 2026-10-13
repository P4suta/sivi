//! The Typst source of a bound deck: every slide placed on its own page, every bound region measured, and every author text inside a string literal.
//!
//! The emitter and the adapter share `protocol`, so the labels written here are the labels the adapter reads back.
//! Author text reaches the source only through [`string_literal`], so Typst markup in a deck is text and never code.

use crate::bind::{BoundDeck, BoundRegion, BoundSlide};
use crate::diagnostic::{Code, Diagnostic};
use crate::escape_rules::string_literal;
use crate::ir::{
    self, Alignment, AssetRef, Block, BlockBody, CodeBlock, Deck, Heading, HeadingLevel, Image,
    Inline, LinkDest, List, ListKind, Paragraph, Slide, Table,
};
use crate::length::{Length, Rect};
use crate::mark_rules;
use crate::measure::{Expected, ExpectedRegion};
use crate::protocol::{
    MeasureLabel, PREFIXES_KEY, REQUIRED_KEY, RegionIndex, SLIDE_KEY, SlideIndex, SlideLabel,
    WIDTHS_KEY,
};
use crate::softbreak_rules::{Joiner, joiner};
use crate::theme::{
    Align, Decor, FamilyName, Field, FontStack, Mark, Measure, Slot, StyleName, TextStyle, Theme,
    Thousandths, Underline, VerticalAlign,
};
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

/// The generated Typst source.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypstSource(String);

impl TypstSource {
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for TypstSource {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// The natural line box of each font family as a multiple of the size, in thousandths, which the leading of a style subtracts from its pitch.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct LineBoxes(Vec<(FamilyName, Thousandths)>);

impl LineBoxes {
    #[must_use]
    pub const fn new(boxes: Vec<(FamilyName, Thousandths)>) -> Self {
        Self(boxes)
    }

    /// The line box of `family`, or one em for a family no font describes.
    #[must_use]
    pub fn of(&self, family: &FamilyName) -> Thousandths {
        self.0
            .iter()
            .find(|(known, _)| known == family)
            .map_or(Thousandths::ONE, |(_, line_box)| *line_box)
    }
}

/// Where a file the source names comes from.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum AssetOrigin {
    /// An image of the deck, relative to the deck.
    Deck(AssetRef),
    /// An image of the theme, relative to the theme.
    Theme(String),
}

/// A file the source names by its virtual path.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct AssetUse {
    pub path: String,
    pub origin: AssetOrigin,
    /// The width the file is shown at, from which its resolution follows.
    pub width: Length,
}

/// The source, what the adapter must read back from it, and the files it names.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Emitted {
    pub source: TypstSource,
    pub expected: Expected,
    pub assets: Vec<AssetUse>,
}

/// The virtual directory of the deck's images.
pub const DECK_ASSET_PREFIX: &str = "deck/";
/// The virtual directory of the theme's images.
pub const THEME_ASSET_PREFIX: &str = "theme/";
/// The pattern in a page number format that the number replaces.
const PAGE_NUMBER_PATTERN: &str = "{n}";
/// The numbering of page numbers, Arabic.
const PAGE_NUMBERING: &str = "1";
/// The stroke under every table cell.
const TABLE_RULE: &str = "0.5pt";
/// The factor of the free height a region's content shifts by, per vertical alignment.
const TOP_SHIFT: &str = "0.0";
const MIDDLE_SHIFT: &str = "0.5";
const BOTTOM_SHIFT: &str = "1.0";
const THOUSAND: i64 = 1000;
/// The heading levels of a slide title and of a section heading, which the PDF outline nests.
const TITLE_LEVEL: u8 = 1;
const SECTION_LEVEL: u8 = 2;

/// One part of a region's content, measured on its own for the split suggestion.
struct Part {
    code: String,
    /// An expression whose natural width is the widest line of the part, for a part that never wraps.
    natural: Option<String>,
}

/// One bound region to place.
#[derive(Clone, Copy)]
struct Placement<'a> {
    slide: &'a Slide,
    slot: &'a Slot,
    region: &'a BoundRegion,
    at: (SlideIndex, RegionIndex),
}

fn lit(text: &str) -> String {
    string_literal(text).as_str().into()
}

/// A multiple of the font size, such as `1.250em`.
fn em(value: Thousandths) -> String {
    let whole = value.0.checked_div(THOUSAND).unwrap_or(0);
    let fraction = value.0.checked_rem(THOUSAND).unwrap_or(0).unsigned_abs();
    format!("{whole}.{fraction:03}em")
}

fn measure(value: Measure) -> String {
    match value {
        Measure::Points(length) => format!("{length}"),
        Measure::Em(value) => em(value),
    }
}

const fn align_name(align: Align) -> &'static str {
    match align {
        Align::Left => "left",
        Align::Center => "center",
        Align::Right => "right",
    }
}

const fn column_alignment(alignment: Alignment) -> &'static str {
    match alignment {
        Alignment::Unset => "auto",
        Alignment::Left => "left",
        Alignment::Center => "center",
        Alignment::Right => "right",
    }
}

const fn shift_factor(valign: VerticalAlign) -> &'static str {
    match valign {
        VerticalAlign::Top => TOP_SHIFT,
        VerticalAlign::Middle => MIDDLE_SHIFT,
        VerticalAlign::Bottom => BOTTOM_SHIFT,
    }
}

struct Emitter<'a> {
    theme: &'a Theme,
    deck: &'a Deck,
    boxes: &'a LineBoxes,
    source: String,
    expected: Expected,
    assets: Vec<AssetUse>,
    errors: Vec<Diagnostic>,
}

impl Emitter<'_> {
    fn push(&mut self, text: &str) {
        self.source.push_str(text);
    }

    fn internal(&mut self, message: String) {
        self.errors.push(Diagnostic::new(Code::Internal, message));
    }

    /// The color `name` as Typst spells it, or black after recording that the theme lacks it.
    fn rgb(&mut self, name: &StyleName) -> String {
        let theme = self.theme;
        theme.color(name).map_or_else(
            || {
                self.internal(format!("the theme names the color `{name}` but lacks it"));
                String::from("black")
            },
            |color| {
                format!(
                    "rgb(\"#{:02x}{:02x}{:02x}\")",
                    color.red, color.green, color.blue
                )
            },
        )
    }

    /// The font list of `stack`, continuing with the theme's fallback families it does not already name.
    fn families(&self, stack: &FontStack) -> String {
        let mut listed: Vec<String> = stack
            .families()
            .map(|family| lit(family.as_str()))
            .collect();
        for family in &self.theme.typography.fallback {
            if !stack.names(family) {
                listed.push(lit(family.as_str()));
            }
        }
        format!("({},)", listed.join(", "))
    }

    /// The leading that gives `family` at `size` a line pitch of `line_height` times the size.
    fn leading(&self, family: &FamilyName, size: Length, line_height: Thousandths) -> Length {
        let extra = line_height.0.saturating_sub(self.boxes.of(family).0);
        let millipoints = size
            .0
            .saturating_mul(extra)
            .checked_div(THOUSAND)
            .unwrap_or(0);
        Length::from_millipoints(millipoints.max(0))
    }

    /// The set rules that put a block in `style`, aligned by `align` when it overrides the style.
    fn settings(&mut self, style: &TextStyle, align: Option<Align>) -> String {
        let leading = self.leading(style.font.primary(), style.size, style.line_height);
        let fill = self.rgb(&style.color);
        format!(
            "set text(font: {}, size: {}, weight: {}, fill: {}); set par(leading: {leading}, justify: false); set align({});",
            self.families(&style.font),
            style.size,
            style.weight.value(),
            fill,
            align_name(align.unwrap_or(style.align))
        )
    }

    fn styled(&mut self, style: &TextStyle, align: Option<Align>, inner: &str) -> String {
        format!("{{ {} {inner} }}", self.settings(style, align))
    }

    fn preamble(&mut self) {
        let theme = self.theme;
        let fill = self.rgb(&theme.page.background);
        self.push(&format!(
            "#set page(width: {}, height: {}, margin: 0pt, fill: {fill})\n",
            theme.page.size.width, theme.page.size.height
        ));
        let first = theme
            .typography
            .fallback
            .first()
            .cloned()
            .map(FontStack::single)
            .or_else(|| theme.text.first().map(|(_, style)| style.font.clone()));
        let lang = self.deck.head.lang.as_ref().map_or(String::new(), |lang| {
            let region = lang
                .as_str()
                .split_once('-')
                .map_or(String::new(), |(_, region)| {
                    format!(", region: {}", lit(region))
                });
            format!(", lang: {}{region}", lit(lang.primary()))
        });
        if let Some(stack) = first {
            self.push(&format!(
                "#set text(font: {}, fallback: false, top-edge: \"ascender\", bottom-edge: \"descender\"{lang})\n",
                self.families(&stack)
            ));
        }
        self.push("#set par(justify: false, spacing: 0pt)\n");
        if let Some(title) = &self.deck.head.title {
            self.push(&format!("#set document(title: {})\n", lit(title.as_str())));
        }
        if let Some(author) = &self.deck.head.author {
            self.push(&format!(
                "#set document(author: {})\n",
                lit(author.as_str())
            ));
        }
        self.push("#set document(date: none)\n");
        self.push("#set heading(numbering: none, outlined: true, bookmarked: true)\n");
        let highlight = if theme.code.highlight {
            ""
        } else {
            ", theme: none"
        };
        self.push(&format!(
            "#set raw(tab-size: {}{highlight})\n",
            theme.code.tab_width
        ));
        self.push(&format!(
            "#show raw: set text(font: {})\n",
            self.families(&theme.code.font)
        ));
        self.push("#show raw.line: it => box(it)\n");
    }

    fn slide(&mut self, bound: &BoundSlide) {
        let (Ok(position), Some(master)) = (
            usize::try_from(bound.index.0),
            self.theme.masters.get(bound.master),
        ) else {
            self.internal(format!(
                "slide {} is bound to master {}, which the theme lacks",
                bound.index.0, bound.master
            ));
            return;
        };
        let Some(slide) = self.deck.slides.get(position) else {
            self.internal(format!("slide {position} is bound but the deck lacks it"));
            return;
        };
        self.push(&format!(
            "#metadata(({SLIDE_KEY}: {})) <{}>\n",
            bound.index.0,
            SlideLabel::new(bound.index)
        ));
        for decor in &master.decor {
            self.decor(decor);
        }
        for (number, region) in bound.regions.iter().enumerate() {
            let (Some(slot), Ok(index)) = (master.slots.get(region.slot), u32::try_from(number))
            else {
                self.internal(format!(
                    "slide {} binds slot {}, which master `{}` lacks",
                    slide.number, region.slot, master.name
                ));
                continue;
            };
            self.region(Placement {
                slide,
                slot,
                region,
                at: (bound.index, RegionIndex(index)),
            });
        }
    }

    /// The value of a head field, when the deck sets it.
    fn field(&self, field: Field) -> Option<String> {
        let head = &self.deck.head;
        match field {
            Field::Title => head
                .title
                .as_ref()
                .map(|title| String::from(title.as_str())),
            Field::Author => head
                .author
                .as_ref()
                .map(|author| String::from(author.as_str())),
            Field::Date => head.date.map(|date| format!("{date}")),
        }
    }

    fn page_number(&mut self, style: &StyleName, pattern: &str) -> String {
        let Some(style) = self.theme.text_style(style) else {
            self.internal(format!(
                "a page number names the style `{style}`, which the theme lacks"
            ));
            return String::new();
        };
        let settings = self.settings(style, None);
        let counter = format!("(context counter(page).display({}))", lit(PAGE_NUMBERING));
        let content = pattern
            .split(PAGE_NUMBER_PATTERN)
            .map(|text| format!("text({})", lit(text)))
            .collect::<Vec<_>>()
            .join(&format!(" + {counter} + "));
        format!("{{ {settings} par({content}) }}")
    }

    fn decor(&mut self, decor: &Decor) {
        let region = decor.region();
        let (width, height) = (region.size.width, region.size.height);
        let inner = match decor {
            Decor::Rect { color, .. } => {
                let fill = self.rgb(color);
                format!("rect(width: {width}, height: {height}, fill: {fill}, stroke: none)")
            }
            Decor::Image { path, .. } => {
                let virtual_path = format!("{THEME_ASSET_PREFIX}{path}");
                self.assets.push(AssetUse {
                    path: virtual_path.clone(),
                    origin: AssetOrigin::Theme(path.clone()),
                    width,
                });
                format!(
                    "image({}, width: {width}, height: {height}, fit: \"contain\")",
                    lit(&virtual_path)
                )
            }
            Decor::PageNumber {
                text,
                align,
                format: pattern,
                ..
            } => {
                let body = self.page_number(text, pattern);
                format!(
                    "block(width: {width}, height: {height}, align({} + horizon, {body}))",
                    align_name(*align)
                )
            }
            Decor::Field {
                text, align, field, ..
            } => {
                let (Some(value), Some(style)) = (self.field(*field), self.theme.text_style(text))
                else {
                    return;
                };
                let settings = self.settings(style, Some(*align));
                format!(
                    "block(width: {width}, height: {height}, align({} + horizon, {{ {settings} par(text({})) }}))",
                    align_name(*align),
                    lit(&value)
                )
            }
        };
        self.push(&format!(
            "#place(top + left, dx: {}, dy: {}, {inner})\n",
            region.origin.x, region.origin.y
        ));
    }

    /// The gap between the blocks of a region, resolved against the slot's text size.
    fn gap(&self, slot: &Slot) -> Length {
        match self.theme.typography.paragraph_gap {
            Measure::Points(length) => length,
            Measure::Em(value) => {
                let size = self
                    .theme
                    .text_style(&slot.text)
                    .map_or(Length::ZERO, |style| style.size);
                Length::from_millipoints(
                    size.0
                        .saturating_mul(value.0)
                        .checked_div(THOUSAND)
                        .unwrap_or(0),
                )
            }
        }
    }

    fn region(&mut self, placement: Placement<'_>) {
        let Placement {
            slide,
            slot,
            region,
            at: (slide_index, region_index),
        } = placement;
        let sole = region.blocks.len() == 1;
        let mut parts = Vec::new();
        for position in &region.blocks {
            let Some(block) = slide.blocks.get(*position) else {
                self.internal(format!(
                    "slide {} binds block {position}, which it lacks",
                    slide.number
                ));
                continue;
            };
            if let Some(part) = self.part(slot, block, sole) {
                parts.push(part);
            }
        }
        let Ok(split_points) = u32::try_from(parts.len()) else {
            self.internal(format!("slide {} holds too many blocks", slide.number));
            return;
        };
        if parts.is_empty() {
            return;
        }
        let bounds: Rect = slot.region;
        let (width, height) = (bounds.size.width, bounds.size.height);
        let gap = self.gap(slot);
        let label = MeasureLabel::new(slide_index, region_index);
        let codes: Vec<&str> = parts.iter().map(|part| part.code.as_str()).collect();
        let widths: Vec<String> = parts
            .iter()
            .map(|part| {
                part.natural.as_ref().map_or_else(
                    || String::from("0pt"),
                    |natural| format!("measure({natural}).width"),
                )
            })
            .collect();
        let factor = shift_factor(slot.valign);
        self.push(&format!(
            "#place(top + left, dx: {}, dy: {}, block(width: {width}, height: {height}, {{\n  let parts = ({},)\n  let body = stack(dir: ttb, spacing: {gap}, ..parts)\n  context {{\n    let m = measure(width: {width}, body)\n    let prefixes = range(1, parts.len() + 1).map(k => measure(width: {width}, stack(dir: ttb, spacing: {gap}, ..parts.slice(0, k))).height)\n    let widths = ({},)\n    let shift = calc.max(0pt, ({height} - m.height) * {factor})\n    [#metadata(({REQUIRED_KEY}: m.height, {PREFIXES_KEY}: prefixes, {WIDTHS_KEY}: widths)) <{label}>]\n    place(top + left, dy: shift, body)\n  }}\n}}))\n",
            bounds.origin.x,
            bounds.origin.y,
            codes.join(", "),
            widths.join(", ")
        ));
        self.expected.regions.push(ExpectedRegion {
            slide: slide_index,
            region: region_index,
            bounds,
            split_points,
        });
    }

    fn part(&mut self, slot: &Slot, block: &Block, sole: bool) -> Option<Part> {
        let theme = self.theme;
        let Some(style) = theme.text_style(&slot.text) else {
            self.internal(format!(
                "slot `{}` names the text style `{}`, which the theme lacks",
                slot.name, slot.text
            ));
            return None;
        };
        let mut natural = None;
        let code = match &block.body {
            BlockBody::Heading(heading) => self.heading(slot, style, heading),
            BlockBody::Paragraph(paragraph) => {
                let inner = self.paragraph(paragraph);
                self.styled(style, slot.align, &inner)
            }
            BlockBody::List(list) => {
                let inner = self.list(list);
                self.styled(style, slot.align, &inner)
            }
            BlockBody::Code(code) => {
                let (placed, widest) = self.code(style, code);
                natural = Some(widest);
                placed
            }
            BlockBody::Quote(paragraph) => self.quote(style, slot.align, paragraph),
            BlockBody::Image(image) => self.image(image, slot.region, sole),
            BlockBody::Table(table) => {
                let inner = self.table(table);
                let Some(table_style) = theme.text_style(&theme.tables.text) else {
                    self.internal(format!(
                        "the tables name the text style `{}`, which the theme lacks",
                        theme.tables.text
                    ));
                    return None;
                };
                self.styled(table_style, slot.align, &inner)
            }
        };
        Some(Part { code, natural })
    }

    fn heading(&mut self, slot: &Slot, style: &TextStyle, heading: &Heading) -> String {
        let theme = self.theme;
        let (level, style) = match heading.level {
            HeadingLevel::Title => (TITLE_LEVEL, style),
            HeadingLevel::Section => (
                SECTION_LEVEL,
                theme
                    .text_style(
                        &StyleName::new("heading").unwrap_or_else(|_invalid| slot.text.clone()),
                    )
                    .unwrap_or(style),
            ),
        };
        let inner = self.inlines(&heading.inlines);
        let fill = self.rgb(&style.color);
        let leading = self.leading(style.font.primary(), style.size, style.line_height);
        let rules = format!(
            "show heading: set text(font: {}, size: {}, weight: {}, fill: {fill}); show heading: set block(above: 0pt, below: 0pt); show heading: set par(leading: {leading}, justify: false);",
            self.families(&style.font),
            style.size,
            style.weight.value()
        );
        self.styled(
            style,
            slot.align,
            &format!("{rules} heading(level: {level}, {inner})"),
        )
    }

    fn paragraph(&mut self, paragraph: &Paragraph) -> String {
        format!("par({})", self.inlines(&paragraph.inlines))
    }

    fn quote(&mut self, style: &TextStyle, align: Option<Align>, paragraph: &Paragraph) -> String {
        let theme = self.theme;
        let bar = self.rgb(&theme.quotes.bar);
        let inner = self.paragraph(paragraph);
        self.styled(
            style,
            align,
            &format!(
                "block(width: 100%, inset: (left: {}), stroke: (left: {} + {bar}), {inner})",
                theme.quotes.indent, theme.quotes.bar_width
            ),
        )
    }

    fn code(&mut self, style: &TextStyle, code: &CodeBlock) -> (String, String) {
        let theme = self.theme;
        let leading = self.leading(
            theme.code.font.primary(),
            theme.code.size,
            theme.code.line_height,
        );
        let fill = self.rgb(&style.color);
        let background = self.rgb(&theme.code.background);
        let lang = code.language.as_ref().map_or(String::new(), |language| {
            format!(", lang: {}", lit(language))
        });
        // The raw element sets its own font, so only a show-set rule reaches it.
        let rules = format!(
            "show raw: set text(font: {}, size: {}, fill: {fill}); set par(leading: {leading}, justify: false); set align(left);",
            self.families(&theme.code.font),
            theme.code.size
        );
        let raw = format!("raw(block: true{lang}, {})", lit(&code.text));
        (
            format!(
                "{{ {rules} block(width: 100%, fill: {background}, inset: {}, {raw}) }}",
                theme.code.padding
            ),
            format!("{{ {rules} {raw} }}"),
        )
    }

    fn image(&mut self, image: &Image, bounds: Rect, sole: bool) -> String {
        let virtual_path = format!("{DECK_ASSET_PREFIX}{}", image.path.as_str());
        if !self.assets.iter().any(|asset| asset.path == virtual_path) {
            self.assets.push(AssetUse {
                path: virtual_path.clone(),
                origin: AssetOrigin::Deck(image.path.clone()),
                width: bounds.size.width,
            });
        }
        let size = if sole {
            format!(
                "width: {}, height: {}",
                bounds.size.width, bounds.size.height
            )
        } else {
            String::from("width: 100%")
        };
        format!(
            "image({}, alt: {}, {size}, fit: \"contain\")",
            lit(&virtual_path),
            lit(image.alt.as_str())
        )
    }

    fn list(&mut self, list: &List) -> String {
        let theme = self.theme;
        let mut items = Vec::new();
        for item in &list.items {
            let mut body = self.inlines(&item.inlines);
            if let Some(nested) = &item.nested {
                body = format!("{body} + {}", self.list(nested));
            }
            items.push(body);
        }
        let spacing = format!(
            "indent: 0pt, body-indent: {}, spacing: {}, tight: true",
            measure(theme.lists.indent),
            measure(theme.lists.gap)
        );
        match list.kind {
            ListKind::Bullet => {
                let markers: Vec<String> = theme
                    .lists
                    .bullets
                    .iter()
                    .map(|bullet| format!("text({})", lit(bullet)))
                    .collect();
                let items: Vec<String> = items
                    .into_iter()
                    .map(|body| format!("list.item({body})"))
                    .collect();
                format!(
                    "list(marker: ({},), {spacing}, {})",
                    markers.join(", "),
                    items.join(", ")
                )
            }
            ListKind::Ordered { start } => {
                let items: Vec<String> = items
                    .into_iter()
                    .map(|body| format!("enum.item({body})"))
                    .collect();
                format!(
                    "enum(start: {start}, numbering: {}, {spacing}, {})",
                    lit(&theme.lists.ordered),
                    items.join(", ")
                )
            }
        }
    }

    fn table(&mut self, table: &Table) -> String {
        let theme = self.theme;
        let rule = self.rgb(&theme.tables.rule);
        let columns = table.header.cells.len();
        let alignments: Vec<&str> = table
            .alignments
            .iter()
            .map(|alignment| column_alignment(*alignment))
            .collect();
        let mut header = Vec::new();
        for cell in &table.header.cells {
            let inner = self.inlines(cell);
            header.push(format!(
                "text(weight: {}, {inner})",
                theme.tables.header_weight.value()
            ));
        }
        let mut cells = Vec::new();
        for row in &table.rows {
            for cell in &row.cells {
                cells.push(self.inlines(cell));
            }
        }
        format!(
            "table(columns: {columns}, align: ({},), inset: {}, stroke: (bottom: {TABLE_RULE} + {rule}), table.header({},), {})",
            alignments.join(", "),
            theme.tables.padding,
            header.join(", "),
            cells.join(", ")
        )
    }

    /// Wraps `inner` in the text changes of `mark`.
    fn marked(&mut self, mark: &Mark, inner: String) -> String {
        let mut arguments = Vec::new();
        if mark.italic {
            arguments.push(String::from("style: \"italic\""));
        }
        if let Some(weight) = mark.weight {
            arguments.push(format!("weight: {}", weight.value()));
        }
        if let Some(color) = &mark.color {
            arguments.push(format!("fill: {}", self.rgb(color)));
        }
        if let Some(font) = &mark.font {
            arguments.push(format!("font: {}", self.families(font)));
        }
        let mut content = if arguments.is_empty() {
            inner
        } else {
            format!("text({}, {inner})", arguments.join(", "))
        };
        if let Some(background) = &mark.background {
            content = format!("highlight(fill: {}, {content})", self.rgb(background));
        }
        if let Some(marker) = &mark.marker {
            content = format!(
                "highlight(fill: {}, top-edge: \"{}\", bottom-edge: \"{}\", extent: {}, radius: {}, {content})",
                self.rgba(&marker.color, marker.opacity),
                marker.top.name(),
                marker.bottom.name(),
                marker.extent,
                marker.radius
            );
        }
        if let Some(underline) = &mark.underline {
            let prefix = self.underline_arguments(underline);
            content = format!("underline({prefix}{content})");
        }
        content
    }

    /// The arguments of `underline` before its body, each followed by a comma, or nothing for the plain line.
    fn underline_arguments(&mut self, underline: &Underline) -> String {
        let mut pieces: Vec<String> = Vec::new();
        let color = underline
            .color
            .as_ref()
            .map(|name| self.rgba(name, underline.opacity));
        let stroke = match (underline.thickness, color) {
            (Some(thickness), Some(color)) => Some(format!("{thickness} + {color}")),
            (Some(thickness), None) => Some(format!("{thickness}")),
            (None, Some(color)) => Some(color),
            (None, None) => None,
        };
        if let Some(stroke) = stroke {
            pieces.push(format!("stroke: {stroke}, "));
        }
        if let Some(offset) = underline.offset {
            pieces.push(format!("offset: {offset}, "));
        }
        if underline.extent != Length::ZERO {
            pieces.push(format!("extent: {}, ", underline.extent));
        }
        if underline.behind {
            pieces.push(String::from("background: true, evade: false, "));
        }
        pieces.concat()
    }

    /// The color `name` at `opacity`, as `rgb("#rrggbbaa")`.
    fn rgba(&mut self, name: &StyleName, opacity: Thousandths) -> String {
        let Some(alpha) = mark_rules::alpha(opacity) else {
            self.internal(format!(
                "the theme sets the opacity {} thousandths, which leaves clear to solid",
                opacity.0
            ));
            return self.rgb(name);
        };
        if alpha == mark_rules::OPAQUE {
            return self.rgb(name);
        }
        let theme = self.theme;
        theme.color(name).map_or_else(
            || {
                self.internal(format!("the theme names the color `{name}` but lacks it"));
                String::from("black")
            },
            |color| {
                format!(
                    "rgb(\"#{:02x}{:02x}{:02x}{alpha:02x}\")",
                    color.red, color.green, color.blue
                )
            },
        )
    }

    fn link(&mut self, dest: &LinkDest, inner: String) -> String {
        match dest {
            LinkDest::Url(url) => format!("link({}, {inner})", lit(url)),
            LinkDest::Slide(id) => {
                let target = self
                    .deck
                    .slides
                    .iter()
                    .position(|slide| slide.id.as_ref() == Some(id));
                let Some(position) = target else {
                    self.internal(format!(
                        "a link names the slide `{id}`, which the deck lacks"
                    ));
                    return inner;
                };
                let Ok(index) = u32::try_from(position) else {
                    self.internal(format!(
                        "the slide `{id}` lies beyond the slides the adapter counts"
                    ));
                    return inner;
                };
                format!(
                    "link(label({}), {inner})",
                    lit(SlideLabel::new(SlideIndex(index)).as_str())
                )
            }
        }
    }

    /// The space a soft break becomes between `inlines[..index]` and the rest, or nothing between East Asian characters.
    fn soft_break(inlines: &[Inline], index: usize) -> Option<String> {
        let before = inlines
            .get(..index)
            .map(ir::plain_text)
            .and_then(|text| text.as_str().chars().last());
        let after = inlines
            .get(index.saturating_add(1)..)
            .map(ir::plain_text)
            .and_then(|text| text.as_str().chars().next());
        if let (Some(before), Some(after)) = (before, after)
            && joiner(before, after) == Joiner::Nothing
        {
            return None;
        }
        Some(format!("text({})", lit(" ")))
    }

    /// The content of inlines as one Typst expression.
    fn inlines(&mut self, inlines: &[Inline]) -> String {
        let theme = self.theme;
        let mut pieces = Vec::new();
        for (index, inline) in inlines.iter().enumerate() {
            let piece = match inline {
                Inline::Text(text) => format!("text({})", lit(text)),
                Inline::Emph(inner) => {
                    let inner = self.inlines(inner);
                    self.marked(&theme.inline.emphasis, inner)
                }
                Inline::Strong(inner) => {
                    let inner = self.inlines(inner);
                    self.marked(&theme.inline.strong, inner)
                }
                Inline::Code(text) => {
                    self.marked(&theme.inline.code, format!("text({})", lit(text)))
                }
                Inline::Strike(inner) => format!("strike({})", self.inlines(inner)),
                Inline::Mark(inner) => {
                    let inner = self.inlines(inner);
                    self.marked(&theme.inline.mark, inner)
                }
                Inline::Link {
                    dest,
                    inlines: inner,
                } => {
                    let inner = self.inlines(inner);
                    let marked = self.marked(&theme.inline.link, inner);
                    self.link(dest, marked)
                }
                Inline::SoftBreak => match Self::soft_break(inlines, index) {
                    Some(space) => space,
                    None => continue,
                },
            };
            pieces.push(piece);
        }
        if pieces.is_empty() {
            format!("text({})", lit(""))
        } else {
            pieces.join(" + ")
        }
    }
}

/// The Typst source of `bound`, what the adapter must read back, and the files the source names.
///
/// # Errors
///
/// Returns internal diagnostics when the binding names a master, slot, block, style, or color the theme or deck lacks.
pub fn typst_source(
    theme: &Theme,
    deck: &Deck,
    bound: &BoundDeck,
    boxes: &LineBoxes,
) -> Result<Emitted, Vec<Diagnostic>> {
    let mut emitter = Emitter {
        theme,
        deck,
        boxes,
        source: String::new(),
        expected: Expected::default(),
        assets: Vec::new(),
        errors: Vec::new(),
    };
    emitter.preamble();
    for (position, slide) in bound.slides.iter().enumerate() {
        if position > 0 {
            emitter.push("#pagebreak()\n");
        }
        emitter.slide(slide);
    }
    match u32::try_from(bound.slides.len()) {
        Ok(slides) => emitter.expected.slides = slides,
        Err(_beyond) => emitter.internal(String::from(
            "the deck holds more slides than the adapter counts",
        )),
    }
    if emitter.errors.is_empty() {
        Ok(Emitted {
            source: TypstSource(emitter.source),
            expected: emitter.expected,
            assets: emitter.assets,
        })
    } else {
        Err(emitter.errors)
    }
}

#[cfg(test)]
mod tests {
    use super::{AssetOrigin, DECK_ASSET_PREFIX, LineBoxes, typst_source};
    use crate::bind::bind;
    use crate::ir::{
        AssetRef, Block, BlockBody, Deck, Head, Heading, HeadingLevel, Image, Inline, LinkDest,
        List, ListItem, ListKind, Paragraph, PlainText, Slide,
    };
    use crate::length::{Length, Point, Rect, Size};
    use crate::protocol::{RegionIndex, SlideIndex};
    use crate::span::Span;
    use crate::text::{Identifier, Lang};
    use crate::theme::fixture::{family, slot, style, tiny};
    use crate::theme::{
        Align, Binding, BlockKindSet, BottomEdge, Decor, Marker, Slot, Theme, Thousandths, TopEdge,
        Underline,
    };
    use alloc::string::String;
    use alloc::vec;
    use alloc::vec::Vec;

    const WIDTH_MILLIPOINTS: i64 = 400_000;
    const HEIGHT_MILLIPOINTS: i64 = 100_000;

    fn region(y: i64) -> Rect {
        Rect::new(
            Point {
                x: Length::ZERO,
                y: Length::from_millipoints(y),
            },
            Size {
                width: Length::from_millipoints(WIDTH_MILLIPOINTS),
                height: Length::from_millipoints(HEIGHT_MILLIPOINTS),
            },
        )
    }

    /// The theme `tiny` with a master `page`: a title, a body of paragraphs, lists, and images, and a page number.
    fn theme() -> Theme {
        let mut theme = tiny();
        theme.typography.fallback = vec![family()];
        let mut page = crate::theme::fixture::master("page", &[]);
        page.slots = vec![
            Slot {
                binds: Binding::Title,
                accepts: BlockKindSet::EMPTY,
                region: region(0),
                ..slot("title")
            },
            Slot {
                accepts: BlockKindSet::EMPTY
                    .with(crate::ir::BlockKind::Paragraph)
                    .with(crate::ir::BlockKind::List)
                    .with(crate::ir::BlockKind::Image),
                region: region(HEIGHT_MILLIPOINTS),
                align: Some(Align::Center),
                ..slot("body")
            },
        ];
        page.decor = vec![Decor::PageNumber {
            region: region(2 * HEIGHT_MILLIPOINTS),
            text: style("meta"),
            align: Align::Right,
            format: String::from("p. {n}"),
        }];
        theme.masters.push(page);
        theme
    }

    fn block(body: BlockBody) -> Block {
        Block {
            body,
            span: Span::line(1, 1),
        }
    }

    fn slide(number: usize, id: Option<&str>, blocks: Vec<Block>) -> Slide {
        Slide {
            number,
            span: Span::line(1, 1),
            master: Some(Identifier::new("page").unwrap()),
            id: id.map(|name| Identifier::new(name).unwrap()),
            blocks,
            notes: None,
        }
    }

    /// Two slides: a title with a paragraph of soft breaks, marks, and a slide link, then a list; and twice the same image.
    fn sample() -> Deck {
        Deck {
            head: Head {
                lang: Some(Lang::new("ja").unwrap()),
                ..Head::default()
            },
            slides: vec![
                slide(
                    1,
                    Some("first"),
                    vec![
                        block(BlockBody::Heading(Heading {
                            level: HeadingLevel::Title,
                            inlines: vec![Inline::Text(String::from("say \"hi\" #no"))],
                        })),
                        block(BlockBody::Paragraph(Paragraph {
                            inlines: vec![
                                Inline::Text(String::from("猫")),
                                Inline::SoftBreak,
                                Inline::Text(String::from("犬")),
                                Inline::SoftBreak,
                                Inline::Emph(vec![Inline::Text(String::from("x"))]),
                                Inline::Link {
                                    dest: LinkDest::Slide(Identifier::new("second").unwrap()),
                                    inlines: vec![Inline::Text(String::from("go"))],
                                },
                            ],
                        })),
                        block(BlockBody::List(List {
                            kind: ListKind::Ordered { start: 3 },
                            items: vec![ListItem {
                                span: Span::line(1, 1),
                                inlines: vec![Inline::Code(String::from("c"))],
                                nested: None,
                            }],
                        })),
                    ],
                ),
                slide(
                    2,
                    Some("second"),
                    vec![
                        block(BlockBody::Image(Image {
                            path: AssetRef::new("pics/a.svg").unwrap(),
                            alt: PlainText::default(),
                        })),
                        block(BlockBody::Image(Image {
                            path: AssetRef::new("pics/a.svg").unwrap(),
                            alt: PlainText::default(),
                        })),
                    ],
                ),
            ],
        }
    }

    #[test]
    fn the_source_sets_the_language_the_fonts_the_markers_and_the_page_breaks() {
        let theme = theme();
        let deck = sample();
        let bound = bind(&deck, &theme).unwrap();
        let emitted = typst_source(&theme, &deck, &bound, &LineBoxes::default()).unwrap();
        let source = emitted.source.as_str();
        assert!(source.contains("lang: \"ja\""), "{source}");
        assert!(source.contains("<sivi-slide-0>"), "{source}");
        assert!(source.contains("<sivi-m-0-0>") && source.contains("<sivi-m-0-1>"));
        assert!(source.contains("#pagebreak()\n#metadata((slide: 1)) <sivi-slide-1>"));
        assert!(!source.contains("#pagebreak()\n#metadata((slide: 0))"));
        assert!(source.contains("font: (\"BIZ UDPGothic\",)"), "{source}");
        assert!(
            !source.contains("\"BIZ UDPGothic\", \"BIZ UDPGothic\""),
            "{source}"
        );
        assert_eq!(emitted.expected.slides, 2);
        assert_eq!(emitted.expected.regions.len(), 3);
        assert_eq!(emitted.expected.regions[2].split_points, 2);
        assert_eq!(emitted.expected.regions[1].slide, SlideIndex(0));
        assert_eq!(emitted.expected.regions[1].region, RegionIndex(1));
        assert_eq!(emitted.expected.regions[1].split_points, 2);
        assert_eq!(
            emitted.expected.regions[1].bounds,
            region(HEIGHT_MILLIPOINTS)
        );
        assert_eq!(emitted.assets.len(), 1);
        assert_eq!(
            emitted.assets[0].path,
            alloc::format!("{DECK_ASSET_PREFIX}pics/a.svg")
        );
        assert_eq!(
            emitted.assets[0].origin,
            AssetOrigin::Deck(AssetRef::new("pics/a.svg").unwrap())
        );
    }

    #[test]
    fn the_source_keeps_author_text_in_literals_and_spells_every_block() {
        let theme = theme();
        let deck = sample();
        let bound = bind(&deck, &theme).unwrap();
        let emitted = typst_source(&theme, &deck, &bound, &LineBoxes::default()).unwrap();
        let source = emitted.source.as_str();
        assert!(source.contains("text(\"say \\\"hi\\\" #no\")"), "{source}");
        assert!(!source.contains("\n#no"), "{source}");
        assert!(
            source.contains("text(\"猫\") + text(\"犬\") + text(\" \") + "),
            "{source}"
        );
        assert!(
            source.contains("link(label(\"sivi-slide-1\"), text(\"go\"))"),
            "{source}"
        );
        assert!(
            source.contains("enum(start: 3, numbering: \"1.\""),
            "{source}"
        );
        assert!(source.contains("heading(level: 1, text("), "{source}");
        assert!(source.contains("counter(page).display(\"1\")"), "{source}");
        assert!(source.contains("set align(center);"), "{source}");
        assert!(
            source.contains("image(\"deck/pics/a.svg\", alt: \"\", width: 100%, fit: \"contain\")"),
            "{source}"
        );
    }

    #[test]
    fn a_mark_paints_a_translucent_band_and_an_underline_can_lie_behind_the_text() {
        let mut theme = theme();
        theme.inline.mark.marker = Some(Marker {
            color: style("ink"),
            opacity: Thousandths(500),
            top: TopEdge::XHeight,
            bottom: BottomEdge::Baseline,
            extent: Length::from_millipoints(1_000),
            radius: Length::from_millipoints(2_000),
        });
        theme.inline.link.underline = Some(Underline {
            color: Some(style("ink")),
            opacity: Thousandths(300),
            thickness: Some(Length::from_millipoints(3_000)),
            offset: Some(Length::from_millipoints(2_000)),
            extent: Length::from_millipoints(1_000),
            behind: true,
        });
        theme.inline.emphasis.underline = Some(Underline::default());
        theme.inline.strong.underline = Some(Underline {
            color: Some(style("ink")),
            ..Underline::default()
        });
        let deck = Deck {
            head: Head::default(),
            slides: vec![slide(
                1,
                None,
                vec![block(BlockBody::Paragraph(Paragraph {
                    inlines: vec![
                        Inline::Mark(vec![Inline::Text(String::from("m"))]),
                        Inline::Link {
                            dest: LinkDest::Url(String::from("https://example.com")),
                            inlines: vec![Inline::Text(String::from("l"))],
                        },
                        Inline::Emph(vec![Inline::Text(String::from("e"))]),
                        Inline::Strong(vec![Inline::Text(String::from("s"))]),
                    ],
                }))],
            )],
        };
        let bound = bind(&deck, &theme).unwrap();
        let emitted = typst_source(&theme, &deck, &bound, &LineBoxes::default()).unwrap();
        let source = emitted.source.as_str();
        assert!(
            source.contains(
                "highlight(fill: rgb(\"#01020380\"), top-edge: \"x-height\", bottom-edge: \"baseline\", extent: 1.000pt, radius: 2.000pt, text(\"m\"))"
            ),
            "{source}"
        );
        assert!(
            source.contains(
                "underline(stroke: 3.000pt + rgb(\"#0102034d\"), offset: 2.000pt, extent: 1.000pt, background: true, evade: false, text(\"l\"))"
            ),
            "{source}"
        );
        assert!(source.contains("underline(text(\"e\"))"), "{source}");
        assert!(
            source.contains("underline(stroke: rgb(\"#010203\"), text(\"s\"))"),
            "{source}"
        );
        assert_eq!(
            crate::ir::plain_text(&[Inline::Mark(vec![Inline::Text(String::from("m"))])]).as_str(),
            "m"
        );
    }

    #[test]
    fn a_sole_image_fills_its_region_and_line_boxes_answer_by_family() {
        let theme = theme();
        let deck = Deck {
            head: Head::default(),
            slides: vec![slide(
                1,
                None,
                vec![block(BlockBody::Image(Image {
                    path: AssetRef::new("pics/a.svg").unwrap(),
                    alt: PlainText::default(),
                }))],
            )],
        };
        let bound = bind(&deck, &theme).unwrap();
        let emitted = typst_source(&theme, &deck, &bound, &LineBoxes::default()).unwrap();
        assert!(
            emitted
                .source
                .as_str()
                .contains("alt: \"\", width: 400.000pt, height: 100.000pt, fit: \"contain\")"),
            "{}",
            emitted.source
        );
        let mono = crate::theme::FamilyName::new("JetBrains Mono").unwrap();
        let boxes = LineBoxes::new(vec![
            (family(), Thousandths(1200)),
            (mono.clone(), Thousandths(1500)),
        ]);
        assert_eq!(boxes.of(&family()), Thousandths(1200));
        assert_eq!(boxes.of(&mono), Thousandths(1500));
        assert_eq!(
            boxes.of(&crate::theme::FamilyName::new("Nowhere").unwrap()),
            Thousandths::ONE
        );
    }

    #[test]
    fn a_binding_the_theme_cannot_place_is_an_internal_error() {
        let theme = theme();
        let deck = Deck {
            head: Head::default(),
            slides: vec![slide(
                1,
                None,
                vec![block(BlockBody::Paragraph(Paragraph {
                    inlines: Vec::new(),
                }))],
            )],
        };
        let mut bound = bind(&deck, &theme).unwrap();
        bound.slides[0].master = 9;
        let errors = typst_source(&theme, &deck, &bound, &LineBoxes::default()).unwrap_err();
        assert_eq!(errors[0].code, crate::diagnostic::Code::Internal);
    }
}
