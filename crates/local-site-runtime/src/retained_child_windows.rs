use std::{ffi::OsStr, io, os::windows::process::ExitStatusExt, path::Path, process::ExitStatus};
use winsafe::{co, guard::CloseHandlePiGuard};

/// Windows reclaims exited processes without a Unix-style reaper. Dropping
/// this handle releases observation ownership and never terminates the Site.
pub struct RetainedChild(CloseHandlePiGuard);
impl RetainedChild {
    pub(super) fn spawn(executable: &Path, arguments: &[&OsStr]) -> io::Result<Self> {
        if !executable.is_absolute() {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        let program = text(executable.as_os_str())?;
        let mut command = quote(program);
        for argument in arguments {
            command.push(' ');
            command.push_str(&quote(text(argument)?));
        }
        if command.encode_utf16().count() >= 32_767 {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        // bInheritHandles=false is essential: even when stdio is redirected to
        // NUL, a generic inheriting spawn can retain a caller's capture pipe.
        winsafe::CreateProcess(
            Some(program),
            Some(&command),
            None,
            None,
            false,
            co::CREATE::DETACHED_PROCESS | co::CREATE::NEW_PROCESS_GROUP,
            &[],
            None,
            &mut winsafe::STARTUPINFO::default(),
        )
        .map(Self)
        .map_err(io::Error::other)
    }
    pub fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        match self
            .0
            .hProcess
            .WaitForSingleObject(Some(0))
            .map_err(io::Error::other)?
        {
            co::WAIT::TIMEOUT => Ok(None),
            co::WAIT::OBJECT_0 => self
                .0
                .hProcess
                .GetExitCodeProcess()
                .map(|status| Some(ExitStatus::from_raw(status)))
                .map_err(io::Error::other),
            _ => Err(io::Error::other("unexpected retained process wait result")),
        }
    }
}
fn text(value: &OsStr) -> io::Result<&str> {
    value
        .to_str()
        .filter(|text| !text.contains('\0'))
        .ok_or_else(|| io::ErrorKind::InvalidInput.into())
}
// Microsoft CRT argv quoting: every argument is quoted, backslashes double
// before a quote or the closing quote. There is no command shell evaluation.
fn quote(value: &str) -> String {
    let mut output = String::from("\"");
    let mut slashes = 0;
    for character in value.chars() {
        if character == '\\' {
            slashes += 1;
            continue;
        }
        output.extend(std::iter::repeat_n(
            '\\',
            if character == '"' {
                2 * slashes + 1
            } else {
                slashes
            },
        ));
        slashes = 0;
        output.push(character);
    }
    output.extend(std::iter::repeat_n('\\', 2 * slashes));
    output.push('"');
    output
}

#[cfg(test)]
#[path = "retained_child_windows/tests.rs"]
mod tests;
