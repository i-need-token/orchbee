//! Bounded explicit release acquisition. Every subprocess belongs to the invocation's Job.
use super::{
    Context, Error, manifest, platform,
    signature::{DOCUMENT_LIMIT, MANIFEST_LIMIT},
    source_path::LocalPath,
};
use crate::release_store::open_release_file;
use orchbee_local_runtime::UpdateChannel;
use std::{
    fs::File,
    io::{self, Read, Write},
    path::Path,
    process::Command,
};

/// The signature document lives beside the manifest under this suffix.
const SIGNATURE_SUFFIX: &str = ".sig";

pub(super) enum Source {
    Local(LocalPath),
    Https(String),
}
impl Source {
    /// Admits the configured manifest location after expanding `{channel}`. An HTTPS location
    /// carries no query, so its signature document is the same URL with [`SIGNATURE_SUFFIX`].
    pub fn configured(value: Option<&str>, channel: UpdateChannel) -> Result<Self, Error> {
        let value = value
            .filter(|value| !value.trim().is_empty())
            .ok_or(Error::SourceUnconfigured)?;
        let source = Self::parse(&value.replace("{channel}", channel.as_str()))?;
        match &source {
            Self::Https(url) if url.contains('?') => Err(Error::SourceInvalid),
            _ => Ok(source),
        }
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
        }
    }
    /// Reads at most [`MANIFEST_LIMIT`] manifest bytes; a longer manifest is refused unread.
    pub fn manifest(&self, context: &mut Context) -> Result<Vec<u8>, Error> {
        self.read(None, MANIFEST_LIMIT, context)
            .map_err(|error| match error {
                Fetch::Failed(error) => error,
                Fetch::Absent => Error::LocalManifestRead,
                Fetch::Oversized => Error::ManifestInvalid,
            })
    }
    /// Reads at most [`DOCUMENT_LIMIT`] bytes of the signature document beside the manifest. A
    /// missing or oversized local document is a signature refusal, not a read failure.
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
                    Err(_) => return Err(Fetch::Failed(Error::LocalManifestRead)),
                };
                let mut bytes = Vec::new();
                file.take(limit as u64 + 1)
                    .read_to_end(&mut bytes)
                    .map_err(|_| Fetch::Failed(Error::LocalManifestRead))?;
                if bytes.len() > limit {
                    return Err(Fetch::Oversized);
                }
                context.check().map_err(Fetch::Failed)?;
                Ok(bytes)
            }
            Self::Https(url) => {
                require_curl(context).map_err(Fetch::Failed)?;
                let url = format!("{url}{}", suffix.unwrap_or(""));
                let mut command = curl(&url, limit as u64).map_err(Fetch::Failed)?;
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
                let (parent, name) = source.parent_and_name().ok_or(Error::SourceInvalid)?;
                let mut input =
                    open_release_file(Path::new(parent), name).map_err(|_| Error::Download)?;
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
                    file.write_all(&buffer[..n]).map_err(|_| Error::Storage)?;
                }
                if copied != size {
                    return Err(Error::Checksum);
                }
            }
            Self::Https(url) => {
                require_curl(context)?;
                let mut command = curl(url, size.max(1))?;
                command.arg("--output").arg(path);
                let result = platform::execute(command, context).map_err(|_| Error::Download)?;
                context.command_result(&result)?;
            }
        }
        file.sync_all().map_err(|_| Error::Storage)?;
        Ok(())
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
fn curl(url: &str, limit: u64) -> Result<Command, Error> {
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
        "=https",
        "--proto-redir",
        "=https",
        "--globoff",
        "--connect-timeout",
        "20",
        "--max-time",
        "600",
        "--max-filesize",
    ]);
    command.arg(limit.to_string()).arg("--url").arg(url);
    Ok(command)
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
    let result = platform::execute(command, context).map_err(|_| Error::Download)?;
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
