//! Where a value sits in the deck, as the front end reads it and every diagnostic reports it.

/// A line and a column of the deck, each counted from one.
///
/// The column counts bytes, so a multibyte character advances it by its width.
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd, Hash)]
pub struct LineColumn {
    pub line: usize,
    pub column: usize,
}

impl LineColumn {
    #[must_use]
    pub const fn new(line: usize, column: usize) -> Self {
        Self { line, column }
    }
}

/// A span of the deck from `start` to `end`, both inclusive, as CommonMark parsers report positions.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash)]
pub struct Span {
    pub start: LineColumn,
    pub end: LineColumn,
}

impl Span {
    #[must_use]
    pub const fn new(start: LineColumn, end: LineColumn) -> Self {
        Self { start, end }
    }

    /// The span of one whole line of `width` bytes; an empty line spans its first column.
    #[must_use]
    pub const fn line(line: usize, width: usize) -> Self {
        let last = if width == 0 { 1 } else { width };
        Self {
            start: LineColumn::new(line, 1),
            end: LineColumn::new(line, last),
        }
    }

    /// The same span counted from `first_line` instead of line one, which maps a slide's own lines to the deck's.
    #[must_use]
    pub const fn from_line(self, first_line: usize) -> Self {
        let offset = first_line.saturating_sub(1);
        Self {
            start: LineColumn::new(self.start.line.saturating_add(offset), self.start.column),
            end: LineColumn::new(self.end.line.saturating_add(offset), self.end.column),
        }
    }

    /// The smallest span holding both.
    #[must_use]
    pub fn union(self, other: Self) -> Self {
        Self {
            start: self.start.min(other.start),
            end: self.end.max(other.end),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{LineColumn, Span};

    #[test]
    fn a_span_moves_down_with_its_slide_and_joins_with_another() {
        let local = Span::new(LineColumn::new(2, 3), LineColumn::new(4, 1));
        let moved = local.from_line(10);
        assert_eq!(moved.start, LineColumn::new(11, 3));
        assert_eq!(moved.end, LineColumn::new(13, 1));
        assert_eq!(local.from_line(1), local);
        assert_eq!(local.from_line(0), local);
        let later = Span::new(LineColumn::new(3, 1), LineColumn::new(9, 2));
        assert_eq!(
            local.union(later),
            Span::new(LineColumn::new(2, 3), LineColumn::new(9, 2))
        );
        assert_eq!(
            Span::line(5, 0),
            Span::new(LineColumn::new(5, 1), LineColumn::new(5, 1))
        );
        assert_eq!(Span::line(5, 7).end, LineColumn::new(5, 7));
    }
}
