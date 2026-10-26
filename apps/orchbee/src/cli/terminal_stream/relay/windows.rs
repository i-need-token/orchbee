//! Windows stdout helper ownership and native pipes; payload remains literal pipe data.
use super::NativeRelay;
use orchbee_local_ipc::{ChildInputWriter, ChildOutputReader};
use std::{
    io::{self, IsTerminal, Read, Write},
    os::windows::process::CommandExt,
    path::PathBuf,
    process::{Child, Command, Stdio},
};
const SCRIPT: &str = r#"$ErrorActionPreference='Stop'; try {
$i=[Console]::OpenStandardInput(); $o=[Console]::OpenStandardOutput(); $a=[Console]::OpenStandardError()
$console=-not [Console]::IsOutputRedirected; $utf8=[Text.UTF8Encoding]::new($false,$true)
if ($console) { [Console]::OutputEncoding=[Text.Encoding]::Unicode }
$header=[byte[]]::new(4)
while ($true) {
  $pos=0
  while ($pos -lt 4) { $n=$i.Read($header,$pos,4-$pos); if ($n -eq 0) { if ($pos -eq 0) { exit 0 }; exit 1 }; $pos+=$n }
  $size=[BitConverter]::ToUInt32($header,0); if ($size -eq 0 -or $size -gt 8388608) { exit 1 }
  $body=[byte[]]::new($size); $pos=0
  while ($pos -lt $size) { $n=$i.Read($body,$pos,$size-$pos); if ($n -eq 0) { exit 1 }; $pos+=$n }
  if ($console) { [Console]::Write($utf8.GetString($body)); [Console]::Out.Flush() }
  else { $o.Write($body,0,$body.Length); $o.Flush() }
  $a.WriteByte(1); $a.Flush()
}
} catch { exit 1 }"#;

struct Relay {
    child: Child,
    input: ChildInputWriter,
    ack: ChildOutputReader,
}
impl Relay {
    fn spawn(output: Stdio, console: bool) -> io::Result<Self> {
        let executable = PathBuf::from(winsafe::GetSystemDirectory().map_err(io::Error::other)?)
            .join("WindowsPowerShell/v1.0/powershell.exe");
        let (input, child_input) = ChildInputWriter::pipe()?;
        let (ack, child_ack) = ChildOutputReader::pipe()?;
        let mut command = Command::new(executable);
        command
            .args([
                "-NoLogo",
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                SCRIPT,
            ])
            .stdin(child_input)
            .stdout(output)
            .stderr(child_ack)
            // A detached helper silently discards inherited console output. Retain
            // that console, but isolate Ctrl+C so the owner can finish its record
            // and reap the helper. Redirected output needs no console at all.
            .creation_flags(if console { 0x0000_0200 } else { 0x0800_0000 });
        let child = command.spawn()?;
        drop(command);
        Ok(Self { child, input, ack })
    }
}
pub(super) fn start() -> io::Result<Box<dyn NativeRelay>> {
    Relay::spawn(Stdio::inherit(), io::stdout().is_terminal())
        .map(|native| Box::new(native) as Box<dyn NativeRelay>)
}
#[cfg(test)]
pub(super) fn with_output(output: Stdio) -> io::Result<Box<dyn NativeRelay>> {
    Relay::spawn(output, false).map(|native| Box::new(native) as Box<dyn NativeRelay>)
}
impl NativeRelay for Relay {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.input.write(bytes)
    }
    fn read_ack(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        self.ack.read(bytes)
    }
    fn stop(&mut self) -> io::Result<()> {
        let _ = self.child.kill();
        self.child.wait().map(|_| ())
    }
}
