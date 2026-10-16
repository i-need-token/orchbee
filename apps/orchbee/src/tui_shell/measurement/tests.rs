use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
    error::Error,
    fmt,
    io::{self, BufWriter, IoSlice, Write},
    rc::Rc,
    sync::Arc,
    time::{Duration, Instant},
};

use super::{
    Active, Batch, CompletionFailure, Event, Journal, MeasuredWriter, Measurements, Session,
    finish_run, write_journal,
};
use crate::{
    localization::Locale, terminal_lifecycle::SynchronizedUpdateLease,
    tui_backend::RestorationGatedWriter,
};

fn observe(limit: usize) -> Measurements {
    Measurements(Some(Rc::new(Active {
        origin: Instant::now(),
        utc_alignment: None,
        journal: RefCell::new(Journal::new(limit).expect("bounded test journal")),
    })))
}

fn document(measurements: &Measurements, ran: bool, restored: bool) -> serde_json::Value {
    let mut bytes = Vec::new();
    write_journal(
        &mut bytes,
        measurements.0.as_ref().expect("enabled fixture"),
        ran,
        restored,
    )
    .expect("serialize to a vector");
    serde_json::from_slice(&bytes).expect("complete diagnostic JSON")
}

#[test]
fn disabled_observer_has_no_sample_storage_and_never_calls_the_clock() -> io::Result<()> {
    let session = Session::open(None, Locale::EnUs)?;
    let measurements = session.measurements();
    assert!(
        measurements.0.is_none(),
        "no journal, vector or shared allocation"
    );
    assert!(
        measurements
            .start_with(|| panic!("disabled clock sampled"))
            .is_none()
    );
    assert!(measurements.frame().is_none());
    let mut writer = MeasuredWriter::new(io::sink(), measurements);
    assert_eq!(writer.write(b"ab")?, 2);
    writer.write_all(b"cd")?;
    assert_eq!(writer.write_vectored(&[IoSlice::new(b"ef")])?, 2);
    writer.flush()?;
    let restored = Cell::new(false);
    session.finish_after_restore(
        Ok(()),
        || {
            restored.set(true);
            Ok(())
        },
        Locale::EnUs,
    )?;
    assert!(restored.get());
    Ok(())
}

#[test]
fn sample_cap_retains_the_first_intervals_and_declares_dropped_coverage() {
    let measurements = observe(2);
    let active = measurements.0.as_ref().expect("enabled");
    for milliseconds in [1, 3] {
        measurements
            .start_with(|| active.origin + Duration::from_millis(milliseconds))
            .expect("room")
            .finish_at(
                Event::Control { succeeded: true },
                active.origin + Duration::from_millis(milliseconds + 1),
            );
    }
    assert!(
        measurements
            .start_with(|| panic!("full collector sampled time"))
            .is_none()
    );
    let value = document(&measurements, true, true);
    assert_eq!(value["sample_limit"], 2);
    assert_eq!(value["samples"].as_array().expect("samples").len(), 2);
    assert_eq!(value["samples"][0]["started_ns"], 1_000_000);
    assert_eq!(value["samples"][1]["ended_ns"], 4_000_000);
    assert_eq!(value["dropped_samples"], 1);
    assert_eq!(value["outstanding_samples"], 0);
    assert_eq!(value["complete"], false);
}

#[test]
fn nested_completion_at_the_cap_and_abandoned_spans_cannot_look_complete() {
    let measurements = observe(1);
    let outer = measurements.start().expect("outer");
    measurements.start().expect("inner").control_completed(true);
    outer.control_completed(true);
    assert_eq!(document(&measurements, true, true)["dropped_samples"], 1);

    let measurements = observe(3);
    let outstanding = measurements.start().expect("outstanding");
    let value = document(&measurements, true, true);
    assert_eq!(value["outstanding_samples"], 1);
    assert_eq!(value["complete"], false);
    drop(outstanding);
    let value = document(&measurements, true, true);
    assert_eq!(value["outstanding_samples"], 0);
    assert_eq!(value["abandoned_samples"], 1);
    assert_eq!(value["complete"], false);
}

#[test]
fn failed_operations_invalid_time_and_counter_exhaustion_remain_explicit() {
    let measurements = observe(4);
    measurements
        .start()
        .expect("room")
        .finish(Event::Flush { succeeded: false });
    assert_eq!(document(&measurements, true, true)["failed_samples"], 1);
    assert_eq!(document(&measurements, true, true)["complete"], false);

    let active = measurements.0.as_ref().expect("enabled");
    measurements
        .start_with(|| active.origin + Duration::from_millis(2))
        .expect("room")
        .finish_at(
            Event::Control { succeeded: true },
            active.origin + Duration::from_millis(1),
        );
    assert_eq!(
        document(&measurements, true, true)["invalid_clock_or_counts"],
        true
    );
    assert_eq!(document(&measurements, true, true)["dropped_samples"], 1);

    let measurements = observe(4);
    let active = measurements.0.as_ref().expect("enabled");
    active.journal.borrow_mut().batches = u64::MAX;
    assert_eq!(
        measurements
            .start()
            .expect("room")
            .input_completed(&Ok(Some(vec![b'x']))),
        None
    );
    active.journal.borrow_mut().frames = u64::MAX;
    assert!(measurements.frame().is_none());
    assert_eq!(document(&measurements, true, true)["complete"], false);
    assert_eq!(active.journal.borrow().batches, u64::MAX);
    assert_eq!(active.journal.borrow().frames, u64::MAX);
}

#[test]
fn completed_packets_keep_distinct_batch_identity_without_recording_their_contents() {
    let measurements = observe(10);
    let first = measurements
        .start()
        .expect("read")
        .input_completed(&Ok(Some(b"private-input".to_vec())));
    measurements
        .start()
        .expect("dispatch")
        .dispatch_completed(first, true);
    let second = measurements
        .start()
        .expect("read")
        .input_completed(&Ok(Some(b"xy".to_vec())));
    assert_eq!((first, second), (Some(Batch(1)), Some(Batch(2))));
    assert_eq!(
        measurements
            .start()
            .expect("read")
            .input_completed(&Ok(None)),
        None
    );
    let mut frame = measurements.frame().expect("frame");
    frame.buffer_started();
    frame.buffer_prepared();
    frame.finish(true);
    let value = document(&measurements, true, true);
    assert_eq!(value["samples"][0]["event"]["bytes"], 13);
    assert_eq!(value["samples"][1]["event"]["batch"], 1);
    assert_eq!(value["samples"][3]["event"]["result"], "idle");
    assert_eq!(value["samples"][4]["event"]["last_input_batch"], 2);
    assert_eq!(value["complete"], true);
    assert!(!value.to_string().contains("private-input"));
    assert_eq!(document(&measurements, false, true)["complete"], false);
    assert_eq!(document(&measurements, true, false)["complete"], false);
}

#[test]
fn successful_frame_without_buffer_checkpoints_is_an_invalid_trace() {
    let measurements = observe(3);
    measurements.frame().expect("frame").finish(true);
    let value = document(&measurements, true, true);
    assert_eq!(value["invalid_clock_or_counts"], true);
    assert_eq!(value["complete"], false);
}

#[derive(Debug, Eq, PartialEq)]
enum Call {
    Write(Vec<u8>),
    All(Vec<u8>),
    Vectored(Vec<Vec<u8>>),
    Flush,
    Dropped,
}

#[derive(Debug)]
struct Tagged(u8);
impl fmt::Display for Tagged {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "fixture-{}", self.0)
    }
}
impl Error for Tagged {}

fn fault(kind: io::ErrorKind, tag: u8) -> io::Error {
    io::Error::new(kind, Tagged(tag))
}

struct Spy {
    calls: Rc<RefCell<Vec<Call>>>,
    writes: usize,
    all_fails: bool,
}
impl Write for Spy {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        self.calls.borrow_mut().push(Call::Write(buffer.to_vec()));
        self.writes += 1;
        if self.writes == 1 {
            Ok(buffer.len().min(2))
        } else {
            Err(fault(io::ErrorKind::BrokenPipe, 7))
        }
    }
    fn write_all(&mut self, buffer: &[u8]) -> io::Result<()> {
        self.calls.borrow_mut().push(Call::All(buffer.to_vec()));
        if self.all_fails {
            Err(fault(io::ErrorKind::PermissionDenied, 9))
        } else {
            Ok(())
        }
    }
    fn write_vectored(&mut self, buffers: &[IoSlice<'_>]) -> io::Result<usize> {
        self.calls.borrow_mut().push(Call::Vectored(
            buffers.iter().map(|buffer| buffer.to_vec()).collect(),
        ));
        Ok(3)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.calls.borrow_mut().push(Call::Flush);
        Err(fault(io::ErrorKind::TimedOut, 8))
    }
}
impl Drop for Spy {
    fn drop(&mut self) {
        self.calls.borrow_mut().push(Call::Dropped);
    }
}

#[test]
fn measured_writer_forwards_short_writes_errors_vectored_and_overridden_write_all_once() {
    for all_fails in [false, true] {
        let measurements = observe(10);
        let calls = Rc::new(RefCell::new(Vec::new()));
        let mut writer = MeasuredWriter::new(
            Spy {
                calls: Rc::clone(&calls),
                writes: 0,
                all_fails,
            },
            measurements.clone(),
        );
        assert_eq!(writer.write(b"abcdef").expect("short"), 2);
        let error = writer.write(b"cdef").expect_err("exact error");
        assert_eq!(error.kind(), io::ErrorKind::BrokenPipe);
        assert_eq!(
            error
                .get_ref()
                .and_then(|error| error.downcast_ref::<Tagged>())
                .map(|error| error.0),
            Some(7)
        );
        assert_eq!(writer.write_all(b"private-output").is_err(), all_fails);
        assert_eq!(
            writer
                .write_vectored(&[IoSlice::new(b"ab"), IoSlice::new(b"cde")])
                .expect("vectored"),
            3
        );
        let error = writer.flush().expect_err("flush error");
        assert_eq!(error.kind(), io::ErrorKind::TimedOut);
        assert_eq!(
            error
                .get_ref()
                .and_then(|error| error.downcast_ref::<Tagged>())
                .map(|error| error.0),
            Some(8)
        );
        drop(writer);
        assert_eq!(
            *calls.borrow(),
            [
                Call::Write(b"abcdef".to_vec()),
                Call::Write(b"cdef".to_vec()),
                Call::All(b"private-output".to_vec()),
                Call::Vectored(vec![b"ab".to_vec(), b"cde".to_vec()]),
                Call::Flush,
                Call::Dropped
            ]
        );
        let value = document(&measurements, true, true);
        assert_eq!(value["samples"].as_array().expect("samples").len(), 5);
        assert_eq!(value["samples"][0]["event"]["accepted_bytes"], 2);
        assert_eq!(
            value["samples"][1]["event"]["accepted_bytes"],
            serde_json::Value::Null
        );
        assert_eq!(value["samples"][2]["event"]["call"], "write_all");
        assert_eq!(value["samples"][3]["event"]["requested_bytes"], 5);
        assert_eq!(value["samples"][3]["event"]["accepted_bytes"], 3);
        assert!(!value.to_string().contains("private-output"));
        assert_eq!(value["complete"], false);
    }
}

#[derive(Default)]
struct DefaultWriter {
    calls: Vec<Vec<u8>>,
    attempts: usize,
}
impl Write for DefaultWriter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        self.calls.push(buffer.to_vec());
        self.attempts += 1;
        if self.attempts == 2 {
            Err(io::ErrorKind::Interrupted.into())
        } else {
            Ok(buffer.len().min(2))
        }
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn underlying_default_methods_keep_their_original_short_write_and_interrupted_policy()
-> io::Result<()> {
    let mut original = DefaultWriter::default();
    original.write_all(b"abcde")?;
    let mut measured = MeasuredWriter::new(DefaultWriter::default(), observe(10));
    measured.write_all(b"abcde")?;
    assert_eq!(measured.inner.calls, original.calls);
    let buffers = [IoSlice::new(b""), IoSlice::new(b"def")];
    assert_eq!(
        measured.write_vectored(&buffers)?,
        original.write_vectored(&buffers)?
    );
    assert_eq!(measured.inner.calls, original.calls);
    Ok(())
}

#[test]
fn the_real_buffer_and_restoration_fence_do_not_replay_a_failed_prefix_on_drop() {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let lease = Arc::new(SynchronizedUpdateLease::new());
    let measurements = observe(10);
    let writer = MeasuredWriter::new(
        Spy {
            calls: Rc::clone(&calls),
            writes: 0,
            all_fails: false,
        },
        measurements.clone(),
    );
    let mut buffered =
        BufWriter::with_capacity(4, RestorationGatedWriter::new(writer, Arc::clone(&lease)));
    assert_eq!(
        buffered
            .write_all(b"abcdef")
            .expect_err("prefix then error")
            .kind(),
        io::ErrorKind::BrokenPipe
    );
    lease.block_frame_output();
    drop(buffered);
    assert_eq!(
        *calls.borrow(),
        [
            Call::Write(b"abcdef".to_vec()),
            Call::Write(b"cdef".to_vec()),
            Call::Dropped
        ]
    );
    assert_eq!(
        document(&measurements, false, true)["samples"]
            .as_array()
            .expect("samples")
            .len(),
        2
    );
}

fn primary_tag(error: &io::Error) -> Option<u8> {
    let inner = error.get_ref()?;
    if let Some(failure) = inner.downcast_ref::<CompletionFailure>() {
        primary_tag(&failure.primary)
    } else {
        inner.downcast_ref::<Tagged>().map(|error| error.0)
    }
}

#[test]
fn restoration_always_precedes_serialization_and_all_failures_keep_the_primary_cause() {
    for locale in [Locale::EnUs, Locale::ZhCn] {
        for runtime_fails in [false, true] {
            for restore_fails in [false, true] {
                for diagnostic_fails in [false, true] {
                    let order = RefCell::new(Vec::new());
                    let runtime = if runtime_fails {
                        Err(fault(io::ErrorKind::BrokenPipe, 10))
                    } else {
                        Ok(())
                    };
                    let result = finish_run(
                        runtime,
                        || {
                            order.borrow_mut().push("restore");
                            if restore_fails {
                                Err(fault(io::ErrorKind::TimedOut, 20))
                            } else {
                                Ok(())
                            }
                        },
                        |ran, restored| {
                            order.borrow_mut().push("serialize");
                            assert_eq!((ran, restored), (!runtime_fails, !restore_fails));
                            if diagnostic_fails {
                                Err(fault(io::ErrorKind::PermissionDenied, 30))
                            } else {
                                Ok(())
                            }
                        },
                        locale,
                    );
                    assert_eq!(*order.borrow(), ["restore", "serialize"]);
                    let expected = if runtime_fails {
                        Some((io::ErrorKind::BrokenPipe, 10))
                    } else if restore_fails {
                        Some((io::ErrorKind::TimedOut, 20))
                    } else if diagnostic_fails {
                        Some((io::ErrorKind::PermissionDenied, 30))
                    } else {
                        None
                    };
                    match expected {
                        Some((kind, tag)) => {
                            let error = result.expect_err("a failure remains a failure");
                            assert_eq!(error.kind(), kind);
                            assert_eq!(primary_tag(&error), Some(tag));
                            if runtime_fails && restore_fails {
                                assert!(
                                    error
                                        .to_string()
                                        .contains(locale.tui_restoration_also_failed())
                                );
                            }
                            if (runtime_fails || restore_fails) && diagnostic_fails {
                                assert!(
                                    error
                                        .to_string()
                                        .contains(locale.tui_measurement_also_failed())
                                );
                            }
                        }
                        None => assert!(result.is_ok()),
                    }
                }
            }
        }
    }
}

#[test]
fn configuration_path_diagnostics_preserve_the_env_name_in_both_locales() {
    for locale in [Locale::EnUs, Locale::ZhCn] {
        let error = Session::open(Some("relative.json".into()), locale)
            .err()
            .expect("relative path refused before I/O");
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        assert_eq!(error.to_string(), locale.tui_measurement_path_invalid());
        assert!(error.to_string().contains("ORCHBEE_TUI_MEASUREMENTS"));
        assert!(Session::open(Some("".into()), locale).is_err());
    }
}

#[test]
fn the_actual_loop_correlates_read_dispatch_and_draw_without_claiming_feedback() {
    use super::super::{Chrome, InputSource, LoopExit, ViewportSource};
    use crate::{terminal_device::WindowDimensions, termination_signal::TerminationSource};
    use ratatui::{Terminal, TerminalOptions, Viewport, backend::TestBackend};
    struct Inputs {
        packets: VecDeque<Vec<u8>>,
        measurements: Measurements,
    }
    impl InputSource for Inputs {
        fn next_input(&mut self, _: Duration) -> io::Result<Option<Vec<u8>>> {
            self.packets
                .pop_front()
                .map(Some)
                .ok_or_else(|| io::ErrorKind::UnexpectedEof.into())
        }
        fn measurements(&self) -> Measurements {
            self.measurements.clone()
        }
    }
    struct ViewportSize(WindowDimensions);
    impl ViewportSource for ViewportSize {
        fn window_dimensions(&self) -> io::Result<Option<WindowDimensions>> {
            Ok(Some(self.0))
        }
    }
    struct NeverTerminate;
    impl TerminationSource for NeverTerminate {
        fn requested(&self) -> bool {
            false
        }
    }
    let measurements = observe(100);
    let mut inputs = Inputs {
        packets: VecDeque::from([vec![2], vec![b'q']]),
        measurements: measurements.clone(),
    };
    let dimensions = WindowDimensions {
        columns: 120,
        rows: 40,
        pixel_width: 0,
        pixel_height: 0,
    };
    let mut terminal = Terminal::with_options(
        TestBackend::new(120, 40),
        TerminalOptions {
            viewport: Viewport::Fixed(super::super::to_area(dimensions)),
        },
    )
    .expect("test terminal");
    let result = super::super::run_event_loop(
        &mut terminal,
        Chrome {
            locale: Locale::EnUs,
            theme: &crate::tui_theme::Theme::terminal_native(),
        },
        &mut inputs,
        &mut crate::terminal_input::UnavailableTerminalInputPort,
        &NeverTerminate,
        &ViewportSize(dimensions),
        dimensions,
        false,
    );
    assert!(matches!(result, Ok(LoopExit::User)), "{result:?}");
    let value = document(&measurements, true, true);
    let samples = value["samples"].as_array().expect("samples");
    let reads: Vec<_> = samples
        .iter()
        .filter(|sample| sample["event"]["kind"] == "input_read")
        .collect();
    let dispatches: Vec<_> = samples
        .iter()
        .filter(|sample| sample["event"]["kind"] == "dispatch")
        .collect();
    let frames: Vec<_> = samples
        .iter()
        .filter(|sample| sample["event"]["kind"] == "frame")
        .collect();
    assert_eq!(reads.len(), 2);
    assert_eq!(dispatches.len(), 2);
    assert_eq!(reads[0]["event"]["batch"], 1);
    assert_eq!(dispatches[0]["event"]["batch"], 1);
    assert_eq!(reads[1]["event"]["batch"], 2);
    assert_eq!(dispatches[1]["event"]["batch"], 2);
    assert_eq!(
        frames.first().expect("initial draw")["event"]["last_input_batch"],
        serde_json::Value::Null
    );
    assert!(
        frames
            .iter()
            .any(|frame| frame["event"]["last_input_batch"] == 1)
    );
    assert!(
        !frames
            .iter()
            .any(|frame| frame["event"]["last_input_batch"] == 2),
        "exit has no following draw; metadata is not a feedback acknowledgement"
    );
    assert_eq!(value["complete"], true);
}
