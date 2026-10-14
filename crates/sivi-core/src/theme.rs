//! The theme model: masters of typed slots, the styles they name, and the page they lay out on.
//!
//! The theme loader builds these values from a theme file and validates them; the binder and the emitter read them.

use crate::ir::BlockKind;
use crate::length::{Length, Rect, Size};
use crate::syntax_theme::Scheme;
use crate::text::Identifier;
use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

/// Thousandths of a unit: the fraction of an em, the ratio of a line height, or a percentage of a page side.
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd, Hash)]
pub struct Thousandths(pub i64);

impl Thousandths {
    /// One whole unit.
    pub const ONE: Self = Self(1000);
}

/// Why a name was refused.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum InvalidName {
    /// Not a lowercase letter followed by lowercase letters, digits, or underscores.
    Style,
    /// Empty, or holding a control character.
    Family,
}

/// The name of a color or a text style as the theme spells it: a lowercase letter, then lowercase letters, digits, or underscores.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Hash)]
pub struct StyleName(String);

impl StyleName {
    /// # Errors
    ///
    /// Returns `InvalidName::Style` when `text` leaves the grammar.
    pub fn new(text: &str) -> Result<Self, InvalidName> {
        let mut bytes = text.bytes();
        let starts_well = bytes.next().is_some_and(|byte| byte.is_ascii_lowercase());
        let continues_well =
            bytes.all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_');
        if starts_well && continues_well {
            Ok(Self(String::from(text)))
        } else {
            Err(InvalidName::Style)
        }
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for StyleName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// A font family as its files name it, such as `BIZ UDPGothic`.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Hash)]
pub struct FamilyName(String);

impl FamilyName {
    /// # Errors
    ///
    /// Returns `InvalidName::Family` when `text` is empty or holds a control character.
    pub fn new(text: &str) -> Result<Self, InvalidName> {
        if text.is_empty() || text.chars().any(char::is_control) {
            Err(InvalidName::Family)
        } else {
            Ok(Self(String::from(text)))
        }
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for FamilyName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// The families a run of text is set in: the primary for every glyph it has, then each fallback for the glyphs the earlier ones lack.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct FontStack {
    primary: FamilyName,
    fallbacks: Vec<FamilyName>,
}

impl FontStack {
    /// The stack over `families`, or `None` when there is none.
    #[must_use]
    pub fn new(mut families: Vec<FamilyName>) -> Option<Self> {
        if families.is_empty() {
            return None;
        }
        let primary = families.remove(0);
        Some(Self {
            primary,
            fallbacks: families,
        })
    }

    /// The stack of one family.
    #[must_use]
    pub const fn single(family: FamilyName) -> Self {
        Self {
            primary: family,
            fallbacks: Vec::new(),
        }
    }

    /// The family every glyph is first looked up in, whose metrics set the line.
    #[must_use]
    pub const fn primary(&self) -> &FamilyName {
        &self.primary
    }

    /// The families after the primary, in order.
    #[must_use]
    pub fn fallbacks(&self) -> &[FamilyName] {
        &self.fallbacks
    }

    /// Every family, the primary first.
    pub fn families(&self) -> impl Iterator<Item = &FamilyName> {
        core::iter::once(&self.primary).chain(self.fallbacks.iter())
    }

    /// Whether the stack names `family`.
    #[must_use]
    pub fn names(&self, family: &FamilyName) -> bool {
        self.families().any(|known| known == family)
    }
}

impl fmt::Display for FontStack {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, family) in self.families().enumerate() {
            if index > 0 {
                formatter.write_str(", ")?;
            }
            formatter.write_str(family.as_str())?;
        }
        Ok(())
    }
}

/// The weights a font file provides: one for a static face, a span for a variable one.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct WeightRange {
    pub lowest: Weight,
    pub highest: Weight,
}

impl WeightRange {
    /// The range of one weight.
    #[must_use]
    pub const fn single(weight: Weight) -> Self {
        Self {
            lowest: weight,
            highest: weight,
        }
    }

    /// Whether `weight` lies within the range.
    #[must_use]
    pub fn contains(self, weight: Weight) -> bool {
        self.lowest <= weight && weight <= self.highest
    }
}

/// A color, as `#rrggbb` spells it, which is how it prints.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash)]
pub struct Color {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
}

impl fmt::Display for Color {
    /// `#rrggbb` in lowercase.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "#{:02x}{:02x}{:02x}",
            self.red, self.green, self.blue
        )
    }
}

/// A length on the page or relative to the current font size.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum Measure {
    Points(Length),
    /// A multiple of the font size, in thousandths.
    Em(Thousandths),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum Align {
    Left,
    Center,
    Right,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum VerticalAlign {
    Top,
    Middle,
    Bottom,
}

/// The lightest and heaviest weights a face may carry, as OpenType counts them.
pub const MIN_WEIGHT: u16 = 100;
pub const MAX_WEIGHT: u16 = 900;

/// A font weight from one hundred to nine hundred.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Hash)]
pub struct Weight(u16);

impl Weight {
    /// # Errors
    ///
    /// Returns `None` when `value` lies outside one hundred to nine hundred.
    #[must_use]
    pub const fn new(value: u16) -> Option<Self> {
        if value >= MIN_WEIGHT && value <= MAX_WEIGHT {
            Some(Self(value))
        } else {
            None
        }
    }

    #[must_use]
    pub const fn value(self) -> u16 {
        self.0
    }
}

/// How a run of text is set.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct TextStyle {
    pub font: FontStack,
    pub size: Length,
    pub weight: Weight,
    pub color: StyleName,
    /// The line pitch as a multiple of the size, in thousandths.
    pub line_height: Thousandths,
    pub align: Align,
}

/// Where the top of a band behind the text lies, as Typst names the edges of a run.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash)]
pub enum TopEdge {
    #[default]
    Ascender,
    CapHeight,
    XHeight,
    Baseline,
    Bounds,
}

impl TopEdge {
    /// The edge as the theme and Typst spell it.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Ascender => "ascender",
            Self::CapHeight => "cap-height",
            Self::XHeight => "x-height",
            Self::Baseline => "baseline",
            Self::Bounds => "bounds",
        }
    }
}

/// Where the bottom of a band behind the text lies.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash)]
pub enum BottomEdge {
    #[default]
    Descender,
    Baseline,
    Bounds,
}

impl BottomEdge {
    /// The edge as the theme and Typst spell it.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Descender => "descender",
            Self::Baseline => "baseline",
            Self::Bounds => "bounds",
        }
    }
}

/// A translucent band behind the text, as a highlighter pen leaves it.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct Marker {
    pub color: StyleName,
    /// How opaque the band is, in thousandths from clear to solid.
    pub opacity: Thousandths,
    pub top: TopEdge,
    pub bottom: BottomEdge,
    /// How far the band reaches past the text on each side.
    pub extent: Length,
    pub radius: Length,
}

/// A line under the text.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct Underline {
    /// The color of the line, or the color of the text when unset.
    pub color: Option<StyleName>,
    /// How opaque the line is, in thousandths, which applies to a color the underline sets.
    pub opacity: Thousandths,
    /// The thickness of the line, or the font's own when unset.
    pub thickness: Option<Length>,
    /// How far below the baseline the line lies, or the font's own when unset.
    pub offset: Option<Length>,
    /// How far the line reaches past the text on each side.
    pub extent: Length,
    /// Whether the line lies behind the glyphs, crossing their descenders, rather than in front and around them.
    pub behind: bool,
}

impl Default for Underline {
    /// The plain underline: the text color at full opacity, the font's thickness and offset, in front of the glyphs.
    fn default() -> Self {
        Self {
            color: None,
            opacity: Thousandths::ONE,
            thickness: None,
            offset: None,
            extent: Length::ZERO,
            behind: false,
        }
    }
}

/// What an inline mark changes about the text it marks.
#[derive(Clone, Debug, Default, Eq, PartialEq, Hash)]
pub struct Mark {
    pub italic: bool,
    pub color: Option<StyleName>,
    pub weight: Option<Weight>,
    pub font: Option<FontStack>,
    /// A solid box behind the text.
    pub background: Option<StyleName>,
    /// A translucent band behind the text.
    pub marker: Option<Marker>,
    pub underline: Option<Underline>,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct InlineStyles {
    pub emphasis: Mark,
    pub strong: Mark,
    pub code: Mark,
    pub link: Mark,
    /// The mark of `==text==`.
    pub mark: Mark,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct ListStyle {
    /// The markers of each nesting level, outermost first.
    pub bullets: Vec<String>,
    /// The marker pattern of ordered items, such as `1.`.
    pub ordered: String,
    pub indent: Measure,
    pub gap: Measure,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct CodeStyle {
    pub font: FontStack,
    pub size: Length,
    pub line_height: Thousandths,
    pub background: StyleName,
    pub padding: Length,
    pub highlight: bool,
    pub tab_width: u8,
    /// The scheme that colors the code, whose background and foreground replace `background` and the style's color.
    pub theme: Option<Scheme>,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct TableStyle {
    pub text: StyleName,
    pub header_weight: Weight,
    pub rule: StyleName,
    pub padding: Length,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct QuoteStyle {
    pub bar: StyleName,
    pub bar_width: Length,
    pub indent: Length,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct Typography {
    /// The smallest size any text may be set at.
    pub min_size: Length,
    pub paragraph_gap: Measure,
    /// The families Typst falls back to, in order.
    pub fallback: Vec<FamilyName>,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct Page {
    pub size: Size,
    pub background: StyleName,
}

/// The hexadecimal digest of a file the theme pins.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct Digest(String);

/// How many hexadecimal digits a SHA-256 digest spells.
pub const DIGEST_DIGITS: usize = 64;

impl Digest {
    /// # Errors
    ///
    /// Returns `None` when `text` is not sixty-four lowercase hexadecimal digits.
    #[must_use]
    pub fn new(text: &str) -> Option<Self> {
        let hex = text.len() == DIGEST_DIGITS
            && text
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte));
        if hex {
            Some(Self(String::from(text)))
        } else {
            None
        }
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A font file the theme loads beside itself.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct FontFile {
    pub family: FamilyName,
    /// A relative path with slashes, inside the theme's directory.
    pub path: String,
    pub sha256: Digest,
}

/// What a slot binds: the title, the subtitle, one numbered section, or the flow of blocks.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
#[cfg_attr(kani, derive(kani::Arbitrary))]
pub enum Binding {
    Title,
    Subtitle,
    /// The section opened by the `##` of this number, counted from one.
    Section(u32),
    Flow,
}

/// A set of block kinds, as the kinds a slot accepts.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash)]
#[cfg_attr(kani, derive(kani::Arbitrary))]
pub struct BlockKindSet(u8);

const HEADING_BIT: u8 = 1;
const PARAGRAPH_BIT: u8 = 1 << 1;
const LIST_BIT: u8 = 1 << 2;
const CODE_BIT: u8 = 1 << 3;
const QUOTE_BIT: u8 = 1 << 4;
const IMAGE_BIT: u8 = 1 << 5;
const TABLE_BIT: u8 = 1 << 6;

const fn bit(kind: BlockKind) -> u8 {
    match kind {
        BlockKind::Heading => HEADING_BIT,
        BlockKind::Paragraph => PARAGRAPH_BIT,
        BlockKind::List => LIST_BIT,
        BlockKind::Code => CODE_BIT,
        BlockKind::Quote => QUOTE_BIT,
        BlockKind::Image => IMAGE_BIT,
        BlockKind::Table => TABLE_BIT,
    }
}

impl BlockKindSet {
    pub const EMPTY: Self = Self(0);

    #[must_use]
    pub const fn with(self, kind: BlockKind) -> Self {
        Self(self.0 | bit(kind))
    }

    #[must_use]
    pub const fn contains(self, kind: BlockKind) -> bool {
        self.0 & bit(kind) != 0
    }

    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
}

/// How much content a slot holds; `None` leaves a dimension unbounded, up to what fits the region.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash)]
#[cfg_attr(kani, derive(kani::Arbitrary))]
pub struct Capacity {
    pub blocks: Option<u32>,
    pub items: Option<u32>,
    pub depth: Option<u32>,
    pub rows: Option<u32>,
    pub columns: Option<u32>,
    pub lines: Option<u32>,
}

/// A slot of a master: what it binds, what it accepts, where it sits, and how its text is set.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct Slot {
    pub name: Identifier,
    pub binds: Binding,
    pub accepts: BlockKindSet,
    pub required: bool,
    /// The slot whose content this slot's content must follow directly.
    pub follows: Option<Identifier>,
    pub capacity: Capacity,
    pub region: Rect,
    pub text: StyleName,
    pub valign: VerticalAlign,
    pub align: Option<Align>,
}

/// A value of the deck's head a decor shows.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum Field {
    Title,
    Author,
    Date,
}

/// Something a master draws that binds no content.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum Decor {
    Rect {
        region: Rect,
        color: StyleName,
    },
    Image {
        region: Rect,
        path: String,
    },
    PageNumber {
        region: Rect,
        text: StyleName,
        align: Align,
        /// The pattern with `{n}` for the page number.
        format: String,
    },
    Field {
        region: Rect,
        text: StyleName,
        align: Align,
        field: Field,
    },
}

impl Decor {
    #[must_use]
    pub const fn region(&self) -> Rect {
        match self {
            Self::Rect { region, .. }
            | Self::Image { region, .. }
            | Self::PageNumber { region, .. }
            | Self::Field { region, .. } => *region,
        }
    }
}

/// A slide master: its slots in declaration order and its decor.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct Master {
    pub name: Identifier,
    pub slots: Vec<Slot>,
    pub decor: Vec<Decor>,
}

impl Master {
    #[must_use]
    pub fn slot(&self, name: &Identifier) -> Option<&Slot> {
        self.slots.iter().find(|slot| slot.name == *name)
    }
}

/// A whole theme.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct Theme {
    pub name: Identifier,
    pub page: Page,
    pub colors: Vec<(StyleName, Color)>,
    pub typography: Typography,
    pub text: Vec<(StyleName, TextStyle)>,
    pub inline: InlineStyles,
    pub lists: ListStyle,
    pub code: CodeStyle,
    pub tables: TableStyle,
    pub quotes: QuoteStyle,
    pub fonts: Vec<FontFile>,
    pub masters: Vec<Master>,
}

impl Theme {
    #[must_use]
    pub fn master(&self, name: &Identifier) -> Option<&Master> {
        self.masters.iter().find(|master| master.name == *name)
    }

    #[must_use]
    pub fn color(&self, name: &StyleName) -> Option<Color> {
        self.colors
            .iter()
            .find(|(known, _)| known == name)
            .map(|(_, color)| *color)
    }

    #[must_use]
    pub fn text_style(&self, name: &StyleName) -> Option<&TextStyle> {
        self.text
            .iter()
            .find(|(known, _)| known == name)
            .map(|(_, style)| style)
    }

    /// Every family the theme sets text in, each once, in the order the theme names them.
    #[must_use]
    pub fn families(&self) -> Vec<&FamilyName> {
        let mut found: Vec<&FamilyName> = Vec::new();
        let stacks = self
            .text
            .iter()
            .map(|(_, style)| &style.font)
            .chain(core::iter::once(&self.code.font))
            .chain(
                [
                    &self.inline.emphasis,
                    &self.inline.strong,
                    &self.inline.code,
                    &self.inline.link,
                    &self.inline.mark,
                ]
                .into_iter()
                .filter_map(|mark| mark.font.as_ref()),
            );
        for family in stacks
            .flat_map(FontStack::families)
            .chain(self.typography.fallback.iter())
            .chain(self.fonts.iter().map(|file| &file.family))
        {
            if !found.contains(&family) {
                found.push(family);
            }
        }
        found
    }
}

/// A small theme and its parts, which the tests of this crate build on.
#[cfg(test)]
pub mod fixture {
    use super::{
        Align, Binding, BlockKindSet, Capacity, CodeStyle, Color, FamilyName, FontStack,
        InlineStyles, ListStyle, Mark, Master, Measure, Page, QuoteStyle, Slot, StyleName,
        TableStyle, TextStyle, Theme, Thousandths, Typography, VerticalAlign, Weight,
    };
    use crate::ir::BlockKind;
    use crate::length::{Length, Rect, Size};
    use crate::text::Identifier;
    use alloc::string::String;
    use alloc::vec;
    use alloc::vec::Vec;

    pub(crate) fn id(name: &str) -> Identifier {
        Identifier::new(name).unwrap()
    }

    pub(crate) fn style(name: &str) -> StyleName {
        StyleName::new(name).unwrap()
    }

    pub(crate) fn family() -> FamilyName {
        FamilyName::new("BIZ UDPGothic").unwrap()
    }

    pub(crate) fn text_style(size: i64) -> TextStyle {
        TextStyle {
            font: FontStack::single(family()),
            size: Length::from_millipoints(size),
            weight: Weight::new(400).unwrap(),
            color: style("ink"),
            line_height: Thousandths::ONE,
            align: Align::Left,
        }
    }

    /// A flow slot that accepts paragraphs, with no region and no capacity.
    pub(crate) fn slot(name: &str) -> Slot {
        Slot {
            name: id(name),
            binds: Binding::Flow,
            accepts: BlockKindSet::EMPTY.with(BlockKind::Paragraph),
            required: false,
            follows: None,
            capacity: Capacity::default(),
            region: Rect::default(),
            text: style("body"),
            valign: VerticalAlign::Top,
            align: None,
        }
    }

    pub(crate) fn master(name: &str, slots: &[&str]) -> Master {
        Master {
            name: id(name),
            slots: slots.iter().map(|slot_name| slot(slot_name)).collect(),
            decor: Vec::new(),
        }
    }

    /// The theme `tiny`: two colors, the styles `body` and `meta`, and the masters `a` over slots `x`, `y` and `b` over `y`, `z`.
    pub(crate) fn tiny() -> Theme {
        Theme {
            name: id("tiny"),
            page: Page {
                size: Size::default(),
                background: style("paper"),
            },
            colors: vec![
                (
                    style("paper"),
                    Color {
                        red: 255,
                        green: 255,
                        blue: 255,
                    },
                ),
                (
                    style("ink"),
                    Color {
                        red: 1,
                        green: 2,
                        blue: 3,
                    },
                ),
            ],
            typography: Typography {
                min_size: Length::from_millipoints(18_000),
                paragraph_gap: Measure::Em(Thousandths(600)),
                fallback: Vec::new(),
            },
            text: vec![
                (style("body"), text_style(24_000)),
                (style("meta"), text_style(18_000)),
            ],
            inline: InlineStyles {
                emphasis: Mark::default(),
                strong: Mark::default(),
                code: Mark::default(),
                link: Mark::default(),
                mark: Mark::default(),
            },
            lists: ListStyle {
                bullets: vec![String::from("•")],
                ordered: String::from("1."),
                indent: Measure::Em(Thousandths::ONE),
                gap: Measure::Em(Thousandths(350)),
            },
            code: CodeStyle {
                font: FontStack::single(family()),
                size: Length::from_millipoints(18_000),
                line_height: Thousandths::ONE,
                background: style("paper"),
                padding: Length::ZERO,
                highlight: false,
                tab_width: 4,
                theme: None,
            },
            tables: TableStyle {
                text: style("body"),
                header_weight: Weight::new(700).unwrap(),
                rule: style("ink"),
                padding: Length::ZERO,
            },
            quotes: QuoteStyle {
                bar: style("ink"),
                bar_width: Length::ZERO,
                indent: Length::ZERO,
            },
            fonts: Vec::new(),
            masters: vec![master("a", &["x", "y"]), master("b", &["y", "z"])],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::fixture::{family, id, style, tiny};
    use super::{
        BlockKindSet, Color, Digest, FamilyName, FontStack, InvalidName, StyleName, Weight,
        WeightRange,
    };
    use crate::ir::BlockKind;
    use alloc::string::{String, ToString};
    use alloc::vec;
    use alloc::vec::Vec;

    const KINDS: [BlockKind; 7] = [
        BlockKind::Heading,
        BlockKind::Paragraph,
        BlockKind::List,
        BlockKind::Code,
        BlockKind::Quote,
        BlockKind::Image,
        BlockKind::Table,
    ];

    #[test]
    fn every_kind_has_its_own_bit_and_a_set_keeps_what_it_holds() {
        for kind in KINDS {
            let set = BlockKindSet::EMPTY.with(kind);
            assert!(set.contains(kind), "{kind:?}");
            assert!(set.with(kind).contains(kind), "{kind:?}");
            for other in KINDS {
                assert_eq!(set.contains(other), other == kind, "{kind:?} {other:?}");
            }
        }
    }

    #[test]
    fn lookups_find_exactly_the_named_master_slot_color_and_style() {
        let theme = tiny();
        assert_eq!(
            theme.master(&id("b")).map(|found| found.name.as_str()),
            Some("b")
        );
        assert_eq!(
            theme.master(&id("a")).map(|found| found.name.as_str()),
            Some("a")
        );
        assert!(theme.master(&id("c")).is_none());
        let second = theme.master(&id("b")).unwrap();
        assert_eq!(
            second.slot(&id("z")).map(|found| found.name.as_str()),
            Some("z")
        );
        assert_eq!(
            second.slot(&id("y")).map(|found| found.name.as_str()),
            Some("y")
        );
        assert!(second.slot(&id("x")).is_none());
        assert_eq!(
            theme.color(&style("ink")),
            Some(Color {
                red: 1,
                green: 2,
                blue: 3
            })
        );
        assert_eq!(
            theme.color(&style("paper")).map(|color| color.red),
            Some(255)
        );
        assert!(theme.color(&style("none")).is_none());
        assert_eq!(
            theme
                .text_style(&style("meta"))
                .map(|found| found.size.millipoints()),
            Some(18_000)
        );
        assert_eq!(
            theme
                .text_style(&style("body"))
                .map(|found| found.size.millipoints()),
            Some(24_000)
        );
        assert!(theme.text_style(&style("none")).is_none());
        assert_eq!(
            Digest::new(&"ab".repeat(32)).map(|digest| String::from(digest.as_str())),
            Some("ab".repeat(32))
        );
    }

    #[test]
    fn names_weights_digests_and_kind_sets_follow_their_grammars() {
        assert_eq!(StyleName::new("code_bg").unwrap().as_str(), "code_bg");
        assert_eq!(StyleName::new("Code"), Err(InvalidName::Style));
        assert_eq!(StyleName::new(""), Err(InvalidName::Style));
        assert_eq!(StyleName::new("a-b"), Err(InvalidName::Style));
        assert_eq!(
            FamilyName::new("BIZ UDPGothic").unwrap().as_str(),
            "BIZ UDPGothic"
        );
        assert_eq!(FamilyName::new(""), Err(InvalidName::Family));
        assert_eq!(FamilyName::new("a\nb"), Err(InvalidName::Family));
        assert_eq!(Weight::new(700).map(Weight::value), Some(700));
        assert_eq!(Weight::new(50), None);
        assert_eq!(Weight::new(901), None);
        assert!(Digest::new(&"a".repeat(64)).is_some());
        assert!(Digest::new(&"A".repeat(64)).is_none());
        assert!(Digest::new(&"a".repeat(63)).is_none());
        let set = BlockKindSet::EMPTY
            .with(BlockKind::List)
            .with(BlockKind::Table);
        assert!(set.contains(BlockKind::List));
        assert!(!set.contains(BlockKind::Code));
        assert!(!set.is_empty());
        assert!(BlockKindSet::EMPTY.is_empty());
    }

    #[test]
    fn a_font_stack_names_its_families_in_order_and_a_weight_range_holds_its_ends() {
        let gothic = family();
        let mincho = FamilyName::new("Shippori Mincho").unwrap();
        let stack = FontStack::new(vec![gothic.clone(), mincho.clone()]).unwrap();
        assert_eq!(stack.primary(), &gothic);
        assert_eq!(stack.fallbacks(), core::slice::from_ref(&mincho));
        assert_eq!(stack.families().collect::<Vec<_>>(), vec![&gothic, &mincho]);
        assert!(stack.names(&mincho));
        assert!(!stack.names(&FamilyName::new("Montserrat").unwrap()));
        assert_eq!(stack.to_string(), "BIZ UDPGothic, Shippori Mincho");
        assert_eq!(
            FontStack::single(gothic.clone()).to_string(),
            "BIZ UDPGothic"
        );
        assert!(FontStack::new(Vec::new()).is_none());
        let range = WeightRange {
            lowest: Weight::new(300).unwrap(),
            highest: Weight::new(700).unwrap(),
        };
        assert!(range.contains(Weight::new(300).unwrap()));
        assert!(range.contains(Weight::new(500).unwrap()));
        assert!(range.contains(Weight::new(700).unwrap()));
        assert!(!range.contains(Weight::new(299).unwrap()));
        assert!(!range.contains(Weight::new(701).unwrap()));
        let single = WeightRange::single(Weight::new(400).unwrap());
        assert!(single.contains(Weight::new(400).unwrap()));
        assert!(!single.contains(Weight::new(401).unwrap()));
        let mut theme = tiny();
        assert_eq!(theme.families(), vec![&gothic]);
        theme.typography.fallback = vec![mincho.clone(), gothic.clone()];
        assert_eq!(theme.families(), vec![&gothic, &mincho]);
    }
}
