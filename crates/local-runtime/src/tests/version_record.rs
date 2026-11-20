use super::*;
use crate::data_root::read_record_with;

const WHOLE: &[u8] = b"{\"layout\":1,\"newestVersion\":\"0.1.0\"}\n";

// Desktop Main creates `internal/version.json` exclusively and fills it in place, so a reader can
// find it empty for a moment. Here the writer's write lands between the reader's two reads.
#[test]
fn a_record_a_writer_is_still_filling_is_read_again_until_it_is_whole() {
    let local = Fixture::new();
    let path = local.0.join("version.json");
    write_private(&path, b"").unwrap();
    let mut pauses = 0;
    let read = read_record_with(&path, |_| {
        pauses += 1;
        write_private(&path, WHOLE).unwrap();
    })
    .unwrap();
    assert_eq!(read.as_deref(), Some(WHOLE));
    assert_eq!(pauses, 1);
}

// A record that stays cut short is re-read for about one second, then stands as read, so its caller
// refuses it as before.
#[test]
fn a_record_still_cut_short_after_the_window_stands_for_refusal() {
    let local = Fixture::new();
    let path = local.0.join("version.json");
    let cut_short = br#"{"layout":1,"newest"#;
    write_private(&path, &cut_short[..]).unwrap();
    let mut waited = Duration::ZERO;
    let read = read_record_with(&path, |pause| waited += pause).unwrap();
    assert_eq!(read.as_deref(), Some(&cut_short[..]));
    assert!(
        waited >= Duration::from_millis(900) && waited <= Duration::from_millis(1100),
        "waited {waited:?}"
    );
}

// Any other content is judged at once: a whole record of another layout, or bytes that are no
// prefix of a record, is never read again.
#[test]
fn a_record_that_is_whole_or_no_prefix_of_one_is_read_once() {
    let local = Fixture::new();
    let path = local.0.join("version.json");
    for bytes in [
        &br#"{"layout":2,"newestVersion":"9.0.0"}"#[..],
        &b"not a record"[..],
    ] {
        write_private(&path, bytes).unwrap();
        let mut pauses = 0;
        let read = read_record_with(&path, |_| pauses += 1).unwrap();
        assert_eq!(read.as_deref(), Some(bytes));
        assert_eq!(pauses, 0);
    }
}
