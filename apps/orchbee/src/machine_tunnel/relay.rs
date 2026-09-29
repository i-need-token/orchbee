//! Bounded full-duplex pump with owned binary stdio/SSH children and explicit settlement.
use orchbee_local_ipc::{ChildInputWriter, ChildOutputReader, LocalStream};
use orchbee_local_site_runtime::WindowsCommandChild;
use std::{
    io::{self, Read, Write},
    path::PathBuf,
    process::{Command, Stdio},
    time::{Duration, Instant},
};
const CAPACITY: usize = 64 * 1024;
const COPY: &str = "$ErrorActionPreference='Stop';try{[Console]::OpenStandardInput().CopyTo([Console]::OpenStandardOutput());exit 0}catch{exit 1}";
#[derive(Default)]
struct Buffer {
    bytes: Vec<u8>,
    offset: usize,
}
impl Buffer {
    fn read(&mut self, reader: &mut impl Read) -> io::Result<bool> {
        if self.offset != self.bytes.len() {
            return Ok(true);
        }
        self.bytes.clear();
        self.offset = 0;
        let mut bytes = [0; CAPACITY];
        match reader.read(&mut bytes) {
            Ok(0) => Ok(false),
            Ok(n) => {
                self.bytes.extend_from_slice(&bytes[..n]);
                Ok(true)
            }
            Err(e)
                if matches!(
                    e.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                ) =>
            {
                Ok(true)
            }
            Err(e) => Err(e),
        }
    }
    fn write(&mut self, writer: &mut impl Write) -> io::Result<()> {
        if self.offset == self.bytes.len() {
            return Ok(());
        }
        match writer.write(&self.bytes[self.offset..]) {
            Ok(0) => Err(io::ErrorKind::WriteZero.into()),
            Ok(n) => {
                self.offset += n;
                Ok(())
            }
            Err(e)
                if matches!(
                    e.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                ) =>
            {
                Ok(())
            }
            Err(e) => Err(e),
        }
    }
    fn empty(&self) -> bool {
        self.offset == self.bytes.len()
    }
}
pub(super) struct Lane {
    local: LocalStream,
    input: Option<ChildInputWriter>,
    stdio: bool,
    draining_stdout: Option<Instant>,
    output: ChildOutputReader,
    errors: Option<ChildOutputReader>,
    children: Vec<WindowsCommandChild>,
    to_child: Buffer,
    to_local: Buffer,
    eof: bool,
}
impl Lane {
    pub(super) fn command(local: LocalStream, mut command: Command) -> io::Result<Self> {
        local.set_nonblocking(true)?;
        let (input, stdin) = ChildInputWriter::pipe()?;
        let (output, stdout) = ChildOutputReader::pipe()?;
        let (errors, stderr) = ChildOutputReader::pipe()?;
        command.stdin(stdin).stdout(stdout).stderr(stderr);
        let child = WindowsCommandChild::spawn(command).map_err(|e| e.error)?;
        Ok(Self {
            local,
            input: Some(input),
            stdio: false,
            draining_stdout: None,
            output,
            errors: Some(errors),
            children: vec![child],
            to_child: Buffer::default(),
            to_local: Buffer::default(),
            eof: false,
        })
    }
    pub(super) fn stdio(local: LocalStream) -> io::Result<Self> {
        local.set_nonblocking(true)?;
        let (input, stdin) = ChildInputWriter::pipe()?;
        let (output, stdout) = ChildOutputReader::pipe()?;
        let mut reader = copier()?;
        reader
            .stdin(Stdio::inherit())
            .stdout(stdout)
            .stderr(Stdio::null());
        let mut writer = copier()?;
        let mut reader = WindowsCommandChild::spawn(reader).map_err(|e| e.error)?;
        writer
            .stdin(stdin)
            .stdout(Stdio::inherit())
            .stderr(Stdio::null());
        let writer = match WindowsCommandChild::spawn(writer) {
            Ok(child) => child,
            Err(error) => {
                let _settled = reader.settle();
                return Err(error.error);
            }
        };
        Ok(Self {
            local,
            input: Some(input),
            stdio: true,
            draining_stdout: None,
            output,
            errors: None,
            children: vec![reader, writer],
            to_child: Buffer::default(),
            to_local: Buffer::default(),
            eof: false,
        })
    }
    fn drain_stdout(&mut self) -> io::Result<bool> {
        // Closing the writer's input is the only acknowledgement that it has
        // consumed all accepted bytes. Do not kill it before its stdout flush.
        if let Some(input) = &mut self.input {
            self.to_child.write(input)?;
        }
        if self.to_child.empty() {
            self.input.take();
        }
        if let Some(status) = self.children[1].try_wait()? {
            return if status.success() {
                Ok(false)
            } else {
                Err(io::ErrorKind::BrokenPipe.into())
            };
        }
        if self
            .draining_stdout
            .is_some_and(|deadline| Instant::now() >= deadline)
        {
            return Err(io::ErrorKind::TimedOut.into());
        }
        Ok(true)
    }
    pub(super) fn step(&mut self) -> io::Result<bool> {
        if self.draining_stdout.is_some() {
            return self.drain_stdout();
        }
        if let Some(errors) = &mut self.errors {
            let mut discard = [0; 4096];
            for _ in 0..8 {
                match errors.read(&mut discard) {
                    Ok(0) => break,
                    Ok(_) => {}
                    Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
                    Err(e) => return Err(e),
                }
            }
        }
        self.to_child
            .write(self.input.as_mut().expect("active lane input"))?;
        self.to_local.write(&mut self.local)?;
        if !self.eof {
            self.eof = !self.to_child.read(&mut self.local)?;
            if self.stdio && self.eof {
                self.draining_stdout = Some(Instant::now() + Duration::from_secs(5));
                return self.drain_stdout();
            }
            self.eof |= !self.to_local.read(&mut self.output)?;
        }
        self.to_child
            .write(self.input.as_mut().expect("active lane input"))?;
        self.to_local.write(&mut self.local)?;
        // An exited writer/SSH process cannot make progress. Reader EOF is
        // drained above before retirement; no unbounded stdout/stderr is retained.
        for child in &mut self.children {
            let _status = child.try_wait()?;
        }
        Ok(!self.eof || !self.to_child.empty() || !self.to_local.empty())
    }
    pub(super) fn settle(&mut self) -> io::Result<()> {
        let _closed = self.local.shutdown(std::net::Shutdown::Both);
        let mut failure = None;
        for child in &mut self.children {
            if let Err(error) = child.settle() {
                failure = Some(error);
            }
        }
        failure.map_or(Ok(()), Err)
    }
}
impl Drop for Lane {
    fn drop(&mut self) {
        let _settled = self.settle();
    }
}
fn copier() -> io::Result<Command> {
    let mut command = Command::new(
        PathBuf::from(winsafe::GetSystemDirectory().map_err(io::Error::other)?)
            .join("WindowsPowerShell/v1.0/powershell.exe"),
    );
    command.args(["-NoLogo", "-NoProfile", "-NonInteractive", "-Command", COPY]);
    Ok(command)
}
