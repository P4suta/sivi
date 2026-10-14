//! A raster shrunk to a grid of terminal cells, two picture rows to a cell, for a preview where no image protocol is.
//!
//! Every cell shows its upper half in one color and its lower half in another, as the upper half block glyph paints them, so a grid of `columns` by `rows` cells shows the picture at `columns` by twice `rows` samples.

use crate::theme::Color;
use alloc::vec::Vec;

/// The bytes of one pixel: red, green, blue, and alpha.
const CHANNELS: usize = 4;
/// The picture rows one cell shows.
pub const ROWS_PER_CELL: usize = 2;
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

/// One cell of the mosaic: the color of its upper half and of its lower half.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct Tile {
    pub upper: Color,
    pub lower: Color,
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
        let (mut red, mut green, mut blue, mut count): (u64, u64, u64, u64) = (0, 0, 0, 0);
        for y in y0..y1.min(self.height) {
            for x in x0..x1.min(self.width) {
                let pixel = self.pixel(x, y);
                red = red.saturating_add(u64::from(pixel.red));
                green = green.saturating_add(u64::from(pixel.green));
                blue = blue.saturating_add(u64::from(pixel.blue));
                count = count.saturating_add(1);
            }
        }
        if count == 0 {
            return Color::default();
        }
        let channel =
            |sum: u64| u8::try_from(sum.checked_div(count).unwrap_or(0)).unwrap_or(u8::MAX);
        Color {
            red: channel(red),
            green: channel(green),
            blue: channel(blue),
        }
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

/// The raster shown over `columns` by `rows` cells, each cell the mean of its two halves, in row order.
///
/// A column or row count of zero yields no cells; a raster that holds fewer bytes than its size names reads as black where the bytes fall short.
#[must_use]
pub fn cells(raster: &Raster, columns: usize, rows: usize) -> Vec<Tile> {
    let samples = rows.saturating_mul(ROWS_PER_CELL);
    let mut found = Vec::with_capacity(columns.saturating_mul(rows));
    for row in 0..rows {
        let upper_row = row.saturating_mul(ROWS_PER_CELL);
        let lower_row = upper_row.saturating_add(1);
        for column in 0..columns {
            let (x0, x1) = (
                edge(column, columns, raster.width),
                edge(column.saturating_add(1), columns, raster.width)
                    .max(edge(column, columns, raster.width).saturating_add(1)),
            );
            let span = |sample: usize| {
                let y0 = edge(sample, samples, raster.height);
                let y1 = edge(sample.saturating_add(1), samples, raster.height)
                    .max(y0.saturating_add(1));
                (y0, y1)
            };
            let (upper_y0, upper_y1) = span(upper_row);
            let (lower_y0, lower_y1) = span(lower_row);
            found.push(Tile {
                upper: raster.mean(x0, x1, upper_y0, upper_y1),
                lower: raster.mean(x0, x1, lower_y0, lower_y1),
            });
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
    use super::{ROWS_PER_CELL, Raster, Tile, cells, fitted};
    use crate::theme::Color;
    use alloc::vec;
    use alloc::vec::Vec;

    fn solid(red: u8, green: u8, blue: u8) -> Color {
        Color { red, green, blue }
    }

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

    #[test]
    fn a_cell_shows_its_upper_and_lower_halves_and_the_grid_keeps_the_aspect() {
        let raster = halves();
        assert!(raster.is_sound());
        let one = cells(&raster, 1, 1);
        assert_eq!(
            one,
            vec![Tile {
                upper: solid(255, 0, 0),
                lower: solid(0, 0, 255),
            }]
        );
        let four = cells(&raster, 2, 2);
        assert_eq!(four.len(), 4);
        assert_eq!(four[0].upper, solid(255, 0, 0));
        assert_eq!(four[0].lower, solid(255, 0, 0));
        assert_eq!(four[3].upper, solid(0, 0, 255));
        assert_eq!(four[3].lower, solid(0, 0, 255));
        assert_eq!(cells(&raster, 0, 3), Vec::new());
        assert_eq!(cells(&raster, 3, 0), Vec::new());
        // More cells than pixels repeat the nearest pixel rather than reading past the edge.
        let many = cells(&raster, 8, 8);
        assert_eq!(many.len(), 64);
        assert_eq!(many[0].upper, solid(255, 0, 0));
        assert_eq!(many[63].lower, solid(0, 0, 255));
        let short = Raster {
            width: 4,
            height: 4,
            rgba: vec![255; 8],
        };
        assert!(!short.is_sound());
        assert_eq!(cells(&short, 1, 1)[0].lower, Color::default());
        assert_eq!(fitted(960, 540, 80, 40), (80, 23));
        assert_eq!(fitted(960, 540, 20, 40), (20, 6));
        assert_eq!(fitted(100, 400, 80, 40), (20, 40));
        assert_eq!(fitted(0, 10, 80, 40), (0, 0));
        assert_eq!(fitted(10, 10, 0, 40), (0, 0));
        assert_eq!(fitted(1, 1, 1, 1), (1, 1));
        assert_eq!(ROWS_PER_CELL, 2);
    }
}
