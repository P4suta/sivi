//! The CommonMark parse of one slide, read into the slide model with every forbidden construct refused.

use crate::scanner::RawSlide;
use comrak::nodes::{
    AstNode, ListType, NodeCodeBlock, NodeList, NodeTable, NodeValue, Sourcepos, TableAlignment,
};
use comrak::options::{BrokenLinkCallback, BrokenLinkReference};
use comrak::{Arena, Options, ResolvedReference, parse_document};
use sivi_core::deck_rules;
use sivi_core::diagnostic::{Code, Diagnostic};
use sivi_core::front_matter::SlideMeta;
use sivi_core::ir::{
    Alignment, AssetRef, Block, BlockBody, CodeBlock, Heading, HeadingLevel, Image, Inline,
    InvalidAsset, InvalidLink, LinkDest, List, ListItem, ListKind, Notes, Paragraph, PlainText,
    Slide, Table, TableRow, plain_text,
};
use sivi_core::span::{LineColumn, Span};
use sivi_core::subset_rules::{Admission, NodeKind, admit};
use sivi_core::text::Identifier;
use std::sync::{Arc, Mutex};

/// The CommonMark HTML block type of a comment, `<!-- -->`.
const HTML_COMMENT_BLOCK: u8 = 2;
const HTML_COMMENT_START: &str = "<!--";
/// The info string that marks speaker notes.
const NOTES_INFO: &str = "notes";
/// The help every image placement diagnostic gives.
const IMAGE_HELP: &str =
    "put the image in a paragraph of its own, with a blank line before and after";
/// The help every refusal gives when nothing more specific applies.
const SUBSET_HELP: &str = "see the deck format for what a slide may hold";

/// One slide read from its parse, with every link to a slide and every diagnostic.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Converted {
    pub slide: Slide,
    /// Every link to a slide id, with where it was written, for the deck to resolve.
    pub links: Vec<(Identifier, Span)>,
    pub diagnostics: Vec<Diagnostic>,
}

/// The reference links the parser could not resolve, which it reports through this callback.
#[derive(Debug, Default)]
struct BrokenLinks(Mutex<Vec<String>>);

impl BrokenLinks {
    /// Every reference the parser reported, each once, in the order first seen.
    fn take(&self) -> Vec<String> {
        let found = {
            let mut guard = match self.0.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            std::mem::take(&mut *guard)
        };
        let mut unique: Vec<String> = Vec::new();
        for reference in found {
            if !unique.contains(&reference) {
                unique.push(reference);
            }
        }
        unique
    }
}

impl BrokenLinkCallback for BrokenLinks {
    fn resolve(&self, reference: BrokenLinkReference<'_>) -> Option<ResolvedReference> {
        let mut found = match self.0.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        found.push(String::from(reference.original));
        None
    }
}

/// The parser options: every extension the subset admits, and every extension the subset names so it can refuse it.
fn options(broken: Arc<BrokenLinks>) -> Options<'static> {
    let mut options = Options::default();
    options.extension.table = true;
    options.extension.strikethrough = true;
    options.extension.tasklist = true;
    options.extension.footnotes = true;
    options.extension.math_dollars = true;
    options.extension.cjk_friendly_emphasis = true;
    options.parse.broken_link_callback = Some(broken);
    options
}

/// The kind of a parsed node, as the subset rules decide on it.
fn kind_of(value: &NodeValue) -> NodeKind {
    match value {
        NodeValue::Document => NodeKind::Document,
        NodeValue::FrontMatter(_) => NodeKind::FrontMatter,
        NodeValue::BlockQuote => NodeKind::BlockQuote,
        NodeValue::List(_) => NodeKind::List,
        NodeValue::Item(_) => NodeKind::Item,
        NodeValue::DescriptionList => NodeKind::DescriptionList,
        NodeValue::DescriptionItem(_) => NodeKind::DescriptionItem,
        NodeValue::DescriptionTerm => NodeKind::DescriptionTerm,
        NodeValue::DescriptionDetails => NodeKind::DescriptionDetails,
        NodeValue::CodeBlock(block) => NodeKind::CodeBlock {
            fenced: block.fenced,
        },
        NodeValue::HtmlBlock(block) => NodeKind::HtmlBlock {
            comment: block.block_type == HTML_COMMENT_BLOCK,
        },
        NodeValue::Paragraph => NodeKind::Paragraph,
        NodeValue::Heading(heading) => NodeKind::Heading {
            level: heading.level,
            setext: heading.setext,
        },
        NodeValue::ThematicBreak => NodeKind::ThematicBreak,
        NodeValue::FootnoteDefinition(_) => NodeKind::FootnoteDefinition,
        NodeValue::Table(_) => NodeKind::Table,
        NodeValue::TableRow(_) => NodeKind::TableRow,
        NodeValue::TableCell => NodeKind::TableCell,
        NodeValue::Text(_) => NodeKind::Text,
        NodeValue::TaskItem(_) => NodeKind::TaskItem,
        NodeValue::SoftBreak => NodeKind::SoftBreak,
        NodeValue::LineBreak => NodeKind::LineBreak,
        NodeValue::Code(_) => NodeKind::Code,
        NodeValue::HtmlInline(html) => NodeKind::HtmlInline {
            comment: html.starts_with(HTML_COMMENT_START),
        },
        NodeValue::Raw(_) => NodeKind::Raw,
        NodeValue::Emph => NodeKind::Emph,
        NodeValue::Strong => NodeKind::Strong,
        NodeValue::Strikethrough => NodeKind::Strikethrough,
        NodeValue::Highlight => NodeKind::Highlight,
        NodeValue::Insert => NodeKind::Insert,
        NodeValue::Superscript => NodeKind::Superscript,
        NodeValue::Link(_) => NodeKind::Link,
        NodeValue::Image(_) => NodeKind::Image,
        NodeValue::FootnoteReference(_) => NodeKind::FootnoteReference,
        NodeValue::Math(_) => NodeKind::Math,
        NodeValue::MultilineBlockQuote(_) => NodeKind::MultilineBlockQuote,
        NodeValue::Escaped => NodeKind::Escaped,
        NodeValue::WikiLink(_) => NodeKind::WikiLink,
        NodeValue::Underline => NodeKind::Underline,
        NodeValue::Subscript => NodeKind::Subscript,
        NodeValue::SpoileredText => NodeKind::SpoileredText,
        NodeValue::EscapedTag(_) => NodeKind::EscapedTag,
        NodeValue::Alert(_) => NodeKind::Alert,
        NodeValue::Subtext => NodeKind::Subtext,
        NodeValue::BlockDirective(_) => NodeKind::BlockDirective,
    }
}

/// What each refusal says and what it suggests, by code.
const REFUSALS: [(Code, &str, &str); 12] = [
    (
        Code::HtmlBlock,
        "a raw HTML block",
        "write the content in Markdown; the theme decides how it looks",
    ),
    (
        Code::HtmlInline,
        "raw inline HTML",
        "write the content in Markdown; the theme decides how it looks",
    ),
    (
        Code::HtmlComment,
        "an HTML comment, which carries no directive",
        "name the master in the slide's front matter, as `master: <name>` right after the `---` that starts the slide",
    ),
    (
        Code::SetextHeading,
        "a heading underlined with `===` or `---`",
        "write `#` or `##` before the heading text",
    ),
    (
        Code::HeadingTooDeep,
        "a heading below level two",
        "a slide holds `#` for its title and `##` for a section; split deeper structure into slides",
    ),
    (
        Code::Footnote,
        "a footnote",
        "put the text inline or on a later slide",
    ),
    (
        Code::TaskList,
        "a task list item",
        "write a plain list item; the theme decides the marker",
    ),
    (
        Code::Math,
        "a math span",
        "math is not part of this version",
    ),
    (
        Code::IndentedCode,
        "an indented code block",
        "fence the code with ``` and name its language after the opening fence",
    ),
    (
        Code::HardBreak,
        "a hard line break",
        "end the line without a backslash or trailing spaces; the layout decides where lines break",
    ),
    (
        Code::ThematicBreak,
        "a thematic break, which is not a slide separator",
        "separate slides with `---` at the start of its own line, after a blank line",
    ),
    (
        Code::OutsideSubset,
        "a construct outside the Markdown subset",
        SUBSET_HELP,
    ),
];

/// The diagnostic of a refused construct, by its code.
fn refusal(code: Code, kind: NodeKind) -> Diagnostic {
    let (message, help) = REFUSALS.iter().find(|(known, _, _)| *known == code).map_or(
        ("a construct the slide cannot hold", SUBSET_HELP),
        |(_, message, help)| (*message, *help),
    );
    let diagnostic = Diagnostic::new(code, format!("{message} is not content"));
    let diagnostic = if code == Code::OutsideSubset {
        diagnostic.note(format!("the parser read {kind:?}"))
    } else {
        diagnostic
    };
    diagnostic.help(String::from(help))
}

fn image_placement(message: &str, span: Span, label: String) -> Diagnostic {
    Diagnostic::new(Code::ImagePlacement, String::from(message))
        .at(span, label)
        .help(String::from(IMAGE_HELP))
}

fn empty_block(what: &str, span: Span, help: &str) -> Diagnostic {
    Diagnostic::new(Code::EmptyBlock, format!("{what} without text"))
        .at(span, format!("{what} is empty"))
        .help(String::from(help))
}

struct Converter<'a> {
    raw: &'a RawSlide,
    blocks: Vec<Block>,
    notes: Option<Notes>,
    links: Vec<(Identifier, Span)>,
    diagnostics: Vec<Diagnostic>,
}

impl<'a> Converter<'a> {
    /// The deck span of a parsed node.
    const fn span(&self, position: Sourcepos) -> Span {
        Span::new(
            LineColumn::new(position.start.line, position.start.column),
            LineColumn::new(position.end.line, position.end.column),
        )
        .from_line(self.raw.first_line)
    }

    /// The slide's own line at `line`, counted from one within the slide.
    fn source_line(&self, line: usize) -> &'a str {
        self.raw
            .text
            .lines()
            .nth(line.saturating_sub(1))
            .unwrap_or_default()
    }

    fn refuse(&mut self, code: Code, kind: NodeKind, span: Span) {
        self.diagnostics
            .push(refusal(code, kind).at(span, String::from("written here")));
    }

    /// Whether the node may stay; a refused node leaves a diagnostic.
    fn admitted(&mut self, node: &'a AstNode<'a>) -> bool {
        let data = node.data.borrow();
        let kind = kind_of(&data.value);
        match admit(kind) {
            Admission::Admit => true,
            Admission::Reject(code) => {
                let span = self.span(data.sourcepos);
                self.refuse(code, kind, span);
                false
            }
        }
    }

    fn blocks(&mut self, root: &'a AstNode<'a>) {
        for node in root.children() {
            if !self.admitted(node) {
                continue;
            }
            let data = node.data.borrow();
            let span = self.span(data.sourcepos);
            if let NodeValue::Heading(heading) = &data.value {
                self.heading(node, heading.level, span);
            } else if matches!(data.value, NodeValue::Paragraph) {
                self.paragraph(node, span);
            } else if let NodeValue::List(list) = &data.value {
                if let Some(list) = self.list(node, list) {
                    self.blocks.push(Block {
                        body: BlockBody::List(list),
                        span,
                    });
                }
            } else if let NodeValue::CodeBlock(block) = &data.value {
                self.code(block, span);
            } else if matches!(data.value, NodeValue::BlockQuote) {
                self.quote(node, span);
            } else if let NodeValue::Table(table) = &data.value {
                self.table(node, table, span);
            } else {
                self.refuse(Code::OutsideSubset, kind_of(&data.value), span);
            }
        }
    }

    fn heading(&mut self, node: &'a AstNode<'a>, level: u8, span: Span) {
        let inlines = self.inlines(node);
        if inlines.is_empty() {
            self.diagnostics.push(empty_block(
                "a heading",
                span,
                "write the heading text after the `#` marks",
            ));
            return;
        }
        let level = if level == 1 {
            HeadingLevel::Title
        } else {
            HeadingLevel::Section
        };
        self.blocks.push(Block {
            body: BlockBody::Heading(Heading { level, inlines }),
            span,
        });
    }

    fn paragraph(&mut self, node: &'a AstNode<'a>, span: Span) {
        let children: Vec<&'a AstNode<'a>> = node.children().collect();
        let images = children
            .iter()
            .filter(|child| matches!(child.data.borrow().value, NodeValue::Image(_)))
            .count();
        if let [only] = children.as_slice()
            && let NodeValue::Image(link) = &only.data.borrow().value
        {
            let alt = plain_text(&self.inlines(only));
            self.image(&link.url, alt, span);
            return;
        }
        if images > 0 {
            self.diagnostics.push(image_placement(
                "an image shares its paragraph with other content",
                span,
                String::from("this paragraph holds an image and more"),
            ));
            return;
        }
        let inlines = self.inlines(node);
        self.blocks.push(Block {
            body: BlockBody::Paragraph(Paragraph { inlines }),
            span,
        });
    }

    fn image(&mut self, url: &str, alt: PlainText, span: Span) {
        match AssetRef::new(url) {
            Ok(path) => {
                if alt.is_empty() {
                    self.diagnostics.push(
                        Diagnostic::new(
                            Code::MissingAlt,
                            String::from("an image without alternative text"),
                        )
                        .at(span, String::from("this image has no alternative text"))
                        .help(String::from(
                            "write what the image shows between the brackets: `![what it shows](path)`",
                        )),
                    );
                }
                self.blocks.push(Block {
                    body: BlockBody::Image(Image { path, alt }),
                    span,
                });
            }
            Err(why) => {
                let reason = match why {
                    InvalidAsset::Empty => "the path is empty",
                    InvalidAsset::Absolute => "the path is absolute",
                    InvalidAsset::Backslash => "the path holds a backslash",
                    InvalidAsset::DotSegment => "the path holds a `.` or `..` segment",
                    InvalidAsset::Colon => "the path holds a colon, as a scheme or a drive would",
                    InvalidAsset::Extension => {
                        "the path does not end in `.png`, `.jpg`, `.jpeg`, or `.svg`"
                    }
                };
                self.diagnostics.push(
                    Diagnostic::new(
                        Code::ImagePlacement,
                        format!("an image path that does not stay beside the deck: {reason}"),
                    )
                    .at(span, format!("`{url}` is this path"))
                    .help(String::from(
                        "name the image relative to the deck, with `/` between directories, and keep it inside the deck's directory",
                    )),
                );
            }
        }
    }

    fn list(&mut self, node: &'a AstNode<'a>, list: &NodeList) -> Option<List> {
        let kind = match list.list_type {
            ListType::Bullet => ListKind::Bullet,
            ListType::Ordered => ListKind::Ordered { start: list.start },
        };
        let mut items = Vec::new();
        for item in node.children() {
            if !self.admitted(item) {
                continue;
            }
            let span = self.span(item.data.borrow().sourcepos);
            if let Some(found) = self.item(item, span) {
                items.push(found);
            }
        }
        if items.is_empty() {
            return None;
        }
        Some(List { kind, items })
    }

    /// The nested list of an item's second child, when that child is a list.
    fn nested(&mut self, node: &'a AstNode<'a>) -> Option<List> {
        let data = node.data.borrow();
        if let NodeValue::List(list) = &data.value {
            self.list(node, list)
        } else {
            None
        }
    }

    fn item(&mut self, item: &'a AstNode<'a>, span: Span) -> Option<ListItem> {
        let children: Vec<&'a AstNode<'a>> = item.children().collect();
        let is_paragraph =
            |node: &&'a AstNode<'a>| matches!(node.data.borrow().value, NodeValue::Paragraph);
        let is_list =
            |node: &&'a AstNode<'a>| matches!(node.data.borrow().value, NodeValue::List(_));
        let (inlines, nested) = match children.as_slice() {
            [] => (Vec::new(), None),
            [first] if is_paragraph(first) => (self.inlines(first), None),
            [first, second] if is_paragraph(first) && is_list(second) => {
                (self.inlines(first), self.nested(second))
            }
            _ => {
                self.diagnostics.push(
                    Diagnostic::new(
                        Code::ListItemContent,
                        String::from(
                            "a list item holding more than a paragraph and a nested list",
                        ),
                    )
                    .at(span, String::from("this item holds other blocks"))
                    .help(String::from(
                        "keep each item to one paragraph of text with an optional nested list under it",
                    )),
                );
                return None;
            }
        };
        if inlines.is_empty() {
            self.diagnostics.push(empty_block(
                "a list item",
                span,
                "write the item's text after its marker",
            ));
            return None;
        }
        Some(ListItem {
            span,
            inlines,
            nested,
        })
    }

    fn code(&mut self, block: &NodeCodeBlock, span: Span) {
        let text = String::from(block.literal.strip_suffix('\n').unwrap_or(&block.literal));
        let language = block.info.split_whitespace().next();
        if language == Some(NOTES_INFO) {
            if self.notes.is_some() {
                self.diagnostics.push(
                    Diagnostic::new(
                        Code::NotesCount,
                        String::from("a second notes block on one slide"),
                    )
                    .at(span, String::from("these notes come second"))
                    .help(String::from("join the notes into one ```notes block")),
                );
                return;
            }
            self.notes = Some(Notes { text, span });
            return;
        }
        self.blocks.push(Block {
            body: BlockBody::Code(CodeBlock {
                language: language.map(String::from),
                text,
            }),
            span,
        });
    }

    fn quote(&mut self, node: &'a AstNode<'a>, span: Span) {
        let children: Vec<&'a AstNode<'a>> = node.children().collect();
        if let [only] = children.as_slice()
            && matches!(only.data.borrow().value, NodeValue::Paragraph)
        {
            let inlines = self.inlines(only);
            self.blocks.push(Block {
                body: BlockBody::Quote(Paragraph { inlines }),
                span,
            });
            return;
        }
        self.diagnostics.push(
            Diagnostic::new(
                Code::QuoteContent,
                String::from("a block quote holding other than one paragraph"),
            )
            .at(
                span,
                String::from("this quote holds several blocks or none"),
            )
            .help(String::from(
                "keep a quote to one paragraph; put the attribution in the paragraph after it",
            )),
        );
    }

    /// One row of a table, with a diagnostic when its cell count differs from the header's.
    fn row(&mut self, row: &'a AstNode<'a>, columns: usize) -> Option<(bool, TableRow)> {
        if !self.admitted(row) {
            return None;
        }
        let data = row.data.borrow();
        let NodeValue::TableRow(is_header) = data.value else {
            return None;
        };
        let span = self.span(data.sourcepos);
        let declared =
            deck_rules::table_cells(self.source_line(data.sourcepos.start.line).as_bytes());
        if declared != columns {
            self.diagnostics.push(
                Diagnostic::new(
                    Code::TableCells,
                    format!("a table row with {declared} cells where the header has {columns}"),
                )
                .at(span, String::from("this row"))
                .help(String::from(
                    "give every row as many cells as the header row",
                )),
            );
        }
        let cells = row.children().map(|cell| self.inlines(cell)).collect();
        Some((is_header, TableRow { span, cells }))
    }

    fn table(&mut self, node: &'a AstNode<'a>, table: &NodeTable, span: Span) {
        let alignments = table
            .alignments
            .iter()
            .map(|alignment| match alignment {
                TableAlignment::None => Alignment::Unset,
                TableAlignment::Left => Alignment::Left,
                TableAlignment::Center => Alignment::Center,
                TableAlignment::Right => Alignment::Right,
            })
            .collect();
        let mut header = None;
        let mut rows = Vec::new();
        for row in node.children() {
            match self.row(row, table.num_columns) {
                Some((true, found)) => header = Some(found),
                Some((false, found)) => rows.push(found),
                None => {}
            }
        }
        if let Some(header) = header {
            self.blocks.push(Block {
                body: BlockBody::Table(Table {
                    alignments,
                    header,
                    rows,
                }),
                span,
            });
        }
    }

    /// The link inline of `node`, or a diagnostic when its destination is refused.
    fn link(&mut self, node: &'a AstNode<'a>, url: &str, span: Span) -> Option<Inline> {
        match LinkDest::new(url) {
            Ok(dest) => {
                if let LinkDest::Slide(id) = &dest {
                    self.links.push((id.clone(), span));
                }
                let inlines = self.inlines(node);
                Some(Inline::Link { dest, inlines })
            }
            Err(why) => {
                let (message, help) = match why {
                    InvalidLink::Scheme => (
                        "a link that is neither a web address, a mail address, nor a slide id",
                        "link to `https:`, `http:`, or `mailto:` addresses, or to a slide as `#its-id`",
                    ),
                    InvalidLink::Fragment => (
                        "a link to a slide whose id is not an identifier",
                        "a slide id is a lowercase letter, then up to thirty-one lowercase letters, digits, or dashes",
                    ),
                };
                self.diagnostics.push(
                    Diagnostic::new(Code::LinkScheme, String::from(message))
                        .at(span, format!("`{url}` is this destination"))
                        .help(String::from(help)),
                );
                None
            }
        }
    }

    fn inlines(&mut self, parent: &'a AstNode<'a>) -> Vec<Inline> {
        let mut out = Vec::new();
        for node in parent.children() {
            if !self.admitted(node) {
                continue;
            }
            let data = node.data.borrow();
            let span = self.span(data.sourcepos);
            if let NodeValue::Text(text) = &data.value {
                push_text(&mut out, text);
            } else if matches!(data.value, NodeValue::SoftBreak) {
                out.push(Inline::SoftBreak);
            } else if let NodeValue::Code(code) = &data.value {
                out.push(Inline::Code(code.literal.clone()));
            } else if matches!(data.value, NodeValue::Emph) {
                let inner = self.inlines(node);
                out.push(Inline::Emph(inner));
            } else if matches!(data.value, NodeValue::Strong) {
                let inner = self.inlines(node);
                out.push(Inline::Strong(inner));
            } else if matches!(data.value, NodeValue::Strikethrough) {
                let inner = self.inlines(node);
                out.push(Inline::Strike(inner));
            } else if let NodeValue::Link(link) = &data.value {
                if let Some(inline) = self.link(node, &link.url, span) {
                    out.push(inline);
                }
            } else if matches!(data.value, NodeValue::Image(_)) {
                self.diagnostics.push(image_placement(
                    "an image inside text",
                    span,
                    String::from("this image sits among text"),
                ));
            } else {
                self.refuse(Code::OutsideSubset, kind_of(&data.value), span);
            }
        }
        out
    }
}

/// Appends `text`, joining it onto a preceding text inline.
fn push_text(out: &mut Vec<Inline>, text: &str) {
    if let Some(Inline::Text(last)) = out.last_mut() {
        last.push_str(text);
    } else {
        out.push(Inline::Text(String::from(text)));
    }
}

/// The slide `raw` holds, numbered `number` and carrying `meta`, with every diagnostic of its content.
#[must_use]
pub fn convert(raw: &RawSlide, number: usize, meta: SlideMeta) -> Converted {
    let broken = Arc::new(BrokenLinks::default());
    let options = options(Arc::clone(&broken));
    let arena = Arena::new();
    let root = parse_document(&arena, &raw.text, &options);
    let mut converter = Converter {
        raw,
        blocks: Vec::new(),
        notes: None,
        links: Vec::new(),
        diagnostics: Vec::new(),
    };
    converter.blocks(root);
    for reference in broken.take() {
        converter.diagnostics.push(
            Diagnostic::new(
                Code::UnresolvedReference,
                format!("a reference link `[{reference}]` without a definition"),
            )
            .at(raw.span, String::from("somewhere on this slide"))
            .help(String::from(
                "write the link as `[text](https://...)`, or escape the brackets as `\\[` and `\\]`",
            )),
        );
    }
    Converted {
        slide: Slide {
            number,
            span: raw.span,
            master: meta.master,
            id: meta.id,
            blocks: converter.blocks,
            notes: converter.notes,
        },
        links: converter.links,
        diagnostics: converter.diagnostics,
    }
}

#[cfg(test)]
mod tests {
    use super::{Converted, convert};
    use crate::scanner::RawSlide;
    use sivi_core::diagnostic::Code;
    use sivi_core::front_matter::SlideMeta;
    use sivi_core::ir::{BlockBody, BlockKind, HeadingLevel, Inline, LinkDest, ListKind};
    use sivi_core::span::{LineColumn, Span};

    fn slide(text: &str) -> Converted {
        let raw = RawSlide {
            first_line: 10,
            text: String::from(text),
            front: Vec::new(),
            span: Span::new(LineColumn::new(10, 1), LineColumn::new(20, 1)),
        };
        convert(&raw, 1, SlideMeta::default())
    }

    fn codes(converted: &Converted) -> Vec<Code> {
        converted.diagnostics.iter().map(|d| d.code).collect()
    }

    fn kinds(converted: &Converted) -> Vec<BlockKind> {
        converted
            .slide
            .blocks
            .iter()
            .map(|block| block.body.kind())
            .collect()
    }

    #[test]
    fn every_admitted_block_reads_into_the_model_with_its_deck_span() {
        let converted = slide(
            "# Title *em* **strong** `code` ~~gone~~\n\nPara one\nline two\n\n- a\n- b\n  1. c\n  2. d\n\n```rust\nfn main() {}\n```\n\n> Quote\n\n![A chart](figures/chart.png)\n\n| H1 | H2 |\n| :-- | --: |\n| a | b |\n\n```notes\nSpeak.\n```\n",
        );
        assert_eq!(codes(&converted), Vec::<Code>::new());
        assert_eq!(
            kinds(&converted),
            vec![
                BlockKind::Heading,
                BlockKind::Paragraph,
                BlockKind::List,
                BlockKind::Code,
                BlockKind::Quote,
                BlockKind::Image,
                BlockKind::Table,
            ]
        );
        let BlockBody::Heading(heading) = &converted.slide.blocks[0].body else {
            panic!("a heading");
        };
        assert_eq!(heading.level, HeadingLevel::Title);
        assert_eq!(heading.inlines.len(), 8);
        assert_eq!(converted.slide.blocks[0].span.start, LineColumn::new(10, 1));
        let BlockBody::Paragraph(paragraph) = &converted.slide.blocks[1].body else {
            panic!("a paragraph");
        };
        assert_eq!(
            paragraph.inlines,
            vec![
                Inline::Text(String::from("Para one")),
                Inline::SoftBreak,
                Inline::Text(String::from("line two"))
            ]
        );
        assert_eq!(
            converted.slide.blocks[1].span,
            Span::new(LineColumn::new(12, 1), LineColumn::new(13, 8))
        );
        let BlockBody::List(list) = &converted.slide.blocks[2].body else {
            panic!("a list");
        };
        assert_eq!(list.kind, ListKind::Bullet);
        assert_eq!(list.depth(), 2);
        assert_eq!(
            list.items[1].nested.as_ref().map(|nested| nested.kind),
            Some(ListKind::Ordered { start: 1 })
        );
        let BlockBody::Code(code) = &converted.slide.blocks[3].body else {
            panic!("code");
        };
        assert_eq!(code.language.as_deref(), Some("rust"));
        assert_eq!(code.text, "fn main() {}");
        let BlockBody::Image(image) = &converted.slide.blocks[5].body else {
            panic!("an image");
        };
        assert_eq!(image.alt.as_str(), "A chart");
        assert_eq!(image.path.as_str(), "figures/chart.png");
        let BlockBody::Table(table) = &converted.slide.blocks[6].body else {
            panic!("a table");
        };
        assert_eq!(table.header.cells.len(), 2);
        assert_eq!(table.rows.len(), 1);
        assert_eq!(
            converted
                .slide
                .notes
                .as_ref()
                .map(|notes| notes.text.as_str()),
            Some("Speak.")
        );
    }

    #[test]
    fn links_go_to_the_web_or_to_a_slide_and_slide_links_are_collected() {
        let converted = slide(
            "See [the plan](#plan) and [docs](https://example.test/) and [bad](ftp://x) and [Bad](#Plan).\n",
        );
        assert_eq!(codes(&converted), vec![Code::LinkScheme, Code::LinkScheme]);
        assert_eq!(converted.links.len(), 1);
        assert_eq!(converted.links[0].0.as_str(), "plan");
        let BlockBody::Paragraph(paragraph) = &converted.slide.blocks[0].body else {
            panic!("a paragraph");
        };
        assert!(paragraph.inlines.iter().any(|inline| matches!(
            inline,
            Inline::Link {
                dest: LinkDest::Slide(_),
                ..
            }
        )));
    }

    #[test]
    fn each_forbidden_construct_is_refused_with_its_code() {
        let cases: [(&str, Code); 16] = [
            ("<div>x</div>\n", Code::HtmlBlock),
            ("<!-- _class: lead -->\n", Code::HtmlComment),
            ("a <b>x</b> b\n", Code::HtmlInline),
            ("a <!-- c --> b\n", Code::HtmlComment),
            ("Title\n===\n", Code::SetextHeading),
            ("### Deep\n", Code::HeadingTooDeep),
            ("a[^1]\n\n[^1]: note\n", Code::Footnote),
            ("- [ ] task\n", Code::TaskList),
            ("$x$\n", Code::Math),
            ("    indented\n", Code::IndentedCode),
            ("a  \nb\n", Code::HardBreak),
            ("***\n", Code::ThematicBreak),
            ("#\n", Code::EmptyBlock),
            ("> a\n>\n> b\n", Code::QuoteContent),
            ("a ![x](y.png) b\n", Code::ImagePlacement),
            ("![x](../y.png)\n", Code::ImagePlacement),
        ];
        for (text, code) in cases {
            let converted = slide(text);
            assert!(
                codes(&converted).contains(&code),
                "{text:?}: {:?}",
                codes(&converted)
            );
        }
        let notes = slide("```notes\na\n```\n\n```notes\nb\n```\n");
        assert_eq!(codes(&notes), vec![Code::NotesCount]);
        let item = slide("- a\n\n  ```\n  code\n  ```\n");
        assert_eq!(codes(&item), vec![Code::ListItemContent]);
        let cells = slide("| a | b |\n| - | - |\n| c |\n| d | e | f |\n");
        assert_eq!(codes(&cells), vec![Code::TableCells, Code::TableCells]);
        let reference = slide("see [missing][ref] here\n");
        assert_eq!(codes(&reference), vec![Code::UnresolvedReference]);
        let alt = slide("![](a.png)\n");
        assert_eq!(codes(&alt), vec![Code::MissingAlt]);
        assert_eq!(kinds(&alt), vec![BlockKind::Image]);
    }
}
