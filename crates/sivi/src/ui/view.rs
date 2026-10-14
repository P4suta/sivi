//! The frames of the interface: a header, the slides beside a preview, the diagnostics beneath, the keys at the foot, and a picker or the help as a window over it all.

use super::app::{App, Rendered};
use super::preview::{Mosaic, Protocol};
use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, BorderType, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap,
};
use sivi_core::diagnostic::Severity;
use sivi_core::ui_rules::{Job, Picker, Screen, Verdict};

/// The palette of the interface: mint for what passed, gold for a warning, coral for an error, slate for the rest,
/// ink for the words, violet for the marks, a deep navy behind it all, and a lighter navy under the selection.
const MINT: Color = Color::Rgb(94, 242, 194);
const GOLD: Color = Color::Rgb(255, 209, 102);
const CORAL: Color = Color::Rgb(255, 99, 99);
const SLATE: Color = Color::Rgb(154, 160, 180);
const INK: Color = Color::Rgb(243, 244, 248);
const VIOLET: Color = Color::Rgb(124, 92, 255);
const BACKGROUND: Color = Color::Rgb(15, 18, 34);
const HIGHLIGHT: Color = Color::Rgb(42, 47, 69);
/// The phases of the spinner, one glyph each.
const SPINNER_PHASES: usize = 4;
const SPINNER_GLYPHS: [&str; SPINNER_PHASES] = ["◐", "◓", "◑", "◒"];
/// The whole of a frame, in percent.
const WHOLE_PERCENT: u16 = 100;
/// How wide the slide list is, in percent of the frame.
const LIST_PERCENT: u16 = 34;
/// How wide the list of a picker is, in percent of its window, with the detail beside it.
const PICKER_LIST_PERCENT: u16 = 55;
/// How tall the diagnostics pane is, in rows, under the overview.
const DIAGNOSTICS_ROWS: u16 = 8;
/// The rows the header and the foot take.
const BAR_ROWS: u16 = 1;
/// How much of the frame a window over it takes, in percent, with the rest split between its two sides.
const WINDOW_PERCENT: u16 = 80;
const SIDES: u16 = 2;
/// How wide a swatch is, in cells.
const SWATCH: &str = "  ";
/// The columns a slide number, a master name, an entry name, and a key take in the lists.
const NUMBER_WIDTH: usize = 3;
const MASTER_WIDTH: usize = 12;
const NAME_WIDTH: usize = 24;
const KEY_WIDTH: usize = 12;

/// The areas of one frame.
struct Areas {
    header: Rect,
    body: Rect,
    foot: Rect,
}

fn areas(frame: Rect) -> Areas {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(BAR_ROWS),
            Constraint::Min(1),
            Constraint::Length(BAR_ROWS),
        ])
        .split(frame);
    match *rows {
        [header, body, foot] => Areas { header, body, foot },
        _ => Areas {
            header: Rect::default(),
            body: frame,
            foot: Rect::default(),
        },
    }
}

/// The list, the preview, and the diagnostics of the overview and the slides screens.
struct Panes {
    list: Rect,
    preview: Rect,
    diagnostics: Rect,
}

/// `area` split in two along `direction` at `percent` of it.
fn halves(area: Rect, direction: Direction, percent: u16) -> (Rect, Rect) {
    let parts = Layout::default()
        .direction(direction)
        .constraints([
            Constraint::Percentage(percent),
            Constraint::Percentage(WHOLE_PERCENT.saturating_sub(percent)),
        ])
        .split(area);
    match *parts {
        [first, second] => (first, second),
        _ => (area, Rect::default()),
    }
}

fn panes(body: Rect, screen: Screen) -> Panes {
    let (upper, diagnostics) = if screen == Screen::Slides {
        (body, Rect::default())
    } else {
        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(DIAGNOSTICS_ROWS)])
            .split(body);
        match *rows {
            [upper, lower] => (upper, lower),
            _ => (body, Rect::default()),
        }
    };
    let (list, preview) = halves(upper, Direction::Horizontal, LIST_PERCENT);
    Panes {
        list,
        preview,
        diagnostics,
    }
}

/// The inner area of a bordered block over `area`.
fn inner(area: Rect) -> Rect {
    Block::bordered().inner(area)
}

/// Where the preview's picture goes, when the screen shows one, for the image protocols.
#[must_use]
pub(super) fn preview_area(app: &App, frame: Rect) -> Option<Rect> {
    if app.protocol == Protocol::Mosaic || app.judged.is_none() {
        return None;
    }
    match app.screen() {
        Screen::Overview | Screen::Slides => {
            Some(inner(panes(areas(frame).body, app.screen()).preview))
        }
        Screen::Diagnostics | Screen::Picker(_) | Screen::Help => None,
    }
}

fn bordered(title: &str) -> Block<'_> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(SLATE))
        .title(Span::styled(
            format!(" {title} "),
            Style::default().fg(INK).add_modifier(Modifier::BOLD),
        ))
}

fn selected() -> Style {
    Style::default().bg(HIGHLIGHT).add_modifier(Modifier::BOLD)
}

fn header(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let verdict = match (app.machine.job, app.machine.last) {
        (Job::Building, _) => Span::styled(
            format!(
                "{} building",
                SPINNER_GLYPHS
                    .get(usize::from(app.machine.tick))
                    .copied()
                    .unwrap_or_default()
            ),
            Style::default().fg(GOLD),
        ),
        (Job::Idle, Some(Verdict::Passed)) => {
            let pages = app
                .outcome
                .as_ref()
                .and_then(|outcome| outcome.product.as_ref())
                .map_or(0, |product| product.pages);
            Span::styled(format!("● built, {pages} pages"), Style::default().fg(MINT))
        }
        (Job::Idle, Some(Verdict::Failed)) => Span::styled("● refused", Style::default().fg(CORAL)),
        (Job::Idle, None) => Span::styled("○ not built", Style::default().fg(SLATE)),
    };
    let theme = app
        .machine
        .theme
        .and_then(|index| app.theme_names.get(index))
        .map_or_else(
            || String::from("the deck's theme"),
            |name| format!("theme {name}"),
        );
    let watch = if app.machine.watch {
        Span::styled(" watch on ", Style::default().fg(Color::Black).bg(MINT))
    } else {
        Span::styled(" watch off ", Style::default().fg(SLATE))
    };
    let line = Line::from(vec![
        Span::styled(
            " sivi ",
            Style::default()
                .fg(Color::Black)
                .bg(VIOLET)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(app.deck.display().to_string(), Style::default().fg(INK)),
        Span::raw("  "),
        Span::styled(theme, Style::default().fg(SLATE)),
        Span::raw("  "),
        verdict,
        Span::raw("  "),
        watch,
    ]);
    frame.render_widget(Paragraph::new(line), area);
}

fn foot(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let keys = match app.screen() {
        Screen::Picker(_) => "j/k move  enter choose  esc back  q quit",
        Screen::Help => "esc back  q quit",
        Screen::Overview | Screen::Slides | Screen::Diagnostics => {
            "q quit  b build  w watch  j/k move  1 overview  2 slides  3 diagnostics  t theme  f fonts  p palette  c code  o open  ? help"
        }
    };
    let line = match &app.note {
        Some((text, _)) => Line::from(Span::styled(
            format!(" {text} "),
            Style::default().fg(Color::Black).bg(GOLD),
        )),
        None => Line::from(Span::styled(keys, Style::default().fg(SLATE))),
    };
    frame.render_widget(Paragraph::new(line), area);
}

fn slide_list(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let items: Vec<ListItem<'_>> = app
        .slides
        .iter()
        .map(|row| {
            let mark = if row.errors > 0 {
                Span::styled("✖ ", Style::default().fg(CORAL))
            } else if row.warnings > 0 {
                Span::styled("▲ ", Style::default().fg(GOLD))
            } else {
                Span::styled("● ", Style::default().fg(MINT))
            };
            ListItem::new(Line::from(vec![
                Span::styled(
                    format!("{:>NUMBER_WIDTH$} ", row.number),
                    Style::default().fg(SLATE),
                ),
                mark,
                Span::styled(
                    format!("{:<MASTER_WIDTH$}", row.master),
                    Style::default().fg(VIOLET),
                ),
                Span::styled(row.title.clone(), Style::default().fg(INK)),
            ]))
        })
        .collect();
    let mut state = ListState::default();
    if !app.slides.is_empty() {
        state.select(Some(app.machine.slide));
    }
    let list = List::new(items)
        .block(bordered("slides"))
        .highlight_style(selected())
        .highlight_symbol("▌");
    frame.render_stateful_widget(list, area, &mut state);
}

fn preview_pane(frame: &mut Frame<'_>, app: &mut App, area: Rect) {
    let page = app.machine.slide;
    let title = format!("preview · slide {}", page.saturating_add(1));
    let block = bordered(&title);
    let picture = block.inner(area);
    frame.render_widget(block, area);
    if app.protocol != Protocol::Mosaic {
        return;
    }
    let Some(Rendered::Pixels(raster)) = app.page(page) else {
        let hint = Paragraph::new(Line::from(Span::styled(
            "build the deck to see its pages",
            Style::default().fg(SLATE),
        )))
        .alignment(Alignment::Center);
        frame.render_widget(hint, picture);
        return;
    };
    frame.render_widget(Mosaic { raster }, picture);
}

/// The diagnostics of the selected slide, and those on no slide, each as its code and message.
fn diagnostics_pane(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let selected_slide = app.slides.get(app.machine.slide).map(|row| row.number);
    let lines: Vec<Line<'_>> = app
        .outcome
        .iter()
        .flat_map(|outcome| outcome.diagnostics.iter())
        .filter(|diagnostic| {
            diagnostic
                .slide
                .as_ref()
                .is_none_or(|at| Some(at.number) == selected_slide)
        })
        .map(|diagnostic| {
            let color = match diagnostic.severity() {
                Severity::Error => CORAL,
                Severity::Warning => GOLD,
            };
            Line::from(vec![
                Span::styled(
                    format!("{} ", diagnostic.code.as_str()),
                    Style::default().fg(color).add_modifier(Modifier::BOLD),
                ),
                Span::styled(diagnostic.message.clone(), Style::default().fg(INK)),
                Span::styled(
                    diagnostic
                        .helps
                        .first()
                        .map_or_else(String::new, |help| format!("  → {help}")),
                    Style::default().fg(SLATE),
                ),
            ])
        })
        .collect();
    let title = if lines.is_empty() {
        "diagnostics · none"
    } else {
        "diagnostics"
    };
    let text = if lines.is_empty() {
        vec![Line::from(Span::styled(
            "nothing to report for this slide",
            Style::default().fg(SLATE),
        ))]
    } else {
        lines
    };
    frame.render_widget(
        Paragraph::new(text)
            .block(bordered(title))
            .wrap(Wrap { trim: true }),
        area,
    );
}

/// The middle of `area` along `direction`, `WINDOW_PERCENT` of it, with the rest split between the sides.
fn middle(area: Rect, direction: Direction) -> Rect {
    let side = WHOLE_PERCENT
        .saturating_sub(WINDOW_PERCENT)
        .checked_div(SIDES)
        .unwrap_or(0);
    let parts = Layout::default()
        .direction(direction)
        .constraints([
            Constraint::Percentage(side),
            Constraint::Percentage(WINDOW_PERCENT),
            Constraint::Percentage(side),
        ])
        .split(area);
    match *parts {
        [_, center, _] => center,
        _ => area,
    }
}

/// A window over the frame, centered.
fn window(frame: Rect) -> Rect {
    middle(middle(frame, Direction::Vertical), Direction::Horizontal)
}

fn swatches(colors: &[sivi_core::theme::Color]) -> Vec<Span<'static>> {
    colors
        .iter()
        .map(|color| {
            Span::styled(
                SWATCH,
                Style::default().bg(Color::Rgb(color.red, color.green, color.blue)),
            )
        })
        .collect()
}

fn picker_window(frame: &mut Frame<'_>, app: &App, kind: Picker, area: Rect) {
    let entries = app.pickers.of(kind);
    let title = match kind {
        Picker::Theme => "themes · enter builds with the theme",
        Picker::Font => "fonts · enter installs the family",
        Picker::Palette => "palettes · the theme text to paste",
        Picker::CodeTheme => "code themes · the theme text to paste",
    };
    let (list_area, detail_area) = halves(area, Direction::Horizontal, PICKER_LIST_PERCENT);
    frame.render_widget(Clear, area);
    let items: Vec<ListItem<'_>> = entries
        .iter()
        .map(|entry| {
            let mut spans = vec![Span::styled(
                format!("{:<NAME_WIDTH$} ", entry.name),
                Style::default().fg(INK),
            )];
            spans.extend(swatches(&entry.swatches));
            ListItem::new(Line::from(spans))
        })
        .collect();
    let mut state = ListState::default();
    if !entries.is_empty() {
        state.select(Some(app.machine.cursor));
    }
    let list = List::new(items)
        .block(bordered(title))
        .highlight_style(selected())
        .highlight_symbol("▌");
    frame.render_stateful_widget(list, list_area, &mut state);
    let detail = entries.get(app.machine.cursor).map_or_else(
        || vec![Line::from("nothing to pick")],
        |entry| {
            vec![
                Line::from(Span::styled(
                    entry.name.clone(),
                    Style::default().fg(INK).add_modifier(Modifier::BOLD),
                )),
                Line::from(Span::styled(
                    entry.detail.clone(),
                    Style::default().fg(SLATE),
                )),
                Line::from(""),
                Line::from(Span::styled(
                    entry.snippet.clone(),
                    Style::default().fg(MINT),
                )),
            ]
        },
    );
    frame.render_widget(
        Paragraph::new(detail)
            .block(bordered("detail"))
            .wrap(Wrap { trim: false }),
        detail_area,
    );
}

fn help_window(frame: &mut Frame<'_>, area: Rect) {
    frame.render_widget(Clear, area);
    let keys = [
        ("q, ctrl-c", "quit"),
        ("b", "build the deck and write the PDF beside it"),
        ("w", "watch the deck's directory and build on every change"),
        ("j, k, ↓, ↑", "move among the slides or the entries"),
        ("1, 2, 3", "the overview, the slides, the diagnostics"),
        ("t", "pick a carried theme and build with it"),
        ("f", "browse the font catalog and install a family"),
        ("p", "browse the carried palettes"),
        ("c", "browse the code themes"),
        ("enter", "choose the entry"),
        ("esc", "leave the picker or the help"),
        ("o", "open the PDF in the viewer"),
    ];
    let lines: Vec<Line<'_>> = keys
        .iter()
        .map(|(key, what)| {
            Line::from(vec![
                Span::styled(
                    format!("{key:<KEY_WIDTH$}"),
                    Style::default().fg(MINT).add_modifier(Modifier::BOLD),
                ),
                Span::styled(*what, Style::default().fg(INK)),
            ])
        })
        .collect();
    frame.render_widget(Paragraph::new(lines).block(bordered("keys")), area);
}

/// Draws one frame of `app`.
pub(super) fn draw(frame: &mut Frame<'_>, app: &mut App) {
    let whole = frame.area();
    frame.render_widget(
        Block::default().style(Style::default().bg(BACKGROUND)),
        whole,
    );
    let areas = areas(whole);
    header(frame, app, areas.header);
    foot(frame, app, areas.foot);
    match app.screen() {
        Screen::Overview | Screen::Slides => {
            let panes = panes(areas.body, app.screen());
            slide_list(frame, app, panes.list);
            preview_pane(frame, app, panes.preview);
            if app.screen() == Screen::Overview {
                diagnostics_pane(frame, app, panes.diagnostics);
            }
        }
        Screen::Diagnostics => {
            let panes = panes(areas.body, Screen::Slides);
            slide_list(frame, app, panes.list);
            diagnostics_pane(frame, app, panes.preview);
        }
        Screen::Picker(kind) => {
            let panes = panes(areas.body, app.screen());
            slide_list(frame, app, panes.list);
            picker_window(frame, app, kind, window(whole));
        }
        Screen::Help => {
            let panes = panes(areas.body, app.screen());
            slide_list(frame, app, panes.list);
            help_window(frame, window(whole));
        }
    }
}
