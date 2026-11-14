//! Bounded explicit release acquisition. Every subprocess belongs to the invocation's Job.
use super::{
    Context, Error, manifest, platform,
    signature::{DOCUMENT_LIMIT, FEED_LIMIT, MANIFEST_LIMIT},
    source_path::LocalPath,
};
use orchbee_local_runtime::{CommandFailure, UpdateChannel};
use std::{
    fs::File,
    io::{self, Read, Write},
    path::Path,
    process::Command,
    time::{Duration, Instant},
};

/// The signature document lives beside the signed document under this suffix.
const SIGNATURE_SUFFIX: &str = ".sig";
/// curl's exit status when a transfer exceeds `--max-filesize`, which is the signed size.
/// <https://curl.se/libcurl/c/libcurl-errors.html#CURLEFILESIZEEXCEEDED>
const CURL_FILESIZE_EXCEEDED: i32 = 63;
/// curl's exit status when `--max-time`, `--connect-timeout` or the `--speed-limit` rule ends a
/// transfer. <https://curl.se/libcurl/c/libcurl-errors.html#CURLEOPERATIONTIMEDOUT>
const CURL_OPERATION_TIMEDOUT: i32 = 28;
/// curl's own limit for a feed or signature document, which is small.
const DOCUMENT_SECONDS: u64 = 600;
/// The archive transfer's floor: one that averages below this many bytes per second over
/// [`STALL_SECONDS`] has stalled and ends as `timeout`.
const FLOOR_BYTES_PER_SECOND: u64 = 4096;
const STALL_SECONDS: u64 = 60;
/// Added to the time the signed size takes at the floor, for connecting and a slow start.
const TRANSFER_SLACK_SECONDS: u64 = 600;
/// The longest an archive transfer may take, whatever its signed size.
const TRANSFER_CEILING_SECONDS: u64 = 6 * 60 * 60;
/// How long past curl's own `--max-time` the Client waits before it ends curl itself.
const TRANSFER_GRACE: Duration = Duration::from_secs(10);

/// The archive transfer's own time limit in seconds, apart from the command's deadline: the time
/// the signed size takes at the floor plus the slack, never more than the ceiling. 25 MB may take
/// about 113 minutes on a link that stays above the floor.
fn transfer_seconds(size: u64) -> u64 {
    size.div_ceil(FLOOR_BYTES_PER_SECOND)
        .saturating_add(TRANSFER_SLACK_SECONDS)
        .min(TRANSFER_CEILING_SECONDS)
}

pub(super) enum Source {
    Local(LocalPath),
    Https(String),
    /// `http://127.0.0.1:<port>/…`, which only a test build admits, for a loopback server that
    /// stands in for the HTTPS host in a rehearsal on one computer.
    Loopback(String),
}
impl Source {
    /// Admits the configured feed location after expanding `{channel}`. An HTTPS location carries
    /// no query, so its signature document is the same URL with [`SIGNATURE_SUFFIX`]. `loopback`
    /// also admits a loopback address; only a test build passes true, and a release build refuses
    /// one whatever it passes.
    pub fn configured(
        value: Option<&str>,
        channel: UpdateChannel,
        loopback: bool,
    ) -> Result<Self, Error> {
        let value = value
            .filter(|value| !value.trim().is_empty())
            .ok_or(Error::SourceUnconfigured)?;
        let value = value.replace("{channel}", channel.as_str());
        if loopback && !orchbee_local_runtime::is_release() && valid_loopback(&value) {
            return Ok(Self::Loopback(value));
        }
        let source = Self::parse(&value)?;
        match &source {
            Self::Https(url) if url.contains('?') => Err(Error::SourceInvalid),
            _ => Ok(source),
        }
    }
    /// The `release.json` of a local signed release directory, such as the one the running
    /// Client sits in. A verbatim `\\?\` drive prefix is read as the plain drive path.
    pub fn release_directory(directory: &Path) -> Result<Self, Error> {
        let text = directory.to_str().ok_or(Error::NotReleaseDirectory)?;
        let text = text
            .strip_prefix(r"\\?\")
            .filter(|rest| rest.as_bytes().get(1) == Some(&b':'))
            .unwrap_or(text);
        LocalPath::parse(&format!(
            "{}\\release.json",
            text.trim_end_matches(['/', '\\'])
        ))
        .map(Self::Local)
        .ok_or(Error::NotReleaseDirectory)
    }
    fn parse(value: &str) -> Result<Self, Error> {
        if manifest::valid_https(value) {
            return Ok(Self::Https(value.into()));
        }
        LocalPath::parse(value)
            .map(Self::Local)
            .ok_or(Error::SourceInvalid)
    }
    pub fn artifact(&self, url: &str) -> Result<Self, Error> {
        if manifest::valid_https(url) {
            return Ok(Self::Https(url.into()));
        }
        if !crate::release_store::valid_release_path(url) {
            return Err(Error::SourceInvalid);
        }
        match self {
            Self::Local(path) => path
                .artifact(url)
                .map(Self::Local)
                .ok_or(Error::SourceInvalid),
            Self::Https(value) => {
                let (parent, _) = value.rsplit_once('/').ok_or(Error::SourceInvalid)?;
                Self::parse(&format!("{parent}/{url}"))
            }
            Self::Loopback(value) => {
                let (parent, _) = value.rsplit_once('/').ok_or(Error::SourceInvalid)?;
                Ok(Self::Loopback(format!("{parent}/{url}")))
            }
        }
    }
    /// Reads at most [`MANIFEST_LIMIT`] manifest bytes; a longer manifest is refused unread.
    pub fn manifest(&self, context: &mut Context) -> Result<Vec<u8>, Error> {
        self.read(None, MANIFEST_LIMIT, context)
            .map_err(|error| match error {
                Fetch::Failed(error) => error,
                Fetch::Absent => Error::LocalSourceRead,
                Fetch::Oversized => Error::ManifestInvalid,
            })
    }
    /// Reads at most [`FEED_LIMIT`] feed bytes into memory; a longer feed is refused unread.
    pub fn feed(&self, context: &mut Context) -> Result<Vec<u8>, Error> {
        self.read(None, FEED_LIMIT, context)
            .map_err(|error| match error {
                Fetch::Failed(error) => error,
                Fetch::Absent => Error::LocalSourceRead,
                Fetch::Oversized => Error::ManifestInvalid,
            })
    }
    /// Reads at most [`DOCUMENT_LIMIT`] bytes of the signature document beside the manifest or
    /// feed. A missing or oversized local document is a signature refusal, not a read failure.
    pub fn signature(&self, context: &mut Context) -> Result<Vec<u8>, Error> {
        self.read(Some(SIGNATURE_SUFFIX), DOCUMENT_LIMIT, context)
            .map_err(|error| match error {
                Fetch::Failed(error) => error,
                Fetch::Absent | Fetch::Oversized => Error::SignatureInvalid,
            })
    }
    fn read(
        &self,
        suffix: Option<&str>,
        limit: usize,
        context: &mut Context,
    ) -> Result<Vec<u8>, Fetch> {
        context.check().map_err(Fetch::Failed)?;
        match self {
            Self::Local(manifest) => {
                let sibling;
                let path = match suffix {
                    Some(suffix) => {
                        sibling = manifest
                            .with_suffix(suffix)
                            .ok_or(Fetch::Failed(Error::SourceInvalid))?;
                        &sibling
                    }
                    None => manifest,
                };
                let file = match platform::open_manifest(path.as_path()) {
                    Ok(Some(file)) => file,
                    Ok(None) => return Err(Fetch::Failed(Error::SourceInvalid)),
                    Err(error) if error.kind() == io::ErrorKind::NotFound => {
                        return Err(Fetch::Absent);
                    }
                    Err(_) => return Err(Fetch::Failed(Error::LocalSourceRead)),
                };
                let mut bytes = Vec::new();
                file.take(limit as u64 + 1)
                    .read_to_end(&mut bytes)
                    .map_err(|_| Fetch::Failed(Error::LocalSourceRead))?;
                if bytes.len() > limit {
                    return Err(Fetch::Oversized);
                }
                context.check().map_err(Fetch::Failed)?;
                Ok(bytes)
            }
            Self::Https(url) | Self::Loopback(url) => {
                require_curl(context).map_err(Fetch::Failed)?;
                let url = format!("{url}{}", suffix.unwrap_or(""));
                let mut command = curl(&url, limit as u64, self.loopback(), Transfer::Document)
                    .map_err(Fetch::Failed)?;
                command.args(["--output", "-"]);
                // One captured byte beyond the bound proves an oversized document.
                let result = platform::execute_capturing(command, limit + 1, context)
                    .map_err(|_| Fetch::Failed(Error::Download))?;
                context.command_result(&result).map_err(Fetch::Failed)?;
                if result.stdout_truncated || result.stdout.len() > limit {
                    return Err(Fetch::Oversized);
                }
                Ok(result.stdout)
            }
        }
    }
    pub fn copy(
        &self,
        path: &std::path::Path,
        file: &mut File,
        size: u64,
        context: &mut Context,
    ) -> Result<(), Error> {
        context.check()?;
        match self {
            Self::Local(source) => {
                let mut input =
                    platform::open_source_file(source.as_path()).map_err(|_| Error::Download)?;
                if input.metadata().map_err(|_| Error::Download)?.len() != size {
                    return Err(Error::Checksum);
                }
                let mut copied = 0_u64;
                let mut buffer = [0_u8; 64 * 1024];
                loop {
                    context.check()?;
                    let n = input.read(&mut buffer).map_err(|_| Error::Download)?;
                    if n == 0 {
                        break;
                    }
                    copied = copied.checked_add(n as u64).ok_or(Error::Checksum)?;
                    if copied > size {
                        return Err(Error::Checksum);
                    }
                    file.write_all(&buffer[..n])
                        .map_err(|error| context.storage(path, &error))?;
                }
                if copied != size {
                    return Err(Error::Checksum);
                }
            }
            Self::Https(url) | Self::Loopback(url) => {
                require_curl(context)?;
                let seconds = transfer_seconds(size);
                let mut command = curl(url, size.max(1), self.loopback(), Transfer::Archive)?;
                command.arg("--output").arg(path);
                // The transfer runs under its own limit, Ctrl+C still ends it at once, and the
                // command's deadline stands still meanwhile, so it keeps covering the rest.
                let started = Instant::now();
                let deadline = started + Duration::from_secs(seconds) + TRANSFER_GRACE;
                let result = platform::execute_transfer(command, deadline, context);
                context.deadline = context
                    .deadline
                    .checked_add(started.elapsed())
                    .unwrap_or(context.deadline);
                let result = result.map_err(|_| Error::Download)?;
                match result.exit_code {
                    // A body longer than the signed size fails the size check, not the transfer.
                    Some(CURL_FILESIZE_EXCEEDED) => return Err(Error::Checksum),
                    // Stalled below the floor, or past the transfer's limit.
                    Some(CURL_OPERATION_TIMEDOUT) => return Err(Error::Timeout),
                    _ => context.command_result(&result)?,
                }
            }
        }
        file.sync_all()
            .map_err(|error| context.storage(path, &error))?;
        Ok(())
    }
    fn loopback(&self) -> bool {
        matches!(self, Self::Loopback(_))
    }
}
/// Why a bounded document read produced no bytes.
enum Fetch {
    Failed(Error),
    /// The local file does not exist.
    Absent,
    /// The document exceeds its bound.
    Oversized,
}
/// What one transfer fetches, which chooses its time limit.
#[derive(Clone, Copy)]
enum Transfer {
    /// A feed or signature document, read into memory.
    Document,
    /// The archive: it ends when it stalls below the floor, and otherwise after
    /// [`transfer_seconds`] of its `--max-filesize`, the signed size.
    Archive,
}
/// One bounded transfer: HTTPS only, at most five redirects, each to HTTPS. A loopback source
/// instead allows plain HTTP to 127.0.0.1 and no proxy; only a test build ever has one.
fn curl(url: &str, limit: u64, loopback: bool, transfer: Transfer) -> Result<Command, Error> {
    let protocol = if loopback { "=http" } else { "=https" };
    let mut command = curl_base()?;
    command.args([
        "--disable",
        "--silent",
        "--show-error",
        "--fail",
        "--location",
        "--max-redirs",
        "5",
        "--proto",
        protocol,
        "--proto-redir",
        protocol,
        "--globoff",
        "--connect-timeout",
        "20",
    ]);
    match transfer {
        Transfer::Document => command.arg("--max-time").arg(DOCUMENT_SECONDS.to_string()),
        Transfer::Archive => command
            .arg("--speed-limit")
            .arg(FLOOR_BYTES_PER_SECOND.to_string())
            .arg("--speed-time")
            .arg(STALL_SECONDS.to_string())
            .arg("--max-time")
            .arg(transfer_seconds(limit).to_string()),
    };
    if loopback {
        command.args(["--noproxy", "127.0.0.1"]);
    }
    command
        .arg("--max-filesize")
        .arg(limit.to_string())
        .arg("--url")
        .arg(url);
    Ok(command)
}

/// `http://127.0.0.1:<port>/<path>` with a port from 1 to 65535 and the literal-URL rules of an
/// HTTPS source.
fn valid_loopback(value: &str) -> bool {
    let Some(rest) = value.strip_prefix("http://127.0.0.1:") else {
        return false;
    };
    let Some((port, _)) = rest.split_once('/') else {
        return false;
    };
    port.len() <= 5
        && port.bytes().all(|b| b.is_ascii_digit())
        && port.parse::<u16>().is_ok_and(|port| port > 0)
        && !value.contains('?')
        && manifest::valid_https(&format!("https://127.0.0.1:{rest}"))
}
fn curl_base() -> Result<Command, Error> {
    platform::curl_command().map_err(|_| Error::Download)
}
fn require_curl(context: &mut Context) -> Result<(), Error> {
    if context.curl_checked {
        return Ok(());
    }
    let mut command = curl_base()?;
    command.args(["--disable", "--version"]);
    let result = platform::execute_contained(command, context).map_err(|_| Error::Download)?;
    // No curl.exe in the system folder: the same answer as a missing tar.exe.
    if matches!(result.failure, Some(CommandFailure::SpawnUnavailable)) {
        return Err(Error::DownloadUnavailable);
    }
    context.command_result(&result)?;
    // Before 8.4, max-filesize did not bound transfers without Content-Length.
    // https://curl.se/docs/manpage.html#--max-filesize
    if !bounded_curl(&result.stdout) {
        return Err(Error::DownloadUnavailable);
    }
    context.curl_checked = true;
    Ok(())
}
fn bounded_curl(bytes: &[u8]) -> bool {
    let Ok(text) = std::str::from_utf8(bytes) else {
        return false;
    };
    let Some(version) = text
        .strip_prefix("curl ")
        .and_then(|s| s.split_whitespace().next())
    else {
        return false;
    };
    let mut numbers = version.split('.');
    match (
        numbers.next().and_then(|s| s.parse::<u32>().ok()),
        numbers.next().and_then(|s| s.parse::<u32>().ok()),
    ) {
        (Some(major), Some(minor)) => (major, minor) >= (8, 4),
        _ => false,
    }
}

#[cfg(test)]
mod tests;
