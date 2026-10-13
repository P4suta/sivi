//! Which CommonMark constructs a slide may hold, decided over the kind of each node the parser yields.

use crate::diagnostic::Code;

/// The kind of a node the Markdown parser yields, mirrored in full so the decision stays total when the parser grows.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
#[cfg_attr(kani, derive(kani::Arbitrary))]
pub enum NodeKind {
    Document,
    FrontMatter,
    BlockQuote,
    List,
    Item,
    DescriptionList,
    DescriptionItem,
    DescriptionTerm,
    DescriptionDetails,
    CodeBlock { fenced: bool },
    HtmlBlock { comment: bool },
    Paragraph,
    Heading { level: u8, setext: bool },
    ThematicBreak,
    FootnoteDefinition,
    Table,
    TableRow,
    TableCell,
    Text,
    TaskItem,
    SoftBreak,
    LineBreak,
    Code,
    HtmlInline { comment: bool },
    Raw,
    Emph,
    Strong,
    Strikethrough,
    Highlight,
    Insert,
    Superscript,
    Link,
    Image,
    FootnoteReference,
    Math,
    MultilineBlockQuote,
    Escaped,
    WikiLink,
    Underline,
    Subscript,
    SpoileredText,
    EscapedTag,
    Alert,
    Subtext,
    BlockDirective,
}

/// The deepest heading a slide holds: `#` binds the title and `##` opens a section.
pub const MAX_HEADING_LEVEL: u8 = 2;

/// Whether a node may stay, or the code that refuses it.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum Admission {
    Admit,
    Reject(Code),
}

/// Whether a node of `kind` belongs to the Markdown subset a slide holds.
#[must_use]
pub const fn admit(kind: NodeKind) -> Admission {
    match kind {
        NodeKind::Document
        | NodeKind::BlockQuote
        | NodeKind::List
        | NodeKind::Item
        | NodeKind::Paragraph
        | NodeKind::Table
        | NodeKind::TableRow
        | NodeKind::TableCell
        | NodeKind::Text
        | NodeKind::SoftBreak
        | NodeKind::Code
        | NodeKind::Emph
        | NodeKind::Strong
        | NodeKind::Strikethrough
        | NodeKind::Link
        | NodeKind::Image
        | NodeKind::CodeBlock { fenced: true } => Admission::Admit,
        NodeKind::CodeBlock { fenced: false } => Admission::Reject(Code::IndentedCode),
        NodeKind::Heading { setext: true, .. } => Admission::Reject(Code::SetextHeading),
        NodeKind::Heading {
            level,
            setext: false,
        } => {
            if level <= MAX_HEADING_LEVEL {
                Admission::Admit
            } else {
                Admission::Reject(Code::HeadingTooDeep)
            }
        }
        NodeKind::HtmlBlock { comment: true } | NodeKind::HtmlInline { comment: true } => {
            Admission::Reject(Code::HtmlComment)
        }
        NodeKind::HtmlBlock { comment: false } => Admission::Reject(Code::HtmlBlock),
        NodeKind::HtmlInline { comment: false } | NodeKind::Raw => {
            Admission::Reject(Code::HtmlInline)
        }
        NodeKind::ThematicBreak => Admission::Reject(Code::ThematicBreak),
        NodeKind::FootnoteDefinition | NodeKind::FootnoteReference => {
            Admission::Reject(Code::Footnote)
        }
        NodeKind::TaskItem => Admission::Reject(Code::TaskList),
        NodeKind::LineBreak => Admission::Reject(Code::HardBreak),
        NodeKind::Math => Admission::Reject(Code::Math),
        NodeKind::FrontMatter
        | NodeKind::DescriptionList
        | NodeKind::DescriptionItem
        | NodeKind::DescriptionTerm
        | NodeKind::DescriptionDetails
        | NodeKind::Highlight
        | NodeKind::Insert
        | NodeKind::Superscript
        | NodeKind::MultilineBlockQuote
        | NodeKind::Escaped
        | NodeKind::WikiLink
        | NodeKind::Underline
        | NodeKind::Subscript
        | NodeKind::SpoileredText
        | NodeKind::EscapedTag
        | NodeKind::Alert
        | NodeKind::Subtext
        | NodeKind::BlockDirective => Admission::Reject(Code::OutsideSubset),
    }
}

/// The properties Kani proves for every node kind and the tests check over the kinds they list.
#[cfg(any(test, kani))]
mod proofs {
    use super::{Admission, MAX_HEADING_LEVEL, NodeKind, admit};
    use crate::diagnostic::Code;

    /// Whether `kind` carries raw markup of any form.
    pub(super) const fn is_markup(kind: NodeKind) -> bool {
        matches!(
            kind,
            NodeKind::HtmlBlock { .. } | NodeKind::HtmlInline { .. } | NodeKind::Raw
        )
    }

    /// The subset, stated as the record lists it: structure, paragraphs, lists, fenced code, quotes, tables, images, and plain inline marks.
    pub(super) const fn in_subset(kind: NodeKind) -> bool {
        match kind {
            NodeKind::Heading { level, setext } => !setext && level <= MAX_HEADING_LEVEL,
            NodeKind::CodeBlock { fenced } => fenced,
            NodeKind::Document
            | NodeKind::BlockQuote
            | NodeKind::List
            | NodeKind::Item
            | NodeKind::Paragraph
            | NodeKind::Table
            | NodeKind::TableRow
            | NodeKind::TableCell
            | NodeKind::Text
            | NodeKind::SoftBreak
            | NodeKind::Code
            | NodeKind::Emph
            | NodeKind::Strong
            | NodeKind::Strikethrough
            | NodeKind::Link
            | NodeKind::Image => true,
            NodeKind::FrontMatter
            | NodeKind::DescriptionList
            | NodeKind::DescriptionItem
            | NodeKind::DescriptionTerm
            | NodeKind::DescriptionDetails
            | NodeKind::HtmlBlock { .. }
            | NodeKind::ThematicBreak
            | NodeKind::FootnoteDefinition
            | NodeKind::TaskItem
            | NodeKind::LineBreak
            | NodeKind::HtmlInline { .. }
            | NodeKind::Raw
            | NodeKind::Highlight
            | NodeKind::Insert
            | NodeKind::Superscript
            | NodeKind::FootnoteReference
            | NodeKind::Math
            | NodeKind::MultilineBlockQuote
            | NodeKind::Escaped
            | NodeKind::WikiLink
            | NodeKind::Underline
            | NodeKind::Subscript
            | NodeKind::SpoileredText
            | NodeKind::EscapedTag
            | NodeKind::Alert
            | NodeKind::Subtext
            | NodeKind::BlockDirective => false,
        }
    }

    /// Markup never passes, and what passes is exactly the subset.
    pub(super) fn spec_admit(kind: NodeKind) {
        let verdict = admit(kind);
        assert_eq!(matches!(verdict, Admission::Admit), in_subset(kind));
        if is_markup(kind) {
            assert!(matches!(
                verdict,
                Admission::Reject(Code::HtmlBlock | Code::HtmlInline | Code::HtmlComment)
            ));
        }
        if let Admission::Reject(code) = verdict {
            assert!(code.is_rejection());
        }
    }

    #[cfg(kani)]
    #[kani::proof]
    fn what_passes_is_exactly_the_subset_and_markup_never_does() {
        let kind: NodeKind = kani::any();
        spec_admit(kind);
        kani::cover!(matches!(admit(kind), Admission::Admit));
        kani::cover!(matches!(admit(kind), Admission::Reject(Code::HtmlComment)));
        kani::cover!(matches!(
            admit(kind),
            Admission::Reject(Code::HeadingTooDeep)
        ));
        kani::cover!(matches!(
            admit(kind),
            Admission::Reject(Code::OutsideSubset)
        ));
    }

    #[cfg(kani)]
    #[kani::proof]
    fn rejects_an_html_comment_as_content() {
        kani::assert(
            matches!(
                admit(NodeKind::HtmlBlock { comment: true }),
                Admission::Admit
            ),
            "NEGATIVE_SUBSET_CONTROL",
        );
    }
}

#[cfg(test)]
mod tests {
    use super::{Admission, NodeKind, admit, proofs};
    use crate::diagnostic::Code;

    const SAMPLES: [NodeKind; 24] = [
        NodeKind::Document,
        NodeKind::Paragraph,
        NodeKind::Heading {
            level: 1,
            setext: false,
        },
        NodeKind::Heading {
            level: 2,
            setext: false,
        },
        NodeKind::Heading {
            level: 3,
            setext: false,
        },
        NodeKind::Heading {
            level: 1,
            setext: true,
        },
        NodeKind::CodeBlock { fenced: true },
        NodeKind::CodeBlock { fenced: false },
        NodeKind::HtmlBlock { comment: true },
        NodeKind::HtmlBlock { comment: false },
        NodeKind::HtmlInline { comment: true },
        NodeKind::HtmlInline { comment: false },
        NodeKind::Raw,
        NodeKind::ThematicBreak,
        NodeKind::TaskItem,
        NodeKind::LineBreak,
        NodeKind::Math,
        NodeKind::FootnoteReference,
        NodeKind::Link,
        NodeKind::Image,
        NodeKind::Strikethrough,
        NodeKind::Table,
        NodeKind::WikiLink,
        NodeKind::Alert,
    ];

    #[test]
    fn every_kind_is_admitted_or_refused_as_the_record_says() {
        for kind in SAMPLES {
            proofs::spec_admit(kind);
        }
        assert_eq!(
            admit(NodeKind::Heading {
                level: 3,
                setext: false
            }),
            Admission::Reject(Code::HeadingTooDeep)
        );
        assert_eq!(
            admit(NodeKind::Heading {
                level: 2,
                setext: true
            }),
            Admission::Reject(Code::SetextHeading)
        );
        assert_eq!(
            admit(NodeKind::HtmlInline { comment: true }),
            Admission::Reject(Code::HtmlComment)
        );
        assert_eq!(
            admit(NodeKind::CodeBlock { fenced: false }),
            Admission::Reject(Code::IndentedCode)
        );
        assert_eq!(
            admit(NodeKind::WikiLink),
            Admission::Reject(Code::OutsideSubset)
        );
        assert_eq!(admit(NodeKind::Strikethrough), Admission::Admit);
    }
}
