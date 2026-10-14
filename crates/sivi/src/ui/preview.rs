//! A page on the terminal: through the kitty or the iTerm2 image protocol where the terminal speaks one, and as a mosaic of half blocks everywhere else.

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
    /// Half blocks in true color, which every terminal shows.
    Mosaic,
}

/// The programs that speak each protocol, as `TERM_PROGRAM` names them.
const ITERM_PROGRAMS: [&str; 2] = ["iTerm.app", "WezTerm"];
const KITTY_PROGRAMS: [&str; 2] = ["ghostty", "kitty"];
/// The terminal name kitty sets.
const KITTY_TERM: &str = "xterm-kitty";
/// The image the preview keeps on a kitty terminal, replaced on every change.
const KITTY_IMAGE: u32 = 1;
/// How many bytes of base64 one kitty chunk carries.
const KITTY_CHUNK: usize = 4096;
/// The escape, the application program command, the string terminator, and the bell.
const ESC: &str = "\x1b";
const APC: &str = "\x1b_G";
const ST: &str = "\x1b\\";
const OSC: &str = "\x1b]";
const BEL: &str = "\x07";
/// The upper half block, whose foreground paints the upper picture row and background the lower.
const UPPER_HALF: &str = "▀";

/// The protocol the terminal speaks, from its environment.
#[must_use]
pub(super) fn protocol(environment: &Environment) -> Protocol {
    let program = environment.term_program.as_deref().unwrap_or_default();
    if environment.kitty_window.is_some()
        || environment.term.as_deref() == Some(KITTY_TERM)
        || KITTY_PROGRAMS.contains(&program)
    {
        Protocol::Kitty
    } else if ITERM_PROGRAMS.contains(&program) {
        Protocol::Iterm2
    } else {
        Protocol::Mosaic
    }
}

/// The kitty command that shows `png` over `columns` by `rows` cells at the cursor, in chunks.
#[must_use]
pub(super) fn kitty_show(png: &[u8], columns: u16, rows: u16) -> Vec<u8> {
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
                format!(
                    "{APC}a=T,f=100,t=d,i={KITTY_IMAGE},c={columns},r={rows},q=2,m={more};{text}{ST}"
                )
            } else {
                format!("{APC}m={more};{text}{ST}")
            }
        })
        .collect();
    commands.concat().into_bytes()
}

/// The kitty command that removes the shown image.
#[must_use]
pub(super) fn kitty_clear() -> Vec<u8> {
    format!("{APC}a=d,d=i,i={KITTY_IMAGE},q=2{ST}").into_bytes()
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

/// A page as half blocks over an area, keeping its aspect.
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
                slot.set_symbol(UPPER_HALF)
                    .set_fg(paint(tile.upper))
                    .set_bg(paint(tile.lower));
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
    use super::{Protocol, iterm2_show, kitty_clear, kitty_show, move_to, protocol};
    use crate::args::Environment;

    #[test]
    fn the_protocol_follows_the_terminal_and_the_commands_carry_the_picture() {
        let plain = Environment::default();
        assert_eq!(protocol(&plain), Protocol::Mosaic);
        let kitty = Environment {
            kitty_window: Some(String::from("1")),
            ..Environment::default()
        };
        assert_eq!(protocol(&kitty), Protocol::Kitty);
        let ghostty = Environment {
            term_program: Some(String::from("ghostty")),
            ..Environment::default()
        };
        assert_eq!(protocol(&ghostty), Protocol::Kitty);
        let iterm = Environment {
            term_program: Some(String::from("iTerm.app")),
            ..Environment::default()
        };
        assert_eq!(protocol(&iterm), Protocol::Iterm2);
        let shown = String::from_utf8(kitty_show(b"png", 40, 10)).unwrap();
        assert!(
            shown.starts_with("\x1b_Ga=T,f=100,t=d,i=1,c=40,r=10,q=2,m=0;cG5n\x1b\\"),
            "{shown:?}"
        );
        let long = String::from_utf8(kitty_show(&[0; 4000], 1, 1)).unwrap();
        assert_eq!(long.matches("\x1b_G").count(), 2, "{}", long.len());
        assert!(long.contains(",m=1;"), "{long:?}");
        assert!(long.ends_with("\x1b\\"));
        assert_eq!(
            String::from_utf8(kitty_clear()).unwrap(),
            "\x1b_Ga=d,d=i,i=1,q=2\x1b\\"
        );
        assert_eq!(
            String::from_utf8(iterm2_show(b"png", 40, 10)).unwrap(),
            "\x1b]1337;File=inline=1;width=40;height=10;preserveAspectRatio=1:cG5n\x07"
        );
        assert_eq!(String::from_utf8(move_to(3, 5)).unwrap(), "\x1b[6;4H");
    }
}
