//! The theme loader of sivi.
//!
//! It reads a theme file into the core's theme model, refuses every value outside the format, and reads font faces for the checks that need their metrics.

pub mod faces;
pub mod load;
pub mod schema;
pub mod units;

use sivi_core::diagnostic::{Code, Diagnostic};
use sivi_core::span::{LineColumn, Span};
use sivi_core::theme::Theme;

/// The theme the binary carries.
pub const PLAIN: &str = include_str!("../../../themes/plain/theme.toml");

/// The name of the theme the binary carries.
pub const PLAIN_NAME: &str = "plain";

/// The most bytes a theme file may hold.
pub const THEME_LIMIT: usize = 1024 * 1024;

/// The text of the theme the binary carries under `name`, if any.
#[must_use]
pub fn builtin(name: &str) -> Option<&'static str> {
    if name == PLAIN_NAME {
        Some(PLAIN)
    } else {
        None
    }
}

/// The position of byte `offset` in `text`, counted from one.
fn position(text: &str, offset: usize) -> LineColumn {
    let before = text.get(..offset).unwrap_or_default();
    let line = before.split('\n').count();
    let column = before
        .rsplit('\n')
        .next()
        .map_or(1, |rest| rest.len().saturating_add(1));
    LineColumn::new(line, column)
}

/// Whether a TOML reader error is about the shape of the file rather than its syntax.
fn is_schema_error(message: &str) -> bool {
    [
        "unknown field",
        "missing field",
        "invalid type",
        "duplicate field",
        "unknown variant",
    ]
    .iter()
    .any(|needle| message.contains(needle))
}

fn toml_error(text: &str, error: &toml::de::Error) -> Diagnostic {
    let code = if is_schema_error(error.message()) {
        Code::ThemeSchema
    } else {
        Code::ThemeToml
    };
    let diagnostic = Diagnostic::new(
        code,
        format!("the theme file cannot be read: {}", error.message()),
    );
    match error.span() {
        Some(range) => {
            let last = range.end.saturating_sub(1).max(range.start);
            diagnostic.at(
                Span::new(position(text, range.start), position(text, last)),
                String::from("here"),
            )
        }
        None => diagnostic,
    }
    .help(String::from(
        "see the theme format for the keys and values of a theme file",
    ))
}

/// The theme `text` describes, or every diagnostic that keeps it from being one.
///
/// # Errors
///
/// Returns a syntax or schema diagnostic, or every value, reference, binding, and geometry fault of the file.
pub fn parse(text: &str) -> Result<Theme, Vec<Diagnostic>> {
    if text.len() > THEME_LIMIT {
        return Err(vec![
            Diagnostic::new(
                Code::LimitExceeded,
                format!(
                    "the theme holds {} bytes, beyond the limit of {THEME_LIMIT}",
                    text.len()
                ),
            )
            .help(String::from("move fonts and images out of the theme file")),
        ]);
    }
    let file: schema::ThemeFile = match toml::from_str(text) {
        Ok(file) => file,
        Err(error) => return Err(vec![toml_error(text, &error)]),
    };
    if file.schema != schema::SCHEMA {
        return Err(vec![
            Diagnostic::new(
                Code::ThemeSchema,
                format!(
                    "the theme declares schema {}, which this version does not read",
                    file.schema
                ),
            )
            .help(format!("set `schema = {}`", schema::SCHEMA)),
        ]);
    }
    load::build(&file)
}

#[cfg(test)]
mod tests {
    use super::{PLAIN, THEME_LIMIT, builtin, parse, position};
    use sivi_core::diagnostic::Code;
    use sivi_core::ir::BlockKind;
    use sivi_core::span::LineColumn;
    use sivi_core::text::Identifier;
    use sivi_core::theme::{Binding, Measure, StyleName, Thousandths, Weight};

    const PAGE_WIDTH_MILLIPOINTS: i64 = 960_000;
    const BODY_TOP_MILLIPOINTS: i64 = 156_000;
    const BODY_LINE_HEIGHT: Thousandths = Thousandths(1450);
    const PARAGRAPH_GAP: Thousandths = Thousandths(600);
    const BULLET_ITEMS: u32 = 8;
    const STRONG_WEIGHT: u16 = 700;

    fn codes(text: &str) -> Vec<Code> {
        match parse(text) {
            Ok(_) => Vec::new(),
            Err(diagnostics) => diagnostics.iter().map(|d| d.code).collect(),
        }
    }

    fn master_names(text: &str) -> Vec<String> {
        parse(text)
            .unwrap()
            .masters
            .iter()
            .map(|master| String::from(master.name.as_str()))
            .collect()
    }

    #[test]
    #[cfg_attr(miri, ignore = "the plain theme loads in minutes under Miri")]
    fn the_plain_theme_reads_into_eight_masters_in_order() {
        assert_eq!(
            master_names(PLAIN),
            [
                "title",
                "section",
                "bullets",
                "comparison",
                "figure",
                "quote",
                "code",
                "blank"
            ]
        );
        let theme = parse(PLAIN).unwrap();
        assert_eq!(theme.name.as_str(), "plain");
        assert_eq!(theme.page.size.width.millipoints(), PAGE_WIDTH_MILLIPOINTS);
        let bullets = theme.master(&Identifier::new("bullets").unwrap()).unwrap();
        assert_eq!(bullets.slots[0].name.as_str(), "title");
        assert_eq!(bullets.slots[0].binds, Binding::Title);
        assert_eq!(bullets.slots[1].binds, Binding::Flow);
        assert!(bullets.slots[1].accepts.contains(BlockKind::List));
        assert!(!bullets.slots[1].accepts.contains(BlockKind::Code));
        assert_eq!(bullets.slots[1].capacity.items, Some(BULLET_ITEMS));
        assert_eq!(
            bullets.slots[1].region.origin.y.millipoints(),
            BODY_TOP_MILLIPOINTS
        );
        assert_eq!(bullets.decor.len(), 1);
        assert_eq!(builtin("plain"), Some(PLAIN));
        assert_eq!(builtin("other"), None);
    }

    #[test]
    #[cfg_attr(miri, ignore = "the plain theme loads in minutes under Miri")]
    fn the_plain_theme_sections_follows_and_styles_read_back() {
        let theme = parse(PLAIN).unwrap();
        let comparison = theme
            .master(&Identifier::new("comparison").unwrap())
            .unwrap();
        assert_eq!(comparison.slots[2].binds, Binding::Section(2));
        let figure = theme.master(&Identifier::new("figure").unwrap()).unwrap();
        assert_eq!(
            figure.slots[2].follows.as_ref().map(Identifier::as_str),
            Some("figure")
        );
        let body = theme.text_style(&StyleName::new("body").unwrap()).unwrap();
        assert_eq!(body.line_height, BODY_LINE_HEIGHT);
        assert_eq!(theme.typography.paragraph_gap, Measure::Em(PARAGRAPH_GAP));
        assert_eq!(
            theme.inline.strong.weight.map(Weight::value),
            Some(STRONG_WEIGHT)
        );
        assert!(theme.inline.link.underline);
    }

    #[test]
    #[cfg_attr(miri, ignore = "the plain theme loads in minutes under Miri")]
    fn syntax_and_schema_faults_are_told_apart_with_their_position() {
        let Err(syntax) = parse("schema = 1\n[theme\n") else {
            panic!("refused")
        };
        assert_eq!(syntax[0].code, Code::ThemeToml);
        assert_eq!(
            syntax[0]
                .primary
                .as_ref()
                .map(|label| label.span.start.line),
            Some(2)
        );
        let misspelled = PLAIN.replace("highlight = true", "highlite = true");
        let Err(unknown) = parse(&misspelled) else {
            panic!("refused")
        };
        assert_eq!(unknown[0].code, Code::ThemeSchema);
        assert!(unknown[0].primary.is_some());
        let version = PLAIN.replace("schema = 1", "schema = 2");
        assert_eq!(codes(&version), vec![Code::ThemeSchema]);
        assert_eq!(position("ab\ncd", 4), LineColumn::new(2, 2));
        assert_eq!(position("ab\ncd", 0), LineColumn::new(1, 1));
    }

    /// Each case replaces a fragment of the plain theme and names the code and the path the loader reports.
    const FAULTS: [(&str, &str, Code, &str); 12] = [
        (
            "size = \"24pt\"\nweight = 400\ncolor = \"ink\"\nline_height = 1.45\nalign = \"left\"\n\n[text.quote]",
            "size = \"24px\"\nweight = 400\ncolor = \"ink\"\nline_height = 1.45\nalign = \"left\"\n\n[text.quote]",
            Code::ThemeValue,
            "text.body.size",
        ),
        (
            "color = \"muted\"\nline_height = 1.2\nalign = \"left\"\n\n[inline]",
            "color = \"mutted\"\nline_height = 1.2\nalign = \"left\"\n\n[inline]",
            Code::UndefinedReference,
            "text.meta.color",
        ),
        (
            "height = \"324pt\" }\ntext = \"body\"\nmax_items = 8",
            "height = \"324pt\" }\ntext = \"bodyy\"\nmax_items = 8",
            Code::UndefinedReference,
            "masters.bullets.slots.body.text",
        ),
        (
            "size = \"18pt\"\nweight = 400\ncolor = \"muted\"\nline_height = 1.3\nalign = \"center\"",
            "size = \"12pt\"\nweight = 400\ncolor = \"muted\"\nline_height = 1.3\nalign = \"center\"",
            Code::BelowMinimumSize,
            "text.caption.size",
        ),
        (
            "region = { x = \"495pt\", y = \"156pt\", width = \"405pt\", height = \"324pt\" }",
            "region = { x = \"495pt\", y = \"156pt\", width = \"505pt\", height = \"324pt\" }",
            Code::RegionOutsidePage,
            "masters.comparison.slots.right.region",
        ),
        (
            "region = { x = \"495pt\", y = \"156pt\", width = \"405pt\", height = \"324pt\" }",
            "region = { x = \"400pt\", y = \"156pt\", width = \"405pt\", height = \"324pt\" }",
            Code::RegionOverlap,
            "masters.comparison.slots.right.region",
        ),
        (
            "section = 2",
            "section = 3",
            Code::BindingConflict,
            "masters.comparison",
        ),
        (
            "follows = \"figure\"",
            "follows = \"caption\"",
            Code::BindingConflict,
            "masters.figure.slots.caption.follows",
        ),
        (
            "follows = \"quote\"",
            "follows = \"nowhere\"",
            Code::BindingConflict,
            "masters.quote.slots.attribution.follows",
        ),
        (
            "binds = \"flow\"\naccepts = [\"code\"]",
            "binds = \"flow\"\naccepts = [\"kode\"]",
            Code::ThemeValue,
            "masters.code.slots.code.accepts[0]",
        ),
        (
            "format = \"{n}\"",
            "format = \"n\"",
            Code::ThemeValue,
            "masters.section.decor[0].format",
        ),
        (
            "weight = 700\ncolor = \"ink\"\nline_height = 1.2\nalign = \"left\"\n\n[text.title]",
            "weight = 950\ncolor = \"ink\"\nline_height = 1.2\nalign = \"left\"\n\n[text.title]",
            Code::ThemeValue,
            "text.display.weight",
        ),
    ];

    #[test]
    #[cfg_attr(miri, ignore = "the plain theme loads in minutes under Miri")]
    fn each_semantic_fault_is_named_by_its_path() {
        for (from, to, code, path) in FAULTS {
            assert!(PLAIN.contains(from), "{from:?} is in the plain theme");
            let text = PLAIN.replacen(from, to, 1);
            let Err(found) = parse(&text) else {
                panic!("{path}: accepted");
            };
            assert!(
                found
                    .iter()
                    .any(|d| d.code == code && d.notes.iter().any(|note| note.contains(path))),
                "{path}: {found:?}"
            );
        }
    }

    /// A theme of one master, which Miri interprets in seconds.
    const SMALL: &str = include_str!("../../../fuzz/corpus/theme_toml/minimal.toml");

    #[test]
    fn a_small_theme_loads_and_its_faulty_twin_is_refused() {
        let theme = parse(SMALL).unwrap();
        assert_eq!(theme.name.as_str(), "tiny");
        assert_eq!(theme.masters.len(), 1);
        assert_eq!(
            theme.masters[0].slots[0].region.size.width,
            theme.page.size.width
        );
        let faulty = SMALL.replacen("background = \"paper\"", "background = \"nothing\"", 1);
        assert_eq!(codes(&faulty), vec![Code::UndefinedReference]);
    }

    #[test]
    #[cfg_attr(miri, ignore = "the plain theme loads in minutes under Miri")]
    fn a_theme_without_masters_or_slots_and_ambiguous_masters_are_refused() {
        let start = PLAIN.find("[masters.title.slots.title]").unwrap();
        let head = PLAIN.get(..start).unwrap();
        let without = format!("{head}[masters]\n");
        assert_eq!(codes(&without), vec![Code::NoMasterOrSlot]);
        let empty_master = format!("{head}[masters.only]\nslots = {{}}\n");
        assert!(codes(&empty_master).contains(&Code::NoMasterOrSlot));
        let twin = PLAIN.replacen(
            "[masters.blank.slots.body]",
            "[masters.blank2.slots.body]\nbinds = \"flow\"\naccepts = [\"paragraph\", \"list\", \"table\", \"image\", \"code\", \"quote\"]\nregion = { x = \"60pt\", y = \"40pt\", width = \"840pt\", height = \"440pt\" }\ntext = \"body\"\n\n[masters.blank.slots.body]",
            1,
        );
        let Err(found) = parse(&twin) else {
            panic!("refused")
        };
        assert!(
            found
                .iter()
                .any(|d| d.code == Code::BindingConflict && d.message.contains("blank2"))
        );
    }

    /// The theme limit the documents promise, in bytes.
    const ONE_MEBIBYTE: usize = 1_048_576;

    #[test]
    #[cfg_attr(
        miri,
        ignore = "a theme at the byte limit parses in minutes under Miri"
    )]
    fn a_theme_at_its_byte_limit_still_loads_and_one_byte_more_is_refused() {
        assert_eq!(THEME_LIMIT, ONE_MEBIBYTE);
        let comment = "#".repeat(THEME_LIMIT.saturating_sub(SMALL.len()).saturating_sub(1));
        let at_limit = format!("{SMALL}\n{comment}");
        assert_eq!(at_limit.len(), THEME_LIMIT);
        parse(&at_limit).unwrap();
        assert_eq!(codes(&format!("{at_limit}#")), vec![Code::LimitExceeded]);
    }

    #[test]
    fn a_flow_slot_may_accept_headings() {
        let headings = SMALL.replacen("accepts = [\"paragraph\"]", "accepts = [\"heading\"]", 1);
        let theme = parse(&headings).unwrap();
        assert!(
            theme.masters[0].slots[0]
                .accepts
                .contains(BlockKind::Heading)
        );
    }

    #[test]
    fn an_image_decor_names_a_file_with_an_image_extension() {
        let half = SMALL.replacen("height = \"100%\"", "height = \"50%\"", 1);
        let logo = format!(
            "{half}\n[[masters.only.decor]]\nkind = \"image\"\npath = \"logo.png\"\nregion = {{ x = \"0pt\", y = \"50pt\", width = \"100pt\", height = \"50pt\" }}\n"
        );
        parse(&logo).unwrap();
        let text_file = logo.replacen("logo.png", "logo.txt", 1);
        assert_eq!(codes(&text_file), vec![Code::ThemeValue]);
    }

    #[test]
    fn code_below_the_minimum_size_is_refused_and_above_it_loads() {
        const CODE_SIZE: &str = "size = \"18pt\"\nline_height = 1.3";
        let small = SMALL.replacen(CODE_SIZE, "size = \"16pt\"\nline_height = 1.3", 1);
        assert_eq!(codes(&small), vec![Code::BelowMinimumSize]);
        let large = SMALL.replacen(CODE_SIZE, "size = \"20pt\"\nline_height = 1.3", 1);
        parse(&large).unwrap();
    }

    #[test]
    fn two_subtitle_slots_conflict_as_two_title_slots_do() {
        let slot = |name: &str, binds: &str, y: &str| {
            format!(
                "[masters.only.slots.{name}]\nbinds = \"{binds}\"\nregion = {{ x = \"0pt\", y = \"{y}\", width = \"100pt\", height = \"20pt\" }}\ntext = \"body\"\n"
            )
        };
        let start = SMALL.find("[masters.only.slots.body]").unwrap();
        let head = SMALL.get(..start).unwrap();
        let subtitles = format!(
            "{head}{}{}{}",
            slot("title", "title", "0pt"),
            slot("first", "subtitle", "20pt"),
            slot("second", "subtitle", "40pt")
        );
        assert_eq!(codes(&subtitles), vec![Code::BindingConflict]);
        let titles = format!(
            "{head}{}{}",
            slot("title", "title", "0pt"),
            slot("again", "title", "20pt")
        );
        assert_eq!(codes(&titles), vec![Code::BindingConflict]);
    }

    #[test]
    fn a_page_without_a_size_withholds_the_masters_and_their_faults() {
        let wide = SMALL.replacen("width = \"100pt\"", "width = \"wide\"", 1);
        assert_eq!(codes(&wide), vec![Code::ThemeValue]);
    }
}
