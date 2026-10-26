//! Bounded explicit release acquisition. Every subprocess belongs to the invocation's Job.
use super::{Context, Error, manifest, platform, source_path::LocalPath};
use crate::release_store::open_release_file;
use orchbee_local_runtime::UpdateChannel;
use std::{
    fs::File,
    io::{Read, Write},
    path::Path,
    process::Command,
};

pub(super) enum Source {
    Local(LocalPath),
    Https(String),
}
impl Source {
    pub fn configured(channel: UpdateChannel) -> Result<Self, Error> {
        let value = platform::manifest_source().map_err(|_| Error::SourceUnconfigured)?;
        if value.trim().is_empty() {
            return Err(Error::SourceUnconfigured);
        }
        Self::parse(&value.replace("{channel}", channel.as_str()))
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
    pub fn manifest(&self, context: &mut Context) -> Result<Vec<u8>, Error> {
        context.check()?;
        match self {
            Self::Local(path) => {
                let mut bytes = Vec::new();
                let file = platform::open_manifest(path.as_path())
                    .map_err(|_| Error::LocalManifestRead)?
                    .ok_or(Error::SourceInvalid)?;
                file.take(manifest::MANIFEST_LIMIT as u64 + 1)
                    .read_to_end(&mut bytes)
                    .map_err(|_| Error::LocalManifestRead)?;
                if bytes.len() > manifest::MANIFEST_LIMIT {
                    return Err(Error::ManifestInvalid);
                }
                context.check()?;
                Ok(bytes)
            }
            Self::Https(url) => {
                require_curl(context)?;
                let mut command = curl(url, manifest::MANIFEST_LIMIT as u64)?;
                command.args(["--output", "-"]);
                let result = platform::execute(command, context).map_err(|_| Error::Download)?;
                context.command_result(&result)?;
                if result.stdout_truncated {
                    return Err(Error::ManifestInvalid);
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
