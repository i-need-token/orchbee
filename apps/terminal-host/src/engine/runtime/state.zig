const std = @import("std");
const ghostty_vt = @import("ghostty-vt");
const identifiers = @import("terminal-host-model").identifiers;
const platform = @import("terminal-host-platform");
const monotonic = platform.monotonic;
const pty_process = platform.process;
const thread_sync = platform.thread_sync;
const terminal_geometry = @import("terminal-host-model").geometry;
const terminal_history = @import("../history/root.zig");
const terminal_input_ownership = @import("../input_ownership.zig");
const terminal_input_queue = @import("../input_queue.zig");
const terminal_projection = @import("../projection/root.zig");
const terminal_runtime_containment = @import("../runtime_containment.zig");
const terminal_runtime_startup = @import("../runtime_startup.zig");
const terminal_stream = @import("../terminal_stream.zig");
const contracts = @import("contracts.zig");

pub const State = struct {
    allocator: std.mem.Allocator,
    // terminal_mutex protects the monotonically increasing count of visible
    // time-driven Kitty animation frame changes.
    animation_condition: thread_sync.Condition = .{},
    animation_tick: u64 = 0,
    animation_thread: ?std.Thread = null,
    // terminal_mutex protects the canonical count and its deferred overflow
    // marker because Ghostty invokes bell synchronously while parsing output.
    bell_sequence: u64 = 0,
    bell_sequence_overflow_pending: bool = false,
    application_signal_pending: [contracts.max_pending_application_signals]contracts.ApplicationSignal = undefined,
    application_signal_pending_count: usize = 0,
    application_signal_pending_head: usize = 0,
    application_signal_sequence: u64 = 0,
    application_signal_sequence_staged: u64 = 0,
    application_signal_staged: [contracts.max_pending_application_signals]contracts.ApplicationSignal = undefined,
    application_signal_staged_count: usize = 0,
    clipboard_write_in_flight: bool = false,
    clipboard_write_pending: ?contracts.ClipboardWriteRequest = null,
    clipboard_write_sequence: u64 = 0,
    clipboard_write_staged: ?[]u8 = null,
    command_batch_active: bool = false,
    command_observation: @import("command_observation.zig").Tracker = .{},
    command_completion: ?contracts.TerminalCommandCompletion = null,
    command_completion_staged: ?contracts.TerminalCommandCompletion = null,
    command_sequence: u64 = 0,
    command_sequence_staged: u64 = 0,
    command_started_at_ns: ?u64 = null,
    command_started_at_ns_staged: ?u64 = null,
    command_timer: ?monotonic.Timer = null,
    cell_pixel_height: u16,
    cell_pixel_width: u16,
    // terminal_mutex protects the complete viewport requested by the latest
    // successfully committed resize owner. Terminal-driven geometry effects
    // may change canonical geometry but never replace this restore target.
    viewport_geometry: terminal_geometry.Geometry,
    child_exited: std.atomic.Value(bool) = .init(false),
    client_input_closed: bool = false,
    containment_confirmed: std.atomic.Value(bool) = .init(false),
    containment_mutex: thread_sync.Mutex = .{},
    exit_result: ?pty_process.ExitResult = null,
    // terminal_mutex and input_mutex jointly protect this irreversible gate.
    // Parser/state readers observe it while holding terminal_mutex; queue and
    // writer-start readers observe it while holding input_mutex.
    failure_fenced: bool = false,
    // A successful engine fence winner owns the first bounded containment
    // attempt. A Host-originated fence reserves that attempt for Host cleanup;
    // later Host retries wait for an engine_pending owner to publish done.
    first_containment_attempt: terminal_runtime_containment.FirstAttemptGate = .{},
    feedback_sequence_overflow_pending: bool = false,
    generation: u64 = 1,
    history: terminal_history.Manager,
    input_ownership: terminal_input_ownership.Ledger,
    // input_mutex and input_condition protect attachment input ownership,
    // queue order, charged bytes, reservation identifiers, the writer stop
    // transition, and completion notifications that must not be missed between
    // a predicate check and wait.
    input_condition: thread_sync.Condition = .{},
    input_queue: terminal_input_queue.Queue,
    // Ghostty borrows this canonical path for temporary-file validation.
    kitty_temp_directory: ?[:0]u8 = null,
    // Only the Host lifecycle owner accesses this after startup handoff; worker
    // threads never release or borrow the generated document metadata.
    editor_document: ?platform.editor_document.Document = null,
    input_mutex: thread_sync.Mutex = .{},
    metadata_change_pending: bool = false,
    metadata_published_connection_id: ?u64 = null,
    metadata_published_revision: u64 = 0,
    metadata_revision: u64 = 0,
    // terminal_mutex and input_mutex jointly protect this monotonic epoch and
    // its irreversible exhaustion marker. Mouse preparation reads it under
    // terminal_mutex, while final publication and writer start read it under
    // both locks; every joint access follows terminal -> input order.
    mouse_input_epoch: u64 = 1,
    mouse_input_epoch_exhausted: bool = false,
    // Distinguishes an owner-requested read interruption from a normal PTY EOF.
    // Only the reader may publish pty_drained, and only after a non-aborted EOF.
    output_abort_requested: std.atomic.Value(bool) = .init(false),
    // One terminal atomic classification makes normal completion and fatal
    // failure mutually exclusive. Zero is pending, RuntimeFailureKind values
    // are failures, and normal_completion_classification is normal completion.
    // Once normal is published, later disposal diagnostics cannot rewrite the
    // retained terminal outcome.
    completion_classification: contracts.CompletionClassification = .{},
    reader_thread: ?std.Thread = null,
    release_process_identity: bool = false,
    revision: std.atomic.Value(u64) = .init(0),
    output_batches_since_sync: std.atomic.Value(u64) = .init(0),
    process: pty_process.Process,
    profile: contracts.OwnedProfile,
    progress: contracts.TerminalProgress = .{},
    progress_staged: ?contracts.TerminalProgress = null,
    // projection_mutex gives one encoder ownership of projection capture state.
    // The Host owner alone updates the projection cadence timestamp; engine
    // workers never read it.
    next_projection_at_ns: i128 = 0,
    projection: terminal_projection.Projection,
    projection_mutex: thread_sync.Mutex = .{},
    pty_drained: std.atomic.Value(bool) = .init(false),
    // Ghostty may discover a reply-admission failure synchronously while a
    // reader batch is being parsed. terminal_mutex protects this deferred
    // marker until the complete batch has committed coherently.
    reply_failure_pending: bool = false,
    // A failed Ghostty resize may leave its internal screen state undefined.
    // Once set, no new terminal work is admitted and the owner must tear down
    // the runtime instead of attempting to continue with mixed geometry.
    resize_fenced: std.atomic.Value(bool) = .init(false),
    // terminal_mutex owns the interval between a successful resize preflight
    // and its canonical commit or platform compensation. Normal completion
    // cannot publish while this effect is in flight.
    resize_effect_in_flight: bool = false,
    // A terminal-driven resize temporarily releases terminal_mutex for its
    // platform effect while the reader still owns the enclosing batch commit.
    // Shutdown and unrelated failure fencing wait for that exact transaction.
    reader_commit_in_progress: bool = false,
    // terminal_mutex protects this one-shot epoch. Failure publication advances
    // it so a frame encoded before the fence cannot commit after containment
    // makes retained read-only state available again.
    read_commit_epoch: u64 = 0,
    stream: terminal_stream.Stream,
    stream_allocator: terminal_stream.ParserAllocator,
    scrollback_bytes: usize,
    // terminal_mutex owns Ghostty's opaque compression activity observation,
    // the client-input/output interaction serial, the idle deadline, and the
    // outcome of the latest bounded step.
    scrollback_compression: contracts.ScrollbackCompressionSchedule = .{},
    // terminal_mutex protects the bounded DEC synchronized-output interval.
    // The deadline shares the runtime monotonic clock used by terminal
    // feedback and is absent whenever the canonical mode is disabled.
    synchronized_output_deadline_ns: ?u64 = null,
    // terminal_mutex protects the bounded wait for a paint that began by hiding the cursor.
    cursor_paint_deadline_ns: ?u64 = null,
    startup: terminal_runtime_startup.Gate = .{},
    terminal: ghostty_vt.Terminal,
    // terminal_mutex protects terminal parsing, history, generation, scrollback,
    // and cell geometry as one state transition.
    terminal_condition: thread_sync.Condition = .{},
    terminal_driven_resize_target: ?terminal_geometry.Geometry = null,
    terminal_mutex: thread_sync.Mutex = .{},
    terminal_resize_failure_pending: bool = false,
    testing_hooks: contracts.RuntimeTestingHooks = .{},
    threads_joined: bool = false,
    wait_thread: ?std.Thread = null,
    writer_stop: bool = false,
    writer_stopped: std.atomic.Value(bool) = .init(false),
    writer_thread: ?std.Thread = null,
    written_since_sync: std.atomic.Value(usize) = .init(0),
};
