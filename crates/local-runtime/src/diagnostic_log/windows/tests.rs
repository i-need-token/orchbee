// Retention and the size cap are the folder's bound (ORC-397 accepted choice 6); these tests prove
// the rules on real files, through a day change and past the limit.
use super::super::{DAILY_LOG_LIMIT_BYTES, Stamp, days_from_civil, encode, file_name};
use super::{Admission, admission, append_in, ends_capped, expired, parse_file_name, prune};
use std::{fs, path::PathBuf};

struct Folder(PathBuf);
impl Folder {
    fn new(name: &str) -> Self {
        let path = crate::test_temporary_directory().join(format!(
            "orchbee-diagnostic-log-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir(&path).expect("create the test folder");
        Self(path)
    }
    fn names(&self) -> Vec<String> {
        let mut names: Vec<_> = fs::read_dir(&self.0)
            .expect("list the test folder")
            .map(|entry| {
                entry
                    .expect("entry")
                    .file_name()
                    .into_string()
                    .expect("name")
            })
            .collect();
        names.sort();
        names
    }
}
impl Drop for Folder {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn stamp(day: i64) -> Stamp {
    Stamp {
        unix_milliseconds: u64::try_from(day).expect("a day after 1970") * 86_400_000,
        day,
    }
}

fn record(day: i64, event: &'static str) -> Vec<u8> {
    encode(stamp(day).unix_milliseconds, "info", "server", event, &[])
}

fn capped(day: i64) -> Vec<u8> {
    encode(
        stamp(day).unix_milliseconds,
        "warning",
        "server",
        "log-capped",
        &[],
    )
}

#[test]
fn a_day_keeps_itself_and_the_six_days_before_it() {
    let today = days_from_civil(2026, 3, 3);
    assert!(!expired(&file_name(today), today));
    assert!(!expired(&file_name(today - 6), today));
    assert!(expired(&file_name(today - 7), today));
    assert!(!expired(&file_name(today + 1), today));
    for name in [
        "destructive-commands.jsonl",
        "destructive-commands.1.jsonl",
        "2026-02-30.jsonl",
        "2026-1-01.jsonl",
        "notes.txt",
    ] {
        assert!(!expired(name, today), "{name}");
    }
    assert_eq!(
        parse_file_name("2024-02-29.jsonl"),
        Some(days_from_civil(2024, 2, 29))
    );
}

#[test]
fn opening_a_new_day_removes_only_expired_daily_files() {
    let folder = Folder::new("retention");
    let today = days_from_civil(2003, 5, 20);
    for name in [
        file_name(today - 8),
        file_name(today - 7),
        file_name(today - 6),
        file_name(today - 1),
        "destructive-commands.jsonl".to_owned(),
        "destructive-commands.1.jsonl".to_owned(),
    ] {
        fs::write(folder.0.join(name), b"").expect("seed a file");
    }
    append_in(
        &folder.0,
        &record(today, "started"),
        &capped(today),
        stamp(today),
    )
    .expect("append");
    assert_eq!(
        folder.names(),
        [
            file_name(today - 6),
            file_name(today - 1),
            file_name(today),
            "destructive-commands.1.jsonl".to_owned(),
            "destructive-commands.jsonl".to_owned(),
        ]
    );
    prune(&folder.0, today + 7);
    assert_eq!(
        folder.names(),
        [
            "destructive-commands.1.jsonl".to_owned(),
            "destructive-commands.jsonl".to_owned(),
        ]
    );
}

#[test]
fn records_after_midnight_go_to_the_next_days_file() {
    let folder = Folder::new("day-change");
    let first = days_from_civil(2004, 7, 9);
    for day in [first, first, first + 1] {
        append_in(&folder.0, &record(day, "started"), &capped(day), stamp(day)).expect("append");
    }
    assert_eq!(folder.names(), [file_name(first), file_name(first + 1)]);
    let earlier = fs::read(folder.0.join(file_name(first))).expect("read");
    assert_eq!(
        earlier,
        [record(first, "started"), record(first, "started")].concat()
    );
}

#[test]
fn a_full_day_ends_with_one_capped_record_inside_the_limit() {
    assert_eq!(admission(0, false, 200), Admission::Append);
    assert_eq!(
        admission(DAILY_LOG_LIMIT_BYTES - 456, false, 200),
        Admission::Append
    );
    assert_eq!(
        admission(DAILY_LOG_LIMIT_BYTES - 455, false, 200),
        Admission::Cap
    );
    assert_eq!(admission(10, true, 1), Admission::Closed);

    let folder = Folder::new("cap");
    let day = days_from_civil(2005, 1, 2);
    let path = folder.0.join(file_name(day));
    let line = record(day, "started");
    let filler = DAILY_LOG_LIMIT_BYTES as usize - 300;
    let mut seeded = vec![b'x'; filler - 1];
    seeded.push(b'\n');
    fs::write(&path, &seeded).expect("seed a nearly full day");
    for _ in 0..3 {
        append_in(&folder.0, &line, &capped(day), stamp(day)).expect("append");
    }
    let bytes = fs::read(&path).expect("read");
    assert!(bytes.len() as u64 <= DAILY_LOG_LIMIT_BYTES);
    assert_eq!(&bytes[filler..], capped(day).as_slice());
    assert!(ends_capped(&bytes[filler - 1..]));
}
