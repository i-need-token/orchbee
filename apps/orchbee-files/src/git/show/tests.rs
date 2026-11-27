//! The content `git.show` keeps: content a read holds stays, content no read holds goes 3 minutes
//! after its last read ended or when the bound needs its room, and content that cannot fit is
//! served without being kept, so what is kept never exceeds the bound.

use super::{KEPT_FOR, Key, Shows};
use std::time::{Duration, Instant};

fn key(blob: &str) -> Key {
    Key {
        repository: "repository".to_owned(),
        path: "file".to_owned(),
        blob: blob.to_owned(),
    }
}

#[test]
fn kept_content_goes_after_its_stay_or_for_room_but_never_while_read() {
    let mut shows = Shows::with_capacity(10);
    let kept = |shows: &Shows, blob: &str| shows.kept.contains_key(&key(blob));
    let held = shows.insert(key("held"), vec![0; 4]);
    let before = Instant::now();
    drop(shows.insert(key("read"), vec![0; 4]));
    shows.sweep(before + KEPT_FOR - Duration::from_secs(1));
    assert!(kept(&shows, "held") && kept(&shows, "read"));

    // Only the content no read holds can make room.
    drop(shows.insert(key("room"), vec![0; 4]));
    assert!(kept(&shows, "held") && !kept(&shows, "read") && kept(&shows, "room"));
    let large = shows.insert(key("large"), vec![0; 8]);
    assert_eq!((*large).as_ref().len(), 8);
    assert!(!kept(&shows, "large") && kept(&shows, "room"));
    assert_eq!(shows.bytes, 8);

    let after = Instant::now();
    shows.sweep(after + KEPT_FOR);
    assert!(kept(&shows, "held") && !kept(&shows, "room"));
    drop(held);
    shows.sweep(Instant::now() + KEPT_FOR);
    assert!(shows.kept.is_empty());
    assert_eq!(shows.bytes, 0);
}
