//! A bounded, acknowledged stdout relay. Fixed system script; payload is only pipe data.
use orchbee_local_ipc::{ChildInputWriter, ChildOutputReader};
use std::{
    io::{self, Read, Write},
    os::windows::process::CommandExt,
    path::PathBuf,
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};

const SCRIPT: &str = r#"$ErrorActionPreference='Stop'; try {
$i=[Console]::OpenStandardInput(); $o=[Console]::OpenStandardOutput(); $a=[Console]::OpenStandardError()
$header=[byte[]]::new(4)
while ($true) {
  $pos=0
  while ($pos -lt 4) { $n=$i.Read($header,$pos,4-$pos); if ($n -eq 0) { if ($pos -eq 0) { exit 0 }; exit 1 }; $pos+=$n }
  $size=[BitConverter]::ToUInt32($header,0); if ($size -eq 0 -or $size -gt 8388608) { exit 1 }
  $body=[byte[]]::new($size); $pos=0
  while ($pos -lt $size) { $n=$i.Read($body,$pos,$size-$pos); if ($n -eq 0) { exit 1 }; $pos+=$n }
  $o.Write($body,0,$body.Length); $o.Flush(); $a.WriteByte(1); $a.Flush()
}
} catch { exit 1 }"#;

pub(super) struct Relay {
    child: Child,
    input: ChildInputWriter,
    ack: ChildOutputReader,
    healthy: bool,
}
impl Relay {
    pub(super) fn start() -> io::Result<Self> {
        Self::with_output(Stdio::inherit())
    }
    pub(super) fn with_output(output: Stdio) -> io::Result<Self> {
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
            .creation_flags(0x0800_0000);
        let child = command.spawn()?;
        drop(command);
        Ok(Self {
            child,
            input,
            ack,
            healthy: true,
        })
    }
    pub(super) fn healthy(&self) -> bool {
        self.healthy
    }
    pub(super) fn record(
        &mut self,
        bytes: &[u8],
        deadline: Instant,
        cancelled: &dyn Fn() -> bool,
    ) -> io::Result<()> {
        if !self.healthy || bytes.is_empty() || bytes.len() > super::super::output::LIMIT {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        // Any interrupted record may have a written prefix. Never append a terminal record to it.
        self.healthy = false;
        let header = u32::try_from(bytes.len())
            .map_err(io::Error::other)?
            .to_le_bytes();
        for part in [header.as_slice(), bytes] {
            let mut remaining = part;
            while !remaining.is_empty() {
                check(deadline, cancelled)?;
                match self.input.write(remaining) {
                    Ok(0) => return Err(io::ErrorKind::WriteZero.into()),
                    Ok(n) => remaining = &remaining[n..],
                    Err(e) if e.kind() == io::ErrorKind::WouldBlock => pause(),
                    Err(e) => return Err(e),
                }
            }
        }
        loop {
            check(deadline, cancelled)?;
            let mut ack = [0];
            match self.ack.read(&mut ack) {
                Ok(1) if ack[0] == 1 => {
                    self.healthy = true;
                    return Ok(());
                }
                Ok(_) => return Err(io::ErrorKind::BrokenPipe.into()),
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => pause(),
                Err(e) => return Err(e),
            }
        }
    }
}
impl Drop for Relay {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
fn check(deadline: Instant, cancelled: &dyn Fn() -> bool) -> io::Result<()> {
    if cancelled() {
        return Err(io::ErrorKind::Interrupted.into());
    }
    if Instant::now() >= deadline {
        return Err(io::ErrorKind::TimedOut.into());
    }
    Ok(())
}
fn pause() {
    thread::sleep(Duration::from_millis(2));
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn relay_delivers_a_large_record_after_the_consumer_resumes() {
        let (mut output, child_output) = ChildOutputReader::pipe().unwrap();
        let mut relay = Relay::with_output(child_output).unwrap();
        let bytes = vec![b'x'; 2 * 1024 * 1024];
        let deadline = Instant::now() + Duration::from_secs(5);
        let received = std::thread::scope(|scope| {
            let reader = scope.spawn(|| {
                std::thread::sleep(Duration::from_millis(100));
                let mut received = Vec::new();
                let mut buffer = [0; 4096];
                while received.len() < bytes.len() {
                    if Instant::now() >= deadline {
                        return Err(io::Error::from(io::ErrorKind::TimedOut));
                    }
                    match output.read(&mut buffer) {
                        Ok(0) => return Err(io::ErrorKind::UnexpectedEof.into()),
                        Ok(count) => received.extend_from_slice(&buffer[..count]),
                        Err(error) if error.kind() == io::ErrorKind::WouldBlock => pause(),
                        Err(error) => return Err(error),
                    }
                }
                Ok(received)
            });
            let written = relay.record(&bytes, deadline, &|| false);
            let received = reader.join().unwrap();
            written.unwrap();
            received.unwrap()
        });
        assert_eq!(received, bytes);
        assert!(relay.healthy());
    }
    #[test]
    fn relay_flushes_literal_records_and_reaps_after_stalled_output() {
        let (mut output, child_output) = ChildOutputReader::pipe().unwrap();
        let mut relay = Relay::with_output(child_output).unwrap();
        let bytes = "{\"literal\":\"中文 $(`text`)\"}\n".as_bytes();
        for _ in 0..3 {
            relay
                .record(bytes, Instant::now() + Duration::from_secs(2), &|| false)
                .unwrap();
            let mut received = vec![0; bytes.len()];
            output.read_exact(&mut received).unwrap();
            assert_eq!(received, bytes);
        }
        let started = Instant::now();
        let huge = vec![b'x'; super::super::super::output::LIMIT];
        assert_eq!(
            relay
                .record(&huge, started + Duration::from_millis(80), &|| false)
                .unwrap_err()
                .kind(),
            io::ErrorKind::TimedOut
        );
        assert!(!relay.healthy());
        let _ = relay.child.kill();
        assert!(relay.child.wait().is_ok());
        assert!(started.elapsed() < Duration::from_secs(5));
        assert!(
            relay
                .record(
                    b"closed\n",
                    Instant::now() + Duration::from_secs(1),
                    &|| false
                )
                .is_err()
        );
    }
}
