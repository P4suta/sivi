//! The verdicts over a measured deck: which region its content overflows and by how much, which slide spilled, and what change would make it fit.
//!
//! Every number in a diagnostic comes from the adapter's measurement; the report never guesses a layout.

use crate::attribute_rules::{part_at, region_of};
use crate::bind::{BoundDeck, BoundRegion};
use crate::diagnostic::{Code, Diagnostic, SlideRef};
use crate::fit_rules::{Fit, PageVerdict, fit, pages};
use crate::ir::{self, Block, BlockBody, BlockKind, Deck, Slide};
use crate::length::{EPSILON, Length, Point, Rect};
use crate::measure::{Expected, Leaf, Measurement, RegionMeasure};
use crate::protocol::SlideIndex;
use crate::span::Span;
use crate::split_rules::{SplitVerdict, split_point};
use crate::theme::{Binding, Master, Slot, Theme, VerticalAlign};
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

/// Where the catalog of diagnostics explains each code.
pub const DIAGNOSTICS_DOC: &str = "docs/diagnostics.md";
/// The fill beyond which a region is nearly full, in percent.
const NEARLY_FULL_PERCENT: i64 = 95;
const PERCENT: i64 = 100;
const THOUSAND: i64 = 1000;
const HALF: i64 = 2;

/// What the adapter measured and what it was asked to measure.
#[derive(Clone, Copy, Debug)]
pub struct Measured<'a> {
    pub expected: &'a Expected,
    pub measurement: &'a Measurement,
}

/// One measured region with everything the diagnostics name about it.
struct Site<'a> {
    slide: &'a Slide,
    master: &'a Master,
    slot: &'a Slot,
    region: &'a BoundRegion,
    bounds: Rect,
    measure: &'a RegionMeasure,
}

impl Site<'_> {
    fn reference(&self) -> SlideRef {
        SlideRef {
            number: self.slide.number,
            id: self.slide.id.clone(),
            master: Some(self.master.name.clone()),
        }
    }

    fn block(&self, part: usize) -> Option<&Block> {
        self.region
            .blocks
            .get(part)
            .and_then(|position| self.slide.blocks.get(*position))
    }

    fn first_span(&self) -> Span {
        self.block(0).map_or(self.slide.span, |block| block.span)
    }

    fn last_span(&self) -> Option<Span> {
        (self.region.blocks.len() > 1)
            .then(|| self.block(self.region.blocks.len().saturating_sub(1)))
            .flatten()
            .map(|block| block.span)
    }

    /// The vertical shift the emitter gives the content, from the slot's vertical alignment.
    fn shift(&self) -> Length {
        let free = self
            .bounds
            .size
            .height
            .saturating_sub(self.measure.required)
            .max(Length::ZERO);
        match self.slot.valign {
            VerticalAlign::Top => Length::ZERO,
            VerticalAlign::Middle => {
                Length::from_millipoints(free.0.checked_div(HALF).unwrap_or(0))
            }
            VerticalAlign::Bottom => free,
        }
    }

    /// The line pitch of the slot's text style.
    fn pitch(&self, theme: &Theme) -> Option<Length> {
        let style = theme.text_style(&self.slot.text)?;
        let pitch = style
            .size
            .0
            .checked_mul(style.line_height.0)?
            .checked_div(THOUSAND)?;
        Some(Length::from_millipoints(pitch))
    }

    /// How many lines the content renders and how many fit, from the pitch.
    fn lines(&self, pitch: Length) -> Option<(i64, i64)> {
        let half = pitch.0.checked_div(HALF)?;
        let rendered = self
            .measure
            .required
            .0
            .checked_add(half)?
            .checked_div(pitch.0)?;
        let fit = self
            .bounds
            .size
            .height
            .0
            .checked_add(EPSILON.0)?
            .checked_div(pitch.0)?;
        Some((rendered, fit))
    }

    /// The kinds of the region's blocks that a slot must accept, which leaves out the headings every slot takes.
    fn kinds(&self) -> Vec<BlockKind> {
        let mut kinds = Vec::new();
        for part in 0..self.region.blocks.len() {
            if let Some(block) = self.block(part) {
                let kind = block.body.kind();
                if kind != BlockKind::Heading && !kinds.contains(&kind) {
                    kinds.push(kind);
                }
            }
        }
        kinds
    }
}

/// The percent of `whole` that `part` is, saturated.
fn percent(part: Length, whole: Length) -> i64 {
    part.0
        .saturating_mul(PERCENT)
        .checked_div(whole.0)
        .unwrap_or(0)
}

struct Reporter<'a> {
    theme: &'a Theme,
    deck: &'a Deck,
    bound: &'a BoundDeck,
    measured: Measured<'a>,
    diagnostics: Vec<Diagnostic>,
    /// The slides that already carry an overflow error, which the stray item check leaves alone.
    overflowing: Vec<SlideIndex>,
}

impl<'a> Reporter<'a> {
    fn slide_of(&self, index: SlideIndex) -> Option<&'a Slide> {
        self.deck
            .slides
            .get(usize::try_from(index.0).unwrap_or(usize::MAX))
    }

    fn reference(&self, index: SlideIndex) -> Option<SlideRef> {
        let slide = self.slide_of(index)?;
        let master = self
            .bound
            .slides
            .iter()
            .find(|bound| bound.index == index)
            .and_then(|bound| self.theme.masters.get(bound.master))
            .map(|master| master.name.clone());
        Some(SlideRef {
            number: slide.number,
            id: slide.id.clone(),
            master,
        })
    }

    fn site(&self, measure: &'a RegionMeasure) -> Option<Site<'a>> {
        let slide = self.slide_of(measure.slide)?;
        let bound = self
            .bound
            .slides
            .iter()
            .find(|bound| bound.index == measure.slide)?;
        let master = self.theme.masters.get(bound.master)?;
        let region = bound
            .regions
            .get(usize::try_from(measure.region.0).unwrap_or(usize::MAX))?;
        let slot = master.slots.get(region.slot)?;
        let bounds = self
            .measured
            .expected
            .regions
            .iter()
            .find(|expected| expected.slide == measure.slide && expected.region == measure.region)?
            .bounds;
        Some(Site {
            slide,
            master,
            slot,
            region,
            bounds,
            measure,
        })
    }

    fn page_count(&mut self) {
        let expected = self.measured.expected;
        let measurement = self.measured.measurement;
        match pages(expected.slides, measurement.pages.0) {
            PageVerdict::Matches => {}
            PageVerdict::Extra(extra) => {
                let spilled = measurement
                    .slide_pages
                    .iter()
                    .find(|(slide, page)| page.0 != slide.0.saturating_add(1))
                    .map_or(SlideIndex(0), |(slide, _)| *slide);
                let (span, reference) = self
                    .slide_of(spilled)
                    .map_or((Span::line(1, 1), None), |slide| {
                        (slide.span, self.reference(spilled))
                    });
                let mut diagnostic = Diagnostic::new(
                    Code::MultiplePages,
                    format!(
                        "the deck has {} slides but the document has {} pages",
                        expected.slides, measurement.pages.0
                    ),
                )
                .at(span, format!("this slide spilled onto {extra} more pages"))
                .help(String::from(
                    "shorten the slide; a slide never continues on a second page",
                ));
                if let Some(reference) = reference {
                    diagnostic = diagnostic.on_slide(reference);
                }
                self.diagnostics.push(diagnostic);
            }
            PageVerdict::Missing(missing) => self.diagnostics.push(Diagnostic::new(
                Code::Internal,
                format!("the document lacks {missing} of the pages its slides need"),
            )),
        }
    }

    /// The masters whose slot of the same binding would hold `site`'s content.
    fn roomier(&self, site: &Site<'_>, required: Length) -> Vec<String> {
        let kinds = site.kinds();
        let mut found = Vec::new();
        for master in &self.theme.masters {
            if master.name == site.master.name {
                continue;
            }
            for slot in &master.slots {
                let same_binding = match (slot.binds, site.slot.binds) {
                    (Binding::Flow, Binding::Flow)
                    | (Binding::Title, Binding::Title)
                    | (Binding::Subtitle, Binding::Subtitle) => true,
                    (Binding::Section(left), Binding::Section(right)) => left == right,
                    (
                        Binding::Flow | Binding::Section(_) | Binding::Title | Binding::Subtitle,
                        Binding::Flow | Binding::Section(_) | Binding::Title | Binding::Subtitle,
                    ) => false,
                };
                if same_binding
                    && kinds.iter().all(|kind| slot.accepts.contains(*kind))
                    && slot.region.size.height >= required
                {
                    found.push(format!(
                        "master `{}` has the slot `{}` of {} that holds it",
                        master.name, slot.name, slot.region.size.height
                    ));
                }
            }
        }
        found
    }

    fn vertical_overflow(&mut self, site: &Site<'_>, by: Length) {
        let required = site.measure.required;
        let bounds = site.bounds;
        let mut diagnostic = Diagnostic::new(
            Code::VerticalOverflow,
            format!(
                "content bound to slot `{}` is {by} taller than its region",
                site.slot.name
            ),
        )
        .at(
            site.first_span(),
            format!("content bound to `{}` starts here", site.slot.name),
        );
        if let Some(last) = site.last_span() {
            diagnostic = diagnostic.also_at(last, String::from("and ends here"));
        }
        diagnostic = diagnostic.note(format!(
            "region {} × {} at ({}, {}); content needs {required}",
            bounds.size.width, bounds.size.height, bounds.origin.x, bounds.origin.y
        ));
        if let Some(pitch) = site.pitch(self.theme)
            && let Some((rendered, fit)) = site.lines(pitch)
            && let Some(style) = self.theme.text_style(&site.slot.text)
        {
            diagnostic = diagnostic.note(format!(
                "{rendered} rendered lines at {} × {}.{:03} = {pitch} pitch; {fit} lines fit",
                style.size,
                style.line_height.0.checked_div(THOUSAND).unwrap_or(0),
                style.line_height.0.checked_rem(THOUSAND).unwrap_or(0)
            ));
            let remove = rendered.saturating_sub(fit);
            if remove > 0 {
                diagnostic = diagnostic.help(format!("remove {remove} lines"));
            }
        }
        let blocks = site.region.blocks.len();
        match split_point(
            site.measure.prefixes.as_slice(),
            bounds.size.height,
            EPSILON,
        ) {
            SplitVerdict::AllFit => {}
            SplitVerdict::SplitAfter(last) => {
                let moved = blocks.saturating_sub(last.saturating_add(1));
                diagnostic = diagnostic.help(format!(
                    "move the last {moved} of the {blocks} blocks to another slide"
                ));
            }
            SplitVerdict::NothingFits => {
                diagnostic = diagnostic.help(String::from(
                    "shorten the first block, which alone overflows the region",
                ));
            }
        }
        let roomier = self.roomier(site, required);
        if roomier.is_empty() {
            diagnostic = diagnostic.help(format!(
                "no master in theme `{}` has a slot of {required} or more for this content; split the slide",
                self.theme.name
            ));
        }
        for line in roomier {
            diagnostic = diagnostic.help(line);
        }
        self.diagnostics.push(
            diagnostic
                .note(format!(
                    "sivi never shrinks or splits content; see {DIAGNOSTICS_DOC}#e0401"
                ))
                .on_slide(site.reference()),
        );
        self.overflowing.push(site.measure.slide);
    }

    fn code_width(&mut self, site: &Site<'_>, part: usize, by: Length) {
        let Some(block) = site.block(part) else {
            return;
        };
        let mut diagnostic = Diagnostic::new(
            Code::HorizontalOverflow,
            format!(
                "a code line in slot `{}` is {by} wider than its region",
                site.slot.name
            ),
        )
        .at(block.span, String::from("this code block holds the line"));
        if let BlockBody::Code(code) = &block.body
            && let Some((line, text)) = code
                .text
                .lines()
                .enumerate()
                .max_by_key(|(_, text)| text.chars().count())
            && let Some((_, natural)) = site
                .measure
                .natural_widths
                .iter()
                .find(|(index, _)| usize::try_from(*index) == Ok(part))
        {
            let characters = text.chars().count();
            let fit = i64::try_from(characters)
                .unwrap_or(i64::MAX)
                .saturating_mul(site.bounds.size.width.0)
                .checked_div(natural.0)
                .unwrap_or(0);
            diagnostic = diagnostic.note(format!(
                "the longest line is line {} of the block, {characters} characters wide; about {fit} characters fit",
                line.saturating_add(1)
            ));
        }
        self.diagnostics.push(
            diagnostic
                .help(String::from(
                    "break the line, or use a master with a wider slot",
                ))
                .note(format!(
                    "sivi never wraps or shrinks code; see {DIAGNOSTICS_DOC}#e0402"
                ))
                .on_slide(site.reference()),
        );
        self.overflowing.push(site.measure.slide);
    }

    fn too_many_lines(&mut self, site: &Site<'_>) {
        let Some(most) = site.slot.capacity.lines else {
            return;
        };
        let Some(pitch) = site.pitch(self.theme) else {
            return;
        };
        let Some((rendered, _)) = site.lines(pitch) else {
            return;
        };
        if rendered <= i64::from(most) {
            return;
        }
        self.diagnostics.push(
            Diagnostic::new(
                Code::TooManyLines,
                format!(
                    "content bound to slot `{}` renders {rendered} lines, more than its `max_lines` of {most}",
                    site.slot.name
                ),
            )
            .at(site.first_span(), String::from("this content"))
            .help(String::from("shorten the content, or split the slide"))
            .on_slide(site.reference()),
        );
    }

    fn region(&mut self, measure: &'a RegionMeasure) {
        let Some(site) = self.site(measure) else {
            self.diagnostics.push(Diagnostic::new(
                Code::Internal,
                format!(
                    "the adapter measured region {} of slide {}, which the binding lacks",
                    measure.region.0, measure.slide.0
                ),
            ));
            return;
        };
        match fit(measure.required, site.bounds.size.height, EPSILON) {
            Fit::Overflows { by } => self.vertical_overflow(&site, by),
            Fit::Fits => {
                let images_only = (0..site.region.blocks.len()).all(|part| {
                    site.block(part)
                        .is_some_and(|block| matches!(block.body, BlockBody::Image(_)))
                });
                if !images_only
                    && percent(measure.required, site.bounds.size.height) >= NEARLY_FULL_PERCENT
                {
                    self.diagnostics.push(
                        Diagnostic::new(
                            Code::NearlyFull,
                            format!(
                                "slot `{}` is {}% full",
                                site.slot.name,
                                percent(measure.required, site.bounds.size.height)
                            ),
                        )
                        .at(
                            site.first_span(),
                            String::from("this content nearly fills its region"),
                        )
                        .note(format!(
                            "content needs {} of {}",
                            measure.required, site.bounds.size.height
                        ))
                        .on_slide(site.reference()),
                    );
                }
                self.too_many_lines(&site);
            }
        }
        for (part, width) in &measure.natural_widths {
            if let Fit::Overflows { by } = fit(*width, site.bounds.size.width, EPSILON) {
                self.code_width(&site, usize::try_from(*part).unwrap_or(usize::MAX), by);
            }
        }
    }

    /// Checks every laid-out item against the region that placed it and against the page.
    fn leaves(&mut self) {
        let page_size = self.theme.page.size;
        let page_box = Rect::new(
            Point {
                x: Length::ZERO,
                y: Length::ZERO,
            },
            page_size,
        );
        let measurement = self.measured.measurement;
        let mut strayed: Vec<SlideIndex> = Vec::new();
        for (index, page) in measurement.slide_pages.iter().copied() {
            if self.overflowing.contains(&index) {
                continue;
            }
            let regions: Vec<&RegionMeasure> = measurement
                .regions
                .iter()
                .filter(|region| region.slide == index)
                .collect();
            let sites: Vec<Site<'_>> = regions
                .iter()
                .filter_map(|region| self.site(region))
                .collect();
            let boxes: Vec<Rect> = sites.iter().map(|site| site.bounds).collect();
            for leaf in measurement.leaves.iter().filter(|leaf| leaf.page == page) {
                self.leaf(leaf, &sites, &boxes, &page_box, &mut strayed);
            }
        }
    }

    fn leaf(
        &mut self,
        leaf: &Leaf,
        sites: &[Site<'_>],
        boxes_and_page: &[Rect],
        page_box: &Rect,
        strayed: &mut Vec<SlideIndex>,
    ) {
        let beyond_page = leaf.bounds.right().is_none_or(|right| {
            matches!(
                fit(right, page_box.size.width, EPSILON),
                Fit::Overflows { .. }
            )
        }) || leaf.bounds.bottom().is_none_or(|bottom| {
            matches!(
                fit(bottom, page_box.size.height, EPSILON),
                Fit::Overflows { .. }
            )
        }) || leaf.bounds.origin.x < Length::ZERO
            || leaf.bounds.origin.y < Length::ZERO;
        let Some(owner) = region_of(boxes_and_page, leaf.bounds.origin) else {
            if beyond_page {
                self.stray(leaf, sites, strayed);
            }
            return;
        };
        let Some(site) = sites.get(owner) else {
            return;
        };
        let top = site
            .bounds
            .origin
            .y
            .checked_add(site.shift())
            .unwrap_or(site.bounds.origin.y);
        let bottom = leaf.bounds.bottom();
        let right = leaf.bounds.right();
        let region_bottom = site.bounds.bottom();
        let region_right = site.bounds.right();
        if let (Some(bottom), Some(region_bottom)) = (bottom, region_bottom)
            && let Fit::Overflows { by } = fit(bottom, region_bottom, EPSILON)
        {
            self.disagreement(site, by);
            return;
        }
        if let (Some(right), Some(region_right)) = (right, region_right)
            && let Fit::Overflows { by } = fit(right, region_right, EPSILON)
        {
            let offset = leaf.bounds.origin.y.saturating_sub(top);
            let part = part_at(site.measure.prefixes.as_slice(), offset).unwrap_or(0);
            self.item_width(site, part, by);
        }
    }

    fn stray(&mut self, leaf: &Leaf, sites: &[Site<'_>], strayed: &mut Vec<SlideIndex>) {
        let Some(site) = sites.first() else {
            return;
        };
        if strayed.contains(&site.measure.slide) {
            return;
        }
        strayed.push(site.measure.slide);
        self.diagnostics.push(
            Diagnostic::new(
                Code::OutsidePage,
                format!(
                    "an item of slide {} lies beyond the page, at ({}, {})",
                    site.slide.number, leaf.bounds.origin.x, leaf.bounds.origin.y
                ),
            )
            .at(site.slide.span, String::from("this slide"))
            .note(String::from(
                "the item belongs to no bound region, so a decor of the theme placed it",
            ))
            .help(String::from("move the decor inside the page in the theme"))
            .on_slide(site.reference()),
        );
    }

    fn disagreement(&mut self, site: &Site<'_>, by: Length) {
        if self.overflowing.contains(&site.measure.slide) {
            return;
        }
        self.overflowing.push(site.measure.slide);
        self.diagnostics.push(
            Diagnostic::new(
                Code::OracleDisagreement,
                format!(
                    "the measure says slot `{}` fits, but a laid-out item ends {by} below the region",
                    site.slot.name
                ),
            )
            .at(site.first_span(), String::from("the content measured here"))
            .note(format!(
                "content needs {} by the measure; the region is {} tall",
                site.measure.required, site.bounds.size.height
            ))
            .help(String::from(
                "this is a defect of sivi or of the pinned Typst; report it with the deck",
            ))
            .on_slide(site.reference()),
        );
    }

    fn item_width(&mut self, site: &Site<'_>, part: usize, by: Length) {
        if self.overflowing.contains(&site.measure.slide) {
            return;
        }
        self.overflowing.push(site.measure.slide);
        let span = site
            .block(part)
            .map_or_else(|| site.first_span(), |block| block.span);
        self.diagnostics.push(
            Diagnostic::new(
                Code::HorizontalOverflow,
                format!(
                    "an item in slot `{}` is {by} wider than its region",
                    site.slot.name
                ),
            )
            .at(span, String::from("this block holds the item"))
            .help(String::from(
                "break the long word, link, or image, or use a master with a wider slot",
            ))
            .on_slide(site.reference()),
        );
    }

    fn missing_glyphs(&mut self) {
        let measurement = self.measured.measurement;
        let mut seen: Vec<char> = Vec::new();
        for glyph in &measurement.missing_glyphs {
            if seen.contains(&glyph.character) {
                continue;
            }
            seen.push(glyph.character);
            let located = measurement
                .slide_pages
                .iter()
                .find(|(_, page)| *page == glyph.page)
                .and_then(|(index, _)| {
                    let shown = self.slide_of(*index)?;
                    let span = shown
                        .blocks
                        .iter()
                        .find(|block| text_of(block).contains(glyph.character))
                        .map_or(shown.span, |block| block.span);
                    Some((span, self.reference(*index)))
                });
            let (span, reference) = located.unwrap_or((Span::line(1, 1), None));
            let fonts: Vec<String> = self
                .theme
                .text
                .iter()
                .map(|(_, style)| format!("`{}`", style.font))
                .chain(
                    self.theme
                        .typography
                        .fallback
                        .iter()
                        .map(|family| format!("`{family}`")),
                )
                .collect::<Vec<_>>();
            let mut fonts_listed: Vec<String> = Vec::new();
            for font in fonts {
                if !fonts_listed.contains(&font) {
                    fonts_listed.push(font);
                }
            }
            let mut diagnostic = Diagnostic::new(
                Code::MissingGlyph,
                format!(
                    "no font of the theme has a glyph for `{}` (U+{:04X})",
                    glyph.character,
                    u32::from(glyph.character)
                ),
            )
            .at(span, String::from("this text shows the character"))
            .note(format!("the fonts tried: {}", fonts_listed.join(", ")))
            .help(String::from(
                "replace the character, or add a font that has it under `[[fonts]]`",
            ));
            if let Some(reference) = reference {
                diagnostic = diagnostic.on_slide(reference);
            }
            self.diagnostics.push(diagnostic);
        }
    }
}

/// The plain text of a block, in which a missing glyph is sought.
fn text_of(block: &Block) -> String {
    match &block.body {
        BlockBody::Heading(heading) => String::from(ir::plain_text(&heading.inlines).as_str()),
        BlockBody::Paragraph(paragraph) | BlockBody::Quote(paragraph) => {
            String::from(ir::plain_text(&paragraph.inlines).as_str())
        }
        BlockBody::List(list) => list_text(list),
        BlockBody::Code(code) => code.text.clone(),
        BlockBody::Image(image) => String::from(image.alt.as_str()),
        BlockBody::Table(table) => {
            let mut text = String::new();
            for row in core::iter::once(&table.header).chain(table.rows.iter()) {
                for cell in &row.cells {
                    text.push_str(ir::plain_text(cell).as_str());
                }
            }
            text
        }
    }
}

fn list_text(list: &ir::List) -> String {
    let mut text = String::new();
    for item in &list.items {
        text.push_str(ir::plain_text(&item.inlines).as_str());
        if let Some(nested) = &item.nested {
            text.push_str(&list_text(nested));
        }
    }
    text
}

/// The diagnostics a measured deck earns: nothing when every region fits on its page.
#[must_use]
pub fn diagnose(
    theme: &Theme,
    deck: &Deck,
    bound: &BoundDeck,
    measured: Measured<'_>,
) -> Vec<Diagnostic> {
    let mut reporter = Reporter {
        theme,
        deck,
        bound,
        measured,
        diagnostics: Vec::new(),
        overflowing: Vec::new(),
    };
    reporter.page_count();
    for measure in &measured.measurement.regions {
        reporter.region(measure);
    }
    reporter.leaves();
    reporter.missing_glyphs();
    reporter.diagnostics
}

#[cfg(test)]
mod tests {
    use super::{DIAGNOSTICS_DOC, Measured, diagnose};
    use crate::bind::bind;
    use crate::diagnostic::Code;
    use crate::emit::{LineBoxes, typst_source};
    use crate::ir::{
        Block, BlockBody, BlockKind, CodeBlock, Deck, Head, Heading, HeadingLevel, Inline,
        Paragraph, Slide,
    };
    use crate::length::{Length, Point, Rect, Size};
    use crate::measure::{
        ItemKind, Leaf, Measurement, MissingGlyph, PageCount, PageNumber, Prefixes, RegionMeasure,
    };
    use crate::protocol::{RegionIndex, SlideIndex};
    use crate::span::Span;
    use crate::text::Identifier;
    use crate::theme::fixture::{master, slot, tiny};
    use crate::theme::{Binding, BlockKindSet, Capacity, Slot, Theme, VerticalAlign};
    use alloc::string::String;
    use alloc::vec;
    use alloc::vec::Vec;

    const PAGE_WIDTH: i64 = 960_000;
    const PAGE_HEIGHT: i64 = 540_000;
    const REGION_HEIGHT: i64 = 100_000;
    const REGION_WIDTH: i64 = 400_000;
    const SHORT_HEIGHT: i64 = 50_000;
    const MAX_LINES: u32 = 2;

    const fn mp(millipoints: i64) -> Length {
        Length::from_millipoints(millipoints)
    }

    fn theme() -> Theme {
        let mut theme = tiny();
        theme.page.size = Size {
            width: mp(PAGE_WIDTH),
            height: mp(PAGE_HEIGHT),
        };
        let region = Rect::new(
            Point { x: mp(0), y: mp(0) },
            Size {
                width: mp(REGION_WIDTH),
                height: mp(REGION_HEIGHT),
            },
        );
        let mut page = master("page", &[]);
        page.slots = vec![Slot {
            accepts: BlockKindSet::EMPTY
                .with(BlockKind::Paragraph)
                .with(BlockKind::Code),
            region,
            valign: VerticalAlign::Middle,
            capacity: Capacity {
                lines: Some(MAX_LINES),
                ..Capacity::default()
            },
            ..slot("body")
        }];
        theme.masters.push(page);
        let mut wide = master("wide", &[]);
        wide.slots = vec![Slot {
            accepts: BlockKindSet::EMPTY
                .with(BlockKind::Paragraph)
                .with(BlockKind::Code),
            region: Rect::new(
                region.origin,
                Size {
                    width: mp(REGION_WIDTH),
                    height: mp(PAGE_HEIGHT),
                },
            ),
            ..slot("body")
        }];
        theme.masters.push(wide);
        let flow = |name: &str, accepts: BlockKindSet, height: i64| {
            let mut found = master(name, &[]);
            found.slots = vec![Slot {
                accepts,
                region: Rect::new(
                    region.origin,
                    Size {
                        width: mp(REGION_WIDTH),
                        height: mp(height),
                    },
                ),
                ..slot("body")
            }];
            found
        };
        let text = BlockKindSet::EMPTY
            .with(BlockKind::Paragraph)
            .with(BlockKind::Code);
        theme.masters.push(flow(
            "tall-images",
            BlockKindSet::EMPTY.with(BlockKind::Image),
            PAGE_HEIGHT,
        ));
        theme.masters.push(flow("short", text, SHORT_HEIGHT));
        let section = |name: &str, number: u32, height: i64| {
            let mut found = flow(name, text, height);
            if let Some(only) = found.slots.first_mut() {
                only.binds = Binding::Section(number);
            }
            found
        };
        theme.masters.push(section("sectioned", 1, PAGE_HEIGHT));
        theme.masters.push(section("pair", 1, REGION_HEIGHT));
        theme.masters.push(section("twin", 2, PAGE_HEIGHT));
        theme
    }

    fn slide_of(number: usize, master: &str, blocks: Vec<Block>) -> Slide {
        Slide {
            number,
            span: Span::line(1, 1),
            master: Some(Identifier::new(master).unwrap()),
            id: None,
            blocks,
            notes: None,
        }
    }

    fn deck_of(slides: Vec<Slide>) -> Deck {
        Deck {
            head: Head::default(),
            slides,
        }
    }

    fn deck(blocks: Vec<Block>) -> Deck {
        deck_of(vec![slide_of(1, "page", blocks)])
    }

    fn section_heading(line: usize) -> Block {
        Block {
            body: BlockBody::Heading(Heading {
                level: HeadingLevel::Section,
                inlines: vec![Inline::Text(String::from("h"))],
            }),
            span: Span::line(line, 1),
        }
    }

    fn paragraph(line: usize, text: &str) -> Block {
        Block {
            body: BlockBody::Paragraph(Paragraph {
                inlines: vec![Inline::Text(String::from(text))],
            }),
            span: Span::line(line, 1),
        }
    }

    fn code(line: usize, text: &str) -> Block {
        Block {
            body: BlockBody::Code(CodeBlock {
                language: None,
                text: String::from(text),
            }),
            span: Span::line(line, 1),
        }
    }

    fn region(required: i64, prefixes: &[i64], widths: &[(u32, i64)]) -> RegionMeasure {
        RegionMeasure {
            slide: SlideIndex(0),
            region: RegionIndex(0),
            required: mp(required),
            prefixes: Prefixes::new(prefixes.iter().map(|height| mp(*height)).collect()).unwrap(),
            natural_widths: widths
                .iter()
                .map(|(part, width)| (*part, mp(*width)))
                .collect(),
        }
    }

    fn leaf(origin: (i64, i64), size: (i64, i64)) -> Leaf {
        Leaf {
            page: PageNumber(1),
            kind: ItemKind::Text,
            bounds: Rect::new(
                Point {
                    x: mp(origin.0),
                    y: mp(origin.1),
                },
                Size {
                    width: mp(size.0),
                    height: mp(size.1),
                },
            ),
        }
    }

    fn measurement(regions: Vec<RegionMeasure>, leaves: Vec<Leaf>) -> Measurement {
        Measurement {
            regions,
            pages: PageCount(1),
            slide_pages: vec![(SlideIndex(0), PageNumber(1))],
            leaves,
            missing_glyphs: Vec::new(),
        }
    }

    fn report(deck: &Deck, measurement: &Measurement) -> Vec<crate::diagnostic::Diagnostic> {
        let theme = theme();
        let bound = bind(deck, &theme).unwrap();
        let emitted = typst_source(&theme, deck, &bound, &LineBoxes::default()).unwrap();
        diagnose(
            &theme,
            deck,
            &bound,
            Measured {
                expected: &emitted.expected,
                measurement,
            },
        )
    }

    #[test]
    fn content_that_fits_earns_nothing_and_a_nearly_full_region_a_warning() {
        let deck = deck(vec![paragraph(1, "a"), paragraph(3, "b")]);
        let fits = measurement(
            vec![region(50_000, &[20_000, 50_000], &[])],
            vec![leaf((0, 25_000), (100_000, 20_000))],
        );
        assert_eq!(report(&deck, &fits), Vec::new());
        let nearly = measurement(vec![region(96_000, &[50_000, 96_000], &[])], Vec::new());
        let found = report(&deck, &nearly);
        assert_eq!(found[0].code, Code::NearlyFull);
        assert!(found[0].message.contains("96%"), "{}", found[0].message);
    }

    #[test]
    fn a_taller_content_names_the_excess_the_lines_and_the_split() {
        let deck = deck(vec![
            paragraph(1, "a"),
            paragraph(3, "b"),
            paragraph(5, "c"),
        ]);
        let tall = measurement(
            vec![region(182_800, &[60_000, 120_000, 182_800], &[])],
            Vec::new(),
        );
        let found = report(&deck, &tall);
        assert_eq!(found.len(), 1, "{found:?}");
        let overflow = &found[0];
        assert_eq!(overflow.code, Code::VerticalOverflow);
        assert_eq!(
            overflow.message,
            "content bound to slot `body` is 82.800pt taller than its region"
        );
        assert_eq!(overflow.primary.as_ref().unwrap().span, Span::line(1, 1));
        assert_eq!(overflow.secondary[0].span, Span::line(5, 1));
        assert!(
            overflow.notes[0].contains("content needs 182.800pt"),
            "{:?}",
            overflow.notes
        );
        assert!(
            overflow.notes[1]
                .contains("rendered lines at 24.000pt × 1.000 = 24.000pt pitch; 4 lines fit"),
            "{:?}",
            overflow.notes
        );
        assert!(
            overflow.helps.contains(&String::from("remove 4 lines")),
            "{:?}",
            overflow.helps
        );
        assert!(
            overflow.helps.contains(&String::from(
                "move the last 2 of the 3 blocks to another slide"
            )),
            "{:?}",
            overflow.helps
        );
        assert!(
            overflow
                .helps
                .iter()
                .any(|help| help.contains("master `wide` has the slot `body` of 540.000pt")),
            "{:?}",
            overflow.helps
        );
        assert!(overflow.notes[2].contains(DIAGNOSTICS_DOC));
        assert_eq!(
            overflow
                .slide
                .as_ref()
                .unwrap()
                .master
                .as_ref()
                .unwrap()
                .as_str(),
            "page"
        );
    }

    #[test]
    fn a_wide_code_line_names_the_line_and_the_columns_that_fit() {
        let deck = deck(vec![code(1, "short\nthe longest line here")]);
        let wide = measurement(vec![region(50_000, &[50_000], &[(0, 800_000)])], Vec::new());
        let found = report(&deck, &wide);
        assert_eq!(found[0].code, Code::HorizontalOverflow);
        assert!(
            found[0].message.contains("400.000pt wider"),
            "{}",
            found[0].message
        );
        assert!(
            found[0].notes[0]
                .contains("line 2 of the block, 21 characters wide; about 10 characters fit"),
            "{:?}",
            found[0].notes
        );
    }

    #[test]
    fn the_frame_walk_catches_what_the_measure_missed() {
        let deck = deck(vec![paragraph(1, "a")]);
        let low = measurement(
            vec![region(40_000, &[40_000], &[])],
            vec![leaf((0, 30_000), (100_000, 80_000))],
        );
        let found = report(&deck, &low);
        assert_eq!(found[0].code, Code::OracleDisagreement);
        let wide = measurement(
            vec![region(40_000, &[40_000], &[])],
            vec![leaf((0, 30_000), (500_000, 10_000))],
        );
        assert_eq!(report(&deck, &wide)[0].code, Code::HorizontalOverflow);
        let stray = measurement(
            vec![region(40_000, &[40_000], &[])],
            vec![leaf((900_000, 530_000), (100_000, 20_000))],
        );
        assert_eq!(report(&deck, &stray)[0].code, Code::OutsidePage);
    }

    #[test]
    fn extra_pages_missing_glyphs_and_too_many_lines_are_reported() {
        let deck = deck(vec![paragraph(1, "a"), paragraph(3, "b🦀")]);
        let mut spilled = measurement(vec![region(50_000, &[20_000, 50_000], &[])], Vec::new());
        spilled.pages = PageCount(2);
        let spilled_found = report(&deck, &spilled);
        assert_eq!(spilled_found[0].code, Code::MultiplePages);
        assert_eq!(
            spilled_found[0]
                .slide
                .as_ref()
                .and_then(|slide| slide.master.as_ref())
                .map(Identifier::as_str),
            Some("page")
        );
        let mut glyph = measurement(vec![region(50_000, &[20_000, 50_000], &[])], Vec::new());
        glyph.missing_glyphs = vec![
            MissingGlyph {
                page: PageNumber(1),
                character: '🦀',
            },
            MissingGlyph {
                page: PageNumber(1),
                character: '🦀',
            },
        ];
        let glyph_found = report(&deck, &glyph);
        assert_eq!(glyph_found.len(), 1);
        assert_eq!(glyph_found[0].code, Code::MissingGlyph);
        assert!(glyph_found[0].message.contains("U+1F980"));
        assert_eq!(
            glyph_found[0].primary.as_ref().unwrap().span,
            Span::line(3, 1)
        );
        assert_eq!(
            glyph_found[0].notes,
            vec!["the fonts tried: `BIZ UDPGothic`"]
        );
        let lines = measurement(vec![region(80_000, &[40_000, 80_000], &[])], Vec::new());
        let lines_found = report(&deck, &lines);
        assert_eq!(lines_found[0].code, Code::TooManyLines);
        assert!(
            lines_found[0].message.contains("renders 3 lines"),
            "{}",
            lines_found[0].message
        );
    }

    #[test]
    fn a_single_block_overflow_has_no_end_label_and_a_small_excess_no_line_help() {
        let deck = deck(vec![paragraph(1, "a")]);
        let barely = measurement(vec![region(101_000, &[101_000], &[])], Vec::new());
        let found = report(&deck, &barely);
        assert_eq!(found[0].code, Code::VerticalOverflow);
        assert_eq!(found[0].secondary, Vec::new());
        assert!(
            !found[0]
                .helps
                .iter()
                .any(|help| help.starts_with("remove ")),
            "{:?}",
            found[0].helps
        );
    }

    fn masters_suggested(helps: &[String]) -> Vec<String> {
        helps
            .iter()
            .filter_map(|help| {
                help.strip_prefix("master `")
                    .and_then(|rest| rest.split_once('`'))
                    .map(|(name, _)| String::from(name))
            })
            .collect()
    }

    #[test]
    fn suggestions_name_only_masters_whose_slot_binds_accepts_and_holds_the_content() {
        let deck = deck(vec![paragraph(1, "a"), paragraph(3, "b")]);
        let tall = measurement(vec![region(400_000, &[200_000, 400_000], &[])], Vec::new());
        let found = report(&deck, &tall);
        assert_eq!(found[0].code, Code::VerticalOverflow);
        assert_eq!(masters_suggested(&found[0].helps), vec!["wide"]);
        let sectioned = deck_of(vec![slide_of(
            1,
            "pair",
            vec![section_heading(1), paragraph(3, "a")],
        )]);
        let sectioned_found = report(&sectioned, &tall);
        assert_eq!(sectioned_found[0].code, Code::VerticalOverflow);
        assert_eq!(
            masters_suggested(&sectioned_found[0].helps),
            vec!["sectioned"]
        );
    }

    #[test]
    fn regions_and_pages_are_matched_slide_by_slide() {
        let deck = deck_of(vec![
            slide_of(1, "page", vec![paragraph(1, "a")]),
            slide_of(2, "wide", vec![paragraph(1, "b")]),
        ]);
        let mut second = region(400_000, &[400_000], &[]);
        second.slide = SlideIndex(1);
        let mut fits = measurement(vec![region(50_000, &[50_000], &[]), second], Vec::new());
        fits.pages = PageCount(2);
        fits.slide_pages = vec![
            (SlideIndex(0), PageNumber(1)),
            (SlideIndex(1), PageNumber(2)),
        ];
        assert_eq!(report(&deck, &fits), Vec::new());
        let mut spilled = fits;
        spilled.pages = PageCount(3);
        spilled.slide_pages = vec![
            (SlideIndex(0), PageNumber(1)),
            (SlideIndex(1), PageNumber(3)),
        ];
        let found = report(&deck, &spilled);
        assert_eq!(found[0].code, Code::MultiplePages);
        assert_eq!(found[0].slide.as_ref().map(|slide| slide.number), Some(2));
    }

    #[test]
    fn stray_items_are_reported_only_beyond_the_page() {
        let deck = deck(vec![paragraph(1, "a")]);
        let fits = region(40_000, &[40_000], &[]);
        let beyond_right = measurement(
            vec![fits.clone()],
            vec![leaf((900_000, 300_000), (100_000, 20_000))],
        );
        assert_eq!(report(&deck, &beyond_right)[0].code, Code::OutsidePage);
        let left_of_page = measurement(
            vec![fits.clone()],
            vec![leaf((-1, 300_000), (10_000, 10_000))],
        );
        assert_eq!(report(&deck, &left_of_page)[0].code, Code::OutsidePage);
        let above_page = measurement(
            vec![fits.clone()],
            vec![leaf((500_000, -1), (10_000, 10_000))],
        );
        assert_eq!(report(&deck, &above_page)[0].code, Code::OutsidePage);
        let inside = measurement(
            vec![fits.clone()],
            vec![leaf((0, 300_000), (10_000, 10_000))],
        );
        assert_eq!(report(&deck, &inside), Vec::new());
        let at_top = measurement(vec![fits], vec![leaf((500_000, 0), (10_000, 10_000))]);
        assert_eq!(report(&deck, &at_top), Vec::new());
    }
}
