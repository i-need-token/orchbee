//! Invocation-owned Windows stdin helper; caller bytes remain pipe data.
use super::NativeReader;
use crate::cli::powershell_helper;
use orchbee_local_ipc::ChildOutputReader;
use std::{
    io::{self, Read},
    os::windows::process::CommandExt,
    process::{Child, ExitStatus, Stdio},
};
const SCRIPT: &str = r#"$ErrorActionPreference='Stop'; try {
$i=[Console]::OpenStandardInput(); $o=[Console]::OpenStandardOutput(); $a=[Console]::OpenStandardError(); $b=[byte[]]::new(8192)
$a.WriteByte(2); $a.Flush()
while ($true) { $n=$i.Read($b,0,$b.Length); if ($n -eq 0) { exit 0 }; $o.Write($b,0,$n); $o.Flush() }
} catch { exit 1 }"#;

struct Reader {
    child: Child,
    output: ChildOutputReader,
    status: ChildOutputReader,
}
impl Reader {
    fn with_input(input: Stdio) -> io::Result<Self> {
        let (output, child_output) = ChildOutputReader::pipe()?;
        let (status, child_status) = ChildOutputReader::pipe()?;
        let mut command = powershell_helper::command(SCRIPT)?;
        command
            .stdin(input)
            .stdout(child_output)
            .stderr(child_status)
            .creation_flags(0x0800_0000);
        let child = command.spawn()?;
        drop(command);
        Ok(Self {
            child,
            output,
            status,
        })
    }
}
pub(super) fn start() -> io::Result<Box<dyn NativeReader>> {
    with_input(Stdio::inherit())
}
pub(super) fn with_input(input: Stdio) -> io::Result<Box<dyn NativeReader>> {
    Reader::with_input(input).map(|reader| Box::new(reader) as Box<dyn NativeReader>)
}
impl NativeReader for Reader {
    fn ready(&mut self, cancelled: &dyn Fn() -> bool) -> io::Result<()> {
        powershell_helper::await_ready(&mut self.status, cancelled)
    }
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
