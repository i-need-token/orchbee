use std::{panic, thread};

use orchbee_local_runtime::LogValue;

use super::{capture, install_panic_hook};

#[test]
fn a_thread_panic_writes_one_site_record_without_its_message() {
    install_panic_hook();
    let (expected_line, records) = thread::Builder::new()
        .name("orchbee-local-client".to_owned())
        .spawn(|| {
            capture::records(|| {
                let line = line!() + 2;
                let unwound = panic::catch_unwind(|| {
                    panic!("private-panic-payload");
                });
                assert!(unwound.is_err());
                line
            })
        })
        .unwrap()
        .join()
        .unwrap();
    assert_eq!(records.len(), 1, "{records:?}");
    assert_eq!(records[0].event, "panicked");
    assert_eq!(
        records[0].fields,
        [
            ("thread", LogValue::Token("orchbee-local-client")),
            ("file", LogValue::Token("tests")),
            ("line", LogValue::Count(u64::from(expected_line))),
            ("suppressed", LogValue::Count(0)),
        ]
    );
}
