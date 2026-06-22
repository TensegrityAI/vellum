//! WASM-side integration test for the `Editor` binding.
//!
//! The `Editor` now wraps the [`Document`] aggregate (Task H2), so these tests
//! exercise the flat token wire, the fallible H1 offset contract, undo/redo at
//! the JS boundary, cursor movement, and the UTF-16↔byte conversions — all
//! demonstrating the Phase H acceptance: **no panics across the boundary**.
use vellum_wasm::Editor;
use wasm_bindgen_test::*;

#[wasm_bindgen_test]
fn insert_then_tokens_roundtrip() {
    let mut ed = Editor::new("a ");
    ed.insert(2, "{{ x }}").unwrap();
    assert_eq!(ed.text(), "a {{ x }}");
    let t = ed.tokens(); // [start, end, kind, ...] => [0,2,0, 2,9,1]
    assert_eq!(t, vec![0, 2, 0, 2, 9, 1]);
}

#[wasm_bindgen_test]
fn insert_at_bad_offset_returns_err_and_keeps_instance_usable() {
    let mut ed = Editor::new("a");
    // Offset 99 is far past the end → Err, and the buffer is untouched.
    assert!(ed.insert(99, "x").is_err());
    assert_eq!(ed.text(), "a", "rejected insert must not mutate");
    // The instance is NOT poisoned: a subsequent valid insert still works.
    assert!(ed.insert(1, "b").is_ok());
    assert_eq!(ed.text(), "ab");
}

#[wasm_bindgen_test]
fn delete_inverted_range_returns_err() {
    let mut ed = Editor::new("hello");
    // start > end → Err, buffer untouched, instance still usable.
    assert!(ed.delete(4, 1).is_err());
    assert_eq!(ed.text(), "hello");
    // Still usable: a valid delete afterwards works.
    assert!(ed.delete(0, 1).is_ok());
    assert_eq!(ed.text(), "ello");
}

#[wasm_bindgen_test]
fn insert_on_non_char_boundary_returns_err() {
    // "café": 'é' spans bytes 3..5; byte 4 is its interior (non char boundary).
    let mut ed = Editor::new("café");
    assert!(ed.insert(4, "x").is_err());
    assert_eq!(ed.text(), "café", "rejected insert must not mutate");
    // Inserting at a real boundary (end == len 5) still works.
    assert!(ed.insert(5, "!").is_ok());
    assert_eq!(ed.text(), "café!");
}

// --- Task H2: undo/redo, cursor, conversions across the boundary ----------

#[wasm_bindgen_test]
fn undo_redo_roundtrip_across_boundary() {
    // Mutations now go through the Document aggregate, so undo/redo work at the
    // JS boundary (the win H1's TODO(H2) pointed to).
    let mut ed = Editor::new("");
    ed.insert(0, "hello").unwrap();
    assert_eq!(ed.text(), "hello");
    assert!(ed.can_undo());
    assert!(!ed.can_redo());

    assert!(ed.undo());
    assert_eq!(ed.text(), "");
    assert!(!ed.can_undo());
    assert!(ed.can_redo());

    assert!(ed.redo());
    assert_eq!(ed.text(), "hello");

    // Nothing left to redo.
    assert!(!ed.redo());
}

#[wasm_bindgen_test]
fn insert_at_cursor_and_tokens() {
    // Place the caret after "a " and type a Jinja expression; tokens must reflect
    // the inserted expression on the same flat wire.
    let mut ed = Editor::new("a ");
    ed.set_caret(2).unwrap();
    ed.insert_at_cursor("{{ x }}");
    assert_eq!(ed.text(), "a {{ x }}");
    // Caret advanced past the inserted run.
    assert_eq!(ed.cursor_head(), 2 + "{{ x }}".len());
    // Same flat token wire as the explicit-offset insert: [0,2,0, 2,9,1].
    assert_eq!(ed.tokens(), vec![0, 2, 0, 2, 9, 1]);
}

#[wasm_bindgen_test]
fn cursor_movers_do_not_trap() {
    // Drive the movers past both ends and across a selection — none may trap, and
    // the offsets stay sane (clamped to the buffer).
    let mut ed = Editor::new("ab😀c"); // 😀 = 4 bytes at 2..6, len 7
    ed.set_caret(0).unwrap();

    // Move right past the end: a(1), b(2), 😀(6), c(7), then clamp at 7.
    ed.move_right();
    assert_eq!(ed.cursor_head(), 1);
    ed.move_right();
    ed.move_right();
    assert_eq!(ed.cursor_head(), 6); // skipped the emoji as one grapheme
    ed.move_right();
    assert_eq!(ed.cursor_head(), 7);
    ed.move_right(); // past the end: no-op, no trap
    assert_eq!(ed.cursor_head(), 7);

    // Move left past the start: clamps at 0.
    for _ in 0..10 {
        ed.move_left();
    }
    assert_eq!(ed.cursor_head(), 0);

    // Extend right by one grapheme: from caret 0 the head advances to byte 1
    // (a no-op regression would now be caught — the old `<= 7` bound passed even
    // if the mover did nothing). Anchor stays pinned at 0.
    ed.extend_right();
    assert_eq!(ed.cursor_head(), 1);
    assert_eq!(ed.cursor_anchor(), 0);
    // Word-step stays in range and never traps.
    ed.extend_word_right();
    assert!(ed.cursor_head() >= 1 && ed.cursor_head() <= 7);
    ed.collapse_selection();
    assert_eq!(ed.cursor_anchor(), ed.cursor_head());
}

#[wasm_bindgen_test]
fn set_selection_validates_both_ends_and_keeps_instance_usable() {
    // M-2 audit gap: set_selection (the only multi-validation setter) had no wasm
    // test. "😀" is bytes 0..4 (utf16 len 2); byte 1 is mid-codepoint.
    let mut ed = Editor::new("😀");
    // A mid-codepoint head is rejected without poisoning the instance.
    assert!(ed.set_selection(0, 1).is_err());
    // Both ends on char boundaries: accepted, order preserved.
    assert!(ed.set_selection(0, 4).is_ok());
    assert_eq!(ed.cursor_anchor(), 0);
    assert_eq!(ed.cursor_head(), 4);
}

#[wasm_bindgen_test]
fn utf16_byte_conversion_roundtrips() {
    // "café😀": é=2 bytes (3..5), 😀=4 bytes (5..9). UTF-16: c,a,f at 0..3, é at 3,
    // 😀 is a surrogate pair (4,5). Byte 9 (== len) → utf16 6.
    let ed = Editor::new("café😀");
    for &b in &[0usize, 1, 2, 3, 5, 9] {
        let u = ed.byte_to_utf16(b).unwrap();
        assert_eq!(ed.utf16_to_byte(u).unwrap(), b, "byte {b}");
    }
    // The astral char costs two UTF-16 code units.
    assert_eq!(ed.byte_to_utf16(9).unwrap(), 6);
    assert_eq!(ed.utf16_to_byte(6).unwrap(), 9);

    // Bad UTF-16 offset: mid-surrogate (5 is between the high/low surrogate of 😀)
    // → Err, NOT a trap. utf16 length is 6, so 5 is in bounds but mid-pair.
    assert!(ed.utf16_to_byte(5).is_err());
    // Out-of-range UTF-16 offset → Err.
    assert!(ed.utf16_to_byte(99).is_err());
    // Out-of-range / mid-codepoint byte offset → Err on the inverse too.
    assert!(ed.byte_to_utf16(4).is_err()); // mid-'é'
    assert!(ed.byte_to_utf16(99).is_err());
}

#[wasm_bindgen_test]
fn move_after_edit_that_shifts_multibyte_under_caret_does_not_trap() {
    // H2a Critical, now locked at the JS boundary Phase H actually guards: an edit
    // that shifts a multibyte char under a previously-valid caret used to leave the
    // internal caret mid-codepoint, so the next mover trapped inside GraphemeCursor.
    // The core fix snaps the caret DOWN to a char boundary after every edit.
    //
    // "ab😀": a=0..1, b=1..2, 😀=2..6.
    let mut ed = Editor::new("ab😀");
    ed.set_caret(2).unwrap(); // valid boundary, just before 😀
    ed.delete(0, 1).unwrap(); // remove 'a' → "b😀"; 😀 now 1..5; internal caret 2
                              // was mid-😀 → core clamps it DOWN to byte 1.
    ed.move_right(); // MUST NOT trap: from byte 1 the mover skips the whole emoji.
    assert_eq!(ed.text(), "b😀");
    // "b😀": b=0..1, 😀=1..5 — move_right from byte 1 lands at byte 5 (after 😀).
    assert_eq!(ed.cursor_head(), 5);
}

#[wasm_bindgen_test]
fn backspace_on_empty_and_single_multibyte_does_not_trap() {
    // Backspace at the very start of an empty buffer is a no-op, not a trap.
    let mut empty = Editor::new("");
    assert!(!empty.backspace());
    assert_eq!(empty.text(), "");

    // Single astral char: edits at its boundaries must not feed a mid-codepoint
    // offset to the grapheme primitives.
    let mut emoji = Editor::new("😀"); // 😀 = 0..4
    emoji.set_caret(4).unwrap(); // valid boundary at the end
    assert!(emoji.backspace()); // removes the whole grapheme, no trap
    assert_eq!(emoji.text(), "");

    let mut emoji2 = Editor::new("😀");
    emoji2.set_caret(0).unwrap(); // valid boundary at the start
    assert!(emoji2.delete_forward()); // forward-delete the whole grapheme, no trap
    assert_eq!(emoji2.text(), "");
}

#[wasm_bindgen_test]
fn set_caret_mid_codepoint_returns_err() {
    // "😀" occupies bytes 0..4; byte 1 is mid-codepoint.
    let mut ed = Editor::new("😀");
    assert!(ed.set_caret(1).is_err());
    // The instance is still usable: a valid caret + edit work afterwards.
    assert!(ed.set_caret(0).is_ok());
    ed.insert_at_cursor("x");
    assert_eq!(ed.text(), "x😀");
    // A valid caret at the end is fine.
    assert!(ed.set_caret(ed.text().len()).is_ok());
}

#[wasm_bindgen_test]
fn line_count_crosses_the_boundary() {
    assert_eq!(Editor::new("").line_count(), 1);
    assert_eq!(Editor::new("a\nb\nc").line_count(), 3);
    assert_eq!(Editor::new("a\n").line_count(), 2);
}

#[wasm_bindgen_test]
fn visible_lines_windows_the_viewport_across_the_boundary() {
    // 10 lines, line_height 20, viewport 60 tall, scrolled to the top → [0, 3).
    let ed = Editor::new("0\n1\n2\n3\n4\n5\n6\n7\n8\n9");
    assert_eq!(ed.visible_lines(0.0, 60.0, 20.0), vec![0, 3]);
    // Scrolled to 50px (mid line 2) → lines 2..6.
    assert_eq!(ed.visible_lines(50.0, 60.0, 20.0), vec![2, 6]);
}

#[wasm_bindgen_test]
fn tokens_in_line_returns_line_local_utf16_triples() {
    // "Hello {{ x }}": leading Text "Hello " (kind 0) then the Variable block
    // (kind 1) at bytes 6..13. tokens_in_line reports every kind, as tokens() does;
    // the view skips kind 0 (plain text needs no highlight).
    let ed = Editor::new("Hello {{ x }}");
    assert_eq!(ed.tokens_in_line(0), vec![0, 6, 0, 6, 13, 1]);
}

#[wasm_bindgen_test]
fn tokens_in_line_offsets_are_relative_to_the_line_not_the_document() {
    // Line 1 holds the block; its local offsets start at 0, not the global byte 2.
    let ed = Editor::new("a\n{{ x }}");
    assert_eq!(ed.tokens_in_line(0), vec![0, 1, 0]); // Text "a"
    assert_eq!(ed.tokens_in_line(1), vec![0, 7, 1]); // Variable, line-local
}

#[wasm_bindgen_test]
fn tokens_in_line_uses_utf16_columns_for_multibyte_lines() {
    // "😀{{x}}": 😀 is Text and 2 UTF-16 units, so the block starts at local column 2.
    let ed = Editor::new("😀{{x}}");
    assert_eq!(ed.tokens_in_line(0), vec![0, 2, 0, 2, 7, 1]);
}

#[wasm_bindgen_test]
fn tokens_in_line_clips_a_multiline_block_to_each_line() {
    // A comment spanning two lines is clipped (newline excluded) on each line.
    let ed = Editor::new("{# a\nb #}");
    assert_eq!(ed.tokens_in_line(0), vec![0, 4, 3]); // "{# a"
    assert_eq!(ed.tokens_in_line(1), vec![0, 4, 3]); // "b #}"
}

#[wasm_bindgen_test]
fn tokens_in_line_past_the_end_is_empty() {
    let ed = Editor::new("abc");
    assert_eq!(ed.tokens_in_line(5), Vec::<u32>::new());
}

#[wasm_bindgen_test]
fn tokens_in_line_on_an_empty_line_is_empty() {
    // "a\n" → line 1 is empty (no content, no tokens), and must not underflow.
    let ed = Editor::new("a\n");
    assert_eq!(ed.tokens_in_line(1), Vec::<u32>::new());
}

#[wasm_bindgen_test]
fn line_text_returns_a_line_without_its_break() {
    let ed = Editor::new("ab\n日本\n");
    assert_eq!(ed.line_text(0), "ab");
    assert_eq!(ed.line_text(1), "日本");
    assert_eq!(ed.line_text(2), ""); // trailing empty line
    assert_eq!(ed.line_text(9), ""); // out of range
}

#[wasm_bindgen_test]
fn caret_xy_is_the_head_grid_position_in_pixels() {
    // advance 8, line_height 20. Caret at byte 2 of "abc" → line 0, col 2 → (16, 0).
    let mut ed = Editor::new("abc");
    ed.set_caret(2).unwrap();
    assert_eq!(ed.caret_xy(8.0, 20.0), vec![16.0, 0.0]);
}

#[wasm_bindgen_test]
fn caret_xy_accounts_for_lines_and_graphemes() {
    // "ab\ncd": caret at byte 4 ('d') → line 1, col 1 → (8, 20).
    let mut ed = Editor::new("ab\ncd");
    ed.set_caret(4).unwrap();
    assert_eq!(ed.caret_xy(8.0, 20.0), vec![8.0, 20.0]);

    // "😀x": caret after the emoji (byte 4) → line 0, col 1 (one grapheme) → (8, 0).
    let mut emoji = Editor::new("😀x");
    emoji.set_caret(4).unwrap();
    assert_eq!(emoji.caret_xy(8.0, 20.0), vec![8.0, 0.0]);
}

#[wasm_bindgen_test]
fn selection_in_line_is_empty_when_collapsed() {
    let mut ed = Editor::new("abc");
    ed.set_caret(1).unwrap();
    assert_eq!(ed.selection_in_line(0), Vec::<u32>::new());
}

#[wasm_bindgen_test]
fn selection_in_line_clips_to_the_line_in_utf16() {
    let mut ed = Editor::new("abcdef");
    ed.set_selection(1, 4).unwrap();
    assert_eq!(ed.selection_in_line(0), vec![1, 4]);
}

#[wasm_bindgen_test]
fn selection_in_line_spans_multiple_lines() {
    // "ab\ncd": select bytes 1..5 → line 0 gets [1,2), line 1 gets [0,2).
    let mut ed = Editor::new("ab\ncd");
    ed.set_selection(1, 5).unwrap();
    assert_eq!(ed.selection_in_line(0), vec![1, 2]);
    assert_eq!(ed.selection_in_line(1), vec![0, 2]);
}

#[wasm_bindgen_test]
fn selection_in_line_uses_utf16_columns() {
    // "😀x": select all (bytes 0..5) → line-local UTF-16 [0, 3] (😀 is 2 units).
    let mut ed = Editor::new("😀x");
    ed.set_selection(0, 5).unwrap();
    assert_eq!(ed.selection_in_line(0), vec![0, 3]);
}
