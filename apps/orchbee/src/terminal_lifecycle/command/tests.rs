use std::{fmt, io};

use super::write_ansi_command;

struct SegmentedCommand;

impl crossterm::Command for SegmentedCommand {
    fn write_ansi(&self, output: &mut impl fmt::Write) -> fmt::Result {
        output.write_str("first")?;
        output.write_str("second")
    }

    #[cfg(windows)]
    fn execute_winapi(&self) -> io::Result<()> {
        panic!("direct ANSI encoding must not call the WinAPI fallback")
    }

    #[cfg(windows)]
    fn is_ansi_code_supported(&self) -> bool {
        panic!("direct ANSI encoding must not run a global capability probe")
    }
}

#[derive(Default)]
struct RecordingWriter {
    bytes: Vec<u8>,
    flushes: usize,
}

impl io::Write for RecordingWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.flushes += 1;
        Ok(())
    }
}

#[test]
fn writes_to_the_supplied_stream_without_owning_its_flush_boundary() -> io::Result<()> {
    let mut writer = RecordingWriter::default();

    write_ansi_command(&mut writer, &SegmentedCommand)?;

    assert_eq!(writer.bytes, b"firstsecond");
    assert_eq!(writer.flushes, 0);
    Ok(())
}

struct ContinueAfterFailure;

impl crossterm::Command for ContinueAfterFailure {
    fn write_ansi(&self, output: &mut impl fmt::Write) -> fmt::Result {
        let _ = output.write_str("first");
        let _ = output.write_str("second");
        Ok(())
    }

    #[cfg(windows)]
    fn execute_winapi(&self) -> io::Result<()> {
        unreachable!("the ANSI sink never invokes the WinAPI fallback")
    }

    #[cfg(windows)]
    fn is_ansi_code_supported(&self) -> bool {
        panic!("the ANSI sink never runs a global capability probe")
    }
}

struct FirstWriteFails {
    writes: usize,
}

impl io::Write for FirstWriteFails {
    fn write(&mut self, _bytes: &[u8]) -> io::Result<usize> {
        self.writes += 1;
        Err(if self.writes == 1 {
            io::Error::new(io::ErrorKind::BrokenPipe, "first fault")
        } else {
            io::Error::new(io::ErrorKind::PermissionDenied, "later fault")
        })
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn retains_the_first_io_fault_and_stops_touching_the_writer() {
    let mut writer = FirstWriteFails { writes: 0 };

    let error = write_ansi_command(&mut writer, &ContinueAfterFailure)
        .expect_err("the first writer fault must escape");

    assert_eq!(error.kind(), io::ErrorKind::BrokenPipe);
    assert_eq!(writer.writes, 1);
}

#[derive(Default)]
struct PartialThenFails {
    accepted: Vec<u8>,
    writes: usize,
}

impl io::Write for PartialThenFails {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.writes += 1;
        if self.writes == 1 {
            let accepted = bytes.len().min(2);
            self.accepted.extend_from_slice(&bytes[..accepted]);
            Ok(accepted)
        } else {
            Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "fault after partial progress",
            ))
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn preserves_a_fault_after_partial_progress_without_replaying_later_segments() {
    let mut writer = PartialThenFails::default();

    let error = write_ansi_command(&mut writer, &ContinueAfterFailure)
        .expect_err("partial progress followed by a writer fault must escape");

    assert_eq!(error.kind(), io::ErrorKind::BrokenPipe);
    assert_eq!(writer.accepted, b"fi");
    assert_eq!(writer.writes, 2);
}

#[derive(Default)]
struct ZeroProgressWriter {
    writes: usize,
}

impl io::Write for ZeroProgressWriter {
    fn write(&mut self, _bytes: &[u8]) -> io::Result<usize> {
        self.writes += 1;
        Ok(0)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn turns_zero_progress_into_write_zero_without_trying_a_later_segment() {
    let mut writer = ZeroProgressWriter::default();

    let error = write_ansi_command(&mut writer, &ContinueAfterFailure)
        .expect_err("zero progress on a nonempty command must fail");

    assert_eq!(error.kind(), io::ErrorKind::WriteZero);
    assert_eq!(writer.writes, 1);
}

struct InvalidCommand;

impl crossterm::Command for InvalidCommand {
    fn write_ansi(&self, _output: &mut impl fmt::Write) -> fmt::Result {
        Err(fmt::Error)
    }

    #[cfg(windows)]
    fn execute_winapi(&self) -> io::Result<()> {
        unreachable!("the ANSI sink never invokes the WinAPI fallback")
    }

    #[cfg(windows)]
    fn is_ansi_code_supported(&self) -> bool {
        panic!("the ANSI sink never runs a global capability probe")
    }
}

#[test]
fn classifies_a_command_only_format_fault_without_inventing_an_io_fault() {
    let error = write_ansi_command(&mut io::sink(), &InvalidCommand)
        .expect_err("an invalid command encoder must fail");

    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
}
