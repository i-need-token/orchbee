import {
	decodeTerminalFrame,
	decodeTerminalRowWindow,
	encodeTerminalGraphics,
	PROTOCOL_LIMITS,
	TerminalFrameKind,
	TerminalRowWindowDetail,
	type TerminalExited,
	type TerminalIdentity,
} from '@orchbee/terminal-protocol';
import {
	ProjectedTerminalEventSchema,
	type ProjectedTerminalEvent,
	type TerminalPresentationId,
} from '@orchbee/workbench-client-protocol';
import type { MessagePortMain } from 'electron';

import type { DesktopTerminalProjectionLease } from '../terminal-projection-control';
import type { FileTerminalArchiveStore } from './FileTerminalArchiveStore';
import {
	encodeTerminalArchive,
	encodeTerminalArchiveForCaptureSizing,
	MAX_TERMINAL_ARCHIVE_BYTES,
	MAX_TERMINAL_ARCHIVE_ROWS,
	type TerminalArchiveWrite,
} from './terminal-archive-document';

const DEFAULT_CAPTURE_DEADLINE_MILLISECONDS = 3_000;

export interface TerminalArchiveCaptureServiceDependencies {
	readonly captureDeadlineMilliseconds?: number;
	readonly openCaptureReadOnly: (
		presentationId: TerminalPresentationId,
		identity: TerminalIdentity,
	) => Promise<DesktopTerminalProjectionLease>;
	readonly store: FileTerminalArchiveStore;
}

export class TerminalArchiveCaptureService {
	readonly #captureDeadlineMilliseconds: number;
	readonly #openCaptureReadOnly: TerminalArchiveCaptureServiceDependencies['openCaptureReadOnly'];
	readonly #store: FileTerminalArchiveStore;

	constructor(dependencies: TerminalArchiveCaptureServiceDependencies) {
		this.#captureDeadlineMilliseconds =
			dependencies.captureDeadlineMilliseconds ?? DEFAULT_CAPTURE_DEADLINE_MILLISECONDS;
		if (
			!Number.isSafeInteger(this.#captureDeadlineMilliseconds) ||
			this.#captureDeadlineMilliseconds < 1
		) {
			throw new Error('Terminal archive capture deadline must be a positive safe integer.');
		}
		this.#openCaptureReadOnly = dependencies.openCaptureReadOnly;
		this.#store = dependencies.store;
	}

	async capture(
		presentationId: TerminalPresentationId,
		identity: TerminalIdentity,
		minimumRevision: TerminalExited['finalRevision'],
	): Promise<void> {
		const controller = new AbortController();
		const timeout = setTimeout(() => controller.abort(), this.#captureDeadlineMilliseconds);
		timeout.unref();
		let lease: DesktopTerminalProjectionLease | undefined;
		try {
			lease = await waitForCaptureLease(
				this.#openCaptureReadOnly(presentationId, identity),
				controller.signal,
			);
			const frameEvent = waitForProjectedEvent(
				lease.rendererPort,
				(event) => {
					if (event.kind !== 'frame') return null;
					const decoded = decodeTerminalFrame(binaryBytes(event.frame));
					if (
						!decoded.ok ||
						decoded.value.kind !== TerminalFrameKind.snapshot ||
						decoded.value.attachmentId !== lease?.bootstrap.attachmentId ||
						decoded.value.hostEpoch !== identity.hostEpoch ||
						decoded.value.revision < BigInt(minimumRevision)
					) {
						return null;
					}
					return decoded.value;
				},
				controller.signal,
			);
			lease.rendererPort.start();
			lease.rendererPort.postMessage({
				kind: 'set-projection-demand',
				payload: { attachmentId: lease.bootstrap.attachmentId, demand: 'active' },
			});
			const finalFrame = await frameEvent;
			let requestSequence = 1n;
			const totalRows = finalFrame.scrollbackRows + BigInt(finalFrame.rows);
			const viewportEvent = waitForProjectedEvent(
				lease.rendererPort,
				(event) => {
					if (event.kind !== 'rows') return null;
					const decoded = decodeTerminalRowWindow(binaryBytes(event.rows));
					if (
						!decoded.ok ||
						decoded.value.detail !== TerminalRowWindowDetail.render ||
						decoded.value.attachmentId !== lease?.bootstrap.attachmentId ||
						decoded.value.sequence !== requestSequence ||
						decoded.value.generation !== finalFrame.generation ||
						decoded.value.hostEpoch !== finalFrame.hostEpoch ||
						decoded.value.revision !== finalFrame.revision ||
						decoded.value.startRow !== finalFrame.scrollbackRows ||
						decoded.value.totalRows !== totalRows ||
						decoded.value.frame.rows !== finalFrame.rows ||
						decoded.value.frame.themeRevision !== finalFrame.themeRevision
					) {
						return null;
					}
					return decoded.value;
				},
				controller.signal,
			);
			lease.rendererPort.postMessage({
				kind: 'read-rows',
				payload: {
					attachmentId: lease.bootstrap.attachmentId,
					detail: TerminalRowWindowDetail.render,
					generation: finalFrame.generation.toString(),
					maximumBytes: PROTOCOL_LIMITS.maxRowWindowBytes,
					origin: { kind: 'absolute', row: finalFrame.scrollbackRows.toString() },
					purpose: 'buffer',
					requestSequence: requestSequence.toString(),
					revision: finalFrame.revision.toString(),
					revisionPolicy: 'exact',
					rowCount: finalFrame.rows,
				},
			});
			const viewport = await viewportEvent;
			const viewportBytes = copyBytes(viewport.frame.bytes);
			const encodedGraphics = encodeTerminalGraphics(viewport.graphics);
			if (!encodedGraphics.ok) throw new Error(encodedGraphics.detail);
			const graphicsBytes = copyBytes(encodedGraphics.value);
			const rowWindows: Uint8Array[] = [];
			const initialArchive = archiveWrite(
				presentationId,
				identity,
				lease,
				viewportBytes,
				graphicsBytes,
				rowWindows,
			);
			const initialSizing = encodeTerminalArchiveForCaptureSizing(initialArchive);
			if (!initialSizing.ok) throw new Error(initialSizing.detail);
			let encodedByteLength = initialSizing.value.byteLength;
			let retainedRows = finalFrame.rows;
			let sourceStartRow = finalFrame.scrollbackRows;
			let capturedHistoryRows = 0;
			const availableHistoryRows = bigintToBoundedNumber(
				finalFrame.scrollbackRows,
				MAX_TERMINAL_ARCHIVE_ROWS,
			);
			let rowsRemaining = Math.min(
				Math.max(0, MAX_TERMINAL_ARCHIVE_ROWS - finalFrame.rows),
				availableHistoryRows,
			);
			requestSequence += 1n;
			while (rowsRemaining > 0 && !controller.signal.aborted) {
				const rowCount = Math.min(rowsRemaining, PROTOCOL_LIMITS.maxRowWindowRows);
				const expectedEnd = finalFrame.scrollbackRows - BigInt(capturedHistoryRows);
				const rowsEvent = waitForProjectedEvent(
					lease.rendererPort,
					(event) => {
						if (event.kind !== 'rows') return null;
						const decoded = decodeTerminalRowWindow(binaryBytes(event.rows));
						if (
							!decoded.ok ||
							decoded.value.detail !== TerminalRowWindowDetail.render ||
							decoded.value.attachmentId !== lease?.bootstrap.attachmentId ||
							decoded.value.sequence !== requestSequence ||
							decoded.value.generation !== finalFrame.generation ||
							decoded.value.hostEpoch !== finalFrame.hostEpoch ||
							decoded.value.revision !== finalFrame.revision ||
							decoded.value.frame.scrollbackRows + BigInt(decoded.value.frame.rows) !== expectedEnd
						) {
							return null;
						}
						return { bytes: copyBytes(event.rows), window: decoded.value };
					},
					controller.signal,
				);
				lease.rendererPort.postMessage({
					kind: 'read-rows',
					payload: {
						attachmentId: lease.bootstrap.attachmentId,
						detail: TerminalRowWindowDetail.render,
						generation: finalFrame.generation.toString(),
						maximumBytes: PROTOCOL_LIMITS.maxRowWindowBytes,
						origin: {
							kind: 'tail',
							rowsBefore: finalFrame.rows + capturedHistoryRows,
						},
						purpose: 'buffer',
						requestSequence: requestSequence.toString(),
						revision: finalFrame.revision.toString(),
						revisionPolicy: 'exact',
						rowCount,
					},
				});
				const captured = await rowsEvent;
				const capturedRows = captured.window.frame.rows;
				if (capturedRows < 1 || capturedRows > rowCount) break;
				if (
					captured.window.totalRows !== totalRows ||
					captured.window.frame.columns !== finalFrame.columns ||
					captured.window.frame.cellPixelWidth !== finalFrame.cellPixelWidth ||
					captured.window.frame.cellPixelHeight !== finalFrame.cellPixelHeight ||
					captured.window.frame.themeRevision !== finalFrame.themeRevision
				) {
					break;
				}
				const nextRetainedRows = retainedRows + capturedRows;
				const nextSourceStartRow = captured.window.frame.scrollbackRows;
				// The payload, one chunk-length entry, and the two changing decimal header fields are
				// the complete encoded-size delta; the digest remains a fixed 64-character value.
				const candidateByteLength =
					encodedByteLength +
					captured.bytes.byteLength +
					decimalDigitCount(captured.bytes.byteLength) +
					(rowWindows.length === 0 ? 0 : 1) +
					decimalDigitCount(nextRetainedRows) -
					decimalDigitCount(retainedRows) +
					decimalDigitCount(nextSourceStartRow) -
					decimalDigitCount(sourceStartRow);
				if (candidateByteLength > MAX_TERMINAL_ARCHIVE_BYTES) break;
				rowWindows.push(captured.bytes);
				encodedByteLength = candidateByteLength;
				retainedRows = nextRetainedRows;
				sourceStartRow = nextSourceStartRow;
				capturedHistoryRows += capturedRows;
				rowsRemaining -= capturedRows;
				requestSequence += 1n;
			}
			if (controller.signal.aborted) throw new Error('Terminal archive capture timed out.');
			rowWindows.reverse();
			const archive = archiveWrite(
				presentationId,
				identity,
				lease,
				viewportBytes,
				graphicsBytes,
				rowWindows,
			);
			const admission = encodeTerminalArchive(archive);
			if (!admission.ok) throw new Error(admission.detail);
			if (admission.value.byteLength !== encodedByteLength) {
				throw new Error('Terminal archive capture byte accounting is inconsistent.');
			}
			await this.#store.put(archive);
		} finally {
			clearTimeout(timeout);
			await lease?.dispose();
		}
	}
}

function waitForCaptureLease(
	opening: Promise<DesktopTerminalProjectionLease>,
	signal: AbortSignal,
): Promise<DesktopTerminalProjectionLease> {
	if (signal.aborted) return Promise.reject(new Error('Terminal archive capture timed out.'));
	return new Promise((resolve, reject) => {
		let pending = true;
		const handleAbort = (): void => {
			if (!pending) return;
			pending = false;
			signal.removeEventListener('abort', handleAbort);
			reject(new Error('Terminal archive capture timed out.'));
		};
		signal.addEventListener('abort', handleAbort, { once: true });
		void opening.then(
			(lease) => {
				if (!pending) {
					void lease.dispose().catch(() => undefined);
					return;
				}
				pending = false;
				signal.removeEventListener('abort', handleAbort);
				resolve(lease);
			},
			(error: unknown) => {
				if (!pending) return;
				pending = false;
				signal.removeEventListener('abort', handleAbort);
				reject(error);
			},
		);
		if (signal.aborted) handleAbort();
	});
}

function archiveWrite(
	presentationId: TerminalPresentationId,
	identity: TerminalIdentity,
	lease: DesktopTerminalProjectionLease,
	frame: Uint8Array,
	graphics: Uint8Array,
	rowWindows: readonly Uint8Array[],
): TerminalArchiveWrite {
	return {
		frame,
		graphics,
		identity,
		presentationId,
		rowWindows,
		theme: lease.bootstrap.theme,
	};
}

function waitForProjectedEvent<Value>(
	port: MessagePortMain,
	select: (event: ProjectedTerminalEvent) => Value | null,
	signal: AbortSignal,
): Promise<Value> {
	if (signal.aborted) return Promise.reject(new Error('Terminal archive capture timed out.'));
	return new Promise<Value>((resolve, reject) => {
		const remove = (): void => {
			port.removeListener('message', handleMessage);
			signal.removeEventListener('abort', handleAbort);
		};
		const handleAbort = (): void => {
			remove();
			reject(new Error('Terminal archive capture timed out.'));
		};
		const handleMessage = (message: { readonly data: unknown }): void => {
			const parsed = ProjectedTerminalEventSchema.safeParse(message.data);
			if (!parsed.success) return;
			if (parsed.data.kind === 'failure') {
				remove();
				reject(new Error(`Terminal archive capture failed: ${parsed.data.failure.code}.`));
				return;
			}
			const selected = select(parsed.data);
			if (selected === null) return;
			remove();
			resolve(selected);
		};
		port.on('message', handleMessage);
		signal.addEventListener('abort', handleAbort, { once: true });
		if (signal.aborted) handleAbort();
	});
}

function binaryBytes(value: ArrayBuffer | Uint8Array): Uint8Array {
	return value instanceof Uint8Array ? value : new Uint8Array(value);
}

function copyBytes(value: ArrayBuffer | Uint8Array): Uint8Array {
	const source = binaryBytes(value);
	const copy = new Uint8Array(source.byteLength);
	copy.set(source);
	return copy;
}

function bigintToBoundedNumber(value: bigint, maximum: number): number {
	return value > BigInt(maximum) ? maximum : Number(value);
}

function decimalDigitCount(value: bigint | number): number {
	return value.toString().length;
}
