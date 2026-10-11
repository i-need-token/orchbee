use std::{
    env, fs,
    os::unix::process::ExitStatusExt,
    process::{Command, Stdio},
};

use super::*;
use cli::{source::ArmedSignals, source::SignalWatch};
use nix::sys::signal::{self, Signal};

/// A writer that fails every write — write-failure routing evidence.
struct FailingWriter;

impl Write for FailingWriter {
    fn write(&mut self, _buffer: &[u8]) -> io::Result<usize> {
        Err(io::Error::new(io::ErrorKind::BrokenPipe, "injected"))
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// A writer that raises SIGTERM on its first write — the second signal
/// arriving inside the cancelled-output window.
struct RaisingWriter {
    inner: fs::File,
    raised: bool,
}

impl Write for RaisingWriter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        if !self.raised {
            self.raised = true;
            signal::raise(Signal::SIGTERM).expect("raise second signal");
        }
        self.inner.write(buffer)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

fn answer(finalization: Option<cli::SourceFinalization>) -> cli::Output {
    cli::Output {
        status: 130,
        stdout: b"{\"schema\":\"test\"}\n".to_vec(),
        stderr: String::new(),
        finalization,
    }
}

#[test]
fn emit_consumes_finalization_on_normal_path() {
    // A real token captured at the current mask: `emit` must consume it,
    // restore nothing observable, and report the answer's status.
    let mask = nix::sys::signal::SigSet::thread_get_mask().expect("thread mask");
    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = emit_with(
        answer(Some(cli::SourceFinalization::new(mask))),
        Locale::EnUs,
        &mut out,
        &mut err,
    );
    assert_eq!(code, ExitCode::from(130));
    assert_eq!(out, b"{\"schema\":\"test\"}\n");
}

#[test]
fn emit_consumes_finalization_on_write_failure() {
    // The stdout write fails: the localized output-failure diagnostic
    // lands on stderr, the token is still consumed, and the status is
    // the write-failure code — never the answer's.
    let mask = nix::sys::signal::SigSet::thread_get_mask().expect("thread mask");
    let mut out = FailingWriter;
    let mut err = Vec::new();
    let code = emit_with(
        answer(Some(cli::SourceFinalization::new(mask))),
        Locale::EnUs,
        &mut out,
        &mut err,
    );
    assert_eq!(code, ExitCode::FAILURE);
    assert!(
        !err.is_empty(),
        "the output-failure diagnostic must reach stderr"
    );
}

#[test]
fn emit_reports_restore_failure_as_terminal_internal() {
    // Mock evidence: a token whose release reports a kernel failure must
    // route the localized internal-failure diagnostic and exit 1 — no
    // second machine document, no answer status.
    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = emit_with(
        answer(Some(cli::SourceFinalization::failing())),
        Locale::EnUs,
        &mut out,
        &mut err,
    );
    assert_eq!(code, ExitCode::FAILURE);
    assert!(
        !err.is_empty(),
        "restore failure must be diagnosed on stderr"
    );
    assert_eq!(out, b"{\"schema\":\"test\"}\n");
}

/// Child-mode probe: a second signal arriving mid-output while the
/// finalization token still holds the mask must not preempt the output —
/// the complete document is written before the release unmasks the
/// pending signal into its original disposition.
const CANCELLED_CHILD_ENV: &str = "ORCHBEE_TEST_CANCELLED_OUTPUT_CHILD";

#[test]
fn cancelled_output_completes_before_unmask() {
    if let Ok(path) = env::var(CANCELLED_CHILD_ENV) {
        // Child: capture SIGINT into a token, then emit through a writer
        // that raises SIGTERM mid-write. The mask must hold through the
        // last write; release then delivers SIGTERM by disposition.
        let mut armed = ArmedSignals::arm().expect("arm");
        signal::raise(Signal::SIGINT).expect("capture first signal");
        assert_eq!(armed.take().expect("take"), Some(Signal::SIGINT as i32));
        let token = armed.into_finalization();
        let mut out = RaisingWriter {
            inner: fs::File::create(path).expect("child output file"),
            raised: false,
        };
        let mut err = io::sink();
        let _ = emit_with(answer(Some(token)), Locale::EnUs, &mut out, &mut err);
        // Reaching here means SIGTERM was lost, not held to release.
        std::process::abort();
    }
    let file = env::temp_dir().join(format!(
        "obemit-cancelled-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    let output = Command::new(env::current_exe().expect("test binary"))
        .args([
            "--exact",
            "tests::cancelled_output_completes_before_unmask",
            "--test-threads=1",
            "--nocapture",
        ])
        .env(CANCELLED_CHILD_ENV, &file)
        .stdin(Stdio::null())
        .output()
        .expect("spawn child test");
    assert_eq!(
        output.status.signal(),
        Some(Signal::SIGTERM as i32),
        "the mid-output signal must be delivered at release"
    );
    assert_eq!(
        fs::read(&file).expect("read child output"),
        b"{\"schema\":\"test\"}\n",
        "the complete document lands before the mask is released"
    );
}

/// The same child-mode probe armed through `arm_interrupts` — the Agent
/// INT/TERM watch — proving the finalization token it produces holds the
/// mask through the output window exactly like the source arm's token.
const CANCELLED_INTERRUPTS_CHILD_ENV: &str = "ORCHBEE_TEST_CANCELLED_INTERRUPTS_OUTPUT_CHILD";

#[test]
fn cancelled_interrupts_output_completes_before_unmask() {
    if let Ok(path) = env::var(CANCELLED_INTERRUPTS_CHILD_ENV) {
        // Child: capture SIGINT into a token through the Agent INT/TERM
        // arm, then emit through a writer that raises SIGTERM mid-write.
        let mut armed = ArmedSignals::arm_interrupts().expect("arm interrupts");
        signal::raise(Signal::SIGINT).expect("capture first signal");
        assert_eq!(armed.take().expect("take"), Some(Signal::SIGINT as i32));
        let token = armed.into_finalization();
        let mut out = RaisingWriter {
            inner: fs::File::create(path).expect("child output file"),
            raised: false,
        };
        let mut err = io::sink();
        let _ = emit_with(answer(Some(token)), Locale::EnUs, &mut out, &mut err);
        std::process::abort();
    }
    let file = env::temp_dir().join(format!(
        "obemit-cancelled-interrupts-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    let output = Command::new(env::current_exe().expect("test binary"))
        .args([
            "--exact",
            "tests::cancelled_interrupts_output_completes_before_unmask",
            "--test-threads=1",
            "--nocapture",
        ])
        .env(CANCELLED_INTERRUPTS_CHILD_ENV, &file)
        .stdin(Stdio::null())
        .output()
        .expect("spawn child test");
    assert_eq!(
        output.status.signal(),
        Some(Signal::SIGTERM as i32),
        "the mid-output signal must be delivered at release"
    );
    assert_eq!(
        fs::read(&file).expect("read child output"),
        b"{\"schema\":\"test\"}\n",
        "the complete document lands before the mask is released"
    );
}
