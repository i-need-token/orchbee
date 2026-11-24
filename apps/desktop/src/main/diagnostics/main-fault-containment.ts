import type { DesktopDiagnosticRecorder } from './DesktopDiagnosticLog';

const BUILT_IN_ERROR_CLASSES = [
	'AggregateError',
	'Error',
	'EvalError',
	'RangeError',
	'ReferenceError',
	'SyntaxError',
	'TypeError',
	'URIError',
] as const;

/** System and stream error codes worth naming in a record; any other is `other`. */
const NAMED_ERROR_CODES = [
	'EACCES',
	'EBADF',
	'ECONNRESET',
	'EINVAL',
	'ENOENT',
	'EPERM',
	'EPIPE',
	'ERR_STREAM_DESTROYED',
	'ERR_STREAM_WRITE_AFTER_END',
] as const;

type ErrorClass = (typeof BUILT_IN_ERROR_CLASSES)[number] | 'non-error' | 'other-error';
type ErrorCode = (typeof NAMED_ERROR_CODES)[number] | 'none' | 'other';

function classify(error: unknown): ErrorClass {
	if (!(error instanceof Error)) return 'non-error';
	return BUILT_IN_ERROR_CLASSES.find((name) => name === error.name) ?? 'other-error';
}

function codeOf(error: unknown): ErrorCode {
	if (!(error instanceof Error) || !('code' in error) || typeof error.code !== 'string') {
		return 'none';
	}
	const { code } = error;
	return NAMED_ERROR_CODES.find((named) => named === code) ?? 'other';
}

/**
 * Where a listener Main registers with Electron runs: the notification-area or menu-bar icon, the
 * application menu, an IPC channel, or a document's port.
 */
type DesktopListenerSite =
	| 'application-menu'
	| 'ipc-machine-session-request'
	| 'ipc-server-connection-request'
	| 'ipc-window-caption'
	| 'ipc-window-fault'
	| 'port-background-status'
	| 'port-server-connection'
	| 'tray';

type FaultKind = 'listener' | 'uncaught-exception' | 'unhandled-rejection';

function recordContainedFault(
	recordEvent: DesktopDiagnosticRecorder,
	kind: FaultKind,
	site: DesktopListenerSite | 'none',
	fault: unknown,
): void {
	let code: ErrorCode = 'other';
	let errorClass: ErrorClass = 'other-error';
	try {
		code = codeOf(fault);
		errorClass = classify(fault);
	} catch {
		// A value whose own getters throw keeps the generic class.
	}
	recordEvent('error', 'main-fault-contained', { code, errorClass, kind, site });
}

/**
 * Runs `listener` so that a throw costs only this call: it is recorded under `site` and the
 * listener answers `fallback`, instead of unwinding into Electron, where only the process-wide
 * handler would see it and could not say where it came from.
 */
export function containListenerFault<Args extends readonly unknown[], Result>(
	recordEvent: DesktopDiagnosticRecorder,
	site: DesktopListenerSite,
	listener: (...args: Args) => Result,
	fallback: Result,
): (...args: Args) => Result {
	return (...args) => {
		try {
			return listener(...args);
		} catch (error) {
			recordContainedFault(recordEvent, 'listener', site, error);
			return fallback;
		}
	};
}

/**
 * A throw that escaped a listener or timer, and a rejection nobody handled, each came from one
 * window, connection, message, or request, so Main records its class and goes on: ending Main would
 * end every window for a fault in one of them. Installed once the entry has registered everything,
 * so a fault while the entry itself loads still fails the start. Nothing is written to a console: a
 * closed standard error would fail that write, and a throw from this handler ends the process.
 */
export function containMainProcessFaults(recordEvent: DesktopDiagnosticRecorder): void {
	process.on('uncaughtException', (error) =>
		recordContainedFault(recordEvent, 'uncaught-exception', 'none', error),
	);
	process.on('unhandledRejection', (reason) =>
		recordContainedFault(recordEvent, 'unhandled-rejection', 'none', reason),
	);
}
