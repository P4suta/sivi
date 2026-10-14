//! The theme file read into the model, with every refused value named by its path in the file.

use crate::schema::{
    CodeSection, DecorSection, FontSection, FontSpec, ImageSection, InlineSection, ListsSection,
    MarkSection, MarkerSection, MasterSection, PaletteSection, QuotesSection, RegionSection,
    ScrimSection, ShadowSection, ShadowSpec, SlotSection, TablesSection, TextSection, ThemeFile,
    TypographySection, UnderlineSection, UnderlineSpec,
};
use crate::units::{self, Unit};
use sivi_core::diagnostic::{Code, Diagnostic};
use sivi_core::geometry_rules::{self, GeometryError};
use sivi_core::ir::{self, BlockKind};
use sivi_core::length::{Length, Point, Rect, Size};
use sivi_core::mark_rules;
use sivi_core::master_rules::closest_text;
use sivi_core::palette_rules::{self, Harmony, Mode, Palette, Seed};
use sivi_core::syntax_theme::Scheme;
use sivi_core::text::Identifier;
use sivi_core::theme::{
    Align, Binding, BlockKindSet, BottomEdge, Capacity, CodeStyle, Color, Decor, Digest,
    FamilyName, Field, Fit, FontFile, FontStack, ImageStyle, InlineStyles, Layer, ListStyle, Mark,
    Marker, Master, Measure, Page, QuoteStyle, Scrim, ScrimEdge, Shadow, Slot, StyleName,
    TableStyle, TextStyle, Theme, Thousandths, TopEdge, Typography, Underline, VerticalAlign,
    Weight,
};
use sivi_core::theme_rules::{self, BackdropFault, FollowsFault};

/// The file extensions a font file may carry.
pub const FONT_EXTENSIONS: [&str; 2] = ["ttf", "otf"];
/// The placeholder a page number format must hold.
const PAGE_NUMBER_PLACEHOLDER: &str = "{n}";
/// The thousandths in one whole, for percentages of a page side.
const WHOLE: i64 = 100_000;
/// The soft shadow `shadow = true` gives an image: the ink at a fifth, reaching six points right and down.
const SHADOW_COLOR: &str = "ink";
const SHADOW_OPACITY: Thousandths = Thousandths(200);
const SHADOW_OFFSET: Length = Length::from_millipoints(6_000);
/// How opaque a scrim is at its edge unless the theme says otherwise.
const SCRIM_OPACITY: Thousandths = Thousandths(600);

/// One refusal: its code, what was wrong, and what to write instead.
struct Fault {
    code: Code,
    message: String,
    help: &'static str,
}

/// A value outside its grammar.
const fn value(message: String, help: &'static str) -> Fault {
    Fault {
        code: Code::ThemeValue,
        message,
        help,
    }
}

/// A binding that contradicts another.
const fn conflict(message: String, help: &'static str) -> Fault {
    Fault {
        code: Code::BindingConflict,
        message,
        help,
    }
}

const IDENTIFIER_HELP: &str = "an identifier is a lowercase letter, then up to thirty-one lowercase letters, digits, or dashes";
const UNIT_HELP: &str =
    "write a decimal with up to three digits after the point and `pt`, `em`, or `%`";
const REGION_HELP: &str = "give the region a positive width and height";
const INSIDE_HELP: &str = "keep the region inside the page size";

struct Loader {
    diagnostics: Vec<Diagnostic>,
    colors: Vec<StyleName>,
    styles: Vec<StyleName>,
    page: Size,
}

impl Loader {
    fn refuse(&mut self, path: &str, fault: Fault) {
        self.diagnostics.push(
            Diagnostic::new(fault.code, fault.message)
                .note(format!("at `{path}` in the theme"))
                .help(String::from(fault.help)),
        );
    }

    fn style_name(&mut self, path: &str, text: &str) -> Option<StyleName> {
        match StyleName::new(text) {
            Ok(name) => Some(name),
            Err(_invalid) => {
                self.refuse(
                    path,
                    value(
                        format!("`{text}` is not a style name"),
                        "a style name is a lowercase letter, then lowercase letters, digits, or underscores",
                    ),
                );
                None
            }
        }
    }

    fn family(&mut self, path: &str, text: &str) -> Option<FamilyName> {
        match FamilyName::new(text) {
            Ok(name) => Some(name),
            Err(_invalid) => {
                self.refuse(
                    path,
                    value(
                        format!("`{text}` is not a font family name"),
                        "name the family as its font files do, such as `BIZ UDPGothic`",
                    ),
                );
                None
            }
        }
    }

    /// The families `spec` names in order, refusing an empty list and every name outside the grammar.
    fn stack(&mut self, path: &str, spec: &FontSpec) -> Option<FontStack> {
        let mut families = Vec::new();
        let mut valid = true;
        for (index, text) in spec.families().into_iter().enumerate() {
            let at = match spec {
                FontSpec::One(_) => String::from(path),
                FontSpec::Many(_) => format!("{path}[{index}]"),
            };
            match self.family(&at, text) {
                Some(family) => families.push(family),
                None => valid = false,
            }
        }
        let stack = FontStack::new(families);
        if stack.is_none() {
            self.refuse(
                path,
                value(
                    String::from("the font list names no family"),
                    "name the family to set the text in first, then any family to fall back to",
                ),
            );
        }
        if valid { stack } else { None }
    }

    fn identifier(&mut self, path: &str, text: &str) -> Option<Identifier> {
        match Identifier::new(text) {
            Ok(name) => Some(name),
            Err(_invalid) => {
                self.refuse(
                    path,
                    value(format!("`{text}` is not an identifier"), IDENTIFIER_HELP),
                );
                None
            }
        }
    }

    fn unit(&mut self, path: &str, text: &str) -> Option<Unit> {
        match units::parse(text) {
            Ok(unit) => Some(unit),
            Err(why) => {
                self.refuse(
                    path,
                    value(format!("`{text}` is not a unit: {why:?}"), UNIT_HELP),
                );
                None
            }
        }
    }

    fn points(&mut self, path: &str, text: &str) -> Option<Length> {
        match self.unit(path, text)? {
            Unit::Points(length) => Some(length),
            Unit::Em(_) | Unit::Percent(_) => {
                self.refuse(
                    path,
                    value(
                        format!("`{text}` is not a length in points"),
                        "write the length as `<n>pt`",
                    ),
                );
                None
            }
        }
    }

    fn measure(&mut self, path: &str, text: &str) -> Option<Measure> {
        match self.unit(path, text)? {
            Unit::Points(length) => Some(Measure::Points(length)),
            Unit::Em(ratio) => Some(Measure::Em(ratio)),
            Unit::Percent(_) => {
                self.refuse(
                    path,
                    value(
                        format!("`{text}` is not a length in points or ems"),
                        "write the length as `<n>pt` or `<n>em`",
                    ),
                );
                None
            }
        }
    }

    /// A coordinate or extent in points, or a percentage of `side`.
    fn along(&mut self, path: &str, text: &str, side: Length) -> Option<Length> {
        let scaled = match self.unit(path, text)? {
            Unit::Points(length) => Some(length),
            Unit::Percent(ratio) => side
                .millipoints()
                .checked_mul(ratio.0)
                .and_then(|product| product.checked_div(WHOLE))
                .map(Length::from_millipoints),
            Unit::Em(_) => {
                self.refuse(
                    path,
                    value(
                        format!("`{text}` is not a position on the page"),
                        "write the value as `<n>pt` or `<n>%` of the page side",
                    ),
                );
                return None;
            }
        };
        if scaled.is_none() {
            self.refuse(
                path,
                value(
                    format!("`{text}` is too large"),
                    "write a smaller percentage",
                ),
            );
        }
        scaled
    }

    fn region(&mut self, path: &str, region: &RegionSection) -> Option<Rect> {
        let page = self.page;
        let x = self.along(&format!("{path}.x"), &region.x, page.width);
        let y = self.along(&format!("{path}.y"), &region.y, page.height);
        let width = self.along(&format!("{path}.width"), &region.width, page.width);
        let height = self.along(&format!("{path}.height"), &region.height, page.height);
        Some(Rect::new(
            Point { x: x?, y: y? },
            Size {
                width: width?,
                height: height?,
            },
        ))
    }

    fn ratio(&mut self, path: &str, number: f64) -> Option<Thousandths> {
        match units::decimal(&format!("{number}")) {
            Ok(ratio) => Some(ratio),
            Err(why) => {
                self.refuse(
                    path,
                    value(
                        format!("`{number}` is not a ratio: {why:?}"),
                        "write a decimal with up to three digits after the point, such as `1.45`",
                    ),
                );
                None
            }
        }
    }

    /// An opacity from clear to solid as a ratio, which the alpha rule must accept.
    fn opacity(&mut self, path: &str, number: f64) -> Option<Thousandths> {
        let ratio = self.ratio(path, number)?;
        if mark_rules::alpha(ratio).is_none() {
            self.refuse(
                path,
                value(
                    format!("`{number}` is not an opacity"),
                    "write a number from 0 for clear to 1 for solid",
                ),
            );
            return None;
        }
        Some(ratio)
    }

    /// A length in points, or `default` when the text is left out.
    fn points_or(&mut self, path: &str, text: Option<&String>, default: Length) -> Option<Length> {
        text.map_or(Some(default), |text| self.points(path, text))
    }

    fn top_edge(&mut self, path: &str, text: &str) -> Option<TopEdge> {
        const EDGES: [TopEdge; 5] = [
            TopEdge::Ascender,
            TopEdge::CapHeight,
            TopEdge::XHeight,
            TopEdge::Baseline,
            TopEdge::Bounds,
        ];
        let found = EDGES.into_iter().find(|edge| edge.name() == text);
        if found.is_none() {
            self.refuse(
                path,
                value(
                    format!("`{text}` is not a top edge"),
                    "write `ascender`, `cap-height`, `x-height`, `baseline`, or `bounds`",
                ),
            );
        }
        found
    }

    fn bottom_edge(&mut self, path: &str, text: &str) -> Option<BottomEdge> {
        const EDGES: [BottomEdge; 3] = [
            BottomEdge::Descender,
            BottomEdge::Baseline,
            BottomEdge::Bounds,
        ];
        let found = EDGES.into_iter().find(|edge| edge.name() == text);
        if found.is_none() {
            self.refuse(
                path,
                value(
                    format!("`{text}` is not a bottom edge"),
                    "write `descender`, `baseline`, or `bounds`",
                ),
            );
        }
        found
    }

    fn weight(&mut self, path: &str, number: u16) -> Option<Weight> {
        let weight = Weight::new(number);
        if weight.is_none() {
            self.refuse(
                path,
                value(
                    format!("`{number}` is not a weight"),
                    "a weight is one hundred to nine hundred",
                ),
            );
        }
        weight
    }

    fn align(&mut self, path: &str, text: &str) -> Option<Align> {
        let align = match text {
            "left" => Some(Align::Left),
            "center" => Some(Align::Center),
            "right" => Some(Align::Right),
            _ => None,
        };
        if align.is_none() {
            self.refuse(
                path,
                value(
                    format!("`{text}` is not an alignment"),
                    "write `left`, `center`, or `right`",
                ),
            );
        }
        align
    }

    fn valign(&mut self, path: &str, text: &str) -> Option<VerticalAlign> {
        let valign = match text {
            "top" => Some(VerticalAlign::Top),
            "middle" => Some(VerticalAlign::Middle),
            "bottom" => Some(VerticalAlign::Bottom),
            _ => None,
        };
        if valign.is_none() {
            self.refuse(
                path,
                value(
                    format!("`{text}` is not a vertical alignment"),
                    "write `top`, `middle`, or `bottom`",
                ),
            );
        }
        valign
    }

    fn layer(&mut self, path: &str, text: &str) -> Option<Layer> {
        let layer = match text {
            "front" => Some(Layer::Front),
            "back" => Some(Layer::Back),
            _ => None,
        };
        if layer.is_none() {
            self.refuse(
                path,
                value(
                    format!("`{text}` is not a layer"),
                    "write `front` for a slot the content fills, or `back` for a backdrop",
                ),
            );
        }
        layer
    }

    fn fit(&mut self, path: &str, text: &str) -> Option<Fit> {
        let fit = match text {
            "contain" => Some(Fit::Contain),
            "cover" => Some(Fit::Cover),
            _ => None,
        };
        if fit.is_none() {
            self.refuse(
                path,
                value(
                    format!("`{text}` is not a fit"),
                    "write `contain` to show the whole image, or `cover` to fill the box",
                ),
            );
        }
        fit
    }

    fn scrim_edge(&mut self, path: &str, text: &str) -> Option<ScrimEdge> {
        let edge = match text {
            "bottom" => Some(ScrimEdge::Bottom),
            "top" => Some(ScrimEdge::Top),
            _ => None,
        };
        if edge.is_none() {
            self.refuse(
                path,
                value(
                    format!("`{text}` is not an edge a scrim starts from"),
                    "write `bottom` or `top`",
                ),
            );
        }
        edge
    }

    fn color(&mut self, path: &str, text: &str) -> Option<Color> {
        let color = units::color(text);
        if color.is_none() {
            self.refuse(
                path,
                value(
                    format!("`{text}` is not a color"),
                    "write the color as `#rrggbb`",
                ),
            );
        }
        color
    }

    fn block_kind(&mut self, path: &str, text: &str) -> Option<BlockKind> {
        let kind = match text {
            "heading" => Some(BlockKind::Heading),
            "paragraph" => Some(BlockKind::Paragraph),
            "list" => Some(BlockKind::List),
            "code" => Some(BlockKind::Code),
            "quote" => Some(BlockKind::Quote),
            "image" => Some(BlockKind::Image),
            "table" => Some(BlockKind::Table),
            _ => None,
        };
        if kind.is_none() {
            self.refuse(
                path,
                value(
                    format!("`{text}` is not a block kind"),
                    "write `paragraph`, `list`, `code`, `quote`, `image`, `table`, or `heading`",
                ),
            );
        }
        kind
    }

    fn color_ref(&mut self, path: &str, text: &str) -> Option<StyleName> {
        let name = self.style_name(path, text)?;
        if self.colors.contains(&name) {
            return Some(name);
        }
        self.undefined(path, "color", text);
        None
    }

    fn style_ref(&mut self, path: &str, text: &str) -> Option<StyleName> {
        let name = self.style_name(path, text)?;
        if self.styles.contains(&name) {
            return Some(name);
        }
        self.undefined(path, "text style", text);
        None
    }

    fn undefined(&mut self, path: &str, what: &str, name: &str) {
        self.refuse(
            path,
            Fault {
                code: Code::UndefinedReference,
                message: format!("`{name}` names no {what} of the theme"),
                help: "define it, or name one the theme defines",
            },
        );
    }

    fn relative_path(&mut self, path: &str, text: &str, extensions: &[&str]) -> Option<String> {
        if let Some(fault) = ir::relative_fault(text) {
            self.refuse(
                path,
                value(
                    format!("`{text}` does not stay beside the theme: {fault:?}"),
                    "name the file relative to the theme, with `/` between directories, without `..`",
                ),
            );
            return None;
        }
        if !ir::has_extension(text, extensions) {
            self.refuse(
                path,
                value(
                    format!("`{text}` does not end in {}", extensions.join(" or ")),
                    "name a file with one of those extensions",
                ),
            );
            return None;
        }
        Some(String::from(text))
    }
}

fn text_style(loader: &mut Loader, path: &str, section: &TextSection) -> Option<TextStyle> {
    let font = loader.stack(&format!("{path}.font"), &section.font);
    let size = loader.points(&format!("{path}.size"), &section.size);
    let weight = loader.weight(&format!("{path}.weight"), section.weight);
    let color = loader.color_ref(&format!("{path}.color"), &section.color);
    let line_height = loader.ratio(&format!("{path}.line_height"), section.line_height);
    let align = section.align.as_ref().map_or(Some(Align::Left), |text| {
        loader.align(&format!("{path}.align"), text)
    });
    Some(TextStyle {
        font: font?,
        size: size?,
        weight: weight?,
        color: color?,
        line_height: line_height?,
        align: align?,
    })
}

fn mark_style(loader: &mut Loader, path: &str, style: Option<&String>) -> Option<bool> {
    match style.map(String::as_str) {
        None | Some("color") => Some(false),
        Some("italic") => Some(true),
        Some(other) => {
            loader.refuse(
                &format!("{path}.style"),
                value(
                    format!("`{other}` is not a mark style"),
                    "write `italic` or `color`",
                ),
            );
            None
        }
    }
}

fn mark(loader: &mut Loader, path: &str, section: &MarkSection) -> Option<Mark> {
    let italic = mark_style(loader, path, section.style.as_ref());
    let color = section.color.as_ref().map_or(Some(None), |text| {
        loader.color_ref(&format!("{path}.color"), text).map(Some)
    });
    let weight = section.weight.map_or(Some(None), |number| {
        loader.weight(&format!("{path}.weight"), number).map(Some)
    });
    let font = section.font.as_ref().map_or(Some(None), |spec| {
        loader.stack(&format!("{path}.font"), spec).map(Some)
    });
    let background = section.background.as_ref().map_or(Some(None), |text| {
        loader
            .color_ref(&format!("{path}.background"), text)
            .map(Some)
    });
    let marker = section
        .marker
        .as_ref()
        .map_or(Some(None), |marker_section| {
            marker(loader, &format!("{path}.marker"), marker_section).map(Some)
        });
    // `underline = true` is the plain line, `false` or nothing is no line, and a table shapes the line.
    let underline = match &section.underline {
        None | Some(UnderlineSpec::Plain(false)) => Some(None),
        Some(UnderlineSpec::Plain(true)) => Some(Some(Underline::default())),
        Some(UnderlineSpec::Shaped(shaped)) => {
            shaped_underline(loader, &format!("{path}.underline"), shaped).map(Some)
        }
    };
    Some(Mark {
        italic: italic?,
        color: color?,
        weight: weight?,
        font: font?,
        background: background?,
        marker: marker?,
        underline: underline?,
    })
}

/// The band of `section`, with the solid, ascender-to-descender, flush, square band as the default.
fn marker(loader: &mut Loader, path: &str, section: &MarkerSection) -> Option<Marker> {
    let color = loader.color_ref(&format!("{path}.color"), &section.color);
    let opacity = section.opacity.map_or(Some(mark_rules::SOLID), |number| {
        loader.opacity(&format!("{path}.opacity"), number)
    });
    let top = section
        .top
        .as_ref()
        .map_or(Some(TopEdge::Ascender), |text| {
            loader.top_edge(&format!("{path}.top"), text)
        });
    let bottom = section
        .bottom
        .as_ref()
        .map_or(Some(BottomEdge::Descender), |text| {
            loader.bottom_edge(&format!("{path}.bottom"), text)
        });
    let extent = loader.points_or(
        &format!("{path}.extent"),
        section.extent.as_ref(),
        Length::ZERO,
    );
    let radius = loader.points_or(
        &format!("{path}.radius"),
        section.radius.as_ref(),
        Length::ZERO,
    );
    Some(Marker {
        color: color?,
        opacity: opacity?,
        top: top?,
        bottom: bottom?,
        extent: extent?,
        radius: radius?,
    })
}

/// The line `section` shapes, with the font's thickness and offset and the text color as the defaults.
fn shaped_underline(
    loader: &mut Loader,
    path: &str,
    section: &UnderlineSection,
) -> Option<Underline> {
    let color = section.color.as_ref().map_or(Some(None), |text| {
        loader.color_ref(&format!("{path}.color"), text).map(Some)
    });
    let opacity = section.opacity.map_or(Some(mark_rules::SOLID), |number| {
        loader.opacity(&format!("{path}.opacity"), number)
    });
    if section.opacity.is_some() && section.color.is_none() {
        loader.refuse(
            &format!("{path}.opacity"),
            value(
                String::from("an opacity without a color"),
                "set `color` beside `opacity`, since the line takes the text color otherwise",
            ),
        );
        return None;
    }
    let thickness = section.thickness.as_ref().map_or(Some(None), |text| {
        loader.points(&format!("{path}.thickness"), text).map(Some)
    });
    let offset = section.offset.as_ref().map_or(Some(None), |text| {
        loader.points(&format!("{path}.offset"), text).map(Some)
    });
    let extent = loader.points_or(
        &format!("{path}.extent"),
        section.extent.as_ref(),
        Length::ZERO,
    );
    Some(Underline {
        color: color?,
        opacity: opacity?,
        thickness: thickness?,
        offset: offset?,
        extent: extent?,
        behind: section.behind,
    })
}

fn inline(loader: &mut Loader, section: &InlineSection) -> Option<InlineStyles> {
    let emphasis = mark(loader, "inline.emphasis", &section.emphasis);
    let strong = mark(loader, "inline.strong", &section.strong);
    let code = mark(loader, "inline.code", &section.code);
    let link = mark(loader, "inline.link", &section.link);
    let marked = mark(loader, "inline.mark", &section.mark);
    Some(InlineStyles {
        emphasis: emphasis?,
        strong: strong?,
        code: code?,
        link: link?,
        mark: marked?,
    })
}

fn lists(loader: &mut Loader, section: &ListsSection) -> Option<ListStyle> {
    let indent = loader.measure("lists.indent", &section.indent);
    let gap = loader.measure("lists.gap", &section.gap);
    if section.bullets.is_empty() {
        loader.refuse(
            "lists.bullets",
            value(
                String::from("no bullet marker is listed"),
                "list at least one marker, outermost first",
            ),
        );
        return None;
    }
    Some(ListStyle {
        bullets: section.bullets.clone(),
        ordered: section.ordered.clone(),
        indent: indent?,
        gap: gap?,
    })
}

fn code(loader: &mut Loader, section: &CodeSection, min_size: Option<Length>) -> Option<CodeStyle> {
    let font = loader.stack("code.font", &section.font);
    let size = loader.points("code.size", &section.size);
    let line_height = loader.ratio("code.line_height", section.line_height);
    let background = loader.color_ref("code.background", &section.background);
    let padding = loader.points("code.padding", &section.padding);
    if let (Some(size), Some(min_size)) = (size, min_size)
        && size < min_size
    {
        loader.refuse(
            "code.size",
            Fault {
                code: Code::BelowMinimumSize,
                message: String::from("the code is set below the minimum size"),
                help: "set the size at or above `typography.min_size`, or lower the minimum",
            },
        );
    }
    let theme = section
        .theme
        .as_ref()
        .map_or(Some(None), |slug| code_theme(loader, slug).map(Some));
    Some(CodeStyle {
        font: font?,
        size: size?,
        line_height: line_height?,
        background: background?,
        padding: padding?,
        highlight: section.highlight,
        tab_width: section.tab_width,
        theme: theme?,
    })
}

/// The carried scheme `slug` names, or the refusal that names the nearest slug.
fn code_theme(loader: &mut Loader, slug: &str) -> Option<Scheme> {
    let schemes = match crate::schemes::schemes() {
        Ok(schemes) => schemes,
        Err(why) => {
            loader.diagnostics.push(Diagnostic::new(
                Code::Internal,
                format!("the carried code schemes do not parse: {why}"),
            ));
            return None;
        }
    };
    if let Some(found) = schemes.find(slug) {
        return Some(found.clone());
    }
    let help = closest_text(slug, schemes.slugs()).map_or_else(
        || String::from("`sivi code-themes` lists every scheme"),
        |nearest| format!("did you mean `{nearest}`? `sivi code-themes` lists every scheme"),
    );
    loader.diagnostics.push(
        Diagnostic::new(
            Code::CodeThemeUnknown,
            format!("`{slug}` is not a code theme the binary carries"),
        )
        .note(String::from("at `code.theme` in the theme"))
        .help(help),
    );
    None
}

fn tables(loader: &mut Loader, section: &TablesSection) -> Option<TableStyle> {
    let text = loader.style_ref("tables.text", &section.text);
    let header_weight = loader.weight("tables.header_weight", section.header_weight);
    let rule = loader.color_ref("tables.rule", &section.rule);
    let padding = loader.points("tables.padding", &section.padding);
    Some(TableStyle {
        text: text?,
        header_weight: header_weight?,
        rule: rule?,
        padding: padding?,
    })
}

fn quotes(loader: &mut Loader, section: &QuotesSection) -> Option<QuoteStyle> {
    let bar = loader.color_ref("quotes.bar", &section.bar);
    let bar_width = loader.points("quotes.bar_width", &section.bar_width);
    let indent = loader.points("quotes.indent", &section.indent);
    Some(QuoteStyle {
        bar: bar?,
        bar_width: bar_width?,
        indent: indent?,
    })
}

fn typography(loader: &mut Loader, section: &TypographySection) -> Option<Typography> {
    let min_size = loader.points("typography.min_size", &section.min_size);
    let paragraph_gap = loader.measure("typography.paragraph_gap", &section.paragraph_gap);
    let mut fallback = Vec::new();
    for (index, family) in section.fallback.iter().enumerate() {
        if let Some(found) = loader.family(&format!("typography.fallback[{index}]"), family) {
            fallback.push(found);
        }
    }
    Some(Typography {
        min_size: min_size?,
        paragraph_gap: paragraph_gap?,
        fallback,
    })
}

fn font(loader: &mut Loader, index: usize, section: &FontSection) -> Option<FontFile> {
    let path = format!("fonts[{index}]");
    let family = loader.family(&format!("{path}.family"), &section.family);
    let file = loader.relative_path(&format!("{path}.path"), &section.path, &FONT_EXTENSIONS);
    let sha256 = Digest::new(&section.sha256);
    if sha256.is_none() {
        loader.refuse(
            &format!("{path}.sha256"),
            value(
                String::from("the digest is not sixty-four lowercase hexadecimal digits"),
                "write the SHA-256 of the file as `shasum -a 256` prints it",
            ),
        );
    }
    Some(FontFile {
        family: family?,
        path: file?,
        sha256: sha256?,
    })
}

fn binding(loader: &mut Loader, path: &str, section: &SlotSection) -> Option<Binding> {
    match (section.binds.as_str(), section.section) {
        ("title", None) => Some(Binding::Title),
        ("subtitle", None) => Some(Binding::Subtitle),
        ("flow", None) => Some(Binding::Flow),
        ("section", Some(number)) => Some(Binding::Section(number)),
        ("section", None) => {
            loader.refuse(
                &format!("{path}.section"),
                value(
                    String::from("a section slot names no section number"),
                    "set `section = 1` for the first `##` of the slide, `2` for the second, and so on",
                ),
            );
            None
        }
        ("title" | "subtitle" | "flow", Some(_)) => {
            loader.refuse(
                &format!("{path}.section"),
                value(
                    String::from("only a section slot carries a section number"),
                    "remove `section`, or set `binds = \"section\"`",
                ),
            );
            None
        }
        (other, _) => {
            loader.refuse(
                &format!("{path}.binds"),
                value(
                    format!("`{other}` is not a binding"),
                    "write `title`, `subtitle`, `section`, or `flow`",
                ),
            );
            None
        }
    }
}

fn accepts(
    loader: &mut Loader,
    path: &str,
    kinds: &[String],
    binds: Option<Binding>,
) -> Option<BlockKindSet> {
    let mut accepted = BlockKindSet::EMPTY;
    for (index, kind) in kinds.iter().enumerate() {
        if let Some(kind) = loader.block_kind(&format!("{path}[{index}]"), kind) {
            accepted = accepted.with(kind);
        }
    }
    if matches!(binds, Some(Binding::Flow | Binding::Section(_))) && accepted.is_empty() {
        loader.refuse(
            path,
            value(
                String::from("a flow or section slot accepts no block kind"),
                "list the kinds the slot accepts, such as `accepts = [\"paragraph\", \"list\"]`",
            ),
        );
        return None;
    }
    Some(accepted)
}

/// The shadow `section` shapes, with the ink at a fifth reaching six points as the defaults.
fn shaped_shadow(loader: &mut Loader, path: &str, section: &ShadowSection) -> Option<Shadow> {
    let color = loader.color_ref(
        &format!("{path}.color"),
        section.color.as_deref().unwrap_or(SHADOW_COLOR),
    );
    let opacity = section.opacity.map_or(Some(SHADOW_OPACITY), |number| {
        loader.opacity(&format!("{path}.opacity"), number)
    });
    let offset = loader.points_or(
        &format!("{path}.offset"),
        section.offset.as_ref(),
        SHADOW_OFFSET,
    );
    Some(Shadow {
        color: color?,
        opacity: opacity?,
        offset: offset?,
    })
}

/// The shadow `shadow = true` asks for, which needs the theme to have the ink.
fn default_shadow(loader: &mut Loader, path: &str) -> Option<Shadow> {
    Some(Shadow {
        color: loader.color_ref(path, SHADOW_COLOR)?,
        opacity: SHADOW_OPACITY,
        offset: SHADOW_OFFSET,
    })
}

/// The scrim `section` shapes, from the bottom at six tenths unless it says otherwise.
fn scrim(loader: &mut Loader, path: &str, section: &ScrimSection) -> Option<Scrim> {
    let color = loader.color_ref(&format!("{path}.color"), &section.color);
    let opacity = section.opacity.map_or(Some(SCRIM_OPACITY), |number| {
        loader.opacity(&format!("{path}.opacity"), number)
    });
    let from = section
        .from
        .as_ref()
        .map_or(Some(ScrimEdge::Bottom), |text| {
            loader.scrim_edge(&format!("{path}.from"), text)
        });
    Some(Scrim {
        color: color?,
        opacity: opacity?,
        from: from?,
    })
}

/// How a sole image is shown, with the whole image in a square box and no shadow or scrim as the defaults.
fn image_style(loader: &mut Loader, path: &str, section: &ImageSection) -> Option<ImageStyle> {
    let fit = section.fit.as_ref().map_or(Some(Fit::Contain), |text| {
        loader.fit(&format!("{path}.fit"), text)
    });
    let radius = loader.points_or(
        &format!("{path}.radius"),
        section.radius.as_ref(),
        Length::ZERO,
    );
    // The default for `true`, none for `false` or nothing, or the shaped one.
    let shadow_path = format!("{path}.shadow");
    let shadow = match section.shadow.as_ref() {
        None | Some(ShadowSpec::Plain(false)) => Some(None),
        Some(ShadowSpec::Plain(true)) => default_shadow(loader, &shadow_path).map(Some),
        Some(ShadowSpec::Shaped(section)) => shaped_shadow(loader, &shadow_path, section).map(Some),
    };
    let scrim = section.scrim.as_ref().map_or(Some(None), |section| {
        scrim(loader, &format!("{path}.scrim"), section).map(Some)
    });
    Some(ImageStyle {
        fit: fit?,
        radius: radius?,
        shadow: shadow?,
        scrim: scrim?,
    })
}

/// Refuses a backdrop that binds, accepts, or holds anything other than one image.
fn backdrop_fault(loader: &mut Loader, path: &str, slot: &Slot) {
    if slot.layer != Layer::Back {
        return;
    }
    let fault = theme_rules::backdrop_fault(slot.binds, slot.accepts, slot.capacity.blocks);
    let (key, why) = match fault {
        None => return,
        Some(BackdropFault::Binds) => ("binds", "binds a heading"),
        Some(BackdropFault::Accepts) => ("accepts", "accepts something other than an image alone"),
        Some(BackdropFault::Blocks) => ("max_blocks", "holds other than one block"),
    };
    loader.refuse(
        &format!("{path}.{key}"),
        conflict(
            format!("the backdrop `{}` {why}", slot.name),
            "a backdrop is a flow slot with `accepts = [\"image\"]` and `max_blocks = 1`",
        ),
    );
}

fn slot(loader: &mut Loader, path: &str, name: &str, section: &SlotSection) -> Option<Slot> {
    let slot_name = loader.identifier(path, name);
    let binds = binding(loader, path, section);
    let accepted = accepts(loader, &format!("{path}.accepts"), &section.accepts, binds);
    let layer = section.layer.as_ref().map_or(Some(Layer::Front), |text| {
        loader.layer(&format!("{path}.layer"), text)
    });
    let image = section.image.as_ref().map_or_else(
        || Some(ImageStyle::default()),
        |section| image_style(loader, &format!("{path}.image"), section),
    );
    let follows = section.follows.as_ref().map_or(Some(None), |spelled| {
        loader
            .identifier(&format!("{path}.follows"), spelled)
            .map(Some)
    });
    let region = loader.region(&format!("{path}.region"), &section.region);
    let text = loader.style_ref(&format!("{path}.text"), &section.text);
    let valign = section
        .valign
        .as_ref()
        .map_or(Some(VerticalAlign::Top), |spelled| {
            loader.valign(&format!("{path}.valign"), spelled)
        });
    let align = section.align.as_ref().map_or(Some(None), |spelled| {
        loader.align(&format!("{path}.align"), spelled).map(Some)
    });
    let slot = Slot {
        name: slot_name?,
        binds: binds?,
        accepts: accepted?,
        required: section.required,
        follows: follows?,
        capacity: Capacity {
            blocks: section.max_blocks,
            items: section.max_items,
            depth: section.max_depth,
            rows: section.max_rows,
            columns: section.max_columns,
            lines: section.max_lines,
        },
        region: region?,
        text: text?,
        valign: valign?,
        align: align?,
        layer: layer?,
        image: image?,
    };
    backdrop_fault(loader, path, &slot);
    Some(slot)
}

fn decor_text(
    loader: &mut Loader,
    path: &str,
    section: &DecorSection,
) -> Option<(StyleName, Align)> {
    if section.text.is_none() {
        loader.refuse(
            &format!("{path}.text"),
            value(
                String::from("the decor names no text style"),
                "set `text` to a text style of the theme",
            ),
        );
    }
    let text = section
        .text
        .as_ref()
        .and_then(|spelled| loader.style_ref(&format!("{path}.text"), spelled));
    let align = section.align.as_ref().map_or(Some(Align::Left), |spelled| {
        loader.align(&format!("{path}.align"), spelled)
    });
    Some((text?, align?))
}

fn decor_field(loader: &mut Loader, path: &str, field: Option<&String>) -> Option<Field> {
    let found = match field.map(String::as_str) {
        Some("title") => Some(Field::Title),
        Some("author") => Some(Field::Author),
        Some("date") => Some(Field::Date),
        Some(_) | None => None,
    };
    if found.is_none() {
        loader.refuse(
            &format!("{path}.field"),
            value(
                String::from("the field decor names no field"),
                "set `field` to `title`, `author`, or `date`",
            ),
        );
    }
    found
}

fn decor(loader: &mut Loader, path: &str, section: &DecorSection) -> Option<Decor> {
    let region = loader.region(&format!("{path}.region"), &section.region)?;
    match section.kind.as_str() {
        "rect" => {
            if section.color.is_none() {
                loader.refuse(
                    &format!("{path}.color"),
                    value(
                        String::from("the rect names no color"),
                        "set `color` to a color of the theme",
                    ),
                );
            }
            let color = section
                .color
                .as_ref()
                .and_then(|spelled| loader.color_ref(&format!("{path}.color"), spelled));
            Some(Decor::Rect {
                region,
                color: color?,
            })
        }
        "image" => {
            let path_text = section.path.as_deref().unwrap_or_default();
            let file =
                loader.relative_path(&format!("{path}.path"), path_text, &ir::IMAGE_EXTENSIONS);
            Some(Decor::Image {
                region,
                path: file?,
            })
        }
        "page_number" => {
            let format = section
                .format
                .clone()
                .unwrap_or_else(|| String::from(PAGE_NUMBER_PLACEHOLDER));
            if !format.contains(PAGE_NUMBER_PLACEHOLDER) {
                loader.refuse(
                    &format!("{path}.format"),
                    value(
                        format!("`{format}` holds no `{PAGE_NUMBER_PLACEHOLDER}`"),
                        "write the page number as `{n}` inside the format",
                    ),
                );
                return None;
            }
            let (text, align) = decor_text(loader, path, section)?;
            Some(Decor::PageNumber {
                region,
                text,
                align,
                format,
            })
        }
        "field" => {
            let field = decor_field(loader, path, section.field.as_ref());
            let (text, align) = decor_text(loader, path, section)?;
            Some(Decor::Field {
                region,
                text,
                align,
                field: field?,
            })
        }
        other => {
            loader.refuse(
                &format!("{path}.kind"),
                value(
                    format!("`{other}` is not a decor kind"),
                    "write `rect`, `image`, `page_number`, or `field`",
                ),
            );
            None
        }
    }
}

/// The section numbers a master's slots bind, as the file numbers them.
fn sections(master: &Master) -> Vec<u32> {
    master
        .slots
        .iter()
        .filter_map(|slot| match slot.binds {
            Binding::Section(number) => Some(number),
            Binding::Title | Binding::Subtitle | Binding::Flow => None,
        })
        .collect()
}

/// The slot each slot follows as an index into the master's slots, or an index past the end for an unknown name.
fn follows_indices(master: &Master) -> Vec<Option<usize>> {
    master
        .slots
        .iter()
        .map(|slot| {
            slot.follows.as_ref().map(|name| {
                master
                    .slots
                    .iter()
                    .position(|other| other.name == *name)
                    .unwrap_or(master.slots.len())
            })
        })
        .collect()
}

fn bindings_fault(loader: &mut Loader, path: &str, master: &Master) {
    let titles = master
        .slots
        .iter()
        .filter(|slot| slot.binds == Binding::Title)
        .count();
    let subtitles = master
        .slots
        .iter()
        .filter(|slot| slot.binds == Binding::Subtitle)
        .count();
    if titles > 1 || subtitles > 1 {
        loader.refuse(
            path,
            conflict(
                String::from("two slots bind the same heading"),
                "keep one `title` slot and one `subtitle` slot per master",
            ),
        );
    }
    if let Some(missing) = theme_rules::missing_section(&sections(master)) {
        loader.refuse(
            path,
            conflict(
                format!("the section slots skip section {missing}"),
                "number the section slots from one without gaps or repeats",
            ),
        );
    }
    if let Some(fault) = theme_rules::follows_fault(&follows_indices(master)) {
        let (index, why) = match fault {
            FollowsFault::Unknown(index) => (index, "follows a slot the master does not have"),
            FollowsFault::SelfReference(index) => (index, "follows itself"),
            FollowsFault::Cycle(index) => (index, "starts a chain of `follows` that never ends"),
        };
        let name = master
            .slots
            .get(index)
            .map_or("?", |slot| slot.name.as_str());
        loader.refuse(
            &format!("{path}.slots.{name}.follows"),
            conflict(
                format!("the slot `{name}` {why}"),
                "name another slot of the same master, so the chain ends",
            ),
        );
    }
}

fn geometry_fault(loader: &mut Loader, path: &str, master: &Master) {
    let regions: Vec<Rect> = master.slots.iter().map(|slot| slot.region).collect();
    let layers: Vec<Layer> = master.slots.iter().map(|slot| slot.layer).collect();
    let name = |index: usize| {
        master
            .slots
            .get(index)
            .map_or("?", |slot| slot.name.as_str())
    };
    let page = loader.page;
    match geometry_rules::validate_layered(page, &regions, &layers) {
        Ok(()) => {}
        Err(GeometryError::Degenerate(index)) => loader.refuse(
            &format!("{path}.slots.{}.region", name(index)),
            value(String::from("the region has no area"), REGION_HELP),
        ),
        Err(GeometryError::OutsidePage(index)) => loader.refuse(
            &format!("{path}.slots.{}.region", name(index)),
            Fault {
                code: Code::RegionOutsidePage,
                message: format!("the slot `{}` leaves the page", name(index)),
                help: INSIDE_HELP,
            },
        ),
        Err(GeometryError::Overlap(first, second)) => loader.refuse(
            &format!("{path}.slots.{}.region", name(second)),
            Fault {
                code: Code::RegionOverlap,
                message: format!("the slots `{}` and `{}` overlap", name(first), name(second)),
                help: "move or shrink one region so they share no interior, or set `layer = \"back\"` on a backdrop",
            },
        ),
    }
    for (index, decor) in master.decor.iter().enumerate() {
        match geometry_rules::validate(page, &[decor.region()]) {
            Ok(()) | Err(GeometryError::Overlap(..)) => {}
            Err(GeometryError::Degenerate(_)) => loader.refuse(
                &format!("{path}.decor[{index}].region"),
                value(String::from("the region has no area"), REGION_HELP),
            ),
            Err(GeometryError::OutsidePage(_)) => loader.refuse(
                &format!("{path}.decor[{index}].region"),
                Fault {
                    code: Code::RegionOutsidePage,
                    message: format!("the decor {index} leaves the page"),
                    help: INSIDE_HELP,
                },
            ),
        }
    }
}

/// The grammar of a master as the binder sees it, in a form two masters can be compared by.
/// Whether two masters are one master under two names: the same slots in the same order, and the same decor.
///
/// Two masters that bind alike but lay out apart, as a picture on the left and on the right, are distinct.
fn same_master(master: &Master, other: &Master) -> bool {
    master.slots == other.slots && master.decor == other.decor
}

fn ambiguity_fault(loader: &mut Loader, masters: &[Master]) {
    for (index, master) in masters.iter().enumerate() {
        for other in masters.iter().skip(index.saturating_add(1)) {
            if same_master(master, other) {
                loader.refuse(
                    &format!("masters.{}", other.name),
                    conflict(
                        format!(
                            "the masters `{}` and `{}` bind and lay out every slide the same way",
                            master.name, other.name
                        ),
                        "make a slot differ in what it binds, accepts, requires, holds, or where it lies, or merge the masters",
                    ),
                );
            }
        }
    }
}

fn master(loader: &mut Loader, name: &str, raw: &toml::Value) -> Option<Master> {
    let path = format!("masters.{name}");
    let master_name = loader.identifier(&path, name);
    let section: MasterSection = match raw.clone().try_into() {
        Ok(read) => read,
        Err(error) => {
            loader.refuse(
                &path,
                Fault {
                    code: Code::ThemeSchema,
                    message: format!("the master does not fit the schema: {}", error.message()),
                    help: "see the theme format for the keys of a master",
                },
            );
            return None;
        }
    };
    let mut slots = Vec::new();
    for (slot_name, slot_value) in &section.slots {
        let slot_path = format!("{path}.slots.{slot_name}");
        let slot_section: SlotSection = match slot_value.clone().try_into() {
            Ok(read) => read,
            Err(error) => {
                loader.refuse(
                    &slot_path,
                    Fault {
                        code: Code::ThemeSchema,
                        message: format!("the slot does not fit the schema: {}", error.message()),
                        help: "see the theme format for the keys of a slot",
                    },
                );
                continue;
            }
        };
        if let Some(found) = slot(loader, &slot_path, slot_name, &slot_section) {
            slots.push(found);
        }
    }
    let mut decors = Vec::new();
    for (index, decor_section) in section.decor.iter().enumerate() {
        if let Some(found) = decor(loader, &format!("{path}.decor[{index}]"), decor_section) {
            decors.push(found);
        }
    }
    if slots.is_empty() {
        loader.refuse(
            &path,
            Fault {
                code: Code::NoMasterOrSlot,
                message: String::from("the master has no slot"),
                help: "give the master at least one slot",
            },
        );
        return None;
    }
    let master = Master {
        name: master_name?,
        slots,
        decor: decors,
    };
    bindings_fault(loader, &path, &master);
    geometry_fault(loader, &path, &master);
    Some(master)
}

/// The colors of the file, which every color reference is checked against.
fn colors(loader: &mut Loader, file: &ThemeFile) -> Vec<(StyleName, Color)> {
    let mut colors = Vec::new();
    for (name, text) in &file.colors {
        let path = format!("colors.{name}");
        let named = loader.style_name(&path, name);
        let color = loader.color(&path, text);
        if let (Some(named), Some(color)) = (named, color) {
            colors.push((named, color));
        }
    }
    colors
}

/// The colors of the file with the roles of its palette behind them, when it grows one, which every color reference is then checked against.
///
/// A named color wins over a role of the same name.
fn all_colors(loader: &mut Loader, file: &ThemeFile) -> Vec<(StyleName, Color)> {
    let mut colors = colors(loader, file);
    if let Some(section) = &file.palette
        && let Some(grown) = grown_palette(loader, section)
    {
        for (role, color) in grown.roles() {
            if let Ok(name) = StyleName::new(role)
                && !colors.iter().any(|(known, _)| *known == name)
            {
                colors.push((name, color));
            }
        }
    }
    loader.colors = colors
        .iter()
        .map(|(color_name, _)| color_name.clone())
        .collect();
    colors
}

/// The palette `section` grows: from a seed color or a carried name, in its mode, under its harmony.
fn grown_palette(loader: &mut Loader, section: &PaletteSection) -> Option<Palette> {
    let carried = match (&section.seed, &section.name) {
        (Some(_), None) | (None, Some(_)) => section.name.as_ref().and_then(|name| {
            let found = palette_rules::curated(name);
            if found.is_none() {
                let names: Vec<&str> = palette_rules::CURATED
                    .iter()
                    .map(|named| named.name)
                    .collect();
                loader.diagnostics.push(
                    Diagnostic::new(
                        Code::ThemeValue,
                        format!("`{name}` is not a palette the binary carries"),
                    )
                    .note(String::from("at `palette.name` in the theme"))
                    .help(format!("name one of {}", names.join(", "))),
                );
            }
            found
        }),
        (Some(_), Some(_)) | (None, None) => {
            loader.refuse(
                "palette",
                value(
                    String::from("a palette grows from a seed or a name"),
                    "set exactly one of `seed`, a color, and `name`, a carried palette",
                ),
            );
            return None;
        }
    };
    let seed = match &section.seed {
        Some(text) => Seed::of(loader.color("palette.seed", text)?),
        None => carried.map(|named| named.seed),
    }?;
    let mode = match &section.mode {
        Some(text) => mode(loader, text)?,
        None => carried.map_or(Mode::Light, |named| named.mode),
    };
    let harmony = match &section.harmony {
        Some(text) => harmony(loader, text)?,
        None => carried.map_or(Harmony::Complementary, |named| named.harmony),
    };
    let grown = palette_rules::palette(seed, mode, harmony);
    if grown.is_none() {
        loader.diagnostics.push(Diagnostic::new(
            Code::Internal,
            String::from("the palette could not be grown from its seed"),
        ));
    }
    grown
}

fn mode(loader: &mut Loader, text: &str) -> Option<Mode> {
    let found = Mode::from_name(text);
    if found.is_none() {
        loader.refuse(
            "palette.mode",
            value(
                format!("`{text}` is not a palette mode"),
                "write `light` or `dark`",
            ),
        );
    }
    found
}

fn harmony(loader: &mut Loader, text: &str) -> Option<Harmony> {
    let found = Harmony::from_name(text);
    if found.is_none() {
        loader.refuse(
            "palette.harmony",
            value(
                format!("`{text}` is not a harmony"),
                "write `mono`, `analogous`, `complementary`, `split`, or `triadic`",
            ),
        );
    }
    found
}

/// The text styles of the file, each at or above the minimum size.
fn text_styles(
    loader: &mut Loader,
    file: &ThemeFile,
    min_size: Option<Length>,
) -> Vec<(StyleName, TextStyle)> {
    let mut styles = Vec::new();
    for (name, section) in &file.text {
        let path = format!("text.{name}");
        let named = loader.style_name(&path, name);
        let style = text_style(loader, &path, section);
        if let (Some(named), Some(style)) = (named, style) {
            if let Some(min_size) = min_size
                && style.size < min_size
            {
                loader.refuse(
                    &format!("{path}.size"),
                    Fault {
                        code: Code::BelowMinimumSize,
                        message: format!("the text style `{named}` is set below the minimum size"),
                        help: "set the size at or above `typography.min_size`, or lower the minimum",
                    },
                );
            }
            styles.push((named, style));
        }
    }
    styles
}

fn masters(loader: &mut Loader, file: &ThemeFile) -> Vec<Master> {
    let mut masters = Vec::new();
    for (name, raw) in &file.masters {
        if let Some(found) = master(loader, name, raw) {
            masters.push(found);
        }
    }
    if file.masters.is_empty() {
        loader.refuse(
            "masters",
            Fault {
                code: Code::NoMasterOrSlot,
                message: String::from("the theme has no master"),
                help: "define at least one master with a slot",
            },
        );
    }
    ambiguity_fault(loader, &masters);
    masters
}

/// The theme `file` describes, or every diagnostic that keeps it from being one.
///
/// # Errors
///
/// Returns every refused value, undefined reference, binding conflict, and geometry fault of the file.
pub fn build(file: &ThemeFile) -> Result<Theme, Vec<Diagnostic>> {
    let mut loader = Loader {
        diagnostics: Vec::new(),
        colors: Vec::new(),
        styles: Vec::new(),
        page: Size::default(),
    };
    let name = loader.identifier("theme.name", &file.theme.name);
    let width = loader.points("page.width", &file.page.width);
    let height = loader.points("page.height", &file.page.height);
    if let (Some(width), Some(height)) = (width, height) {
        loader.page = Size { width, height };
    }
    let colors = all_colors(&mut loader, file);
    for style_key in file.text.keys() {
        if let Ok(style) = StyleName::new(style_key) {
            loader.styles.push(style);
        }
    }
    let background = loader.color_ref("page.background", &file.page.background);
    let typography = typography(&mut loader, &file.typography);
    let min_size = typography.as_ref().map(|typography| typography.min_size);
    let text = text_styles(&mut loader, file, min_size);
    let inline = inline(&mut loader, &file.inline);
    let lists = lists(&mut loader, &file.lists);
    let code = code(&mut loader, &file.code, min_size);
    let tables = tables(&mut loader, &file.tables);
    let quotes = quotes(&mut loader, &file.quotes);
    let mut fonts = Vec::new();
    for (index, section) in file.fonts.iter().enumerate() {
        if let Some(found) = font(&mut loader, index, section) {
            fonts.push(found);
        }
    }
    let masters = if width.is_some() && height.is_some() {
        masters(&mut loader, file)
    } else {
        Vec::new()
    };
    if !loader.diagnostics.is_empty() {
        return Err(loader.diagnostics);
    }
    let (
        Some(name),
        Some(background),
        Some(typography),
        Some(inline),
        Some(lists),
        Some(code),
        Some(tables),
        Some(quotes),
    ) = (
        name, background, typography, inline, lists, code, tables, quotes,
    )
    else {
        return Err(vec![Diagnostic::new(
            Code::Internal,
            String::from("the theme loader refused a value without reporting it"),
        )]);
    };
    Ok(Theme {
        name,
        page: Page {
            size: loader.page,
            background,
        },
        colors,
        typography,
        text,
        inline,
        lists,
        code,
        tables,
        quotes,
        fonts,
        masters,
    })
}
