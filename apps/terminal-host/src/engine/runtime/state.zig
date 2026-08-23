const std = @import("std");
const ghostty_vt = @import("ghostty-vt");
const identifiers = @import("terminal-host-model").identifiers;
const pty_process = @import("terminal-host-platform").process;
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
    // terminal_mutex protects the canonical count and its deferred overflow
    // marker because Ghostty invokes bell synchronously while parsing output.
    bell_sequence: u64 = 0,
    bell_sequence_overflow_pending: bool = false,
    clipboard_write_in_flight: bool = false,
    clipboard_write_pending: ?contracts.ClipboardWriteRequest = null,
    clipboard_write_sequence: u64 = 0,
    clipboard_write_staged: ?[]u8 = null,
    // projection_mutex owns the cached close-activity projection and freshness
    // epoch. Activity-affecting input invalidates the cache before publication;
    // a later bounded sample may classify the current process and Ghostty
    // prompt state again.
    close_activity: contracts.TerminalCloseActivity = .unknown,
    close_activity_epoch: u64 = 0,
    close_activity_epoch_exhausted: bool = false,
    command_batch_active: bool = false,
    command_completion: ?contracts.TerminalCommandCompletion = null,
    command_completion_staged: ?contracts.TerminalCommandCompletion = null,
    command_sequence: u64 = 0,
    command_sequence_staged: u64 = 0,
    command_started_at_ns: ?u64 = null,
    command_started_at_ns_staged: ?u64 = null,
    command_timer: ?std.time.Timer = null,
    cell_pixel_height: u16,
    cell_pixel_width: u16,
    child_exited: std.atomic.Value(bool) = .init(false),
    client_input_closed: bool = false,
    containment_confirmed: std.atomic.Value(bool) = .init(false),
    containment_mutex: std.Thread.Mutex = .{},
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
    input_condition: std.Thread.Condition = .{},
    input_queue: terminal_input_queue.Queue,
    input_mutex: std.Thread.Mutex = .{},
    metadata_change_pending: bool = false,
    metadata_published_connection_id: ?u64 = null,
    metadata_published_revision: u64 = 0,
    metadata_revision: u64 = 0,
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
    // Host projection scheduling is the only reader/writer. A distinct
    // deadline prevents outgoing batch rollback from repeating a platform
    // activity query inside the same cadence.
    next_close_activity_sample_at_ns: i128 = 0,
    projection: terminal_projection.Projection,
    projection_mutex: std.Thread.Mutex = .{},
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
    // terminal_mutex protects the bounded DEC synchronized-output interval.
    // The deadline shares the runtime monotonic clock used by terminal
    // feedback and is absent whenever the canonical mode is disabled.
    synchronized_output_deadline_ns: ?u64 = null,
    startup: terminal_runtime_startup.Gate = .{},
    terminal: ghostty_vt.Terminal,
    // terminal_mutex protects terminal parsing, history, generation, scrollback,
    // and cell geometry as one state transition.
    terminal_condition: std.Thread.Condition = .{},
    terminal_driven_resize_target: ?terminal_geometry.Geometry = null,
    terminal_mutex: std.Thread.Mutex = .{},
    terminal_resize_failure_pending: bool = false,
    testing_hooks: contracts.RuntimeTestingHooks = .{},
    threads_joined: bool = false,
    wait_thread: ?std.Thread = null,
    writer_stop: bool = false,
    writer_stopped: std.atomic.Value(bool) = .init(false),
    writer_thread: ?std.Thread = null,
    written_since_sync: std.atomic.Value(usize) = .init(0),
};
