//! Invocation-owned cancellable raw stdin. Caller bytes are pipe data, never script source.
use orchbee_local_ipc::ChildOutputReader;
use std::{
    io::{self, Read},
    os::windows::process::CommandExt,
    path::PathBuf,
    process::{Child, Command, Stdio},
};
const SCRIPT: &str = r#"$ErrorActionPreference='Stop'; try {
$i=[Console]::OpenStandardInput(); $o=[Console]::OpenStandardOutput(); $b=[byte[]]::new(8192)
while ($true) { $n=$i.Read($b,0,$b.Length); if ($n -eq 0) { exit 0 }; $o.Write($b,0,$n); $o.Flush() }
} catch { exit 1 }"#;
pub(in crate::cli) enum Poll {
    Pending,
    Bytes(usize),
    Eof,
}
pub(in crate::cli) struct Reader {
    child: Child,
    output: ChildOutputReader,
    eof: bool,
}
impl Reader {
    pub(in crate::cli) fn start() -> io::Result<Self> {
        Self::with_input(Stdio::inherit())
    }
    pub(in crate::cli) fn with_input(input: Stdio) -> io::Result<Self> {
        let executable = PathBuf::from(winsafe::GetSystemDirectory().map_err(io::Error::other)?)
            .join("WindowsPowerShell/v1.0/powershell.exe");
        let (output, child_output) = ChildOutputReader::pipe()?;
        let mut command = Command::new(executable);
        command
            .args([
                "-NoLogo",
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                SCRIPT,
            ])
            .stdin(input)
            .stdout(child_output)
            .stderr(Stdio::null())
            .creation_flags(0x0800_0000);
        let child = command.spawn()?;
        drop(command);
        Ok(Self {
            child,
            output,
            eof: false,
        })
    }
    pub(in crate::cli) fn poll(&mut self, buffer: &mut [u8]) -> io::Result<Poll> {
        if buffer.is_empty() {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        if !self.eof {
            match self.output.read(buffer) {
                Ok(0) => self.eof = true,
                Ok(count) => return Ok(Poll::Bytes(count)),
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                    return Ok(Poll::Pending);
                }
                Err(error) => return Err(error),
            }
        }
        match self.child.try_wait()? {
            None => Ok(Poll::Pending),
            Some(status) if status.success() => Ok(Poll::Eof),
            Some(_) => Err(io::ErrorKind::BrokenPipe.into()),
        }
    }
}
impl Drop for Reader {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
