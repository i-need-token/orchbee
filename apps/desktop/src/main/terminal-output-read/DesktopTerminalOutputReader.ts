import {
	decodeTerminalFrame,
	decodeTerminalRowWindow,
	PROTOCOL_LIMITS,
	TerminalFrameKind,
	TerminalRowWindowDetail,
	type DecodedTerminalFrame,
	type DecodedTerminalTextRow,
	type DecodedTerminalTextRowWindow,
} from '@orchbee/terminal-protocol';
import {
	ProjectedTerminalEventSchema,
	type ProjectedTerminalEvent,
} from '@orchbee/workbench-client-protocol';
import type { WorkbenchTerminalOutputReadOutput } from '@orchbee/workbench/semantic-commands';
import type { TerminalPresentationId } from '@orchbee/workbench/terminal-presentation';
import type { MessagePortMain } from 'electron';

import type { DesktopTerminalProjectionLease } from '../terminal-projection-control';
import { projectTerminalOutputText, terminalOutputTextLimitReached } from './terminal-output-text';

const DEFAULT_READ_DEADLINE_MILLISECONDS = 5_000;
const MAXIMUM_ROWS_PER_REQUEST = 128;

export interface DesktopTerminalOutputProjectionPort {
	isPublicationTargetCurrent(
		presentationId: TerminalPresentationId,
		lease: DesktopTerminalProjectionLease,
	): boolean;
	openReadOnly(presentationId: TerminalPresentationId): Promise<DesktopTerminalProjectionLease>;
}

export interface DesktopTerminalOutputReaderOptions {
	readonly deadlineMilliseconds?: number;
	readonly projection: DesktopTerminalOutputProjectionPort;
}

export type DesktopTerminalOutputReadResult =
	| { readonly kind: 'completed'; readonly output: WorkbenchTerminalOutputReadOutput }
	| {
			readonly kind: 'cancelled' | 'cleanup-failed' | 'stale' | 'target-retired' | 'unavailable';
	  };

type ReadFailureKind = Exclude<DesktopTerminalOutputReadResult['kind'], 'completed'>;

class TerminalOutputReadError extends Error {
	readonly kind: ReadFailureKind;

	constructor(kind: ReadFailureKind, message: string) {
		super(message);
		this.name = 'TerminalOutputReadError';
		this.kind = kind;
	}
}

export class DesktopTerminalOutputReader {
	readonly #deadlineMilliseconds: number;
	readonly #projection: DesktopTerminalOutputProjectionPort;

	constructor(options: DesktopTerminalOutputReaderOptions) {
		this.#deadlineMilliseconds = options.deadlineMilliseconds ?? DEFAULT_READ_DEADLINE_MILLISECONDS;
		if (!Number.isSafeInteger(this.#deadlineMilliseconds) || this.#deadlineMilliseconds < 1) {
			throw new Error('Terminal output read deadline must be a positive safe integer.');
		}
		this.#projection = options.projection;
	}

	async read(
		presentationId: TerminalPresentationId,
		requestedRows: number,
		signal: AbortSignal,
	): Promise<DesktopTerminalOutputReadResult> {
		if (signal.aborted) return { kind: 'cancelled' };
		const controller = new AbortController();
		let callerCancelled = false;
		let deadlineExpired = false;
		const cancel = (): void => {
			callerCancelled = true;
			controller.abort();
		};
		signal.addEventListener('abort', cancel, { once: true });
		const timeout = setTimeout(() => {
			deadlineExpired = true;
			controller.abort();
		}, this.#deadlineMilliseconds);
		timeout.unref();
		let lease: DesktopTerminalProjectionLease | undefined;
		let result: DesktopTerminalOutputReadResult;
		try {
			lease = await waitForLease(this.#projection.openReadOnly(presentationId), controller.signal);
			if (lease.bootstrap.canInput || lease.bootstrap.canResize) {
				throw new TerminalOutputReadError(
					'unavailable',
					'Terminal output read received an interactive attachment.',
				);
			}
			const output = await readProjectionTail(
				lease,
				presentationId,
				requestedRows,
				controller.signal,
			);
			result = { kind: 'completed', output };
		} catch (error: unknown) {
			result = { kind: classifyReadFailure(error, callerCancelled, deadlineExpired) };
		} finally {
			clearTimeout(timeout);
			signal.removeEventListener('abort', cancel);
		}
		if (lease !== undefined) {
			try {
				await lease.dispose();
			} catch {
				return { kind: 'cleanup-failed' };
			}
		}
		if (signal.aborted) return { kind: 'cancelled' };
		if (result.kind === 'completed' && lease !== undefined) {
			return this.#projection.isPublicationTargetCurrent(presentationId, lease)
				? result
				: { kind: 'target-retired' };
		}
		return result;
	}
}

async function readProjectionTail(
	lease: DesktopTerminalProjectionLease,
	presentationId: TerminalPresentationId,
	requestedRows: number,
	signal: AbortSignal,
): Promise<WorkbenchTerminalOutputReadOutput> {
	const framePromise = waitForProjectedEvent(
		lease.rendererPort,
		(event) => selectSnapshotFrame(event, lease),
		signal,
	);
	lease.rendererPort.start();
	lease.rendererPort.postMessage({
		kind: 'set-projection-demand',
		payload: { attachmentId: lease.bootstrap.attachmentId, demand: 'active' },
	});
	const frame = await framePromise;
	const totalRows = frame.scrollbackRows + BigInt(frame.rows);
	const desiredRows = Number(totalRows < BigInt(requestedRows) ? totalRows : BigInt(requestedRows));
	const rows: DecodedTerminalTextRow[] = [];
	let requestSequence = 1n;
	while (rows.length < desiredRows && !terminalOutputTextLimitReached(rows)) {
		const rowCount = Math.min(MAXIMUM_ROWS_PER_REQUEST, desiredRows - rows.length);
		const rowsBefore = rows.length;
		const rowsPromise = waitForProjectedEvent(
			lease.rendererPort,
			(event) => selectTextRows(event, lease, frame, totalRows, requestSequence, rowsBefore),
			signal,
		);
		lease.rendererPort.postMessage({
			kind: 'read-rows',
			payload: {
				attachmentId: lease.bootstrap.attachmentId,
				detail: TerminalRowWindowDetail.text,
				generation: frame.generation.toString(),
				maximumBytes: PROTOCOL_LIMITS.maxRowWindowBytes,
				origin: { kind: 'tail', rowsBefore },
				purpose: 'buffer',
				requestSequence: requestSequence.toString(),
				revision: frame.revision.toString(),
				revisionPolicy: 'exact',
				rowCount,
			},
		});
		const page = await rowsPromise;
		const newer = rows[0];
		const olderLast = page.rows.at(-1);
		if (
			newer !== undefined &&
			olderLast !== undefined &&
			olderLast.wrap !== newer.wrapContinuation
		) {
			throw new TerminalOutputReadError(
				'unavailable',
				'Terminal row pages disagree at a wrap boundary.',
			);
		}
		rows.unshift(...page.rows);
		requestSequence += 1n;
	}
	return projectTerminalOutputText({
		activeScreen: frame.activeScreen,
		generation: frame.generation,
		presentationId,
		requestedRows,
		revision: frame.revision,
		rows,
		totalRows,
	});
}

function selectSnapshotFrame(
	event: ProjectedTerminalEvent,
	lease: DesktopTerminalProjectionLease,
): DecodedTerminalFrame | null {
	if (event.kind !== 'frame') return null;
	const decoded = decodeTerminalFrame(binaryBytes(event.frame));
	if (!decoded.ok) {
		throw new TerminalOutputReadError('unavailable', decoded.detail);
	}
	if (decoded.value.kind !== TerminalFrameKind.snapshot) return null;
	if (
		decoded.value.attachmentId !== lease.bootstrap.attachmentId ||
		decoded.value.hostEpoch !== lease.bootstrap.hostEpoch
	) {
		throw new TerminalOutputReadError('unavailable', 'Terminal frame names a foreign attachment.');
	}
	return decoded.value;
}

function selectTextRows(
	event: ProjectedTerminalEvent,
	lease: DesktopTerminalProjectionLease,
	frame: DecodedTerminalFrame,
	totalRows: bigint,
	requestSequence: bigint,
	rowsBefore: number,
): DecodedTerminalTextRowWindow | null {
	if (event.kind === 'frame') {
		const decodedFrame = decodeTerminalFrame(binaryBytes(event.frame));
		if (!decodedFrame.ok) {
			throw new TerminalOutputReadError('unavailable', decodedFrame.detail);
		}
		if (
			decodedFrame.value.generation !== frame.generation ||
			decodedFrame.value.revision !== frame.revision
		) {
			throw new TerminalOutputReadError('stale', 'Terminal projection changed during output read.');
		}
		return null;
	}
	if (event.kind !== 'rows') return null;
	const decoded = decodeTerminalRowWindow(binaryBytes(event.rows));
	if (!decoded.ok || decoded.value.detail !== TerminalRowWindowDetail.text) {
		throw new TerminalOutputReadError(
			'unavailable',
			decoded.ok ? 'Terminal Host returned the wrong row detail.' : decoded.detail,
		);
	}
	const window = decoded.value;
	const expectedEnd = totalRows - BigInt(rowsBefore);
	if (
		window.attachmentId !== lease.bootstrap.attachmentId ||
		window.hostEpoch !== frame.hostEpoch ||
		window.generation !== frame.generation ||
		window.revision !== frame.revision ||
		window.sequence !== requestSequence ||
		window.totalRows !== totalRows ||
		window.startRow + BigInt(window.rows.length) !== expectedEnd
	) {
		throw new TerminalOutputReadError('unavailable', 'Terminal Host returned unrelated rows.');
	}
	return window;
}

function waitForLease(
	opening: Promise<DesktopTerminalProjectionLease>,
	signal: AbortSignal,
): Promise<DesktopTerminalProjectionLease> {
	if (signal.aborted) {
		void opening.then((lease) => lease.dispose()).catch(() => undefined);
		return Promise.reject(
			new TerminalOutputReadError('cancelled', 'Terminal output read aborted.'),
		);
	}
	return new Promise((resolve, reject) => {
		let pending = true;
		const remove = (): void => signal.removeEventListener('abort', handleAbort);
		const handleAbort = (): void => {
			if (!pending) return;
			pending = false;
			remove();
			reject(new TerminalOutputReadError('cancelled', 'Terminal output read aborted.'));
		};
		signal.addEventListener('abort', handleAbort, { once: true });
		void opening.then(
			(lease) => {
				if (!pending) {
					void lease.dispose().catch(() => undefined);
					return;
				}
				pending = false;
				remove();
				resolve(lease);
			},
			(error: unknown) => {
				if (!pending) return;
				pending = false;
				remove();
				reject(error);
			},
		);
		if (signal.aborted) handleAbort();
	});
}

function waitForProjectedEvent<Value>(
	port: MessagePortMain,
	select: (event: ProjectedTerminalEvent) => Value | null,
	signal: AbortSignal,
): Promise<Value> {
	if (signal.aborted) {
		return Promise.reject(
			new TerminalOutputReadError('cancelled', 'Terminal output read aborted.'),
		);
	}
	return new Promise((resolve, reject) => {
		const remove = (): void => {
			port.removeListener('message', handleMessage);
			port.removeListener('close', handleClose);
			signal.removeEventListener('abort', handleAbort);
		};
		const handleAbort = (): void => {
			remove();
			reject(new TerminalOutputReadError('cancelled', 'Terminal output read aborted.'));
		};
		const handleClose = (): void => {
			remove();
			reject(
				new TerminalOutputReadError(
					'unavailable',
					'Terminal projection closed during output read.',
				),
			);
		};
		const handleMessage = (message: { readonly data: unknown }): void => {
			const parsed = ProjectedTerminalEventSchema.safeParse(message.data);
			if (!parsed.success) return;
			if (parsed.data.kind === 'failure') {
				remove();
				reject(projectedFailure(parsed.data.failure.code, parsed.data.failure.message));
				return;
			}
			try {
				const selected = select(parsed.data);
				if (selected === null) return;
				remove();
				resolve(selected);
			} catch (error: unknown) {
				remove();
				reject(error);
			}
		};
		port.on('message', handleMessage);
		port.on('close', handleClose);
		signal.addEventListener('abort', handleAbort, { once: true });
		if (signal.aborted) handleAbort();
	});
}

function projectedFailure(code: string, message: string): TerminalOutputReadError {
	if (code === 'stale-generation' || code === 'stale-revision' || message.includes('stale')) {
		return new TerminalOutputReadError('stale', message);
	}
	if (code === 'terminal-projection-retired' || code === 'missing-resource') {
		return new TerminalOutputReadError('target-retired', message);
	}
	return new TerminalOutputReadError('unavailable', message);
}

function classifyReadFailure(
	error: unknown,
	callerCancelled: boolean,
	deadlineExpired: boolean,
): ReadFailureKind {
	if (callerCancelled) return 'cancelled';
	if (deadlineExpired) return 'unavailable';
	if (error instanceof TerminalOutputReadError) return error.kind;
	const cause = error instanceof Error ? error.cause : undefined;
	if (cause !== null && typeof cause === 'object' && 'code' in cause) {
		const code = Reflect.get(cause, 'code');
		if (code === 'terminal-projection-cleanup-failed') return 'cleanup-failed';
		if (code === 'terminal-projection-retired') return 'target-retired';
		if (code === 'stale-generation' || code === 'stale-revision') return 'stale';
	}
	return 'unavailable';
}

function binaryBytes(value: ArrayBuffer | Uint8Array): Uint8Array {
	return value instanceof Uint8Array ? value : new Uint8Array(value);
}
