//! WASM-side integration test for the `Editor` binding and the flat token wire.
use vellum_wasm::Editor;
use wasm_bindgen_test::*;

#[wasm_bindgen_test]
fn insert_then_tokens_roundtrip() {
    let mut ed = Editor::new("a ");
    ed.insert(2, "{{ x }}");
    assert_eq!(ed.text(), "a {{ x }}");
    let t = ed.tokens(); // [start, end, kind, ...] => [0,2,0, 2,9,1]
    assert_eq!(t, vec![0, 2, 0, 2, 9, 1]);
}
