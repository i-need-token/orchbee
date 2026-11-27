//! The automatic helper's progress, which the Session Server follows while the helper runs so that
//! a Client's Check for updates shows the download and the install. Only the helper reports; every
//! other run keeps its standard error as it was. Each record is one line of JSON on standard error:
//! `{"schema":"orchbee.update.progress","version":1,"phase":"download","releaseVersion":…,`
//! `"receivedBytes":…,"totalBytes":…}` while an archive transfers, which counts every archive of
//! the run as one download, the window archive the runtime part expects after it included, and
//! `{"schema":"orchbee.update.progress","version":1,"phase":"install","releaseVersion":…}` once
//! nothing more downloads. Every record names the version the run's first part named, the release
//! the run brings. A download record follows the one before it by at least [`INTERVAL`] unless a
//! transfer starts or ends. A failed write is ignored: progress never changes a run.
use serde_json::{Value, json};
use std::{
    fs::File,
    io::Write,
    time::{Duration, Instant},
};

/// The shortest time between two download records of one transfer, and between two reads of the
/// transfer's file length.
const INTERVAL: Duration = Duration::from_millis(250);

/// What the run reports, and where its download stands.
#[derive(Default)]
pub(super) struct Progress {
    /// Where the records go: the automatic helper's standard error; none for every other run.
    sink: Option<Box<dyn Write>>,
    /// The version every record names, once a part named one.
    version: Option<String>,
    download: Option<Download>,
    written: Option<Instant>,
    polled: Option<Instant>,
}

/// The run's download: every archive it transfers, counted as one.
struct Download {
    /// The bytes of the run's finished transfers.
    finished: u64,
    /// The current transfer's signed size, and the bytes it has received.
    size: u64,
    received: u64,
    /// Every byte the run downloads: its finished transfers, the current one and those it
    /// expects later.
    total: u64,
}

impl Progress {
    /// The run reports its progress to `sink`, as the automatic helper does to standard error.
    pub(super) fn report_to(&mut self, sink: Box<dyn Write>) {
        self.sink = Some(sink);
    }

    pub(super) const fn reporting(&self) -> bool {
        self.sink.is_some()
    }

    /// A part's archive transfer of `size` bytes of `version` starts, and the run expects `later`
    /// bytes of later parts' archives after it.
    pub(super) fn transfer(&mut self, version: &str, size: u64, later: u64) {
        if !self.reporting() {
            return;
        }
        self.version.get_or_insert_with(|| version.to_owned());
        let finished = self.download.as_ref().map_or(0, |download| {
            download.finished.saturating_add(download.size)
        });
        self.download = Some(Download {
            finished,
            size,
            received: 0,
            total: finished.saturating_add(size).saturating_add(later).max(1),
        });
        self.write_download();
    }

    /// The current transfer has received `received` bytes.
    pub(super) fn received(&mut self, received: u64) {
        let reporting = self.reporting();
        let Some(download) = self.download.as_mut().filter(|_| reporting) else {
            return;
        };
        let received = received.min(download.size);
        if received == download.received {
            return;
        }
        download.received = received;
        if received == download.size || self.written.is_none_or(|at| at.elapsed() >= INTERVAL) {
            self.write_download();
        }
    }

    /// Reads how far the current transfer's output `file`, which another program writes, has
    /// grown, at most once per [`INTERVAL`].
    pub(super) fn poll(&mut self, file: &File) {
        if !self.reporting()
            || self.download.is_none()
            || self.polled.is_some_and(|at| at.elapsed() < INTERVAL)
        {
            return;
        }
        self.polled = Some(Instant::now());
        if let Ok(metadata) = file.metadata() {
            self.received(metadata.len());
        }
    }

    /// A part installs `version`. While the run expects `later` bytes of later parts' archives it
    /// still downloads, so only an install after which nothing more downloads is reported.
    pub(super) fn install(&mut self, version: &str, later: u64) {
        if !self.reporting() {
            return;
        }
        let version = self.version.get_or_insert_with(|| version.to_owned());
        if later == 0 {
            let record = json!({
                "schema": "orchbee.update.progress", "version": 1, "phase": "install",
                "releaseVersion": version,
            });
            self.write(&record);
        }
    }

    fn write_download(&mut self) {
        let (Some(download), Some(version)) = (&self.download, &self.version) else {
            return;
        };
        let record = json!({
            "schema": "orchbee.update.progress", "version": 1, "phase": "download",
            "releaseVersion": version,
            "receivedBytes": download.finished.saturating_add(download.received).min(download.total),
            "totalBytes": download.total,
        });
        self.write(&record);
    }

    fn write(&mut self, record: &Value) {
        self.written = Some(Instant::now());
        let mut line = record.to_string();
        line.push('\n');
        if let Some(sink) = self.sink.as_mut() {
            // A Server that ended or reads nothing changes nothing about the run.
            let _ignored = sink.write_all(line.as_bytes()).and_then(|()| sink.flush());
        }
    }
}

#[cfg(test)]
mod tests;
