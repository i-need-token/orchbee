use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
    io,
    rc::Rc,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use ratatui::layout::Rect;

use super::{
    StartupWait, WaitInput, WaitModeGuard, WaitPrompt, WaitViewport, await_presentable_geometry,
    startup_presentable_area,
};
use crate::termination_signal::TerminationSource;
use crate::{
    localization::Locale, terminal_device::WindowDimensions, terminal_input::PointerCaptureRequest,
};

fn dimensions(columns: u16, rows: u16) -> WindowDimensions {
    WindowDimensions {
        columns,
        rows,
        pixel_width: columns.saturating_mul(8),
        pixel_height: rows.saturating_mul(16),
    }
}

fn result(dimensions: WindowDimensions) -> io::Result<WindowDimensions> {
    Ok(dimensions)
}

fn fault(kind: io::ErrorKind) -> io::Result<WindowDimensions> {
    Err(io::Error::new(kind, "scripted viewport fault"))
}

fn clone_sample(sample: &io::Result<WindowDimensions>) -> io::Result<WindowDimensions> {
    match sample {
        Ok(dimensions) => Ok(*dimensions),
        Err(error) => Err(io::Error::new(error.kind(), error.to_string())),
    }
}

struct ScriptedViewport {
    samples: VecDeque<io::Result<WindowDimensions>>,
    fallback: io::Result<WindowDimensions>,
}

impl ScriptedViewport {
    fn new(
        samples: Vec<io::Result<WindowDimensions>>,
        fallback: io::Result<WindowDimensions>,
    ) -> Self {
        Self {
            samples: VecDeque::from(samples),
            fallback,
        }
    }
}

impl WaitViewport for ScriptedViewport {
    fn window_dimensions(&mut self) -> io::Result<WindowDimensions> {
        self.samples
            .pop_front()
            .unwrap_or_else(|| clone_sample(&self.fallback))
    }
}

struct ScriptedInput {
    reads: VecDeque<(Duration, io::Result<Option<Vec<u8>>>)>,
    timeouts: Vec<Duration>,
    clock: Rc<RefCell<Instant>>,
}

impl ScriptedInput {
    fn new(
        reads: Vec<(Duration, io::Result<Option<Vec<u8>>>)>,
        clock: Rc<RefCell<Instant>>,
    ) -> Self {
        Self {
            reads: VecDeque::from(reads),
            timeouts: Vec::new(),
            clock,
        }
    }
}

impl WaitInput for ScriptedInput {
    fn next_input(&mut self, timeout: Duration) -> io::Result<Option<Vec<u8>>> {
        self.timeouts.push(timeout);
        match self.reads.pop_front() {
            Some((advance, outcome)) => {
                *self.clock.borrow_mut() += advance;
                outcome
            }
            None => Ok(None),
        }
    }
}

struct FlagTermination(Cell<bool>);

impl TerminationSource for FlagTermination {
    fn requested(&self) -> bool {
        self.0.get()
    }
}

struct RecordingGuard {
    ops: Arc<Mutex<Vec<&'static str>>>,
    fail_at: Option<&'static str>,
}

impl RecordingGuard {
    fn record(&self, op: &'static str) -> io::Result<()> {
        self.ops.lock().unwrap().push(op);
        if self.fail_at == Some(op) {
            return Err(io::Error::other(format!("scripted {op} failure")));
        }
        Ok(())
    }
}

impl WaitModeGuard for RecordingGuard {
    fn query_host_appearance(
        &mut self,
        _: crate::terminal_host_appearance::Query,
    ) -> io::Result<()> {
        self.record("query-host-appearance")
    }
    fn set_bracketed_paste(&mut self, enabled: bool) -> io::Result<()> {
        self.record(if enabled {
            "enable-bracketed-paste"
        } else {
            "disable-bracketed-paste"
        })
    }
    fn set_pointer_capture(&mut self, request: PointerCaptureRequest) -> io::Result<()> {
        self.record(match request {
            PointerCaptureRequest::Release => "disable-pointer-capture",
            PointerCaptureRequest::ButtonOnly => "enable-pointer-capture",
            PointerCaptureRequest::AnyMotion => "enable-pointer-any-motion",
            PointerCaptureRequest::MenuMotion => "enable-pointer-menu-motion",
        })
    }
    fn restore(&mut self) -> io::Result<()> {
        for op in RESTORE_OPS {
            self.record(op)?;
        }
        Ok(())
    }
}

const ENTER_OPS: [&str; 3] = ["enable-raw", "enter-alt", "hide-cursor"];
const RESTORE_OPS: [&str; 3] = ["show-cursor", "leave-alt", "disable-raw"];

struct Harness {
    clock: Rc<RefCell<Instant>>,
    deadline: Instant,
    ops: Arc<Mutex<Vec<&'static str>>>,
    fail_at: Option<&'static str>,
    termination: Rc<FlagTermination>,
    output: Vec<u8>,
}

impl Harness {
    fn new(offset: Duration) -> Self {
        let base = Instant::now();
        Self {
            clock: Rc::new(RefCell::new(base)),
            deadline: base + Duration::from_secs(20) + offset,
            ops: Arc::new(Mutex::new(Vec::new())),
            fail_at: None,
            termination: Rc::new(FlagTermination(Cell::new(false))),
            output: Vec::new(),
        }
    }

    fn ops(&self) -> Vec<&'static str> {
        self.ops.lock().unwrap().clone()
    }

    fn run<P: FnMut(WindowDimensions) -> Option<Rect>>(
        &mut self,
        viewport: &mut ScriptedViewport,
        input: &mut ScriptedInput,
        presentable: &mut P,
    ) -> StartupWait {
        let clock = Rc::clone(&self.clock);
        let now = move || *clock.borrow();
        let ops = Arc::clone(&self.ops);
        let fail_at = self.fail_at;
        let enter = move || {
            for op in ENTER_OPS {
                ops.lock().unwrap().push(op);
                if fail_at == Some(op) {
                    return Err(io::Error::other(format!("scripted {op} failure")));
                }
            }
            Ok(RecordingGuard {
                ops: Arc::clone(&ops),
                fail_at,
            })
        };
        await_presentable_geometry(
            enter,
            crate::startup_wait::StartupWaitDeps {
                viewport,
                input,
                output: &mut self.output,
                termination: self.termination.as_ref(),
                clock: &now,
                deadline: self.deadline,
                prompt: WaitPrompt {
                    notice: Locale::EnUs.startup_wait_notice(),
                    exit_hint: &Locale::EnUs.startup_wait_hint(),
                },
                presentable,
            },
        )
    }
}

fn always(_: WindowDimensions) -> Option<Rect> {
    Some(Rect::new(1, 1, 10, 5))
}

fn expect_available(outcome: StartupWait) -> WindowDimensions {
    match outcome {
        StartupWait::Available(dimensions) => dimensions,
        StartupWait::Cancelled => panic!("expected Available, got Cancelled"),
        StartupWait::TimedOut => panic!("expected Available, got TimedOut"),
        StartupWait::Fault(error) => panic!("expected Available, got Fault: {error}"),
    }
}

fn name(outcome: &StartupWait) -> &'static str {
    match outcome {
        StartupWait::Available(_) => "Available",
        StartupWait::Cancelled => "Cancelled",
        StartupWait::TimedOut => "TimedOut",
        StartupWait::Fault(_) => "Fault",
    }
}

#[test]
fn drawable_report_returns_without_modes_or_draw() {
    let mut harness = Harness::new(Duration::ZERO);
    let big = dimensions(120, 40);
    let mut viewport = ScriptedViewport::new(vec![result(big)], result(big));
    let mut input = ScriptedInput::new(Vec::new(), Rc::clone(&harness.clock));

    let dimensions = expect_available(harness.run(&mut viewport, &mut input, &mut always));

    assert_eq!(dimensions.columns, 120);
    assert!(harness.ops().is_empty());
    assert!(harness.output.is_empty());
    assert!(input.timeouts.is_empty());
}

#[test]
fn waiting_draws_prompt_once_then_admits_valid_geometry() {
    let mut harness = Harness::new(Duration::ZERO);
    let big = dimensions(120, 40);
    let mut viewport = ScriptedViewport::new(
        vec![
            result(dimensions(120, 4)),
            result(dimensions(120, 4)),
            result(big),
        ],
        result(big),
    );
    let mut input = ScriptedInput::new(Vec::new(), Rc::clone(&harness.clock));

    let admitted =
        expect_available(harness.run(&mut viewport, &mut input, &mut startup_presentable_area));

    assert_eq!(admitted.columns, 120);
    assert_eq!(admitted.rows, 40);
    let mut expected = ENTER_OPS.to_vec();
    expected.extend(RESTORE_OPS);
    assert_eq!(harness.ops(), expected);
    let drawn = String::from_utf8_lossy(&harness.output);
    assert!(drawn.contains("too small"));
    assert!(drawn.contains(Locale::EnUs.startup_wait_hint().trim_end()));
    // One synchronized frame whose lines each return to the first column.
    assert!(drawn.starts_with("\u{1b}[?2026h\u{1b}[2J\u{1b}[1;1H"));
    assert!(drawn.ends_with("\u{1b}[?2026l"));
    assert!(!drawn.replace("\r\n", "").contains('\n'));
    assert_eq!(input.timeouts.len(), 2);
}

#[test]
fn localized_prompt_pairs_exist() {
    assert!(Locale::EnUs.startup_wait_notice().contains("too small"));
    assert!(Locale::ZhCn.startup_wait_notice().contains("终端"));
    // Drawn key hints display the prefix letter in lowercase, as every TUI key hint does.
    for locale in [Locale::EnUs, Locale::ZhCn] {
        let hint = locale.startup_wait_hint();
        assert!(!hint.contains("Ctrl+B"), "{hint}");
    }
}

#[test]
fn prefix_exit_keys_cancel_the_wait() {
    for packet in [vec![0x02, b'q'], vec![0x02, b'd']] {
        let mut harness = Harness::new(Duration::ZERO);
        let small = dimensions(120, 4);
        let mut viewport = ScriptedViewport::new(vec![result(small)], result(small));
        let mut input = ScriptedInput::new(
            vec![(Duration::ZERO, Ok(Some(packet)))],
            Rc::clone(&harness.clock),
        );

        let outcome = harness.run(&mut viewport, &mut input, &mut startup_presentable_area);

        assert!(matches!(outcome, StartupWait::Cancelled));
        let mut expected = ENTER_OPS.to_vec();
        expected.push("disable-bracketed-paste");
        expected.extend(RESTORE_OPS);
        assert_eq!(harness.ops(), expected);
    }
}

#[test]
fn prefix_exit_across_packets_cancels() {
    let mut harness = Harness::new(Duration::ZERO);
    let small = dimensions(120, 4);
    let mut viewport = ScriptedViewport::new(vec![result(small)], result(small));
    let mut input = ScriptedInput::new(
        vec![
            (Duration::ZERO, Ok(Some(vec![0x02]))),
            (Duration::ZERO, Ok(Some(vec![b'q']))),
        ],
        Rc::clone(&harness.clock),
    );

    let outcome = harness.run(&mut viewport, &mut input, &mut startup_presentable_area);

    assert!(matches!(outcome, StartupWait::Cancelled));
}

#[test]
fn pointer_admission_query_is_rejected_during_startup_wait_and_restores_modes() {
    let mut harness = Harness::new(Duration::ZERO);
    let small = dimensions(120, 4);
    let mut viewport = ScriptedViewport::new(vec![result(small)], result(small));
    let mut input = ScriptedInput::new(
        vec![(Duration::ZERO, Ok(Some(vec![0x02, b'm'])))],
        Rc::clone(&harness.clock),
    );

    let outcome = harness.run(&mut viewport, &mut input, &mut startup_presentable_area);

    assert!(matches!(
        outcome,
        StartupWait::Fault(error) if error.kind() == io::ErrorKind::Unsupported
    ));
    let ops = harness.ops();
    let capture = ops
        .iter()
        .position(|operation| *operation == "enable-pointer-capture")
        .expect("the typed ButtonOnly request must complete before the query");
    let restore = ops
        .iter()
        .position(|operation| *operation == "show-cursor")
        .expect("query rejection must restore the lifecycle guard");
    assert!(capture < restore);
    assert!(!ops.contains(&"enable-pointer-any-motion"));
}

#[test]
fn waiting_input_is_consumed_and_never_replayed() {
    let mut harness = Harness::new(Duration::ZERO);
    let small = dimensions(120, 4);
    let big = dimensions(120, 40);
    let mut viewport = ScriptedViewport::new(
        vec![result(small), result(small), result(small), result(big)],
        result(big),
    );
    let mut input = ScriptedInput::new(
        vec![
            (Duration::ZERO, Ok(Some(b"ordinary bytes".to_vec()))),
            (Duration::ZERO, Ok(None)),
            (Duration::ZERO, Ok(Some(vec![0x02, b'w', b'x']))),
        ],
        Rc::clone(&harness.clock),
    );

    let admitted =
        expect_available(harness.run(&mut viewport, &mut input, &mut startup_presentable_area));

    assert_eq!(admitted.rows, 40);
    assert!(input.reads.is_empty());
}

#[test]
fn modal_request_toggles_then_restores_bracketed_paste() {
    let mut harness = Harness::new(Duration::ZERO);
    let small = dimensions(120, 4);
    let big = dimensions(120, 40);
    let mut viewport = ScriptedViewport::new(vec![result(small), result(big)], result(big));
    let mut input = ScriptedInput::new(
        vec![
            (Duration::ZERO, Ok(Some(vec![0x02, b'a']))),
            (Duration::ZERO, Ok(None)),
        ],
        Rc::clone(&harness.clock),
    );

    let admitted =
        expect_available(harness.run(&mut viewport, &mut input, &mut startup_presentable_area));

    assert_eq!(admitted.rows, 40);
    let ops = harness.ops();
    let paste_on = ops.iter().position(|op| *op == "enable-bracketed-paste");
    let paste_off = ops.iter().rposition(|op| *op == "disable-bracketed-paste");
    assert!(paste_on.is_some());
    assert!(paste_off > paste_on);
}

#[test]
fn deadline_wins_when_resize_lands_at_the_same_instant() {
    let mut harness = Harness::new(Duration::ZERO);
    let small = dimensions(120, 4);
    let big = dimensions(120, 40);
    let mut viewport = ScriptedViewport::new(vec![result(small), result(big)], result(big));
    let mut input = ScriptedInput::new(
        vec![(Duration::from_secs(20), Ok(None))],
        Rc::clone(&harness.clock),
    );

    let outcome = harness.run(&mut viewport, &mut input, &mut startup_presentable_area);

    assert!(matches!(outcome, StartupWait::TimedOut));
    let mut expected = ENTER_OPS.to_vec();
    expected.extend(RESTORE_OPS);
    assert_eq!(harness.ops(), expected);
}

#[test]
fn expired_deadline_returns_timed_out_without_entering_modes() {
    let mut harness = Harness::new(Duration::ZERO);
    let small = dimensions(120, 4);
    let mut viewport = ScriptedViewport::new(vec![result(small)], result(small));
    let mut input = ScriptedInput::new(Vec::new(), Rc::clone(&harness.clock));
    *harness.clock.borrow_mut() = harness.deadline;

    let outcome = harness.run(&mut viewport, &mut input, &mut startup_presentable_area);

    assert!(matches!(outcome, StartupWait::TimedOut));
    assert!(harness.ops().is_empty());
    assert!(harness.output.is_empty());
}

#[test]
fn valid_geometry_inside_the_original_budget_is_admitted() {
    let mut harness = Harness::new(Duration::ZERO);
    let small = dimensions(120, 4);
    let big = dimensions(120, 40);
    let mut viewport = ScriptedViewport::new(vec![result(small), result(big)], result(big));
    let mut input = ScriptedInput::new(
        vec![(Duration::from_secs(19), Ok(None))],
        Rc::clone(&harness.clock),
    );

    let admitted =
        expect_available(harness.run(&mut viewport, &mut input, &mut startup_presentable_area));

    assert_eq!(admitted.rows, 40);
    assert!(*harness.clock.borrow() < harness.deadline);
}

#[test]
fn zero_cell_reports_are_ignored_until_real_fault_or_geometry() {
    let mut harness = Harness::new(Duration::ZERO);
    let big = dimensions(120, 40);
    let mut viewport = ScriptedViewport::new(
        vec![
            fault(io::ErrorKind::InvalidData),
            result(dimensions(120, 4)),
            fault(io::ErrorKind::InvalidData),
            result(big),
        ],
        result(big),
    );
    let mut input = ScriptedInput::new(Vec::new(), Rc::clone(&harness.clock));

    let admitted =
        expect_available(harness.run(&mut viewport, &mut input, &mut startup_presentable_area));

    assert_eq!(admitted.rows, 40);
}

#[test]
fn real_geometry_fault_propagates_as_fault() {
    let mut harness = Harness::new(Duration::ZERO);
    let mut viewport = ScriptedViewport::new(
        vec![
            result(dimensions(120, 4)),
            fault(io::ErrorKind::PermissionDenied),
        ],
        fault(io::ErrorKind::PermissionDenied),
    );
    let mut input = ScriptedInput::new(Vec::new(), Rc::clone(&harness.clock));

    let outcome = harness.run(&mut viewport, &mut input, &mut startup_presentable_area);

    match outcome {
        StartupWait::Fault(error) => assert_eq!(error.kind(), io::ErrorKind::PermissionDenied),
        _ => panic!("expected Fault, got {}", name(&outcome)),
    }
    let mut expected = ENTER_OPS.to_vec();
    expected.extend(RESTORE_OPS);
    assert_eq!(harness.ops(), expected);
}

#[test]
fn first_sample_fault_never_enters_modes() {
    let mut harness = Harness::new(Duration::ZERO);
    let mut viewport = ScriptedViewport::new(
        vec![fault(io::ErrorKind::PermissionDenied)],
        fault(io::ErrorKind::PermissionDenied),
    );
    let mut input = ScriptedInput::new(Vec::new(), Rc::clone(&harness.clock));

    let outcome = harness.run(&mut viewport, &mut input, &mut startup_presentable_area);

    assert!(matches!(outcome, StartupWait::Fault(_)));
    assert!(harness.ops().is_empty());
}

#[test]
fn mode_entry_failure_surfaces_as_fault() {
    let mut harness = Harness::new(Duration::ZERO);
    harness.fail_at = Some("enter-alt");
    let small = dimensions(120, 4);
    let mut viewport = ScriptedViewport::new(vec![result(small)], result(small));
    let mut input = ScriptedInput::new(Vec::new(), Rc::clone(&harness.clock));

    let outcome = harness.run(&mut viewport, &mut input, &mut startup_presentable_area);

    assert!(matches!(outcome, StartupWait::Fault(_)));
    assert_eq!(harness.ops(), vec!["enable-raw", "enter-alt"]);
    assert!(harness.output.is_empty());
}

#[test]
fn termination_request_cancels() {
    let mut harness = Harness::new(Duration::ZERO);
    let small = dimensions(120, 4);
    let mut viewport = ScriptedViewport::new(vec![result(small)], result(small));
    let mut input = ScriptedInput::new(Vec::new(), Rc::clone(&harness.clock));
    harness.termination.0.set(true);

    let outcome = harness.run(&mut viewport, &mut input, &mut startup_presentable_area);

    assert!(matches!(outcome, StartupWait::Cancelled));
}

#[test]
fn input_read_faults_and_eof_surface_as_fault() {
    for scripted in [
        io::Error::new(io::ErrorKind::BrokenPipe, "read fault"),
        io::Error::new(io::ErrorKind::UnexpectedEof, "end of file"),
    ] {
        let kind = scripted.kind();
        let mut harness = Harness::new(Duration::ZERO);
        let small = dimensions(120, 4);
        let mut viewport = ScriptedViewport::new(vec![result(small)], result(small));
        let mut input = ScriptedInput::new(
            vec![(Duration::ZERO, Err(scripted))],
            Rc::clone(&harness.clock),
        );

        let outcome = harness.run(&mut viewport, &mut input, &mut startup_presentable_area);

        match outcome {
            StartupWait::Fault(error) => assert_eq!(error.kind(), kind),
            _ => panic!("expected Fault, got {}", name(&outcome)),
        }
    }
}

#[test]
fn poll_timeout_is_capped_by_the_remaining_deadline() {
    let mut harness = Harness::new(Duration::ZERO);
    harness.deadline = *harness.clock.borrow() + Duration::from_millis(50);
    let small = dimensions(120, 4);
    let mut viewport = ScriptedViewport::new(vec![result(small)], result(small));
    let mut input = ScriptedInput::new(
        vec![(Duration::from_millis(50), Ok(None))],
        Rc::clone(&harness.clock),
    );

    let outcome = harness.run(&mut viewport, &mut input, &mut startup_presentable_area);

    assert!(matches!(outcome, StartupWait::TimedOut));
    assert_eq!(input.timeouts, vec![Duration::from_millis(50)]);
}

#[test]
fn restore_failure_surfaces_as_fault() {
    let mut harness = Harness::new(Duration::ZERO);
    harness.fail_at = Some("leave-alt");
    let small = dimensions(120, 4);
    let big = dimensions(120, 40);
    let mut viewport = ScriptedViewport::new(vec![result(small), result(big)], result(big));
    let mut input = ScriptedInput::new(Vec::new(), Rc::clone(&harness.clock));

    let outcome = harness.run(&mut viewport, &mut input, &mut startup_presentable_area);

    assert!(matches!(outcome, StartupWait::Fault(_)));
    assert!(harness.ops().contains(&"leave-alt"));
}

#[test]
fn startup_presentable_area_reports_drawable_content() {
    let wide = startup_presentable_area(dimensions(120, 40)).unwrap();
    // Terminal content reaches the last row; the status floats over it and reserves none.
    assert_eq!(wide, Rect::new(31, 2, 88, 37));
    assert!(startup_presentable_area(dimensions(80, 8)).is_some());
    assert!(startup_presentable_area(dimensions(96, 10)).is_some());
    // A Minimal viewport still presents an unframed pane, so 7 rows is a
    // drawable area, not a refusal.
    assert!(startup_presentable_area(dimensions(120, 7)).is_some());
    assert!(startup_presentable_area(dimensions(80, 7)).is_some());
    // Only the TooSmall tier cannot present: under 20 columns or 5 rows.
    assert!(startup_presentable_area(dimensions(19, 40)).is_none());
    assert!(startup_presentable_area(dimensions(80, 4)).is_none());
    assert!(startup_presentable_area(dimensions(3, 3)).is_none());
    // Presentability is a cell-geometry verdict only; a zero-pixel report with
    // drawable cells still maps Some, and pixel validation stays on the
    // separate Err channel of `terminal_geometry`.
    let mut pixel_less = dimensions(80, 40);
    pixel_less.pixel_width = 0;
    pixel_less.pixel_height = 0;
    assert!(startup_presentable_area(pixel_less).is_some());
    assert!(
        crate::tui_layout::terminal_geometry(pixel_less, &crate::client_view::ClientView::new(80))
            .is_ok()
    );
}
