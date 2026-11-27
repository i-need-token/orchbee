//! Paging through the ring: cursors survive drops, pages never split a character and stay inside a
//! result once escaped, and the pages of a long text join into that text.
use super::*;

#[test]
fn pages_join_into_the_text_and_a_dropped_cursor_restarts_at_the_oldest_text() {
    let mut output = GitOutput::new();
    output.command(["status", "-z"], Duration::from_millis(12), false);
    output.command(["push", "origin"], Duration::from_millis(3), true);
    assert_eq!(
        output.read(None).text,
        "> git status -z [12ms]\n> git push origin [3ms] (cancelled)\n"
    );

    // Control characters escape to six bytes, so a page of them holds a sixth of the escaped bound.
    let text = "é\u{1}x".repeat(CAPACITY / 4);
    let mut output = GitOutput::new();
    output.write(text.as_bytes());
    let mut cursor = None;
    let mut joined = String::new();
    loop {
        let page = output.read(cursor);
        assert_eq!(page.dropped, 0);
        let escaped = page.text.chars().map(escaped_width).sum::<usize>();
        assert!(escaped <= PAGE_ESCAPED, "{escaped}");
        if page.text.is_empty() {
            break;
        }
        joined.push_str(&page.text);
        cursor = Some(page.cursor);
    }
    assert_eq!(joined, text);

    // Writing past the capacity drops the oldest bytes, here half of the first `é`.
    output.write(&vec![b'z'; CAPACITY - text.len() + 1]);
    let page = output.read(Some(0));
    assert_eq!(page.dropped, 2);
    assert!(page.text.starts_with("\u{1}xé"), "{:?}", &page.text[..8]);
    let end = output.read(Some(u64::MAX));
    assert_eq!((end.text.as_str(), end.dropped), ("", 0));
    assert_eq!(end.cursor, CAPACITY as u64 + 1);
}
