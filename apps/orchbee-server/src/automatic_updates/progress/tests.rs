use super::ProgressLines;
use crate::update_job::{Stage, UpdateJob};

fn download(received: u64, total: u64) -> String {
    format!(
        r#"{{"schema":"orchbee.update.progress","version":1,"phase":"download","releaseVersion":"0.1.0-beta.26","receivedBytes":{received},"totalBytes":{total}}}"#
    )
}

/// A record split across reads still counts, and every line that is no record within the bounds a
/// Client reads leaves the stage where it was: a Server never answers what a Client would refuse.
#[test]
fn only_whole_records_within_bounds_move_the_stage() {
    let job = UpdateJob::default();
    job.begin();
    let mut lines = ProgressLines::new(&job);
    let record = download(5, 10) + "\n";
    let (first, second) = record.split_at(40);
    lines.feed(first.as_bytes());
    assert_eq!(job.stage(), Some(Stage::Checking));
    lines.feed(second.as_bytes());
    let five_of_ten = Some(Stage::Downloading {
        version: "0.1.0-beta.26".into(),
        received_bytes: 5,
        total_bytes: 10,
    });
    assert_eq!(job.stage(), five_of_ten);
    for skipped in [
        download(11, 10),
        download(0, 0),
        download(0, 9_007_199_254_740_992),
        download(5, 10).replace("0.1.0-beta.26", "0.1.0 beta"),
        download(5, 10).replace("0.1.0-beta.26", ""),
        download(5, 10).replace(r#""version":1"#, r#""version":2"#),
        download(5, 10).replace("orchbee.update.progress", "orchbee.update"),
        download(5, 10).replace("download", "verify"),
        "not json".into(),
        format!("{}{}", " ".repeat(1024), download(6, 10)),
    ] {
        lines.feed(format!("{skipped}\n").as_bytes());
        assert_eq!(job.stage(), five_of_ten, "{skipped}");
    }
    lines.feed(
        br#"{"schema":"orchbee.update.progress","version":1,"phase":"install","releaseVersion":"0.1.0-beta.26"}
"#,
    );
    assert_eq!(
        job.stage(),
        Some(Stage::Installing {
            version: "0.1.0-beta.26".into()
        })
    );
}
