import type { DesktopDiagnosticRecorder } from '../diagnostics';

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
 * A throw that escaped a listener or timer, and a rejection nobody handled, each came from one
 * window, connection, message, or request, so Main records its class and goes on: ending Main would
 * end every window for a fault in one of them. Installed once the entry has registered everything,
 * so a fault while the entry itself loads still fails the start. Nothing is written to a console: a
 * closed standard error would fail that write, and a throw from this handler ends the process.
 */
export function containMainProcessFaults(recordEvent: DesktopDiagnosticRecorder): void {
	const record = (kind: 'uncaught-exception' | 'unhandled-rejection', fault: unknown): void => {
		let code: ErrorCode = 'other';
		let errorClass: ErrorClass = 'other-error';
		try {
			code = codeOf(fault);
			errorClass = classify(fault);
		} catch {
			// A value whose own getters throw keeps the generic class.
		}
		recordEvent('error', 'main-fault-contained', { code, errorClass, kind });
	};
	process.on('uncaughtException', (error) => record('uncaught-exception', error));
	process.on('unhandledRejection', (reason) => record('unhandled-rejection', reason));
}
