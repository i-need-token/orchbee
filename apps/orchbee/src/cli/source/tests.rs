use std::{
    env, fs,
    os::{fd::BorrowedFd, unix::process::ExitStatusExt},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::atomic::{AtomicU32, Ordering},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use nix::{
    errno::Errno,
    sys::{
        signal::{self, SigSet, Signal},
        stat::Mode,
    },
    unistd::mkfifo,
};

use super::*;

/// The literal `bytes` record bound the materializer enforces.
const BOUND: usize = crate::invocation::MAXIMUM_SEND_BYTES_BYTES;
use crate::invocation::{SendSourceCause, TerminalSendInput, UsageReason};

/// A watch that never reports a signal — the injected baseline for
/// materialization tests that do not exercise the armed guard.
struct IdleWatch;

impl SignalWatch for IdleWatch {
    fn poll_fd(&self) -> Option<BorrowedFd<'_>> {
        None
    }

    fn take(&mut self) -> Result<Option<i32>, Errno> {
        Ok(None)
    }
}

/// A watch whose `take` always fails — mock evidence that a signal-read
/// error routes to internal failure and never collapses to "none".
struct ErrWatch;

impl SignalWatch for ErrWatch {
    fn poll_fd(&self) -> Option<BorrowedFd<'_>> {
        None
    }

    fn take(&mut self) -> Result<Option<i32>, Errno> {
        Err(Errno::EIO)
    }
}

/// A watch reporting a scripted signal — mock evidence for mid-read
/// cancellation routing without touching real process signals.
struct ScriptedSignal(i32);

impl SignalWatch for ScriptedSignal {
    fn poll_fd(&self) -> Option<BorrowedFd<'_>> {
        None
    }

    fn take(&mut self) -> Result<Option<i32>, Errno> {
        Ok(Some(self.0))
    }
}

fn workspace(tag: &str) -> PathBuf {
    static SEQUENCE: AtomicU32 = AtomicU32::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let dir = env::temp_dir().join(format!(
        "obsrc-{tag}-{}-{nanos}-{}",
        std::process::id(),
        SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&dir).expect("create source test workspace");
    dir
}

fn write_file(dir: &Path, name: &str, bytes: &[u8]) -> String {
    let path = dir.join(name);
    fs::write(&path, bytes).expect("write fixture file");
    path.to_str().expect("fixture path is UTF-8").to_owned()
}

fn deadline() -> Instant {
    Instant::now() + Duration::from_secs(30)
}

fn file_input(path: &str, enter: bool) -> TerminalSendInput {
    TerminalSendInput::File {
        path: path.to_owned(),
        enter,
        paste: false,
    }
}

fn file_input_paste(path: &str, enter: bool) -> TerminalSendInput {
    TerminalSendInput::File {
        path: path.to_owned(),
        enter,
        paste: true,
    }
}

/// The localized usage cause a standard source failure carries.
fn cause(failure: &SourceFailure) -> SendSourceCause {
    let SourceFailure::Standard(Failure {
        usage: Some(reason),
        ..
    }) = failure
    else {
        panic!("expected a usage failure, got {failure:?}")
    };
    let UsageReason::SendSource { cause, .. } = reason.as_ref() else {
        panic!("expected a send-source usage reason")
    };
    *cause
}

/// The declared operand a source usage diagnostic echoes, when it does.
fn echoed_value(failure: &SourceFailure) -> Option<&str> {
    let SourceFailure::Standard(Failure {
        usage: Some(reason),
        ..
    }) = failure
    else {
        panic!("expected a usage failure, got {failure:?}")
    };
    let UsageReason::SendSource { value, .. } = reason.as_ref() else {
        panic!("expected a send-source usage reason")
    };
    value.as_deref()
}

/// The literal payload bytes a successful materialization produced —
/// whichever record variant the routing rule chose.
fn materialized_bytes(input: &TerminalSendInput, signals: &mut dyn SignalWatch) -> (Vec<u8>, bool) {
    match materialize(input, deadline(), signals)
        .expect("materialize")
        .into_owned()
    {
        TerminalSendInput::Text { text, enter } => (text.into_bytes(), enter),
        TerminalSendInput::Bytes { bytes, enter } => (bytes, enter),
        TerminalSendInput::Paste { content, enter } => (content, enter),
        other => panic!("materialization did not produce content: {other:?}"),
    }
}

/// The first guarded signal neither inherited-blocked nor inherited-
/// ignored, or none when the process excludes the whole set.
fn first_eligible() -> Option<Signal> {
    let ignored = inherited_ignored().expect("read inherited ignored set");
    let mask = SigSet::thread_get_mask().expect("read thread mask");
    GUARDED
        .into_iter()
        .find(|signal| ignored & (1_u64 << (*signal as u32 - 1)) == 0 && !mask.contains(*signal))
}

/// Proof that a signal stayed pending: a fresh signalfd over the probed
/// set reports the preserved pending signal (and drains it, so the test
/// leaves nothing behind that could fire later).
fn pending_signo(set: SigSet) -> Option<i32> {
    SignalFd::with_flags(&set, SfdFlags::SFD_NONBLOCK)
        .expect("probe signalfd")
        .read_signal()
        .expect("probe read")
        .map(|info| i32::try_from(info.ssi_signo).unwrap_or(0))
}

#[test]
fn status_field_accepts_only_one_valid_hex_value() {
    let good = "Name:\tfoo\nSigIgn:\t0000000000000006\nSigBlk:\t0\n";
    assert_eq!(status_field(good, "SigIgn:").unwrap(), 6);
    assert_eq!(
        status_field("SigIgn:\tffffffffffffffff\n", "SigIgn:").unwrap(),
        u64::MAX
    );
    for bad in [
        "SigIgn:\n",                    // empty
        "SigIgn:   \n",                 // whitespace only
        "SigIgn:\t\n",                  // tab only
        "SigIgn:\t00000000000000006\n", // seventeen digits
        "SigIgn:\t6 extra\n",           // trailing token
        "SigIgn:\tzz\n",                // non-hex
        "NoSuch:\t1\n",                 // missing field
        "SigIgn:\t1\nSigIgn:\t2\n",     // duplicated field
    ] {
        assert!(
            matches!(status_field(bad, "SigIgn:"), Err(ArmError::MalformedStatus)),
            "{bad:?} must be rejected"
        );
    }
}

#[test]
fn file_source_materializes_verbatim() {
    let dir = workspace("file-ok");
    let path = write_file(&dir, "input", b"line one\nline two\n");
    let mut watch = IdleWatch;
    let (bytes, enter) = materialized_bytes(&file_input(&path, false), &mut watch);
    assert_eq!(bytes, b"line one\nline two\n");
    assert!(!enter);
}

#[test]
fn file_record_boundaries() {
    let dir = workspace("file-cap");
    let mut watch = IdleWatch;
    // BOUND-1 bytes + --enter fits: the folded-CR flag counts +1 byte.
    let path = write_file(&dir, "fold-ok", &vec![b'x'; BOUND - 1]);
    let (bytes, enter) = materialized_bytes(&file_input(&path, true), &mut watch);
    assert_eq!(bytes.len(), BOUND - 1);
    assert!(enter, "--enter survives as the wire fold flag");
    // BOUND bytes without --enter is exactly the bound.
    let path = write_file(&dir, "exact", &vec![b'y'; BOUND]);
    let (bytes, enter) = materialized_bytes(&file_input(&path, false), &mut watch);
    assert_eq!(bytes.len(), BOUND);
    assert!(!enter);
    // BOUND bytes + --enter crosses the bound by one.
    let failure = materialize(&file_input(&path, true), deadline(), &mut watch).unwrap_err();
    assert_eq!(cause(&failure), SendSourceCause::Oversize { limit: BOUND });
    // BOUND+1 bytes crosses outright.
    let path = write_file(&dir, "over", &vec![b'z'; BOUND + 1]);
    let failure = materialize(&file_input(&path, false), deadline(), &mut watch).unwrap_err();
    assert_eq!(cause(&failure), SendSourceCause::Oversize { limit: BOUND });
}

/// Routing pins: ≤4096 valid UTF-8 stays the single `text` record;
/// anything else within 1 MiB becomes `bytes`; `--paste` keeps the
/// opaque 256 KiB form regardless of UTF-8 validity.
#[test]
fn file_source_routing() {
    let dir = workspace("file-route");
    let mut watch = IdleWatch;
    let path = write_file(&dir, "text", b"plain utf-8\n");
    let materialized = materialize(&file_input(&path, false), deadline(), &mut watch)
        .expect("small text")
        .into_owned();
    assert!(
        matches!(materialized, TerminalSendInput::Text { enter: false, .. }),
        "small UTF-8 must stay a text record: {materialized:?}"
    );
    // Valid UTF-8 over the text bound routes to bytes.
    let path = write_file(&dir, "large-text", &vec![b'x'; 4097]);
    let materialized = materialize(&file_input(&path, false), deadline(), &mut watch)
        .expect("large text")
        .into_owned();
    assert!(
        matches!(materialized, TerminalSendInput::Bytes { .. }),
        "over-bound UTF-8 must route to bytes: {materialized:?}"
    );
    // Invalid UTF-8 under the text bound also routes to bytes.
    let path = write_file(&dir, "binary", &[0xff, 0x00, 0xfe]);
    let materialized = materialize(&file_input(&path, false), deadline(), &mut watch)
        .expect("binary")
        .into_owned();
    assert!(
        matches!(materialized, TerminalSendInput::Bytes { .. }),
        "non-UTF-8 must route to bytes: {materialized:?}"
    );
}

/// `--paste` carries the opaque 256 KiB bound on its own axis: content
/// at the bound passes, one byte over is refused with the paste bound —
/// and `--enter` stays a separate flag rather than folding into bytes.
#[test]
fn file_source_paste_boundaries() {
    const PASTE_BOUND: usize = crate::invocation::MAXIMUM_SEND_PASTE_BYTES;
    let dir = workspace("file-paste");
    let mut watch = IdleWatch;
    let path = write_file(&dir, "exact", &vec![b'p'; PASTE_BOUND]);
    let (content, enter) = materialized_bytes(&file_input_paste(&path, true), &mut watch);
    assert_eq!(content.len(), PASTE_BOUND);
    assert!(enter, "--enter stays a separate flag on paste");
    let path = write_file(&dir, "over", &vec![b'p'; PASTE_BOUND + 1]);
    let failure = materialize(&file_input_paste(&path, false), deadline(), &mut watch).unwrap_err();
    assert_eq!(
        cause(&failure),
        SendSourceCause::Oversize { limit: PASTE_BOUND }
    );
    // Paste content is opaque: invalid UTF-8 and NULs pass untouched.
    let path = write_file(&dir, "opaque", &[0xff, 0x00, 0x1b, 0x5b]);
    let (content, _) = materialized_bytes(&file_input_paste(&path, false), &mut watch);
    assert_eq!(content, [0xff, 0x00, 0x1b, 0x5b]);
}

/// Every `--file` diagnostic echoes the path the caller named — the
/// oversize caught inside the bounded read used to lose it, so both
/// oversize routes are pinned to the declared path.
#[test]
fn file_oversize_echoes_its_declared_path() {
    let dir = workspace("file-oversize-path");
    let mut watch = IdleWatch;
    // The read loop hits BOUND+1.
    let path = write_file(&dir, "over", &vec![b'z'; BOUND + 1]);
    let failure = materialize(&file_input(&path, false), deadline(), &mut watch).unwrap_err();
    assert_eq!(cause(&failure), SendSourceCause::Oversize { limit: BOUND });
    assert_eq!(echoed_value(&failure), Some(path.as_str()));
    // BOUND + --enter overflows after a complete read — same echo.
    let path = write_file(&dir, "fold-over", &vec![b'y'; BOUND]);
    let failure = materialize(&file_input(&path, true), deadline(), &mut watch).unwrap_err();
    assert_eq!(cause(&failure), SendSourceCause::Oversize { limit: BOUND });
    assert_eq!(echoed_value(&failure), Some(path.as_str()));
}

#[test]
fn file_source_rejects_invalid_and_unsupported() {
    let dir = workspace("file-bad");
    let mut watch = IdleWatch;
    // Invalid UTF-8 and NUL bytes are legal literal bytes.
    let path = write_file(&dir, "binary", &[0xff, 0xfe, 0xfd, 0x00]);
    let (bytes, _) = materialized_bytes(&file_input(&path, false), &mut watch);
    assert_eq!(bytes, [0xff, 0xfe, 0xfd, 0x00]);
    // Empty.
    let path = write_file(&dir, "empty", b"");
    let failure = materialize(&file_input(&path, false), deadline(), &mut watch).unwrap_err();
    assert_eq!(cause(&failure), SendSourceCause::Empty);
    // A directory is not a regular file.
    let failure = materialize(
        &file_input(dir.to_str().expect("dir path"), false),
        deadline(),
        &mut watch,
    )
    .unwrap_err();
    assert_eq!(cause(&failure), SendSourceCause::NotRegularFile);
    // A FIFO is not a regular file — the open must not block on it.
    let fifo_path = dir.join("pipe");
    mkfifo(&fifo_path, Mode::S_IRUSR | Mode::S_IWUSR).expect("mkfifo");
    let failure = materialize(
        &file_input(fifo_path.to_str().expect("fifo path"), false),
        deadline(),
        &mut watch,
    )
    .unwrap_err();
    assert_eq!(cause(&failure), SendSourceCause::NotRegularFile);
    // A missing path is unreadable, never not-regular.
    let failure = materialize(
        &file_input("/nonexistent-obsrc-source-path", false),
        deadline(),
        &mut watch,
    )
    .unwrap_err();
    assert_eq!(cause(&failure), SendSourceCause::Unreadable);
}

#[test]
fn source_deadline_elapses_cleanly() {
    let dir = workspace("file-timeout");
    let path = write_file(&dir, "data", b"payload");
    let mut watch = IdleWatch;
    let past = Instant::now()
        .checked_sub(Duration::from_millis(1))
        .expect("past deadline");
    match materialize(&file_input(&path, false), past, &mut watch) {
        Err(SourceFailure::Timeout {
            source: InputSource::File,
        }) => {}
        other => panic!("past deadline must be a typed timeout, got {other:?}"),
    }
}

#[test]
fn signal_read_error_is_internal_not_none() {
    // Mock evidence: a failing watch must route internal failure — a
    // signal-read error never collapses to "no signal".
    let dir = workspace("file-err");
    let path = write_file(&dir, "data", b"payload");
    let mut watch = ErrWatch;
    match materialize(&file_input(&path, false), deadline(), &mut watch) {
        Err(SourceFailure::Standard(Failure {
            kind: FailureKind::InternalFailure,
            ..
        })) => {}
        other => panic!("signal read error must be internal failure, got {other:?}"),
    }
}

#[test]
fn mid_read_signal_selects_typed_cancellation() {
    let dir = workspace("file-int");
    let path = write_file(&dir, "data", b"payload");
    let mut watch = ScriptedSignal(15);
    match materialize(&file_input(&path, false), deadline(), &mut watch) {
        Err(SourceFailure::Cancelled {
            source: InputSource::File,
            signal: 15,
        }) => {}
        other => panic!("captured signal must select cancellation, got {other:?}"),
    }
}

#[test]
fn armed_capture_and_finalization_restore_exact_mask() {
    let before = SigSet::thread_get_mask().expect("thread mask");
    let Some(signal) = first_eligible() else {
        // The whole set excluded: an empty eligible set is still a real
        // armed policy — arm, observe no capture, finish clear, restore.
        let mut armed = ArmedSignals::arm().expect("arm empty eligible set");
        assert_eq!(armed.take().expect("take"), None);
        match armed.finish().expect("finish") {
            Finish::Clear => {}
            _ => panic!("an empty eligible set must finish clear"),
        }
        assert_eq!(SigSet::thread_get_mask().unwrap(), before);
        return;
    };
    let mut armed = ArmedSignals::arm().expect("arm signals");
    signal::raise(signal).expect("raise captured signal");
    assert_eq!(armed.take().expect("take"), Some(signal as i32));
    armed
        .into_finalization()
        .release()
        .expect("release restores mask");
    assert_eq!(SigSet::thread_get_mask().unwrap(), before);
}

#[test]
fn inherited_blocked_signal_excluded_and_pending_preserved() {
    // SIGINT blocked before arm: excluded from the captured set, and its
    // blocked+pending state must survive arm/finish untouched.
    let mut blocked = SigSet::empty();
    blocked.add(Signal::SIGINT);
    blocked.thread_block().expect("block SIGINT");
    signal::raise(Signal::SIGINT).expect("pend SIGINT");
    let before = SigSet::thread_get_mask().expect("thread mask");
    assert!(before.contains(Signal::SIGINT));
    let armed = ArmedSignals::arm().expect("arm with inherited block");
    match armed.finish().expect("finish") {
        Finish::Clear => {}
        _ => panic!("an excluded signal must not select cancellation"),
    }
    let after = SigSet::thread_get_mask().expect("thread mask");
    assert_eq!(before, after, "exact inherited mask restored");
    assert!(after.contains(Signal::SIGINT));
    // The inherited-blocked signal was excluded from the captured set:
    // a probe descriptor over SIGINT reports the preserved pending one.
    let mut probe = SigSet::empty();
    probe.add(Signal::SIGINT);
    assert_eq!(
        pending_signo(probe),
        Some(Signal::SIGINT as i32),
        "the pending SIGINT must be preserved, not swallowed"
    );
    // The signal stays blocked: unblocking it would deliver a further
    // pending SIGINT and terminate the test process.
}

/// The Agent owner's narrower set captures an eligible interruption and
/// restores the exact mask at finish — the same contract as `arm`, only
/// the membership differs.
#[test]
fn interrupts_capture_and_release_restore_exact_mask() {
    let before = SigSet::thread_get_mask().expect("thread mask");
    let ignored = inherited_ignored().expect("read inherited ignored set");
    let Some(signal) = INTERRUPTS
        .into_iter()
        .find(|signal| ignored & (1_u64 << (*signal as u32 - 1)) == 0 && !before.contains(*signal))
    else {
        // Both interruptions excluded: still a real armed policy — arm,
        // observe no capture, finish clear, restore.
        let mut armed = ArmedSignals::arm_interrupts().expect("arm empty eligible set");
        assert_eq!(armed.take().expect("take"), None);
        match armed.finish().expect("finish") {
            Finish::Clear => {}
            _ => panic!("an empty eligible set must finish clear"),
        }
        assert_eq!(SigSet::thread_get_mask().unwrap(), before);
        return;
    };
    let mut armed = ArmedSignals::arm_interrupts().expect("arm interruptions");
    signal::raise(signal).expect("raise captured signal");
    assert_eq!(armed.take().expect("take"), Some(signal as i32));
    armed
        .into_finalization()
        .release()
        .expect("release restores mask");
    assert_eq!(SigSet::thread_get_mask().unwrap(), before);
}

/// SIGHUP and SIGQUIT are outside the Agent owner's set entirely: arming
/// adds nothing for them to the thread mask, no raised member can be
/// captured, and finish restores the exact inherited mask.
#[test]
fn interrupts_never_mask_or_capture_hup_or_quit() {
    let before = SigSet::thread_get_mask().expect("thread mask");
    let mut armed = ArmedSignals::arm_interrupts().expect("arm interruptions");
    let masked = SigSet::thread_get_mask().expect("armed thread mask");
    for signal in [Signal::SIGHUP, Signal::SIGQUIT] {
        if before.contains(signal) {
            continue;
        }
        assert!(
            !masked.contains(signal),
            "{signal:?} must stay outside the Agent owner's mask"
        );
    }
    assert_eq!(armed.take().expect("take"), None);
    match armed.finish().expect("finish") {
        Finish::Clear => {}
        _ => panic!("no interruption was raised — finish must be clear"),
    }
    assert_eq!(SigSet::thread_get_mask().unwrap(), before);
}

/// An inherited-blocked member of the Agent set is excluded by the same
/// eligibility rule: SIGINT blocked before arm is never captured and the
/// pending signal survives arm/finish untouched.
#[test]
fn interrupts_inherited_blocked_int_excluded_and_pending_preserved() {
    let mut blocked = SigSet::empty();
    blocked.add(Signal::SIGINT);
    blocked.thread_block().expect("block SIGINT");
    signal::raise(Signal::SIGINT).expect("pend SIGINT");
    let before = SigSet::thread_get_mask().expect("thread mask");
    assert!(before.contains(Signal::SIGINT));
    let armed = ArmedSignals::arm_interrupts().expect("arm with inherited block");
    match armed.finish().expect("finish") {
        Finish::Clear => {}
        _ => panic!("an excluded signal must not select cancellation"),
    }
    let after = SigSet::thread_get_mask().expect("thread mask");
    assert_eq!(before, after, "exact inherited mask restored");
    assert!(after.contains(Signal::SIGINT));
    let mut probe = SigSet::empty();
    probe.add(Signal::SIGINT);
    assert_eq!(
        pending_signo(probe),
        Some(Signal::SIGINT as i32),
        "the pending SIGINT must be preserved, not swallowed"
    );
    // The signal stays blocked: unblocking it would deliver a further
    // pending SIGINT and terminate the test process.
}

/// An unwatched signal the process already blocks: SIGQUIT is pending
/// before the Agent arm and must survive it untouched — never captured,
/// never swallowed.
#[test]
fn interrupts_inherited_blocked_quit_pending_preserved() {
    let mut blocked = SigSet::empty();
    blocked.add(Signal::SIGQUIT);
    blocked.thread_block().expect("block SIGQUIT");
    signal::raise(Signal::SIGQUIT).expect("pend SIGQUIT");
    let before = SigSet::thread_get_mask().expect("thread mask");
    assert!(before.contains(Signal::SIGQUIT));
    let armed = ArmedSignals::arm_interrupts().expect("arm with blocked SIGQUIT");
    match armed.finish().expect("finish") {
        Finish::Clear => {}
        _ => panic!("an unwatched signal must never select cancellation"),
    }
    let after = SigSet::thread_get_mask().expect("thread mask");
    assert_eq!(before, after, "exact inherited mask restored");
    assert!(after.contains(Signal::SIGQUIT));
    let mut probe = SigSet::empty();
    probe.add(Signal::SIGQUIT);
    assert_eq!(
        pending_signo(probe),
        Some(Signal::SIGQUIT as i32),
        "the pending SIGQUIT must be preserved, not swallowed"
    );
    // SIGQUIT stays blocked on this thread: unblocking it would deliver
    // the pending signal and core-dump the test process.
}

/// A captured interruption at the finish boundary keeps the eligible
/// mask blocked until the token's release restores the exact inherited
/// mask — the same output-window contract `arm` gives the source stage.
#[test]
fn interrupts_finish_captured_signal_holds_mask_until_release() {
    let ignored = inherited_ignored().expect("read inherited ignored set");
    let before = SigSet::thread_get_mask().expect("thread mask");
    let Some(signal) = INTERRUPTS
        .into_iter()
        .find(|signal| ignored & (1_u64 << (*signal as u32 - 1)) == 0 && !before.contains(*signal))
    else {
        return;
    };
    let armed = ArmedSignals::arm_interrupts().expect("arm interruptions");
    signal::raise(signal).expect("raise captured signal");
    match armed.finish().expect("finish") {
        Finish::Cancelled {
            signo,
            finalization,
        } => {
            assert_eq!(signo, signal as i32);
            assert!(
                SigSet::thread_get_mask().unwrap().contains(signal),
                "the captured signal stays masked until release"
            );
            finalization.release().expect("release");
        }
        _ => panic!("captured signal must select cancellation"),
    }
    assert_eq!(SigSet::thread_get_mask().unwrap(), before);
}

#[test]
fn finish_read_error_restores_mask_then_reports() {
    // Mock evidence: a signalfd read error at the arbitration point
    // still attempts the owed restore before reporting the error.
    let before = SigSet::thread_get_mask().expect("thread mask");
    let armed = ArmedSignals::arm().expect("arm");
    match armed.finish_with(Err(Errno::EIO), || {}) {
        Err(error) => assert_eq!(error, Errno::EIO),
        Ok(_) => panic!("a signal read error must report failure"),
    }
    assert_eq!(SigSet::thread_get_mask().unwrap(), before);
}

#[test]
fn finish_captured_signal_holds_mask_until_release() {
    // A captured signal at the finish boundary keeps the eligible mask
    // blocked; the exact inherited mask is restored only by the token.
    let Some(signal) = first_eligible() else {
        return;
    };
    let before = SigSet::thread_get_mask().expect("thread mask");
    let armed = ArmedSignals::arm().expect("arm");
    signal::raise(signal).expect("raise captured signal");
    match armed.finish().expect("finish") {
        Finish::Cancelled {
            signo,
            finalization,
        } => {
            assert_eq!(signo, signal as i32);
            assert!(
                SigSet::thread_get_mask().unwrap().contains(signal),
                "the captured signal stays masked until release"
            );
            finalization.release().expect("release");
        }
        _ => panic!("captured signal must select cancellation"),
    }
    assert_eq!(SigSet::thread_get_mask().unwrap(), before);
}

/// A scripted monotonic clock for the `read_bounded_at` seam: each
/// `now` call returns the next scripted instant, then holds the last
/// one — the test decides exactly which deadline probe expires, so no
/// wall-clock sleep can pass at an earlier boundary.
struct ScriptedClock {
    times: Vec<Instant>,
    next: usize,
}

impl ScriptedClock {
    fn now(&mut self) -> Instant {
        let index = self.next.min(self.times.len().saturating_sub(1));
        self.next += 1;
        self.times[index]
    }
}

/// A reader that counts `read` invocations — the discriminating
/// observation for which boundary rejected the source.
struct CountingReader<R> {
    inner: R,
    reads: usize,
}

impl<R: Read> Read for CountingReader<R> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        self.reads += 1;
        self.inner.read(buffer)
    }
}

/// A watch whose injected channel is always poll-ready and never
/// reports a signal — pairs with the scripted clock to reach the
/// post-wait arbitration point without writer timing.
struct ReadyWatch<'a>(BorrowedFd<'a>);

impl SignalWatch for ReadyWatch<'_> {
    fn poll_fd(&self) -> Option<BorrowedFd<'_>> {
        Some(self.0)
    }

    fn take(&mut self) -> Result<Option<i32>, Errno> {
        Ok(None)
    }
}

#[test]
fn deadline_arbitrates_ready_data_after_wait() {
    // Script: the loop-top probe is inside the budget; the post-wait,
    // pre-read probe lands past it. Both descriptors are poll-ready so
    // the wait itself never gates on the clock, and the reader count
    // proves the ready byte was rejected before it could be read.
    let t0 = Instant::now();
    let deadline = t0 + Duration::from_secs(1);
    let (source_read, source_write) = nix::unistd::pipe().expect("source pipe");
    let (signal_read, signal_write) = nix::unistd::pipe().expect("signal pipe");
    nix::unistd::write(&source_write, b"x").expect("seed source byte");
    nix::unistd::write(&signal_write, b"s").expect("seed signal byte");
    let source_file = fs::File::from(source_read);
    let mut watch = ReadyWatch(signal_read.as_fd());
    let mut clock = ScriptedClock {
        times: vec![
            t0 + Duration::from_millis(10),
            deadline + Duration::from_millis(10),
        ],
        next: 0,
    };
    let mut reader = CountingReader {
        inner: &source_file,
        reads: 0,
    };
    let failure = read_bounded_at(
        &mut reader,
        ReadPlan {
            fd: source_file.as_fd(),
            pollable: true,
            deadline,
            limit: BOUND,
            source: InputSource::File,
            operand: None,
        },
        &mut watch,
        || clock.now(),
    )
    .unwrap_err();
    assert!(
        matches!(failure, SourceFailure::Timeout { .. }),
        "ready data after deadline expiry must be timeout, got {failure:?}"
    );
    assert_eq!(
        reader.reads, 0,
        "the post-wait check must reject before any read"
    );
    drop(signal_write);
}

#[test]
fn deadline_arbitrates_eof_at_success_boundary() {
    // Script: loop-top and pre-read probes inside the budget, the
    // completion-boundary probe past it. The reader count proves the
    // EOF was actually consumed — the final check fired, not an
    // earlier loop check.
    let t0 = Instant::now();
    let deadline = t0 + Duration::from_secs(1);
    let (read_end, _write_end) = nix::unistd::pipe().expect("pipe");
    let mut watch = IdleWatch;
    let mut clock = ScriptedClock {
        times: vec![
            t0 + Duration::from_millis(10),
            t0 + Duration::from_millis(20),
            deadline + Duration::from_millis(10),
        ],
        next: 0,
    };
    let mut reader = CountingReader {
        inner: io::empty(),
        reads: 0,
    };
    let failure = read_bounded_at(
        &mut reader,
        ReadPlan {
            fd: read_end.as_fd(),
            pollable: false,
            deadline,
            limit: BOUND,
            source: InputSource::Stdin,
            operand: None,
        },
        &mut watch,
        || clock.now(),
    )
    .unwrap_err();
    assert!(
        matches!(failure, SourceFailure::Timeout { .. }),
        "a late EOF must arbitrate to timeout, got {failure:?}"
    );
    assert_eq!(
        reader.reads, 1,
        "the EOF was consumed — the boundary check itself fired"
    );
}

/// A stateful watch reporting nothing on the first drain and the
/// scripted signal on the second — so the test reaches the completion
/// boundary before cancellation is consumed (unlike `ScriptedSignal`,
/// which fires at the loop-top drain).
struct BoundarySignal {
    signal: i32,
    takes: usize,
}

impl SignalWatch for BoundarySignal {
    fn poll_fd(&self) -> Option<BorrowedFd<'_>> {
        None
    }

    fn take(&mut self) -> Result<Option<i32>, Errno> {
        self.takes += 1;
        Ok((self.takes > 1).then_some(self.signal))
    }
}

#[test]
fn signal_wins_over_expired_deadline_at_boundary() {
    // Deterministic trace: drain#1 none, clock#1/#2 in budget, reader
    // returns EOF once, drain#2 yields the signal — cancellation is
    // selected before clock#3 would report the budget closed.
    let t0 = Instant::now();
    let deadline = t0 + Duration::from_secs(1);
    let (read_end, _write_end) = nix::unistd::pipe().expect("pipe");
    let mut watch = BoundarySignal {
        signal: 2,
        takes: 0,
    };
    let mut clock = ScriptedClock {
        times: vec![
            t0 + Duration::from_millis(10),
            t0 + Duration::from_millis(20),
            deadline + Duration::from_millis(10),
        ],
        next: 0,
    };
    let mut reader = CountingReader {
        inner: io::empty(),
        reads: 0,
    };
    let failure = read_bounded_at(
        &mut reader,
        ReadPlan {
            fd: read_end.as_fd(),
            pollable: false,
            deadline,
            limit: BOUND,
            source: InputSource::Stdin,
            operand: None,
        },
        &mut watch,
        || clock.now(),
    )
    .unwrap_err();
    match failure {
        SourceFailure::Cancelled { source, signal } => {
            assert!(matches!(source, InputSource::Stdin));
            assert_eq!(signal, 2);
        }
        other => panic!("a captured signal must outrank the expired deadline, got {other:?}"),
    }
    assert_eq!(reader.reads, 1, "the EOF was consumed — boundary reached");
    assert_eq!(watch.takes, 2, "the second drain selected cancellation");
    assert_eq!(
        clock.next, 2,
        "the expired clock probe was never consumed — signal won first"
    );
}

/// Child-mode probe: a signal raised in the documented late window —
/// after the last arbitration read and before the owed restore — must
/// reach its original disposition, i.e. OS termination before any
/// runtime admission, never typed cancellation or silence.
const LATE_CHILD_ENV: &str = "ORCHBEE_TEST_LATE_SIGNAL_CHILD";

#[test]
fn late_signal_in_finish_window_dies_by_os_disposition() {
    if env::var_os(LATE_CHILD_ENV).is_some() {
        // Child: the barrier runs strictly after the final arbitration
        // read; the restore must unmask the late signal into its
        // original disposition — the process dies by that signal.
        let signal = first_eligible().expect("an eligible signal to probe");
        let armed = ArmedSignals::arm().expect("arm");
        let _ = armed.finish_at(|| signal::raise(signal).expect("raise late signal"));
        // Reaching here means the late signal was silently swallowed.
        std::process::abort();
    }
    let output = Command::new(env::current_exe().expect("test binary"))
        .args([
            "--exact",
            "cli::source::tests::late_signal_in_finish_window_dies_by_os_disposition",
            "--test-threads=1",
            "--nocapture",
        ])
        .env(LATE_CHILD_ENV, "1")
        .stdin(Stdio::null())
        .output()
        .expect("spawn child test");
    let signo = output
        .status
        .signal()
        .expect("child must die by a signal, not exit or abort");
    assert!(
        GUARDED.iter().any(|guarded| *guarded as i32 == signo),
        "death must come from the probed guarded set, got {signo}"
    );
}
