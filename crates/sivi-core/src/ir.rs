//! The slide model: what a deck says, with the span of every block, and nothing about how it looks.

use crate::span::Span;
use crate::text::{Date, Identifier, Lang, ShortText, ThemeRef};
use alloc::string::String;
use alloc::vec::Vec;

/// A whole deck: its head and its slides in order.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Deck {
    pub head: Head,
    pub slides: Vec<Slide>,
}

/// What the head of the deck declares.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Head {
    pub theme: Option<ThemeRef>,
    pub default_master: Option<Identifier>,
    pub title: Option<ShortText>,
    pub author: Option<ShortText>,
    pub date: Option<Date>,
    pub lang: Option<Lang>,
}

/// One slide: its front matter, its blocks in order, and its speaker notes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Slide {
    /// The slide's number in the deck, counted from one.
    pub number: usize,
    pub span: Span,
    pub master: Option<Identifier>,
    pub id: Option<Identifier>,
    pub blocks: Vec<Block>,
    pub notes: Option<Notes>,
}

/// Speaker notes, which reach the JSON report and never the page.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Notes {
    pub text: String,
    pub span: Span,
}

/// A block of a slide with where it was written.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Block {
    pub body: BlockBody,
    pub span: Span,
}

/// The kinds of block a slide holds, which slots accept by kind.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
#[cfg_attr(kani, derive(kani::Arbitrary))]
pub enum BlockKind {
    Heading,
    Paragraph,
    List,
    Code,
    Quote,
    Image,
    Table,
}

/// What a block says.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BlockBody {
    Heading(Heading),
    Paragraph(Paragraph),
    List(List),
    Code(CodeBlock),
    /// A block quote, which holds one paragraph.
    Quote(Paragraph),
    Image(Image),
    Table(Table),
}

impl BlockBody {
    #[must_use]
    pub const fn kind(&self) -> BlockKind {
        match self {
            Self::Heading(_) => BlockKind::Heading,
            Self::Paragraph(_) => BlockKind::Paragraph,
            Self::List(_) => BlockKind::List,
            Self::Code(_) => BlockKind::Code,
            Self::Quote(_) => BlockKind::Quote,
            Self::Image(_) => BlockKind::Image,
            Self::Table(_) => BlockKind::Table,
        }
    }
}

/// The two heading levels a slide holds.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum HeadingLevel {
    /// `#`, which binds the title slot.
    Title,
    /// `##`, which opens a section.
    Section,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Heading {
    pub level: HeadingLevel,
    pub inlines: Vec<Inline>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Paragraph {
    pub inlines: Vec<Inline>,
}

/// Whether a list counts its items.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum ListKind {
    Bullet,
    Ordered {
        /// The number of the first item.
        start: usize,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct List {
    pub kind: ListKind,
    pub items: Vec<ListItem>,
}

impl List {
    /// How deep the list nests, counting itself as one.
    #[must_use]
    pub fn depth(&self) -> usize {
        let below = self
            .items
            .iter()
            .filter_map(|item| item.nested.as_ref().map(Self::depth))
            .max()
            .unwrap_or(0);
        below.saturating_add(1)
    }
}

/// One item: a paragraph of inlines and an optional nested list.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ListItem {
    pub span: Span,
    pub inlines: Vec<Inline>,
    pub nested: Option<List>,
}

/// A fenced code block with the language its info string names.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CodeBlock {
    pub language: Option<String>,
    pub text: String,
}

/// An image alone in its paragraph, with its alternative text.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Image {
    pub path: AssetRef,
    pub alt: PlainText,
}

/// Why an asset path was refused.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum InvalidAsset {
    Empty,
    /// The path starts at a root or a drive.
    Absolute,
    /// The path holds a backslash, which only one platform reads as a separator.
    Backslash,
    /// The path holds a `.` or `..` segment, which could leave the deck's directory.
    DotSegment,
    /// The path holds a scheme or a drive colon.
    Colon,
    /// The path does not end in `.png`, `.jpg`, `.jpeg`, or `.svg`.
    Extension,
}

/// The image kinds the compiler embeds, by extension.
pub const IMAGE_EXTENSIONS: [&str; 4] = ["png", "jpg", "jpeg", "svg"];

/// A path to an asset beside the deck: relative, slash-separated, without dot segments, with a known extension.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct AssetRef(String);

/// Why `path` cannot stay beside the file that names it, before its extension is read.
#[must_use]
pub fn relative_fault(path: &str) -> Option<InvalidAsset> {
    if path.is_empty() {
        return Some(InvalidAsset::Empty);
    }
    if path.contains('\\') {
        return Some(InvalidAsset::Backslash);
    }
    if path.contains(':') {
        return Some(InvalidAsset::Colon);
    }
    if path.starts_with('/') {
        return Some(InvalidAsset::Absolute);
    }
    if path
        .split('/')
        .any(|segment| segment == "." || segment == "..")
    {
        return Some(InvalidAsset::DotSegment);
    }
    None
}

/// Whether `path` ends in one of `extensions` after a file name that is more than the extension.
#[must_use]
pub fn has_extension(path: &str, extensions: &[&str]) -> bool {
    let extension = path.rsplit('.').next().unwrap_or_default();
    extensions.contains(&extension)
        && path.rsplit('/').next().is_some_and(|name| {
            name != extension && !(name.starts_with('.') && name.matches('.').count() == 1)
        })
}

impl AssetRef {
    /// # Errors
    ///
    /// Returns why `path` cannot name an asset beside the deck.
    pub fn new(path: &str) -> Result<Self, InvalidAsset> {
        if let Some(fault) = relative_fault(path) {
            return Err(fault);
        }
        if !has_extension(path, &IMAGE_EXTENSIONS) {
            return Err(InvalidAsset::Extension);
        }
        Ok(Self(String::from(path)))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The extension, lowercase as the grammar requires.
    #[must_use]
    pub fn extension(&self) -> &str {
        self.0.rsplit('.').next().unwrap_or_default()
    }
}

/// How a table column aligns its cells.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum Alignment {
    Unset,
    Left,
    Center,
    Right,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Table {
    pub alignments: Vec<Alignment>,
    pub header: TableRow,
    pub rows: Vec<TableRow>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TableRow {
    pub span: Span,
    pub cells: Vec<Vec<Inline>>,
}

/// An inline of a paragraph, heading, item, or cell.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Inline {
    Text(String),
    Emph(Vec<Self>),
    Strong(Vec<Self>),
    Code(String),
    Strike(Vec<Self>),
    Link {
        dest: LinkDest,
        inlines: Vec<Self>,
    },
    /// A line break in the source, which the soft break rules turn into a space or nothing.
    SoftBreak,
}

/// Why a link destination was refused.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum InvalidLink {
    /// The destination is neither a web address, a mail address, nor a slide id.
    Scheme,
    /// The destination starts with `#` but the rest is not an identifier.
    Fragment,
}

/// The schemes a link may use.
pub const LINK_SCHEMES: [&str; 3] = ["https:", "http:", "mailto:"];

/// Where a link goes.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum LinkDest {
    /// A web or mail address.
    Url(String),
    /// The slide whose id this is.
    Slide(Identifier),
}

impl LinkDest {
    /// # Errors
    ///
    /// Returns why `dest` cannot be a link destination.
    pub fn new(dest: &str) -> Result<Self, InvalidLink> {
        if let Some(fragment) = dest.strip_prefix('#') {
            return Identifier::new(fragment)
                .map(Self::Slide)
                .map_err(|_invalid| InvalidLink::Fragment);
        }
        if LINK_SCHEMES.iter().any(|scheme| dest.starts_with(scheme)) {
            Ok(Self::Url(String::from(dest)))
        } else {
            Err(InvalidLink::Scheme)
        }
    }
}

/// The text of inlines with every mark removed, as alternative text and labels read.
#[derive(Clone, Debug, Default, Eq, PartialEq, Hash)]
pub struct PlainText(String);

impl PlainText {
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl core::fmt::Display for PlainText {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(&self.0)
    }
}

fn append_plain(text: &mut String, inlines: &[Inline]) {
    for inline in inlines {
        match inline {
            Inline::Text(part) | Inline::Code(part) => text.push_str(part),
            Inline::Emph(inner) | Inline::Strong(inner) | Inline::Strike(inner) => {
                append_plain(text, inner);
            }
            Inline::Link {
                inlines: linked, ..
            } => append_plain(text, linked),
            Inline::SoftBreak => text.push(' '),
        }
    }
}

/// The plain text of inlines.
#[must_use]
pub fn plain_text(inlines: &[Inline]) -> PlainText {
    let mut text = String::new();
    append_plain(&mut text, inlines);
    PlainText(text)
}

#[cfg(test)]
mod tests {
    use super::{
        AssetRef, BlockBody, BlockKind, Inline, InvalidAsset, InvalidLink, LinkDest, List,
        ListItem, ListKind, Paragraph, plain_text,
    };
    use crate::span::Span;
    use crate::text::Identifier;
    use alloc::string::String;
    use alloc::vec;

    #[test]
    fn an_asset_path_stays_beside_the_deck_with_a_known_extension() {
        assert_eq!(AssetRef::new("figures/a.png").unwrap().extension(), "png");
        assert_eq!(AssetRef::new("a.JPEG"), Err(InvalidAsset::Extension));
        assert_eq!(AssetRef::new("a.gif"), Err(InvalidAsset::Extension));
        assert_eq!(AssetRef::new("png"), Err(InvalidAsset::Extension));
        assert_eq!(AssetRef::new(".png"), Err(InvalidAsset::Extension));
        assert_eq!(AssetRef::new("dir/.png"), Err(InvalidAsset::Extension));
        assert_eq!(
            AssetRef::new("a..png").map(|asset| asset.as_str().len()),
            Ok(6)
        );
        assert_eq!(AssetRef::new(""), Err(InvalidAsset::Empty));
        assert_eq!(AssetRef::new("/a.png"), Err(InvalidAsset::Absolute));
        assert_eq!(AssetRef::new("../a.png"), Err(InvalidAsset::DotSegment));
        assert_eq!(AssetRef::new("./a.png"), Err(InvalidAsset::DotSegment));
        assert_eq!(AssetRef::new("a\\b.png"), Err(InvalidAsset::Backslash));
        assert_eq!(AssetRef::new("c:/a.png"), Err(InvalidAsset::Colon));
        assert_eq!(AssetRef::new("https://x/a.png"), Err(InvalidAsset::Colon));
    }

    #[test]
    fn a_link_goes_to_the_web_to_mail_or_to_a_slide() {
        assert_eq!(
            LinkDest::new("https://example.test/a"),
            Ok(LinkDest::Url(String::from("https://example.test/a")))
        );
        assert_eq!(
            LinkDest::new("mailto:a@example.test"),
            Ok(LinkDest::Url(String::from("mailto:a@example.test")))
        );
        assert_eq!(
            LinkDest::new("#revenue"),
            Ok(LinkDest::Slide(Identifier::new("revenue").unwrap()))
        );
        assert_eq!(LinkDest::new("#Revenue"), Err(InvalidLink::Fragment));
        assert_eq!(LinkDest::new("#"), Err(InvalidLink::Fragment));
        assert_eq!(LinkDest::new("ftp://x"), Err(InvalidLink::Scheme));
        assert_eq!(LinkDest::new("other.md"), Err(InvalidLink::Scheme));
        assert_eq!(LinkDest::new(""), Err(InvalidLink::Scheme));
    }

    #[test]
    fn a_list_knows_its_depth_and_inlines_read_as_plain_text() {
        let leaf = List {
            kind: ListKind::Bullet,
            items: vec![ListItem {
                span: Span::default(),
                inlines: vec![Inline::Text(String::from("b"))],
                nested: None,
            }],
        };
        let list = List {
            kind: ListKind::Ordered { start: 3 },
            items: vec![
                ListItem {
                    span: Span::default(),
                    inlines: vec![Inline::Text(String::from("a"))],
                    nested: Some(leaf),
                },
                ListItem {
                    span: Span::default(),
                    inlines: vec![],
                    nested: None,
                },
            ],
        };
        assert_eq!(list.depth(), 2);
        assert_eq!(BlockBody::List(list).kind(), BlockKind::List);
        assert_eq!(
            BlockBody::Quote(Paragraph::default()).kind(),
            BlockKind::Quote
        );
        let inlines = vec![
            Inline::Text(String::from("a ")),
            Inline::Emph(vec![Inline::Code(String::from("b"))]),
            Inline::SoftBreak,
            Inline::Link {
                dest: LinkDest::Url(String::from("https://x")),
                inlines: vec![Inline::Strong(vec![Inline::Text(String::from("c"))])],
            },
        ];
        assert_eq!(plain_text(&inlines).as_str(), "a b c");
        assert!(!plain_text(&inlines).is_empty());
        assert!(plain_text(&[]).is_empty());
        assert_eq!(alloc::format!("{}", plain_text(&inlines)), "a b c");
    }
}
