//! The diagnostics the compiler reports: a closed catalog of codes, each with a severity and a one-line summary.
//!
//! Every renderer, human or JSON, reads the same values, so the codes stay stable across releases.

use crate::span::Span;
use crate::text::Identifier;
use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

/// How serious a diagnostic is.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum Severity {
    /// The build fails.
    Error,
    /// The build goes on unless warnings are denied.
    Warning,
}

/// Every diagnostic the compiler can report, with its stable code.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
#[cfg_attr(kani, derive(kani::Arbitrary))]
pub enum Code {
    /// E0001: an internal failure.
    Internal,
    /// E0101: the deck is not UTF-8.
    NotUtf8,
    /// E0102: a front matter block has no closing fence.
    UnclosedFrontMatter,
    /// E0103: a front matter line is not `key: value`.
    MalformedKeyValue,
    /// E0104: a front matter key is unknown or belongs elsewhere.
    UnknownKey,
    /// E0105: a front matter value leaves its grammar.
    InvalidValue,
    /// E0106: a front matter key repeats.
    DuplicateKey,
    /// E0107: two slides share an id.
    DuplicateSlideId,
    /// E0108: the deck holds no slide.
    NoSlides,
    /// E0110: an input exceeds its size limit.
    LimitExceeded,
    /// E0201: a raw HTML block.
    HtmlBlock,
    /// E0202: raw inline HTML.
    HtmlInline,
    /// E0203: an HTML comment, which carries no directive.
    HtmlComment,
    /// E0204: a setext heading, whose underline reads as a separator.
    SetextHeading,
    /// E0205: a heading below level two.
    HeadingTooDeep,
    /// E0206: a footnote.
    Footnote,
    /// E0207: a task list item.
    TaskList,
    /// E0208: a math span.
    Math,
    /// E0209: an indented code block.
    IndentedCode,
    /// E0210: a link whose destination is not a web, mail, or slide address.
    LinkScheme,
    /// E0212: an image that is not alone in its paragraph or whose path leaves the deck.
    ImagePlacement,
    /// E0213: a block quote holding more than one paragraph.
    QuoteContent,
    /// E0214: a hard line break.
    HardBreak,
    /// E0215: a second notes block on one slide.
    NotesCount,
    /// E0216: a thematic break that is not a separator.
    ThematicBreak,
    /// E0217: a construct outside the Markdown subset.
    OutsideSubset,
    /// E0218: a list item holding more than a paragraph and a nested list.
    ListItemContent,
    /// E0219: a table row whose cell count differs from the header's.
    TableCells,
    /// E0220: an empty heading or list item.
    EmptyBlock,
    /// E0221: a reference link without a definition.
    UnresolvedReference,
    /// E0301: a slide without a master and a deck without a default.
    MasterMissing,
    /// E0302: a master the theme does not define.
    UnknownMaster,
    /// E0303: a block no slot of the master accepts.
    NoSlotForBlock,
    /// E0304: a required slot left empty.
    RequiredSlotEmpty,
    /// E0305: more sections than the master holds.
    SectionCount,
    /// E0306: content beyond a slot's capacity.
    CapacityExceeded,
    /// E0307: a title heading away from the start of its slide.
    TitlePosition,
    /// E0308: a link to a slide id no slide carries.
    UnknownSlideId,
    /// E0401: content taller than its region.
    VerticalOverflow,
    /// E0402: a line wider than its region.
    HorizontalOverflow,
    /// E0403: a slide that spilled onto a second page.
    MultiplePages,
    /// E0404: an item laid out beyond the page.
    OutsidePage,
    /// E0405: more lines than the slot allows.
    TooManyLines,
    /// E0501: a theme file that is not TOML.
    ThemeToml,
    /// E0502: a theme with an unknown schema or key.
    ThemeSchema,
    /// E0503: a theme value outside its grammar.
    ThemeValue,
    /// E0504: a region outside the page.
    RegionOutsidePage,
    /// E0505: two slot regions that overlap.
    RegionOverlap,
    /// E0506: a text size below the theme's minimum.
    BelowMinimumSize,
    /// E0507: a reference to a color, text style, or font the theme does not define.
    UndefinedReference,
    /// E0508: slot bindings that contradict each other.
    BindingConflict,
    /// E0509: a theme without a master or a master without a slot.
    NoMasterOrSlot,
    /// E0510: a font face the theme needs but no font provides.
    MissingFace,
    /// E0511: a line height below the face's natural line box.
    LineHeightBelowFace,
    /// E0512: a code theme the carried schemes lack.
    CodeThemeUnknown,
    /// E0513: a text color that reads below four and a half to one on the paper.
    LowContrast,
    /// E0601: a font file the theme names is missing.
    FontMissing,
    /// E0602: a font file whose digest differs from the theme's.
    FontDigest,
    /// E0603: a font file that cannot be read as a font.
    FontUnreadable,
    /// E0604: a character no font of the theme provides.
    MissingGlyph,
    /// E0605: an image file is missing.
    ImageMissing,
    /// E0606: an image file that cannot be decoded.
    ImageUnreadable,
    /// E0607: an image whose header does not match its extension.
    ImageKind,
    /// E0608: a font whose family name differs from the theme's.
    FamilyMismatch,
    /// E0609: a font family that is neither bundled, declared by the theme, nor in the catalog.
    FamilyUnknown,
    /// E0610: a catalog family the theme sets text in that is not installed.
    FontNotInstalled,
    /// E0901: the generated Typst source failed to compile.
    TypstCompile,
    /// E0902: a measurement marker missing or malformed.
    MarkerMissing,
    /// E0903: the measure and the frame walk disagree.
    OracleDisagreement,
    /// E0904: the PDF export failed.
    PdfExport,
    /// E0905: the output could not be published.
    Publish,
    /// W0101: a slide with no content.
    EmptySlide,
    /// W0102: a region filled beyond ninety-five percent.
    NearlyFull,
    /// W0103: an image below one hundred forty-four pixels per inch.
    LowResolution,
    /// W0104: a code language the highlighter does not know.
    UnknownLanguage,
    /// W0105: an image without alternative text.
    MissingAlt,
    /// W0106: a deck without a title.
    NoTitle,
    /// W0107: East Asian text in a deck whose language is not East Asian.
    CjkWithoutLang,
    /// W0108: a deck without a language.
    NoLang,
    /// W0501: a font the theme loads and never uses.
    UnusedFont,
    /// W0502: a fallback family without the exact weight a style sets.
    FallbackWeight,
    /// W0503: more than three hue families among the theme's colors.
    ManyHues,
    /// W0504: a paper whose chroma tints every slide.
    VividPaper,
}

impl Code {
    /// Every code, in the order of the catalog.
    pub const ALL: [Self; 83] = [
        Self::Internal,
        Self::NotUtf8,
        Self::UnclosedFrontMatter,
        Self::MalformedKeyValue,
        Self::UnknownKey,
        Self::InvalidValue,
        Self::DuplicateKey,
        Self::DuplicateSlideId,
        Self::NoSlides,
        Self::LimitExceeded,
        Self::HtmlBlock,
        Self::HtmlInline,
        Self::HtmlComment,
        Self::SetextHeading,
        Self::HeadingTooDeep,
        Self::Footnote,
        Self::TaskList,
        Self::Math,
        Self::IndentedCode,
        Self::LinkScheme,
        Self::ImagePlacement,
        Self::QuoteContent,
        Self::HardBreak,
        Self::NotesCount,
        Self::ThematicBreak,
        Self::OutsideSubset,
        Self::ListItemContent,
        Self::TableCells,
        Self::EmptyBlock,
        Self::UnresolvedReference,
        Self::MasterMissing,
        Self::UnknownMaster,
        Self::NoSlotForBlock,
        Self::RequiredSlotEmpty,
        Self::SectionCount,
        Self::CapacityExceeded,
        Self::TitlePosition,
        Self::UnknownSlideId,
        Self::VerticalOverflow,
        Self::HorizontalOverflow,
        Self::MultiplePages,
        Self::OutsidePage,
        Self::TooManyLines,
        Self::ThemeToml,
        Self::ThemeSchema,
        Self::ThemeValue,
        Self::RegionOutsidePage,
        Self::RegionOverlap,
        Self::BelowMinimumSize,
        Self::UndefinedReference,
        Self::BindingConflict,
        Self::NoMasterOrSlot,
        Self::MissingFace,
        Self::LineHeightBelowFace,
        Self::CodeThemeUnknown,
        Self::LowContrast,
        Self::FontMissing,
        Self::FontDigest,
        Self::FontUnreadable,
        Self::MissingGlyph,
        Self::ImageMissing,
        Self::ImageUnreadable,
        Self::ImageKind,
        Self::FamilyMismatch,
        Self::FamilyUnknown,
        Self::FontNotInstalled,
        Self::TypstCompile,
        Self::MarkerMissing,
        Self::OracleDisagreement,
        Self::PdfExport,
        Self::Publish,
        Self::EmptySlide,
        Self::NearlyFull,
        Self::LowResolution,
        Self::UnknownLanguage,
        Self::MissingAlt,
        Self::NoTitle,
        Self::CjkWithoutLang,
        Self::NoLang,
        Self::UnusedFont,
        Self::FallbackWeight,
        Self::ManyHues,
        Self::VividPaper,
    ];

    /// The stable code, such as `E0401`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Internal => "E0001",
            Self::NotUtf8 => "E0101",
            Self::UnclosedFrontMatter => "E0102",
            Self::MalformedKeyValue => "E0103",
            Self::UnknownKey => "E0104",
            Self::InvalidValue => "E0105",
            Self::DuplicateKey => "E0106",
            Self::DuplicateSlideId => "E0107",
            Self::NoSlides => "E0108",
            Self::LimitExceeded => "E0110",
            Self::HtmlBlock => "E0201",
            Self::HtmlInline => "E0202",
            Self::HtmlComment => "E0203",
            Self::SetextHeading => "E0204",
            Self::HeadingTooDeep => "E0205",
            Self::Footnote => "E0206",
            Self::TaskList => "E0207",
            Self::Math => "E0208",
            Self::IndentedCode => "E0209",
            Self::LinkScheme => "E0210",
            Self::ImagePlacement => "E0212",
            Self::QuoteContent => "E0213",
            Self::HardBreak => "E0214",
            Self::NotesCount => "E0215",
            Self::ThematicBreak => "E0216",
            Self::OutsideSubset => "E0217",
            Self::ListItemContent => "E0218",
            Self::TableCells => "E0219",
            Self::EmptyBlock => "E0220",
            Self::UnresolvedReference => "E0221",
            Self::MasterMissing => "E0301",
            Self::UnknownMaster => "E0302",
            Self::NoSlotForBlock => "E0303",
            Self::RequiredSlotEmpty => "E0304",
            Self::SectionCount => "E0305",
            Self::CapacityExceeded => "E0306",
            Self::TitlePosition => "E0307",
            Self::UnknownSlideId => "E0308",
            Self::VerticalOverflow => "E0401",
            Self::HorizontalOverflow => "E0402",
            Self::MultiplePages => "E0403",
            Self::OutsidePage => "E0404",
            Self::TooManyLines => "E0405",
            Self::ThemeToml => "E0501",
            Self::ThemeSchema => "E0502",
            Self::ThemeValue => "E0503",
            Self::RegionOutsidePage => "E0504",
            Self::RegionOverlap => "E0505",
            Self::BelowMinimumSize => "E0506",
            Self::UndefinedReference => "E0507",
            Self::BindingConflict => "E0508",
            Self::NoMasterOrSlot => "E0509",
            Self::MissingFace => "E0510",
            Self::LineHeightBelowFace => "E0511",
            Self::CodeThemeUnknown => "E0512",
            Self::LowContrast => "E0513",
            Self::FontMissing => "E0601",
            Self::FontDigest => "E0602",
            Self::FontUnreadable => "E0603",
            Self::MissingGlyph => "E0604",
            Self::ImageMissing => "E0605",
            Self::ImageUnreadable => "E0606",
            Self::ImageKind => "E0607",
            Self::FamilyMismatch => "E0608",
            Self::FamilyUnknown => "E0609",
            Self::FontNotInstalled => "E0610",
            Self::TypstCompile => "E0901",
            Self::MarkerMissing => "E0902",
            Self::OracleDisagreement => "E0903",
            Self::PdfExport => "E0904",
            Self::Publish => "E0905",
            Self::EmptySlide => "W0101",
            Self::NearlyFull => "W0102",
            Self::LowResolution => "W0103",
            Self::UnknownLanguage => "W0104",
            Self::MissingAlt => "W0105",
            Self::NoTitle => "W0106",
            Self::CjkWithoutLang => "W0107",
            Self::NoLang => "W0108",
            Self::UnusedFont => "W0501",
            Self::FallbackWeight => "W0502",
            Self::ManyHues => "W0503",
            Self::VividPaper => "W0504",
        }
    }

    /// Whether the code is an error or a warning, which its first letter spells.
    #[must_use]
    pub const fn severity(self) -> Severity {
        match self.as_str().as_bytes().first() {
            Some(b'W') => Severity::Warning,
            Some(_) | None => Severity::Error,
        }
    }

    /// Whether the code refuses a Markdown construct, which the subset rules report.
    #[must_use]
    pub const fn is_rejection(self) -> bool {
        matches!(
            self,
            Self::HtmlBlock
                | Self::HtmlInline
                | Self::HtmlComment
                | Self::SetextHeading
                | Self::HeadingTooDeep
                | Self::Footnote
                | Self::TaskList
                | Self::Math
                | Self::IndentedCode
                | Self::LinkScheme
                | Self::ImagePlacement
                | Self::QuoteContent
                | Self::HardBreak
                | Self::NotesCount
                | Self::ThematicBreak
                | Self::OutsideSubset
                | Self::ListItemContent
                | Self::TableCells
                | Self::EmptyBlock
                | Self::UnresolvedReference
        )
    }
}

impl fmt::Display for Code {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// A span of the deck with the words that explain it.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct Label {
    pub span: Span,
    pub text: String,
}

/// The slide a diagnostic belongs to.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct SlideRef {
    /// The slide's number in the deck, counted from one.
    pub number: usize,
    pub id: Option<Identifier>,
    pub master: Option<Identifier>,
}

/// One diagnostic: a code, a message, the spans it points at, and the notes and helps that follow.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct Diagnostic {
    pub code: Code,
    pub message: String,
    /// The span the message is about.
    pub primary: Option<Label>,
    /// Further spans that explain the message.
    pub secondary: Vec<Label>,
    pub notes: Vec<String>,
    pub helps: Vec<String>,
    pub slide: Option<SlideRef>,
}

impl Diagnostic {
    /// A diagnostic with its code and message and nothing else yet.
    #[must_use]
    pub const fn new(code: Code, message: String) -> Self {
        Self {
            code,
            message,
            primary: None,
            secondary: Vec::new(),
            notes: Vec::new(),
            helps: Vec::new(),
            slide: None,
        }
    }

    /// The same diagnostic pointing at `span` with `text` beside it.
    #[must_use]
    pub fn at(mut self, span: Span, text: String) -> Self {
        self.primary = Some(Label { span, text });
        self
    }

    /// The same diagnostic with a further span explained by `text`.
    #[must_use]
    pub fn also_at(mut self, span: Span, text: String) -> Self {
        self.secondary.push(Label { span, text });
        self
    }

    #[must_use]
    pub fn note(mut self, text: String) -> Self {
        self.notes.push(text);
        self
    }

    #[must_use]
    pub fn help(mut self, text: String) -> Self {
        self.helps.push(text);
        self
    }

    #[must_use]
    pub fn on_slide(mut self, slide: SlideRef) -> Self {
        self.slide = Some(slide);
        self
    }

    #[must_use]
    pub const fn severity(&self) -> Severity {
        self.code.severity()
    }
}

/// Whether any diagnostic is an error.
#[must_use]
pub fn has_error(diagnostics: &[Diagnostic]) -> bool {
    diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity() == Severity::Error)
}

#[cfg(test)]
mod tests {
    use super::{Code, Diagnostic, Severity, has_error};
    use crate::span::Span;
    use alloc::string::String;
    use alloc::vec;

    #[test]
    fn codes_spell_their_severity_and_read_back() {
        assert_eq!(Code::VerticalOverflow.as_str(), "E0401");
        assert_eq!(Code::VerticalOverflow.severity(), Severity::Error);
        assert_eq!(Code::EmptySlide.as_str(), "W0101");
        assert_eq!(Code::EmptySlide.severity(), Severity::Warning);
        assert_eq!(alloc::format!("{}", Code::HtmlComment), "E0203");
        assert!(Code::HtmlComment.is_rejection());
        assert!(!Code::VerticalOverflow.is_rejection());
    }

    #[test]
    fn a_diagnostic_collects_its_spans_notes_and_helps() {
        let diagnostic = Diagnostic::new(Code::HtmlBlock, String::from("raw HTML"))
            .at(Span::line(3, 5), String::from("here"))
            .also_at(Span::line(1, 2), String::from("and here"))
            .note(String::from("a note"))
            .help(String::from("a help"));
        assert_eq!(
            diagnostic.primary.as_ref().map(|label| label.span),
            Some(Span::line(3, 5))
        );
        assert_eq!(diagnostic.secondary.len(), 1);
        assert_eq!(diagnostic.notes, vec![String::from("a note")]);
        assert_eq!(diagnostic.helps, vec![String::from("a help")]);
        assert!(has_error(core::slice::from_ref(&diagnostic)));
        let warning = Diagnostic::new(Code::EmptySlide, String::from("empty"));
        assert!(!has_error(&[warning]));
        assert!(!has_error(&[]));
    }
}
