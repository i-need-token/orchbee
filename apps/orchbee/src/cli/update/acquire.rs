//! Bounded explicit release acquisition. Every subprocess belongs to the invocation's Job.
use super::{Context, Error, manifest};
use crate::release_store::open_release_file;
use orchbee_local_runtime::{UpdateChannel, run_windows_command};
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    os::windows::fs::{MetadataExt, OpenOptionsExt},
    path::{Component, PathBuf, Prefix},
    process::Command,
};

pub(super) enum Source {
    Local(PathBuf),
    Https(String),
}
impl Source {
    pub fn configured(channel: UpdateChannel) -> Result<Self, Error> {
        let value =
            std::env::var("ORCHBEE_UPDATE_MANIFEST").map_err(|_| Error::SourceUnconfigured)?;
        if value.trim().is_empty() {
            return Err(Error::SourceUnconfigured);
        }
        Self::parse(&value.replace("{channel}", channel.as_str()))
    }
    fn parse(value: &str) -> Result<Self, Error> {
        if manifest::valid_https(value) {
            return Ok(Self::Https(value.into()));
        }
        let path = PathBuf::from(value);
        if value.len() > 4096
            || value.contains(['\0', '\r', '\n'])
            || !path.is_absolute()
            || !matches!(path.components().next(), Some(Component::Prefix(prefix)) if matches!(prefix.kind(), Prefix::Disk(_)))
            || path
                .components()
                .any(|part| matches!(part, Component::ParentDir | Component::CurDir))
        {
            return Err(Error::SourceInvalid);
        }
        Ok(Self::Local(path))
    }
    pub fn artifact(&self, url: &str) -> Result<Self, Error> {
        if manifest::valid_https(url) {
            return Ok(Self::Https(url.into()));
        }
        if !crate::release_store::valid_release_path(url) {
            return Err(Error::SourceInvalid);
        }
        match self {
            Self::Local(path) => Ok(Self::Local(
                path.parent().ok_or(Error::SourceInvalid)?.join(url),
            )),
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
                let file = OpenOptions::new()
                    .read(true)
                    .share_mode(1)
                    .custom_flags(0x0020_0000)
                    .open(path)
                    .map_err(|_| Error::LocalManifestRead)?;
                let metadata = file.metadata().map_err(|_| Error::LocalManifestRead)?;
                if !metadata.is_file() || metadata.file_attributes() & 0x400 != 0 {
                    return Err(Error::SourceInvalid);
                }
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
                let result =
                    run_windows_command(command, Some(context.deadline), || context.interrupted());
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
                let parent = source.parent().ok_or(Error::SourceInvalid)?;
                let name = source
                    .file_name()
                    .and_then(|s| s.to_str())
                    .ok_or(Error::SourceInvalid)?;
                let mut input = open_release_file(parent, name).map_err(|_| Error::Download)?;
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
                let result =
                    run_windows_command(command, Some(context.deadline), || context.interrupted());
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
    let system = winsafe::GetSystemDirectory().map_err(|_| Error::Download)?;
    let mut command = Command::new(PathBuf::from(&system).join("curl.exe"));
    command.env_clear().env(
        "SystemRoot",
        PathBuf::from(&system).parent().ok_or(Error::Download)?,
    );
    Ok(command)
}
fn require_curl(context: &mut Context) -> Result<(), Error> {
    if context.curl_checked {
        return Ok(());
    }
    let mut command = curl_base()?;
    command.args(["--disable", "--version"]);
    let result = run_windows_command(command, Some(context.deadline), || context.interrupted());
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
