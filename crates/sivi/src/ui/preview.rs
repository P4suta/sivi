//! A page on the terminal: through the kitty or the iTerm2 image protocol where the terminal speaks one, and as a mosaic of quadrant blocks everywhere else.

use crate::args::Environment;
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color as Paint;
use ratatui::widgets::Widget;
use sivi_core::mosaic::{self, Raster};

/// How the terminal shows a picture.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Protocol {
    /// The kitty graphics protocol, which kitty, Ghostty, and `WezTerm` speak.
    Kitty,
    /// The inline images of iTerm2, which `WezTerm` speaks as well.
    Iterm2,
    /// Quadrant blocks in true color, which every terminal shows.
    Mosaic,
}

/// The programs that speak each protocol, as `TERM_PROGRAM` names them, and the terminal names kitty and Ghostty set in `TERM`.
const ITERM_PROGRAMS: [&str; 2] = ["iTerm.app", "WezTerm"];
const KITTY_PROGRAMS: [&str; 3] = ["ghostty", "kitty", "WezTerm"];
const KITTY_TERMS: [&str; 2] = ["xterm-kitty", "xterm-ghostty"];
/// The terminal iTerm2 names in `LC_TERMINAL`, which it carries over a remote shell.
const ITERM_TERMINAL: &str = "iTerm2";
/// The image the query names, which the terminal answers about and never shows, past any page's own.
const PROBE_IMAGE: u32 = u32::MAX;
/// How many bytes of base64 one kitty chunk carries.
const KITTY_CHUNK: usize = 4096;
/// The escape, the control sequence introducer, the application program command, the string terminator, the operating system command, and the bell.
const ESC: &str = "\x1b";
const CSI: &str = "\x1b[";
const APC: &str = "\x1b_G";
const ST: &str = "\x1b\\";
const OSC: &str = "\x1b]";
const BEL: &str = "\x07";
/// How the terminal's attributes begin and end, which every terminal reports when asked.
const ATTRIBUTES: &str = "\x1b[?";
const ATTRIBUTES_END: u8 = b'c';

/// The query that asks the terminal whether it speaks the kitty graphics protocol, with a one-pixel picture it answers about and never shows, followed by the request for its attributes, which every terminal answers, so the answer has an end.
#[must_use]
pub(super) fn query() -> Vec<u8> {
    format!("{APC}i={PROBE_IMAGE},s=1,v=1,a=q,t=d,f=24;AAAA{ST}{CSI}c").into_bytes()
}

/// Whether `answer` holds the terminal's attributes, after which no more of it comes.
#[must_use]
pub(super) fn answered(answer: &[u8]) -> bool {
    let start = ATTRIBUTES.as_bytes();
    answer
        .windows(start.len())
        .position(|window| window == start)
        .and_then(|at| answer.get(at.saturating_add(start.len())..))
        .is_some_and(|rest| rest.contains(&ATTRIBUTES_END))
}

/// Whether `answer` says the terminal speaks the kitty graphics protocol.
#[must_use]
pub(super) fn speaks_kitty(answer: &[u8]) -> bool {
    let ok = format!("{APC}i={PROBE_IMAGE};OK");
    answer
        .windows(ok.len())
        .any(|window| window == ok.as_bytes())
}

/// Whether the variables name a kitty terminal, for a host where the terminal cannot be asked.
fn names_kitty(environment: &Environment) -> bool {
    let program = environment.term_program.as_deref().unwrap_or_default();
    let term = environment.term.as_deref().unwrap_or_default();
    environment.kitty_window.is_some()
        || KITTY_TERMS.contains(&term)
        || KITTY_PROGRAMS.contains(&program)
}

/// Whether the variables name iTerm2 or a terminal that speaks its protocol.
fn names_iterm(environment: &Environment) -> bool {
    let program = environment.term_program.as_deref().unwrap_or_default();
    ITERM_PROGRAMS.contains(&program) || environment.lc_terminal.as_deref() == Some(ITERM_TERMINAL)
}

/// The protocol the terminal speaks: the kitty protocol when its `answer` to the query says so, or when no query was possible and its variables name a kitty terminal; the iTerm2 protocol when the variables name one; a mosaic elsewhere.
///
/// An answer without the kitty reply outranks the variables, since they also reach a multiplexer that passes no picture through.
#[must_use]
pub(super) fn protocol(environment: &Environment, answer: Option<&[u8]>) -> Protocol {
    let kitty = answer.map_or_else(|| names_kitty(environment), speaks_kitty);
    if kitty {
        Protocol::Kitty
    } else if names_iterm(environment) {
        Protocol::Iterm2
    } else {
        Protocol::Mosaic
    }
}

/// The image a page keeps on a kitty terminal: one past its index, since the protocol reserves zero.
#[must_use]
pub(super) fn kitty_image(page: usize) -> u32 {
    let Ok(image) = u32::try_from(page.saturating_add(1)) else {
        return PROBE_IMAGE;
    };
    image
}

/// The kitty command that sends `png` to the terminal as `image`, in chunks, without showing it.
#[must_use]
pub(super) fn kitty_transmit(png: &[u8], image: u32) -> Vec<u8> {
    let encoded = STANDARD.encode(png);
    let chunks: Vec<&[u8]> = encoded.as_bytes().chunks(KITTY_CHUNK).collect();
    let last = chunks.len().saturating_sub(1);
    let commands: Vec<String> = chunks
        .iter()
        .enumerate()
        .map(|(index, chunk)| {
            let more = u8::from(index < last);
            let text = String::from_utf8_lossy(chunk);
            if index == 0 {
                format!("{APC}a=t,f=100,t=d,i={image},q=2,m={more};{text}{ST}")
            } else {
                format!("{APC}m={more};{text}{ST}")
            }
        })
        .collect();
    commands.concat().into_bytes()
}

/// The kitty command that shows the sent `image` over `columns` by `rows` cells at the cursor, leaving the cursor where it was.
#[must_use]
pub(super) fn kitty_place(image: u32, columns: u16, rows: u16) -> Vec<u8> {
    format!("{APC}a=p,i={image},c={columns},r={rows},C=1,q=2{ST}").into_bytes()
}

/// The kitty command that hides `image`, which the terminal keeps for the next placement.
#[must_use]
pub(super) fn kitty_clear(image: u32) -> Vec<u8> {
    format!("{APC}a=d,d=i,i={image},q=2{ST}").into_bytes()
}

/// The kitty command that drops every image the terminal keeps.
#[must_use]
pub(super) fn kitty_forget() -> Vec<u8> {
    format!("{APC}a=d,d=A,q=2{ST}").into_bytes()
}

/// The iTerm2 command that shows `png` over `columns` by `rows` cells at the cursor.
#[must_use]
pub(super) fn iterm2_show(png: &[u8], columns: u16, rows: u16) -> Vec<u8> {
    format!(
        "{OSC}1337;File=inline=1;width={columns};height={rows};preserveAspectRatio=1:{}{BEL}",
        STANDARD.encode(png)
    )
    .into_bytes()
}

/// The sequence that moves the cursor to column `x` and row `y`, counted from zero.
#[must_use]
pub(super) fn move_to(x: u16, y: u16) -> Vec<u8> {
    format!(
        "{ESC}[{};{}H",
        u32::from(y).saturating_add(1),
        u32::from(x).saturating_add(1)
    )
    .into_bytes()
}

/// A page as quadrant blocks over an area, keeping its aspect.
pub(super) struct Mosaic<'a> {
    pub raster: &'a Raster,
}

const fn paint(color: sivi_core::theme::Color) -> Paint {
    Paint::Rgb(color.red, color.green, color.blue)
}

impl Widget for Mosaic<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        let (columns, rows) = mosaic::fitted(
            self.raster.width,
            self.raster.height,
            usize::from(area.width),
            usize::from(area.height),
        );
        let tiles = mosaic::cells(self.raster, columns, rows);
        for (index, tile) in tiles.iter().enumerate() {
            // The grid lies within the area, so each tile's place fits the terminal's cells.
            let (Ok(column), Ok(row)) = (
                u16::try_from(index.checked_rem(columns).unwrap_or(0)),
                u16::try_from(index.checked_div(columns).unwrap_or(0)),
            ) else {
                continue;
            };
            let x = area.x.saturating_add(column);
            let y = area.y.saturating_add(row);
            if x >= area.right() || y >= area.bottom() {
                continue;
            }
            if let Some(slot) = buffer.cell_mut((x, y)) {
                slot.set_symbol(tile.glyph())
                    .set_fg(paint(tile.fore))
                    .set_bg(paint(tile.back));
            }
        }
    }
}

/// The cells a page fills inside `area` under the image protocols, which keep the slide's aspect themselves, or none for a page or an area without extent.
#[must_use]
pub(super) fn fitted_cells(
    raster_width: usize,
    raster_height: usize,
    area: Rect,
) -> Option<(u16, u16)> {
    let (columns, rows) = mosaic::fitted(
        raster_width,
        raster_height,
        usize::from(area.width),
        usize::from(area.height),
    );
    match (u16::try_from(columns), u16::try_from(rows)) {
        (Ok(columns), Ok(rows)) if columns > 0 && rows > 0 => Some((columns, rows)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Protocol, answered, iterm2_show, kitty_clear, kitty_forget, kitty_image, kitty_place,
        kitty_transmit, move_to, protocol, query,
    };
    use crate::args::Environment;

    const KITTY_ANSWER: &[u8] = b"\x1b_Gi=4294967295;OK\x1b\\\x1b[?62;4;22c";
    const PLAIN_ANSWER: &[u8] = b"\x1b[?1;2c";

    #[test]
    fn the_protocol_follows_the_answer_before_the_variables() {
        let plain = Environment::default();
        assert_eq!(protocol(&plain, None), Protocol::Mosaic);
        assert_eq!(protocol(&plain, Some(PLAIN_ANSWER)), Protocol::Mosaic);
        assert_eq!(protocol(&plain, Some(KITTY_ANSWER)), Protocol::Kitty);
        let kitty = Environment {
            kitty_window: Some(String::from("1")),
            ..Environment::default()
        };
        assert_eq!(protocol(&kitty, None), Protocol::Kitty);
        // A terminal that was asked and did not answer in kind is a multiplexer or a plain one, whatever the variables say.
        assert_eq!(protocol(&kitty, Some(PLAIN_ANSWER)), Protocol::Mosaic);
        assert_eq!(protocol(&kitty, Some(b"")), Protocol::Mosaic);
        for program in ["ghostty", "kitty", "WezTerm"] {
            let named = Environment {
                term_program: Some(String::from(program)),
                ..Environment::default()
            };
            assert_eq!(protocol(&named, None), Protocol::Kitty, "{program}");
        }
        let ghostty = Environment {
            term: Some(String::from("xterm-ghostty")),
            ..Environment::default()
        };
        assert_eq!(protocol(&ghostty, None), Protocol::Kitty);
        let iterm = Environment {
            term_program: Some(String::from("iTerm.app")),
            ..Environment::default()
        };
        assert_eq!(protocol(&iterm, Some(PLAIN_ANSWER)), Protocol::Iterm2);
        let remote = Environment {
            lc_terminal: Some(String::from("iTerm2")),
            ..Environment::default()
        };
        assert_eq!(protocol(&remote, Some(PLAIN_ANSWER)), Protocol::Iterm2);
        let wezterm = Environment {
            term_program: Some(String::from("WezTerm")),
            ..Environment::default()
        };
        assert_eq!(protocol(&wezterm, Some(KITTY_ANSWER)), Protocol::Kitty);
        assert_eq!(protocol(&wezterm, Some(PLAIN_ANSWER)), Protocol::Iterm2);
    }

    #[test]
    fn the_query_ends_with_the_attributes_request_and_the_answer_ends_with_the_attributes() {
        let asked = String::from_utf8(query()).unwrap();
        assert_eq!(
            asked,
            "\x1b_Gi=4294967295,s=1,v=1,a=q,t=d,f=24;AAAA\x1b\\\x1b[c"
        );
        assert!(answered(KITTY_ANSWER));
        assert!(answered(PLAIN_ANSWER));
        assert!(!answered(b"\x1b_Gi=4294967295;OK\x1b\\"));
        assert!(!answered(b"\x1b[?62;4"));
        assert!(!answered(b""));
        assert!(!answered(b"c\x1b[?"));
    }

    #[test]
    fn the_commands_carry_the_picture() {
        assert_eq!(kitty_image(0), 1);
        assert_eq!(kitty_image(usize::MAX), u32::MAX);
        let sent = String::from_utf8(kitty_transmit(b"png", 7)).unwrap();
        assert_eq!(sent, "\x1b_Ga=t,f=100,t=d,i=7,q=2,m=0;cG5n\x1b\\");
        let long = String::from_utf8(kitty_transmit(&[0; 4000], 1)).unwrap();
        assert_eq!(long.matches("\x1b_G").count(), 2, "{}", long.len());
        assert!(
            long.starts_with("\x1b_Ga=t,f=100,t=d,i=1,q=2,m=1;"),
            "{long:?}"
        );
        assert!(long.contains("\x1b\\\x1b_Gm=0;"), "{long:?}");
        assert!(long.ends_with("\x1b\\"));
        assert_eq!(
            String::from_utf8(kitty_place(7, 40, 10)).unwrap(),
            "\x1b_Ga=p,i=7,c=40,r=10,C=1,q=2\x1b\\"
        );
        assert_eq!(
            String::from_utf8(kitty_clear(7)).unwrap(),
            "\x1b_Ga=d,d=i,i=7,q=2\x1b\\"
        );
        assert_eq!(
            String::from_utf8(kitty_forget()).unwrap(),
            "\x1b_Ga=d,d=A,q=2\x1b\\"
        );
        assert_eq!(
            String::from_utf8(iterm2_show(b"png", 40, 10)).unwrap(),
            "\x1b]1337;File=inline=1;width=40;height=10;preserveAspectRatio=1:cG5n\x07"
        );
        assert_eq!(String::from_utf8(move_to(3, 5)).unwrap(), "\x1b[6;4H");
    }
}
