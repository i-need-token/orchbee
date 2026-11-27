use super::Progress;
use serde_json::Value;
use std::{
    cell::RefCell,
    io::{self, Write},
    rc::Rc,
};

/// The records a run wrote, as the Session Server receives them on the helper's standard error.
#[derive(Clone, Default)]
struct Records(Rc<RefCell<Vec<u8>>>);

impl Write for Records {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.borrow_mut().extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// A run that downloads its runtime archive and then the window archive it expected reports them
/// as one download of the release it brings, whose total never changes and which never runs back
/// or past it, and reports its install once, after the last transfer: the records the Session
/// Server accepts.
#[test]
fn a_run_reports_one_download_and_then_its_install() {
    let records = Records::default();
    let mut progress = Progress::default();
    progress.report_to(Box::new(records.clone()));
    progress.transfer("0.1.0-beta.26", 600, 400);
    progress.received(250);
    progress.received(600);
    progress.install("0.1.0-beta.26", 400);
    // The window archive may be of an older window version; the run still brings its release.
    progress.transfer("0.1.0-beta.25", 400, 0);
    progress.received(400);
    progress.install("0.1.0-beta.25", 0);

    let bytes = records.0.borrow().clone();
    let records: Vec<Value> = String::from_utf8(bytes)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    let (install, downloads) = records.split_last().unwrap();
    assert_eq!(install["phase"], "install", "{records:?}");
    let mut last = 0;
    for record in records.iter() {
        assert_eq!(record["schema"], "orchbee.update.progress");
        assert_eq!(record["version"], 1);
        assert_eq!(record["releaseVersion"], "0.1.0-beta.26");
    }
    for record in downloads {
        assert_eq!(record["phase"], "download", "{records:?}");
        assert_eq!(record["totalBytes"], 1000);
        let received = record["receivedBytes"].as_u64().unwrap();
        assert!(last <= received && received <= 1000, "{records:?}");
        last = received;
    }
    assert_eq!(last, 1000, "the last transfer's end is reported");
}
