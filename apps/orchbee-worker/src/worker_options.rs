//! Parses the exact explicit-endpoint Worker invocation into literal Server and Host paths
//! with a checked endpoint identity. It does not discover a runtime, inspect endpoint
//! ownership, or connect either peer.

use std::{ffi::OsString, io, path::PathBuf};

const LOCAL_ENDPOINT_OPTION: &str = "--local-endpoint";
const TERMINAL_HOST_RUNTIME_DIRECTORY_OPTION: &str = "--terminal-host-runtime-directory";
const TERMINAL_HOST_ENDPOINT_ID_OPTION: &str = "--terminal-host-endpoint-id";

pub(crate) struct WorkerOptions {
    pub(crate) server_endpoint: PathBuf,
    pub(crate) terminal_host_endpoint: PathBuf,
}

impl WorkerOptions {
    pub(crate) fn parse(mut arguments: impl Iterator<Item = OsString>) -> io::Result<Self> {
        require_option(&mut arguments, LOCAL_ENDPOINT_OPTION)?;
        let server_endpoint = require_path(&mut arguments, "missing Server endpoint path")?;
        require_option(&mut arguments, TERMINAL_HOST_RUNTIME_DIRECTORY_OPTION)?;
        let runtime_directory = require_path(
            &mut arguments,
            "missing Terminal Host runtime directory path",
        )?;
        require_option(&mut arguments, TERMINAL_HOST_ENDPOINT_ID_OPTION)?;
        let endpoint_id = arguments
            .next()
            .and_then(|value| value.into_string().ok())
            .ok_or_else(invalid_invocation)?;

        if arguments.next().is_some()
            || !server_endpoint.is_absolute()
            || server_endpoint.file_name().is_none()
            || !runtime_directory.is_absolute()
            || !valid_endpoint_id(&endpoint_id)
        {
            return Err(invalid_invocation());
        }

        Ok(Self {
            server_endpoint,
            terminal_host_endpoint: runtime_directory.join(format!("{endpoint_id}.s")),
        })
    }
}

fn require_option(
    arguments: &mut impl Iterator<Item = OsString>,
    expected: &str,
) -> io::Result<()> {
    if arguments.next().as_deref() != Some(std::ffi::OsStr::new(expected)) {
        return Err(invalid_invocation());
    }
    Ok(())
}

fn require_path(
    arguments: &mut impl Iterator<Item = OsString>,
    missing_message: &'static str,
) -> io::Result<PathBuf> {
    arguments
        .next()
        .map(PathBuf::from)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, missing_message))
}

fn valid_endpoint_id(value: &str) -> bool {
    value.len() == 32
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn invalid_invocation() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidInput,
        "invalid private Worker invocation",
    )
}
