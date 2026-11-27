//! The helper's progress: while it runs, a helper that reports progress writes one
//! `orchbee.update.progress` record per line on standard error, each a JSON object of `schema`,
//! `version` 1, `phase` and `releaseVersion`, a `download` record adding `receivedBytes` and
//! `totalBytes`. Like its document, the helper's standard error is untrusted output: only those
//! members are read, and a line longer than [`MAXIMUM_LINE_BYTES`], a record outside the
//! bounds a Client reads, or anything else on the stream is skipped. A helper that reports nothing,
//! such as one of a release before progress, leaves its run at Checking until it ends.
use crate::update_job::{Stage, UpdateJob};
use serde_json::Value;

/// The longest line read as a record; a record is far shorter.
const MAXIMUM_LINE_BYTES: usize = 1024;
/// The largest byte count every JSON binding reads exactly.
const MAXIMUM_BYTES: u64 = 9_007_199_254_740_991;
/// The longest release version a Client reads, as a handshake's `productVersion`.
const MAXIMUM_VERSION_BYTES: usize = 64;

/// Splits the helper's standard error into lines as it arrives and moves the running run's stage
/// on each record.
pub(super) struct ProgressLines<'a> {
    job: &'a UpdateJob,
    line: Vec<u8>,
    /// The current line passed [`MAXIMUM_LINE_BYTES`]; the rest of it is skipped.
    overlong: bool,
}

impl<'a> ProgressLines<'a> {
    pub(super) const fn new(job: &'a UpdateJob) -> Self {
        Self {
            job,
            line: Vec::new(),
            overlong: false,
        }
    }

    /// Takes the next bytes of standard error.
    pub(super) fn feed(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            if byte == b'\n' {
                if !self.overlong
                    && let Some(stage) = record(&self.line)
                {
                    self.job.advance(stage);
                }
                self.line.clear();
                self.overlong = false;
            } else if !self.overlong {
                if self.line.len() < MAXIMUM_LINE_BYTES {
                    self.line.push(byte);
                } else {
                    self.line.clear();
                    self.overlong = true;
                }
            }
        }
    }
}

/// The stage one line reports, if it is a progress record within bounds.
fn record(line: &[u8]) -> Option<Stage> {
    let value = serde_json::from_slice::<Value>(line).ok()?;
    if value["schema"].as_str() != Some("orchbee.update.progress") || value["version"] != 1 {
        return None;
    }
    let version = value["releaseVersion"]
        .as_str()
        .filter(|version| {
            (1..=MAXIMUM_VERSION_BYTES).contains(&version.len())
                && version.bytes().all(|byte| byte.is_ascii_graphic())
        })?
        .to_owned();
    match value["phase"].as_str()? {
        "download" => {
            let received_bytes = value["receivedBytes"].as_u64()?;
            let total_bytes = value["totalBytes"]
                .as_u64()
                .filter(|total| (1..=MAXIMUM_BYTES).contains(total))?;
            (received_bytes <= total_bytes).then_some(Stage::Downloading {
                version,
                received_bytes,
                total_bytes,
            })
        }
        "install" => Some(Stage::Installing { version }),
        _ => None,
    }
}

#[cfg(test)]
mod tests;
