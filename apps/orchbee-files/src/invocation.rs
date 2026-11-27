//! The one invocation the Worker starts this process with: `--local-endpoint <endpoint>`, the
//! absolute address of the one-shot endpoint the Server bound for this start. The start's token
//! arrives on standard input instead, so no other process can read it from the command line.

use std::{
    ffi::{OsStr, OsString},
    path::PathBuf,
};

const LOCAL_ENDPOINT_OPTION: &str = "--local-endpoint";

/// The endpoint the invocation names, or `None` for any other invocation. A relative address
/// would resolve against the working directory, which names no runtime.
pub(crate) fn parse(mut arguments: impl Iterator<Item = OsString>) -> Option<PathBuf> {
    if arguments.next().as_deref() != Some(OsStr::new(LOCAL_ENDPOINT_OPTION)) {
        return None;
    }
    let endpoint = PathBuf::from(arguments.next()?);
    (arguments.next().is_none() && endpoint.is_absolute()).then_some(endpoint)
}
