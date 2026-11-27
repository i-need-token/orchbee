//! The relay helper: this executable as Git's askpass program and editor. Git starts it with the
//! relay's variables, which the engine sets only on the commands of an operation; started with
//! them and without `--local-endpoint`, the process asks its relay and exits without reading the
//! data root or writing the diagnostic log.

use super::{
    command::{EDITOR_OPTION, RELAY_TOKEN_VARIABLE, RELAY_VARIABLE},
    relay::{self, Answer, Asked},
};
use orchbee_local_ipc::LocalStream;
use orchbee_local_peer_credentials::require_same_effective_user;
use std::{
    ffi::{OsStr, OsString},
    io::{self, Write},
    path::Path,
    process::ExitCode,
};

/// The option the Worker starts the service with, which no helper invocation has.
const LOCAL_ENDPOINT_OPTION: &str = "--local-endpoint";
/// Set by ssh: `confirm` asks a yes or no question, `none` only notifies.
const SSH_PROMPT_VARIABLE: &str = "SSH_ASKPASS_PROMPT";

/// Acts as the helper when the environment and `arguments` (without the program) ask for it,
/// answering the exit code; `None` for the service's own invocation.
pub(super) fn helper(arguments: &[OsString]) -> Option<ExitCode> {
    let endpoint = std::env::var_os(RELAY_VARIABLE)?;
    let token = std::env::var_os(RELAY_TOKEN_VARIABLE)?;
    if arguments.first().map(OsString::as_os_str) == Some(OsStr::new(LOCAL_ENDPOINT_OPTION)) {
        return None;
    }
    let editor = arguments.first().map(OsString::as_os_str) == Some(OsStr::new(EDITOR_OPTION));
    let (asked, text) = if editor {
        let Some(Ok(file)) = arguments.get(1).map(std::path::absolute) else {
            return Some(ExitCode::FAILURE);
        };
        (Asked::Editor, file.to_string_lossy().into_owned())
    } else {
        let asked = match std::env::var(SSH_PROMPT_VARIABLE).as_deref() {
            // A notification needs no answer, and ssh ends the helper when it is done.
            Ok("none") => return Some(ExitCode::SUCCESS),
            Ok("confirm") => Asked::Confirm,
            _ => Asked::Askpass,
        };
        let text = arguments
            .first()
            .map(|text| text.to_string_lossy().into_owned())
            .unwrap_or_default();
        (asked, text)
    };
    let answer = token
        .to_str()
        .and_then(relay::token_from_hex)
        .ok_or_else(|| io::Error::from(io::ErrorKind::InvalidInput))
        .and_then(|token| {
            ask(
                Path::new(&endpoint),
                &relay::encode_request(asked, &token, &text),
            )
        });
    Some(match (asked, answer) {
        (Asked::Editor, Ok(Answer::Value(_) | Answer::Empty)) => ExitCode::SUCCESS,
        (Asked::Askpass | Asked::Confirm, Ok(Answer::Value(value))) => print(&value),
        (Asked::Askpass | Asked::Confirm, Ok(Answer::Empty)) => print(""),
        (_, Ok(Answer::Cancelled) | Err(_)) => ExitCode::FAILURE,
    })
}

/// Sends `request` to the relay at `endpoint` once both sides proved their user, and reads the
/// answer, which may wait as long as the prompt stays open.
pub(super) fn ask(endpoint: &Path, request: &[u8]) -> io::Result<Answer> {
    let stream = LocalStream::connect(endpoint)?;
    require_same_effective_user(&stream)?;
    let mut writer = &stream;
    writer.write_all(request)?;
    writer.flush()?;
    let mut reader = &stream;
    relay::read_answer(&mut reader)
}

/// Writes an askpass answer as Git and ssh read it: one line.
fn print(value: &str) -> ExitCode {
    let mut stdout = io::stdout().lock();
    let written = stdout
        .write_all(value.as_bytes())
        .and_then(|()| stdout.write_all(b"\n"))
        .and_then(|()| stdout.flush());
    if written.is_ok() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
