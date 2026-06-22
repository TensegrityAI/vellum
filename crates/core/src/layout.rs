//! Pure-arithmetic layout (ADR-0004). The view measures font metrics **once** via
//! its Canvas `MeasurePort` and feeds them here; everything below is reflow-free
//! arithmetic over the buffer plus those metrics, fully testable without a DOM.
//!
//! Monospace fast path: a code/prompt editor is monospaced, so a cell is one
//! `advance` wide and a caret's x is `column * advance`, its y `line * line_height`.
//! Columns are counted in **graphemes** (ADR-0001), so a caret never splits a
//! cluster. (Double-width cells — CJK, wide emoji — count as one column in
//! Increment 1; width-aware columns are a later refinement.)

use crate::buffer::TextBuffer;
use crate::offset::ByteOffset;
use unicode_segmentation::UnicodeSegmentation;

/// Font metrics measured once by the view's `MeasurePort` (ADR-0004) and fed into
/// the layout arithmetic. Pixel units; `f32` to match Canvas `measureText`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Metrics {
    /// Width of one monospace cell.
    pub advance: f32,
    /// Vertical distance between line baselines (the line box height).
    pub line_height: f32,
}

/// A position on the monospace grid: 0-based `line` and grapheme `column` measured
/// from that line's start.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GridPosition {
    pub line: usize,
    pub column: usize,
}

/// A half-open `[start, end)` range of line indices — the virtualization window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LineRange {
    pub start: usize,
    pub end: usize,
}

/// Locate `byte` as a grid position: its line, and the grapheme count from that
/// line's start to `byte`. `byte` must be a char boundary in `buffer`.
pub fn locate(buffer: &TextBuffer, byte: ByteOffset) -> GridPosition {
    let line = buffer.byte_to_line(byte.get());
    let line_start = buffer.line_to_byte(line).get();
    let column = buffer.slice(line_start..byte.get()).graphemes(true).count();
    GridPosition { line, column }
}

/// The caret's top-left pixel for a grid position under `metrics` (monospace:
/// `column * advance` across, `line * line_height` down).
pub fn caret_pixel(pos: GridPosition, metrics: Metrics) -> (f32, f32) {
    (
        pos.column as f32 * metrics.advance,
        pos.line as f32 * metrics.line_height,
    )
}

/// The half-open range of lines visible in a `viewport_height`-tall viewport
/// scrolled to `scroll_top`, clamped to `[0, line_count]`. Pure windowing — the
/// `scroll_top`/`viewport_height` are passed in by the view; no DOM read happens
/// here (ADR-0004). Partial lines at either edge are included (floor/ceil).
pub fn visible_lines(
    line_count: usize,
    scroll_top: f32,
    viewport_height: f32,
    line_height: f32,
) -> LineRange {
    if line_height <= 0.0 {
        return LineRange {
            start: 0,
            end: line_count,
        };
    }
    let first = (scroll_top / line_height).floor().max(0.0) as usize;
    let last = ((scroll_top + viewport_height) / line_height).ceil() as usize;
    let start = first.min(line_count);
    let end = last.clamp(start, line_count);
    LineRange { start, end }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn metrics() -> Metrics {
        Metrics {
            advance: 8.0,
            line_height: 20.0,
        }
    }

    #[test]
    fn locate_on_a_single_line_is_line_zero_at_the_grapheme_column() {
        let buf = TextBuffer::from_str("abc");
        assert_eq!(
            locate(&buf, ByteOffset::new(2)),
            GridPosition { line: 0, column: 2 }
        );
    }

    #[test]
    fn locate_after_a_newline_resets_the_column() {
        let buf = TextBuffer::from_str("ab\ncd");
        assert_eq!(
            locate(&buf, ByteOffset::new(3)),
            GridPosition { line: 1, column: 0 }
        );
        assert_eq!(
            locate(&buf, ByteOffset::new(4)),
            GridPosition { line: 1, column: 1 }
        );
    }

    #[test]
    fn locate_counts_columns_in_graphemes_not_bytes() {
        // "a😀本": a=1 byte, 😀=4 bytes, 本=3 bytes — three graphemes, three columns.
        let buf = TextBuffer::from_str("a😀本");
        assert_eq!(
            locate(&buf, ByteOffset::new(5)),
            GridPosition { line: 0, column: 2 }
        ); // after 😀
        assert_eq!(
            locate(&buf, ByteOffset::new(8)),
            GridPosition { line: 0, column: 3 }
        ); // after 本
    }

    #[test]
    fn locate_treats_a_combining_sequence_as_one_column() {
        // "e\u{0301}" is one grapheme cluster (e + combining acute).
        let buf = TextBuffer::from_str("e\u{0301}x");
        assert_eq!(
            locate(&buf, ByteOffset::new(3)),
            GridPosition { line: 0, column: 1 }
        ); // after é
    }

    #[test]
    fn caret_pixel_is_column_times_advance_and_line_times_height() {
        let pos = GridPosition { line: 2, column: 3 };
        assert_eq!(caret_pixel(pos, metrics()), (24.0, 40.0));
    }

    #[test]
    fn visible_lines_windows_the_viewport() {
        // 10 lines, line_height 20, viewport 60 tall, scrolled to the top → [0, 3).
        assert_eq!(
            visible_lines(10, 0.0, 60.0, 20.0),
            LineRange { start: 0, end: 3 }
        );
    }

    #[test]
    fn visible_lines_includes_partial_lines_at_both_edges() {
        // Scrolled to 50px (mid line 2) for a 60px viewport → lines 2..6 (ceil 110/20).
        assert_eq!(
            visible_lines(10, 50.0, 60.0, 20.0),
            LineRange { start: 2, end: 6 }
        );
    }

    #[test]
    fn visible_lines_clamps_to_the_line_count() {
        // A viewport taller than the content never windows past the last line.
        assert_eq!(
            visible_lines(3, 0.0, 1000.0, 20.0),
            LineRange { start: 0, end: 3 }
        );
    }

    #[test]
    fn visible_lines_past_the_end_is_an_empty_window_at_the_end() {
        assert_eq!(
            visible_lines(3, 1000.0, 60.0, 20.0),
            LineRange { start: 3, end: 3 }
        );
    }
}
