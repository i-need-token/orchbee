//! Invocation-owned Windows stdin helper; caller bytes remain pipe data.
use super::{NativeReader, Poll};
use orchbee_local_ipc::ChildOutputReader;
use std::{
    io::{self, Read},
    os::windows::process::CommandExt,
    path::PathBuf,
    process::{Child, Command, ExitStatus, Stdio},
};
const SCRIPT: &str = r#"$ErrorActionPreference='Stop'; try {
$i=[Console]::OpenStandardInput(); $o=[Console]::OpenStandardOutput(); $b=[byte[]]::new(8192)
while ($true) { $n=$i.Read($b,0,$b.Length); if ($n -eq 0) { exit 0 }; $o.Write($b,0,$n); $o.Flush() }
} catch { exit 1 }"#;

struct Reader {
    child: Child,
    output: ChildOutputReader,
}
impl Reader {
    fn with_input(input: Stdio) -> io::Result<Self> {
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
        Ok(Self { child, output })
    }
}
pub(super) fn with_input(input: Stdio) -> io::Result<Box<dyn NativeReader>> {
    Reader::with_input(input).map(|reader| Box::new(reader) as Box<dyn NativeReader>)
}
impl NativeReader for Reader {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        self.output.read(bytes)
    }
    fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        self.child.try_wait()
    }
}
impl Drop for Reader {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
