//! WASM-side integration test for the `Editor` binding and the flat token wire.
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
