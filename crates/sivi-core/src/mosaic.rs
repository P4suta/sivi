//! A raster shrunk to a grid of terminal cells, two picture columns and two picture rows to a cell, for a preview where no image protocol is.
//!
//! Every cell shows its four quarters in two colors: the quarters a quadrant glyph paints take the fore color and the rest the back color, split so the two colors stray least from the picture, so a grid of `columns` by `rows` cells shows the picture at twice `columns` by twice `rows` samples.

use crate::theme::Color;
use alloc::vec::Vec;

/// The bytes of one pixel: red, green, blue, and alpha.
const CHANNELS: usize = 4;
/// The picture columns and the picture rows one cell shows.
pub const COLUMNS_PER_CELL: usize = 2;
pub const ROWS_PER_CELL: usize = 2;
/// The quarters of a cell.
pub const QUARTERS: usize = COLUMNS_PER_CELL.saturating_mul(ROWS_PER_CELL);
/// The bit of each quarter in a pattern: the upper left, the upper right, the lower left, and the lower right.
pub const UPPER_LEFT: u8 = 0b1000;
pub const UPPER_RIGHT: u8 = 0b0100;
pub const LOWER_LEFT: u8 = 0b0010;
pub const LOWER_RIGHT: u8 = 0b0001;
/// The bits in quarter order, and the pattern that holds every quarter.
const BITS: [u8; QUARTERS] = [UPPER_LEFT, UPPER_RIGHT, LOWER_LEFT, LOWER_RIGHT];
pub const FULL: u8 = 0b1111;
/// How many patterns the quarters form.
const PATTERNS: usize = 1 << QUARTERS;
/// The glyph of each pattern, by the pattern's value, which paints a quarter where its bit is set.
const GLYPHS: [&str; PATTERNS] = [
    " ", "▗", "▖", "▄", "▝", "▐", "▞", "▟", "▘", "▚", "▌", "▙", "▀", "▜", "▛", "█",
];
/// Where the green and the blue bytes of a pixel lie after the red one.
const GREEN_OFFSET: usize = 1;
const BLUE_OFFSET: usize = 2;

/// A raster in page order, one byte per channel, top row first.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Raster {
    pub width: usize,
    pub height: usize,
    /// `width` times `height` pixels of red, green, blue, and alpha, in row order.
    pub rgba: Vec<u8>,
}

/// One cell of the mosaic: the pattern of quarters that take the fore color, and the two colors.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct Tile {
    /// A bit per quarter, the upper left highest and the lower right lowest, set where the fore color shows.
    pub pattern: u8,
    pub fore: Color,
    pub back: Color,
}

impl Tile {
    /// The quadrant glyph that paints the fore color over the tile's pattern, from the block elements every terminal font carries, or a space for a pattern past them.
    #[must_use]
    pub fn glyph(self) -> &'static str {
        GLYPHS
            .get(usize::from(self.pattern))
            .copied()
            .unwrap_or(" ")
    }
}

/// The running sum of some colors, for their mean.
#[derive(Clone, Copy, Debug, Default)]
struct Sum {
    red: u64,
    green: u64,
    blue: u64,
    count: u64,
}

impl Sum {
    fn add(&mut self, color: Color) {
        self.red = self.red.saturating_add(u64::from(color.red));
        self.green = self.green.saturating_add(u64::from(color.green));
        self.blue = self.blue.saturating_add(u64::from(color.blue));
        self.count = self.count.saturating_add(1);
    }

    /// The mean color, or none of no colors.
    fn mean(self) -> Option<Color> {
        if self.count == 0 {
            return None;
        }
        let channel =
            |sum: u64| u8::try_from(sum.checked_div(self.count).unwrap_or(0)).unwrap_or(u8::MAX);
        Some(Color {
            red: channel(self.red),
            green: channel(self.green),
            blue: channel(self.blue),
        })
    }
}

impl Raster {
    /// Whether the bytes hold exactly the pixels the size names.
    #[must_use]
    pub fn is_sound(&self) -> bool {
        self.width
            .checked_mul(self.height)
            .and_then(|pixels| pixels.checked_mul(CHANNELS))
            .is_some_and(|bytes| bytes == self.rgba.len())
    }

    /// The pixel at `x`, `y`, or black past the edge or in a raster whose bytes fall short.
    fn pixel(&self, x: usize, y: usize) -> Color {
        let at = y
            .checked_mul(self.width)
            .and_then(|row| row.checked_add(x))
            .and_then(|index| index.checked_mul(CHANNELS));
        let Some(at) = at else {
            return Color::default();
        };
        match (
            self.rgba.get(at),
            self.rgba.get(at.wrapping_add(GREEN_OFFSET)),
            self.rgba.get(at.wrapping_add(BLUE_OFFSET)),
        ) {
            (Some(red), Some(green), Some(blue)) => Color {
                red: *red,
                green: *green,
                blue: *blue,
            },
            _ => Color::default(),
        }
    }

    /// The mean color of the pixels from `x0` to `x1` and from `y0` to `y1`, each end exclusive and clipped to the raster; black for an empty span.
    fn mean(&self, x0: usize, x1: usize, y0: usize, y1: usize) -> Color {
        let mut sum = Sum::default();
        for y in y0..y1.min(self.height) {
            for x in x0..x1.min(self.width) {
                sum.add(self.pixel(x, y));
            }
        }
        sum.mean().unwrap_or_default()
    }
}

/// The edge of sample `index` of `samples` across `extent` pixels, which rises with the index.
fn edge(index: usize, samples: usize, extent: usize) -> usize {
    if samples == 0 {
        return 0;
    }
    index
        .saturating_mul(extent)
        .checked_div(samples)
        .unwrap_or(0)
}

/// The pixels of sample `index` of `samples` across `extent` pixels, at least one pixel wide, so a grid finer than the picture repeats a pixel rather than skipping it.
fn span(index: usize, samples: usize, extent: usize) -> (usize, usize) {
    let start = edge(index, samples, extent);
    let end = edge(index.saturating_add(1), samples, extent).max(start.saturating_add(1));
    (start, end)
}

/// The squared distance between two colors, summed over the channels.
fn apart(one: Color, other: Color) -> u64 {
    let gap = |a: u8, b: u8| {
        let difference = u64::from(a.abs_diff(b));
        difference.saturating_mul(difference)
    };
    gap(one.red, other.red)
        .saturating_add(gap(one.green, other.green))
        .saturating_add(gap(one.blue, other.blue))
}

/// The tile that strays least from four quarter colors, given in quarter order.
///
/// Each pattern that holds the upper left quarter names one split of the quarters, since a pattern and its complement split alike; the quarters it names take their mean as the fore color and the rest theirs as the back color, and the split whose two colors lie closest to every quarter wins, the fuller pattern on a tie.
fn tile(quarters: [Color; QUARTERS]) -> Tile {
    let mut best: Option<(u64, Tile)> = None;
    for pattern in (UPPER_LEFT..=FULL).rev() {
        let (mut fore, mut back) = (Sum::default(), Sum::default());
        for (color, bit) in quarters.iter().zip(BITS) {
            if pattern & bit == 0 {
                back.add(*color);
            } else {
                fore.add(*color);
            }
        }
        let Some(fore) = fore.mean() else {
            continue;
        };
        let back = back.mean().unwrap_or(fore);
        let strayed = quarters
            .iter()
            .zip(BITS)
            .map(|(color, bit)| apart(*color, if pattern & bit == 0 { back } else { fore }))
            .fold(0, u64::saturating_add);
        if best.is_none_or(|(least, _)| strayed < least) {
            best = Some((
                strayed,
                Tile {
                    pattern,
                    fore,
                    back,
                },
            ));
        }
    }
    best.map_or_else(
        || Tile {
            pattern: FULL,
            fore: Color::default(),
            back: Color::default(),
        },
        |(_, found)| found,
    )
}

/// The raster shown over `columns` by `rows` cells, each cell split over the mean colors of its four quarters, in row order.
///
/// A column or row count of zero yields no cells; a raster that holds fewer bytes than its size names reads as black where the bytes fall short.
#[must_use]
pub fn cells(raster: &Raster, columns: usize, rows: usize) -> Vec<Tile> {
    let across = columns.saturating_mul(COLUMNS_PER_CELL);
    let down = rows.saturating_mul(ROWS_PER_CELL);
    let mut found = Vec::with_capacity(columns.saturating_mul(rows));
    for row in 0..rows {
        for column in 0..columns {
            let quarter = |right: usize, lower: usize| {
                let (x0, x1) = span(
                    column
                        .saturating_mul(COLUMNS_PER_CELL)
                        .saturating_add(right),
                    across,
                    raster.width,
                );
                let (y0, y1) = span(
                    row.saturating_mul(ROWS_PER_CELL).saturating_add(lower),
                    down,
                    raster.height,
                );
                raster.mean(x0, x1, y0, y1)
            };
            found.push(tile([
                quarter(0, 0),
                quarter(1, 0),
                quarter(0, 1),
                quarter(1, 1),
            ]));
        }
    }
    found
}

/// The cells a raster fills inside a frame of `columns` by `rows` cells, keeping its aspect.
///
/// A cell counts twice as tall as wide; the columns and rows used are each at least one when the frame has any.
#[must_use]
pub fn fitted(width: usize, height: usize, columns: usize, rows: usize) -> (usize, usize) {
    if columns == 0 || rows == 0 || width == 0 || height == 0 {
        return (0, 0);
    }
    // The frame in picture rows, since a cell holds two.
    let frame_height = rows.saturating_mul(ROWS_PER_CELL);
    let by_width = height
        .saturating_mul(columns)
        .checked_div(width)
        .unwrap_or(0);
    if by_width <= frame_height {
        (columns, by_width.div_ceil(ROWS_PER_CELL).max(1))
    } else {
        let used = width
            .saturating_mul(frame_height)
            .checked_div(height)
            .unwrap_or(0);
        (used.max(1), rows)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        BITS, COLUMNS_PER_CELL, FULL, LOWER_LEFT, LOWER_RIGHT, QUARTERS, ROWS_PER_CELL, Raster,
        Tile, UPPER_LEFT, UPPER_RIGHT, cells, fitted,
    };
    use crate::theme::Color;
    use alloc::vec;
    use alloc::vec::Vec;

    const RED: Color = Color {
        red: 255,
        green: 0,
        blue: 0,
    };
    const BLUE: Color = Color {
        red: 0,
        green: 0,
        blue: 255,
    };
    const WHITE: Color = Color {
        red: 255,
        green: 255,
        blue: 255,
    };
    const BLACK: Color = Color {
        red: 0,
        green: 0,
        blue: 0,
    };

    /// A raster whose top half is red and bottom half is blue, four pixels wide and four tall.
    fn halves() -> Raster {
        let mut rgba = Vec::new();
        for y in 0..4 {
            for _ in 0..4 {
                if y < 2 {
                    rgba.extend_from_slice(&[255, 0, 0, 255]);
                } else {
                    rgba.extend_from_slice(&[0, 0, 255, 255]);
                }
            }
        }
        Raster {
            width: 4,
            height: 4,
            rgba,
        }
    }

    /// A two by two raster of the four colors in quarter order.
    fn corners(quarters: [Color; QUARTERS]) -> Raster {
        let rgba = quarters
            .iter()
            .flat_map(|color| [color.red, color.green, color.blue, 255])
            .collect();
        Raster {
            width: 2,
            height: 2,
            rgba,
        }
    }

    fn one(raster: &Raster) -> Tile {
        cells(raster, 1, 1)[0]
    }

    #[test]
    fn a_cell_splits_its_quarters_over_the_two_closest_colors_and_the_grid_keeps_the_aspect() {
        let raster = halves();
        assert!(raster.is_sound());
        let upper = one(&raster);
        assert_eq!(
            upper,
            Tile {
                pattern: UPPER_LEFT | UPPER_RIGHT,
                fore: RED,
                back: BLUE,
            }
        );
        assert_eq!(upper.glyph(), "▀");
        let four = cells(&raster, 2, 2);
        assert_eq!(four.len(), 4);
        // A uniform cell is the full block of its color, which the tie falls to.
        assert_eq!(
            four[0],
            Tile {
                pattern: FULL,
                fore: RED,
                back: RED,
            }
        );
        assert_eq!(four[3].glyph(), "█");
        assert_eq!(four[3].fore, BLUE);
        assert_eq!(cells(&raster, 0, 3), Vec::new());
        assert_eq!(cells(&raster, 3, 0), Vec::new());
        // More cells than pixels repeat the nearest pixel rather than reading past the edge.
        let many = cells(&raster, 8, 8);
        assert_eq!(many.len(), 64);
        assert_eq!(many[0].fore, RED);
        assert_eq!(many[63].back, BLUE);
        let short = Raster {
            width: 4,
            height: 4,
            rgba: vec![255; 8],
        };
        assert!(!short.is_sound());
        assert_eq!(one(&short).back, BLACK);
        assert_eq!(fitted(960, 540, 80, 40), (80, 23));
        assert_eq!(fitted(960, 540, 20, 40), (20, 6));
        assert_eq!(fitted(100, 400, 80, 40), (20, 40));
        assert_eq!(fitted(0, 10, 80, 40), (0, 0));
        assert_eq!(fitted(10, 10, 0, 40), (0, 0));
        assert_eq!(fitted(1, 1, 1, 1), (1, 1));
        assert_eq!((COLUMNS_PER_CELL, ROWS_PER_CELL), (2, 2));
        assert_eq!(BITS.iter().fold(0, |all, bit| all | bit), FULL);
    }

    #[test]
    fn a_lone_quarter_takes_its_own_color_on_either_side_of_the_split() {
        // The fore color always holds the upper left quarter, so a lone bright corner elsewhere shows as the back color.
        let lower_right = one(&corners([BLACK, BLACK, BLACK, WHITE]));
        assert_eq!(
            lower_right,
            Tile {
                pattern: UPPER_LEFT | UPPER_RIGHT | LOWER_LEFT,
                fore: BLACK,
                back: WHITE,
            }
        );
        assert_eq!(lower_right.glyph(), "▛");
        let upper_left = one(&corners([WHITE, BLACK, BLACK, BLACK]));
        assert_eq!(upper_left.pattern, UPPER_LEFT);
        assert_eq!((upper_left.fore, upper_left.back), (WHITE, BLACK));
        assert_eq!(upper_left.glyph(), "▘");
        let diagonal = one(&corners([WHITE, BLACK, BLACK, WHITE]));
        assert_eq!(diagonal.pattern, UPPER_LEFT | LOWER_RIGHT);
        assert_eq!(diagonal.glyph(), "▚");
        // Two shades of one side blend into their mean rather than splitting the other side.
        let shaded = one(&corners([WHITE, RED, BLUE, BLUE]));
        assert_eq!(shaded.pattern, UPPER_LEFT | UPPER_RIGHT);
        assert_eq!(shaded.back, BLUE);
        assert_eq!(
            shaded.fore,
            Color {
                red: 255,
                green: 127,
                blue: 127,
            }
        );
        let past = Tile {
            pattern: u8::MAX,
            fore: BLACK,
            back: BLACK,
        };
        assert_eq!(past.glyph(), " ");
        assert_eq!(
            Tile {
                pattern: 0,
                fore: BLACK,
                back: BLACK
            }
            .glyph(),
            " "
        );
    }
}
