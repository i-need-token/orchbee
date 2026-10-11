use serde_json::json;

use super::{Bounded, LIMIT, TooLarge, encode_document, encode_text, render_text};

#[test]
fn encodes_bounded_documents_with_a_trailing_newline() {
    let encoded = encode_document(&json!({"a": 1})).expect("small document");
    assert_eq!(encoded, b"{\n  \"a\": 1\n}\n");
}

#[test]
fn rejects_documents_beyond_the_bound() {
    let document = json!({"blob": "x".repeat(LIMIT)});
    assert_eq!(encode_document(&document), Err(TooLarge));
}

#[test]
fn boundary_documents_succeed() {
    // Find a payload that lands exactly inside the bound.
    let base = "{\n  \"blob\": \"\"\n}\n".len();
    let inside = LIMIT - base;
    let document = json!({"blob": "x".repeat(inside)});
    let encoded = encode_document(&document).expect("document inside bound");
    assert_eq!(encoded.len(), LIMIT);
}

#[test]
fn text_at_the_bound_succeeds() {
    let body = "x".repeat(LIMIT);
    let encoded = encode_text(&body).expect("text at the bound");
    assert_eq!(encoded.len(), LIMIT);
}

#[test]
fn text_beyond_the_bound_is_rejected_before_any_bytes() {
    let body = "x".repeat(LIMIT + 1);
    assert_eq!(encode_text(&body), Err(TooLarge));
}

#[test]
fn renderer_aborts_at_the_bound_during_construction() {
    // The sink never holds more than the bound, and a renderer that keeps
    // writing fails on the piece that would cross it — the oversized body
    // is never assembled.
    let mut bounded = Bounded::new(LIMIT);
    bounded.push(&"x".repeat(LIMIT - 8)).expect("under bound");
    assert_eq!(bounded.push(&"y".repeat(9)), Err(TooLarge));
    // A failed push appends nothing: the remaining capacity is intact.
    bounded
        .push(&"z".repeat(8))
        .expect("remaining capacity usable");
    assert_eq!(bounded.push("!"), Err(TooLarge), "no bytes fit past it");
    assert_eq!(bounded.finish().len(), LIMIT);

    let rendered = render_text(|out| {
        out.push(&"x".repeat(LIMIT))?;
        out.push("!")
    });
    assert_eq!(rendered, Err(TooLarge));
}

#[test]
fn renderer_at_exactly_the_bound_succeeds() {
    let rendered = render_text(|out| out.push(&"x".repeat(LIMIT))).expect("exactly at the bound");
    assert_eq!(rendered.len(), LIMIT);
}
