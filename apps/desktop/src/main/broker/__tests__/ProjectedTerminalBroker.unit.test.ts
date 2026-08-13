import { EventEmitter } from 'node:events';

import {
	AttachmentIdSchema,
	CanonicalUint64TextSchema,
	CorrelationIdSchema,
	CreationIdSchema,
	CURRENT_PROTOCOL_VERSION,
	encodeSelectionExportChunk,
	HostEpochSchema,
	MessageKind,
	PayloadEncoding,
	TerminalIdSchema,
	type DecodedProtocolEnvelope,
} from '@orchbee/terminal-protocol';
import type { MessagePortMain } from 'electron';
import { describe, expect, it, vi } from 'vitest';

import { HostRequestError, type HostConnection } from '../../host';
import { ProjectedTerminalBroker } from '../ProjectedTerminalBroker';

const textEncoder = new TextEncoder();

function eventEnvelope(
	kind: number,
	payloadEncoding: (typeof PayloadEncoding)[keyof typeof PayloadEncoding],
	payload: Uint8Array,
	hostEpoch: ReturnType<typeof HostEpochSchema.parse>,
): DecodedProtocolEnvelope {
	return {
		correlationId: null,
		event: true,
		hostEpoch,
		ignoredOptionalFlags: 0,
		kind,
		knownMessage: true,
		optionalMessage: false,
		payload,
		payloadEncoding,
		protocol: CURRENT_PROTOCOL_VERSION,
		response: false,
	};
}

describe('ProjectedTerminalBroker', () => {
	it('publishes an attachment-scoped resize-lease promotion to its renderer and lifecycle owner', () => {
		let hostListener: ((envelope: DecodedProtocolEnvelope) => void) | undefined;
		const host = {
			onAttachmentEnvelope: vi.fn<HostConnection['onAttachmentEnvelope']>(
				(_attachmentId, listener) => {
					hostListener = listener;
					return vi.fn<() => void>();
				},
			),
			onEnvelope: vi.fn<HostConnection['onEnvelope']>(() => vi.fn<() => void>()),
		} as unknown as HostConnection;
		const emitter = new EventEmitter();
		const postMessage = vi.fn<(...arguments_: unknown[]) => void>();
		const port = Object.assign(emitter, {
			close: vi.fn<() => void>(),
			postMessage,
			start: vi.fn<() => void>(),
		}) as unknown as MessagePortMain;
		const bootstrap = {
			attachmentId: AttachmentIdSchema.parse('11'.repeat(16)),
			canInput: true,
			canResize: false,
			creationId: CreationIdSchema.parse('22'.repeat(16)),
			hostEpoch: HostEpochSchema.parse('33'.repeat(16)),
			terminalId: TerminalIdSchema.parse('44'.repeat(16)),
			terminalOutcome: null,
		};
		const onResizeLeaseChanged = vi.fn<(ownsResize: boolean) => void>();
		const broker = new ProjectedTerminalBroker(
			host,
			port,
			bootstrap,
			{
				openExternal: vi.fn<(target: string) => Promise<void>>().mockResolvedValue(undefined),
				writeClipboardText: vi.fn<(text: string) => Promise<void>>().mockResolvedValue(undefined),
			},
			'desktop',
			{
				onCleanupRequested: vi.fn<(error: unknown) => void>(),
				onResizeLeaseChanged,
			},
		);
		broker.start();

		hostListener?.(
			eventEnvelope(
				MessageKind.resizeLeaseChanged,
				PayloadEncoding.controlJson,
				textEncoder.encode(
					JSON.stringify({
						attachmentId: bootstrap.attachmentId,
						resizeLease: true,
						terminalId: bootstrap.terminalId,
					}),
				),
				bootstrap.hostEpoch,
			),
		);

		expect(onResizeLeaseChanged).toHaveBeenCalledWith(true);
		expect(postMessage).toHaveBeenCalledWith({
			event: {
				attachmentId: bootstrap.attachmentId,
				resizeLease: true,
				terminalId: bootstrap.terminalId,
			},
			kind: 'resize-lease-changed',
		});
	});

	it('copies exact frame bytes once and never interprets terminal cells in Main', () => {
		let hostListener: ((envelope: DecodedProtocolEnvelope) => void) | undefined;
		let controlListener: ((envelope: DecodedProtocolEnvelope) => void) | undefined;
		const host = {
			onAttachmentEnvelope: vi.fn<HostConnection['onAttachmentEnvelope']>(
				(_attachmentId, listener) => {
					hostListener = listener;
					return vi.fn<() => void>();
				},
			),
			onEnvelope: vi.fn<HostConnection['onEnvelope']>((listener) => {
				controlListener = listener;
				return vi.fn<() => void>();
			}),
		} as unknown as HostConnection;
		const emitter = new EventEmitter();
		const postMessage = vi.fn<(...arguments_: unknown[]) => void>();
		const port = Object.assign(emitter, {
			close: vi.fn<() => void>(),
			postMessage,
			start: vi.fn<() => void>(),
		}) as unknown as MessagePortMain;
		const bootstrap = {
			attachmentId: AttachmentIdSchema.parse('11'.repeat(16)),
			canInput: true,
			canResize: true,
			creationId: CreationIdSchema.parse('22'.repeat(16)),
			hostEpoch: HostEpochSchema.parse('33'.repeat(16)),
			terminalId: TerminalIdSchema.parse('44'.repeat(16)),
			terminalOutcome: null,
		};
		const broker = new ProjectedTerminalBroker(host, port, bootstrap);
		broker.start();
		const backing = new Uint8Array([99, 1, 2, 3, 88]);
		const payload = backing.subarray(1, 4);
		hostListener?.({
			correlationId: null,
			event: true,
			hostEpoch: bootstrap.hostEpoch,
			ignoredOptionalFlags: 0,
			kind: MessageKind.snapshotFrame,
			knownMessage: true,
			optionalMessage: false,
			payload,
			payloadEncoding: PayloadEncoding.terminalFrame,
			protocol: CURRENT_PROTOCOL_VERSION,
			response: false,
		});

		const posted = postMessage.mock.calls[0]?.[0] as { frame: unknown; kind: string };
		expect(posted.kind).toBe('frame');
		expect(posted.frame).toBeInstanceOf(ArrayBuffer);
		if (!(posted.frame instanceof ArrayBuffer)) {
			throw new Error('Expected an exact projected frame ArrayBuffer.');
		}
		expect(posted.frame.byteLength).toBe(payload.byteLength);
		expect(new Uint8Array(posted.frame)).toEqual(new Uint8Array([1, 2, 3]));
		const clonedFrame = structuredClone(posted.frame);
		expect(clonedFrame.byteLength).toBe(payload.byteLength);
		expect(broker.metrics).toMatchObject({ frameBytesForwarded: 3, framesForwarded: 1 });
		hostListener?.(
			eventEnvelope(
				MessageKind.terminalBell,
				PayloadEncoding.controlJson,
				textEncoder.encode(JSON.stringify({ attachmentId: bootstrap.attachmentId, sequence: '4' })),
				bootstrap.hostEpoch,
			),
		);
		expect(postMessage.mock.calls[1]?.[0]).toEqual({
			event: { attachmentId: bootstrap.attachmentId, sequence: '4' },
			kind: 'bell',
		});

		controlListener?.(
			eventEnvelope(
				MessageKind.terminalExited,
				PayloadEncoding.controlJson,
				textEncoder.encode(
					JSON.stringify({
						exit: { code: 0, kind: 'exit-code' },
						finalRevision: '1',
						identity: {
							creationId: '55'.repeat(16),
							hostEpoch: bootstrap.hostEpoch,
							terminalId: '66'.repeat(16),
						},
					}),
				),
				bootstrap.hostEpoch,
			),
		);
		const exit = {
			exit: { code: 0, kind: 'exit-code' },
			finalRevision: '1',
			identity: {
				creationId: bootstrap.creationId,
				hostEpoch: bootstrap.hostEpoch,
				terminalId: bootstrap.terminalId,
			},
		};
		const validExitEnvelope = eventEnvelope(
			MessageKind.terminalExited,
			PayloadEncoding.controlJson,
			textEncoder.encode(JSON.stringify(exit)),
			bootstrap.hostEpoch,
		);
		controlListener?.(validExitEnvelope);
		controlListener?.(validExitEnvelope);
		expect(postMessage).toHaveBeenCalledTimes(3);
		expect(postMessage.mock.calls[2]?.[0]).toEqual({
			event: exit,
			kind: 'terminal-exited',
		});
	});

	it('rejects commands for another attachment before calling the Host', () => {
		const host = {
			onAttachmentEnvelope: vi.fn<HostConnection['onAttachmentEnvelope']>(() =>
				vi.fn<() => void>(),
			),
			onEnvelope: vi.fn<HostConnection['onEnvelope']>(() => vi.fn<() => void>()),
			requestInput: vi.fn<(...arguments_: unknown[]) => Promise<unknown>>(),
			requestControl: vi
				.fn<(...arguments_: unknown[]) => Promise<unknown>>()
				.mockResolvedValue(undefined),
		} as unknown as HostConnection;
		const emitter = new EventEmitter();
		const postMessage = vi.fn<(...arguments_: unknown[]) => void>();
		const port = Object.assign(emitter, {
			close: vi.fn<() => void>(),
			postMessage,
			start: vi.fn<() => void>(),
		}) as unknown as MessagePortMain;
		const broker = new ProjectedTerminalBroker(host, port, {
			attachmentId: AttachmentIdSchema.parse('55'.repeat(16)),
			canInput: true,
			canResize: true,
			creationId: CreationIdSchema.parse('66'.repeat(16)),
			hostEpoch: HostEpochSchema.parse('77'.repeat(16)),
			terminalId: TerminalIdSchema.parse('88'.repeat(16)),
			terminalOutcome: null,
		});
		broker.start();
		emitter.emit('message', {
			data: {
				kind: 'text-input',
				payload: {
					attachmentId: AttachmentIdSchema.parse('99'.repeat(16)),
					text: 'blocked',
				},
			},
		});

		expect(host.requestInput).not.toHaveBeenCalled();
		expect(postMessage).toHaveBeenCalledWith({
			failure: expect.objectContaining({ code: 'renderer-capability-rejected' }),
			kind: 'failure',
		});
		expect(broker.metrics.commandsRejected).toBe(1);
	});

	it('forwards clear and reset as typed Host controls and rejects a mismatched acknowledgement', async () => {
		const bootstrap = {
			attachmentId: AttachmentIdSchema.parse('11'.repeat(16)),
			canInput: true,
			canResize: true,
			creationId: CreationIdSchema.parse('22'.repeat(16)),
			hostEpoch: HostEpochSchema.parse('33'.repeat(16)),
			terminalId: TerminalIdSchema.parse('44'.repeat(16)),
			terminalOutcome: null,
		};
		let resetCount = 0;
		const requestControl = vi.fn<(kind: number) => Promise<unknown>>(async (kind) => {
			const operation = kind === MessageKind.clearTerminal ? 'clear' : 'reset';
			if (kind === MessageKind.resetTerminal) {
				resetCount += 1;
				if (resetCount === 3) {
					throw new HostRequestError(
						{
							code: 'terminal-control-backpressure',
							message: 'The Host control queue is full.',
							retryable: true,
						},
						CorrelationIdSchema.parse('55'.repeat(16)),
					);
				}
			}
			return {
				attachmentId: bootstrap.attachmentId,
				generation: String(resetCount + 1),
				operation: resetCount > 1 ? 'clear' : operation,
				revision: String(resetCount + 1),
				terminalId: bootstrap.terminalId,
			};
		});
		const host = {
			onAttachmentEnvelope: vi.fn<HostConnection['onAttachmentEnvelope']>(() =>
				vi.fn<() => void>(),
			),
			onEnvelope: vi.fn<HostConnection['onEnvelope']>(() => vi.fn<() => void>()),
			requestControl,
		} as unknown as HostConnection;
		const emitter = new EventEmitter();
		const postMessage = vi.fn<(...arguments_: unknown[]) => void>();
		const port = Object.assign(emitter, {
			close: vi.fn<() => void>(),
			postMessage,
			start: vi.fn<() => void>(),
		}) as unknown as MessagePortMain;
		const broker = new ProjectedTerminalBroker(host, port, bootstrap);
		broker.start();

		for (const kind of ['clear-terminal', 'reset-terminal'] as const) {
			emitter.emit('message', {
				data: { kind, payload: { attachmentId: bootstrap.attachmentId } },
			});
		}
		await vi.waitFor(() => expect(requestControl).toHaveBeenCalledTimes(2));
		expect(requestControl).toHaveBeenNthCalledWith(
			1,
			MessageKind.clearTerminal,
			{ attachmentId: bootstrap.attachmentId },
			expect.anything(),
		);
		expect(requestControl).toHaveBeenNthCalledWith(
			2,
			MessageKind.resetTerminal,
			{ attachmentId: bootstrap.attachmentId },
			expect.anything(),
		);
		expect(postMessage).not.toHaveBeenCalled();

		emitter.emit('message', {
			data: { kind: 'reset-terminal', payload: { attachmentId: bootstrap.attachmentId } },
		});
		await vi.waitFor(() =>
			expect(postMessage).toHaveBeenCalledWith({
				failure: expect.objectContaining({
					code: 'invalid-terminal-control-response',
					operation: 'control',
					retryable: false,
				}),
				kind: 'failure',
			}),
		);

		emitter.emit('message', {
			data: { kind: 'reset-terminal', payload: { attachmentId: bootstrap.attachmentId } },
		});
		await vi.waitFor(() =>
			expect(postMessage).toHaveBeenCalledWith({
				failure: {
					code: 'terminal-control-backpressure',
					correlationId: CorrelationIdSchema.parse('55'.repeat(16)),
					message: 'The Host control queue is full.',
					operation: 'control',
					retryable: true,
				},
				kind: 'failure',
			}),
		);
	});

	it('routes one bounded row response without interpreting its cells in Main', async () => {
		const bootstrap = {
			attachmentId: AttachmentIdSchema.parse('11'.repeat(16)),
			canInput: true,
			canResize: true,
			creationId: CreationIdSchema.parse('22'.repeat(16)),
			hostEpoch: HostEpochSchema.parse('33'.repeat(16)),
			terminalId: TerminalIdSchema.parse('44'.repeat(16)),
			terminalOutcome: null,
		};
		const responseBacking = new Uint8Array([99, 7, 8, 9, 88]);
		const responsePayload = responseBacking.subarray(1, 4);
		const requestControlEnvelope = vi
			.fn<
				(...arguments_: unknown[]) => Promise<{
					readonly payload: Uint8Array;
					readonly payloadEncoding: typeof PayloadEncoding.rowWindow;
				}>
			>()
			.mockResolvedValue({
				payload: responsePayload,
				payloadEncoding: PayloadEncoding.rowWindow,
			});
		const host = {
			onAttachmentEnvelope: vi.fn<HostConnection['onAttachmentEnvelope']>(() =>
				vi.fn<() => void>(),
			),
			onEnvelope: vi.fn<HostConnection['onEnvelope']>(() => vi.fn<() => void>()),
			requestControlEnvelope,
		} as unknown as HostConnection;
		const emitter = new EventEmitter();
		const postMessage = vi.fn<(...arguments_: unknown[]) => void>();
		const port = Object.assign(emitter, {
			close: vi.fn<() => void>(),
			postMessage,
			start: vi.fn<() => void>(),
		}) as unknown as MessagePortMain;
		const broker = new ProjectedTerminalBroker(host, port, bootstrap);
		broker.start();
		emitter.emit('message', {
			data: {
				kind: 'read-rows',
				payload: {
					attachmentId: bootstrap.attachmentId,
					detail: 'text',
					generation: '1',
					maximumBytes: 1024,
					origin: { kind: 'absolute', row: '0' },
					purpose: 'buffer',
					requestSequence: '1',
					revision: '1',
					rowCount: 1,
				},
			},
		});

		await vi.waitFor(() => {
			expect(postMessage).toHaveBeenCalledWith(
				expect.objectContaining({ kind: 'rows', rows: expect.any(ArrayBuffer) }),
			);
		});
		const posted = postMessage.mock.calls[0]?.[0] as { rows: unknown };
		expect(posted.rows).toBeInstanceOf(ArrayBuffer);
		if (!(posted.rows instanceof ArrayBuffer)) {
			throw new Error('Expected an exact projected row-window ArrayBuffer.');
		}
		expect(posted.rows.byteLength).toBe(responsePayload.byteLength);
		expect(new Uint8Array(posted.rows)).toEqual(new Uint8Array([7, 8, 9]));
		const clonedRows = structuredClone(posted.rows);
		expect(clonedRows.byteLength).toBe(responsePayload.byteLength);
		expect(broker.metrics).toMatchObject({
			rowWindowBytesForwarded: 3,
			rowWindowsForwarded: 1,
		});
	});

	it('streams a bounded Host selection directly into Main clipboard exactly once', async () => {
		let hostListener: ((envelope: DecodedProtocolEnvelope) => void) | undefined;
		const bootstrap = {
			attachmentId: AttachmentIdSchema.parse('11'.repeat(16)),
			canInput: true,
			canResize: true,
			creationId: CreationIdSchema.parse('22'.repeat(16)),
			hostEpoch: HostEpochSchema.parse('33'.repeat(16)),
			terminalId: TerminalIdSchema.parse('44'.repeat(16)),
			terminalOutcome: null,
		};
		const requestControl = vi.fn<(kind: number, value: unknown) => Promise<unknown>>(
			async (kind, value) => {
				if (kind === MessageKind.selectionExport) {
					const exportRequest = value as { exportSequence: string };
					return {
						attachmentId: bootstrap.attachmentId,
						byteLength: 8,
						exportSequence: exportRequest.exportSequence,
						format: 'plain',
					};
				}
				if (kind === MessageKind.selectionExportChunkApplied) return value;
				throw new Error(`unexpected Host control kind ${kind}`);
			},
		);
		const host = {
			onAttachmentEnvelope: vi.fn<HostConnection['onAttachmentEnvelope']>(
				(_attachmentId, listener) => {
					hostListener = listener;
					return vi.fn<() => void>();
				},
			),
			onEnvelope: vi.fn<HostConnection['onEnvelope']>(() => vi.fn<() => void>()),
			requestControl,
		} as unknown as HostConnection;
		const emitter = new EventEmitter();
		const postMessage = vi.fn<(...arguments_: unknown[]) => void>();
		const port = Object.assign(emitter, {
			close: vi.fn<() => void>(),
			postMessage,
			start: vi.fn<() => void>(),
		}) as unknown as MessagePortMain;
		const writeClipboardText = vi.fn<(text: string) => void>();
		const broker = new ProjectedTerminalBroker(host, port, bootstrap, {
			openExternal: vi.fn<(target: string) => Promise<void>>().mockResolvedValue(undefined),
			writeClipboardText,
		});
		broker.start();
		emitter.emit('message', {
			data: {
				kind: 'selection-copy',
				payload: { attachmentId: bootstrap.attachmentId, requestSequence: '7' },
			},
		});
		await vi.waitFor(() => expect(requestControl).toHaveBeenCalled());
		const chunk = encodeSelectionExportChunk({
			attachmentId: bootstrap.attachmentId,
			bytes: textEncoder.encode('selected'),
			chunkSequence: 0,
			exportSequence: 1n,
			final: true,
		});
		if (!chunk.ok) throw new Error(chunk.detail);
		hostListener?.(
			eventEnvelope(
				MessageKind.selectionExportChunk,
				PayloadEncoding.rawBytes,
				chunk.value,
				bootstrap.hostEpoch,
			),
		);
		await vi.waitFor(() => {
			expect(requestControl).toHaveBeenCalledWith(
				MessageKind.selectionExportChunkApplied,
				expect.objectContaining({ chunkSequence: 0, exportSequence: '1' }),
				expect.anything(),
			);
		});
		hostListener?.(
			eventEnvelope(
				MessageKind.selectionExportFinished,
				PayloadEncoding.controlJson,
				textEncoder.encode(
					JSON.stringify({
						attachmentId: bootstrap.attachmentId,
						byteLength: 8,
						exportSequence: '1',
						status: 'completed',
					}),
				),
				bootstrap.hostEpoch,
			),
		);

		await vi.waitFor(() => expect(writeClipboardText).toHaveBeenCalledWith('selected'));
		expect(writeClipboardText).toHaveBeenCalledTimes(1);
		expect(postMessage).toHaveBeenCalledWith({
			byteLength: 8,
			kind: 'selection-copied',
			requestSequence: CanonicalUint64TextSchema.parse('7'),
		});
		expect(postMessage.mock.calls).not.toContainEqual([
			expect.objectContaining({ text: 'selected' }),
		]);
		expect(broker.metrics).toMatchObject({
			clipboardCommits: 1,
			selectionExportBytesForwarded: 8,
			selectionExportChunksForwarded: 1,
		});
		expect(broker.metrics.lastSelectionCopyMilliseconds).toBeGreaterThanOrEqual(0);
		expect(broker.metrics.lastSelectionExportStartMilliseconds).toBeGreaterThanOrEqual(0);
	});

	it('ignores a predecessor cancellation while the next selection export is still starting', async () => {
		let hostListener: ((envelope: DecodedProtocolEnvelope) => void) | undefined;
		let resolveFirstStart: ((value: unknown) => void) | undefined;
		let resolveSecondStart: ((value: unknown) => void) | undefined;
		let selectionExportRequests = 0;
		const bootstrap = {
			attachmentId: AttachmentIdSchema.parse('11'.repeat(16)),
			canInput: true,
			canResize: true,
			creationId: CreationIdSchema.parse('22'.repeat(16)),
			hostEpoch: HostEpochSchema.parse('33'.repeat(16)),
			terminalId: TerminalIdSchema.parse('44'.repeat(16)),
			terminalOutcome: null,
		};
		const firstStarted = {
			attachmentId: bootstrap.attachmentId,
			byteLength: 8,
			exportSequence: '1',
			format: 'plain',
		};
		const secondStarted = {
			attachmentId: bootstrap.attachmentId,
			byteLength: 8,
			exportSequence: '2',
			format: 'plain',
		};
		const cancelledFirstExport = (): void => {
			hostListener?.(
				eventEnvelope(
					MessageKind.selectionExportFinished,
					PayloadEncoding.controlJson,
					textEncoder.encode(
						JSON.stringify({
							attachmentId: bootstrap.attachmentId,
							byteLength: firstStarted.byteLength,
							exportSequence: firstStarted.exportSequence,
							status: 'cancelled',
						}),
					),
					bootstrap.hostEpoch,
				),
			);
		};
		const failureCount = (): number =>
			postMessage.mock.calls.filter(
				([message]) => (message as { kind?: string }).kind === 'failure',
			).length;
		const requestControl = vi.fn<(kind: number, value: unknown) => Promise<unknown>>(
			async (kind, value) => {
				if (kind === MessageKind.selectionExport) {
					selectionExportRequests += 1;
					const exportRequest = value as { exportSequence: string };
					if (selectionExportRequests === 1) {
						if (exportRequest.exportSequence !== firstStarted.exportSequence) {
							throw new Error(`unexpected first export sequence ${exportRequest.exportSequence}`);
						}
						return new Promise((resolve) => {
							resolveFirstStart = resolve;
						});
					}
					if (
						selectionExportRequests !== 2 ||
						exportRequest.exportSequence !== secondStarted.exportSequence
					) {
						throw new Error(`unexpected selection export request ${selectionExportRequests}`);
					}
					// HostConnection can route an event from the same read before the
					// response to this request is resolved.
					cancelledFirstExport();
					return new Promise((resolve) => {
						resolveSecondStart = resolve;
					});
				}
				if (kind === MessageKind.selectionExportChunkApplied) return value;
				throw new Error(`unexpected Host control kind ${kind}`);
			},
		);
		const host = {
			onAttachmentEnvelope: vi.fn<HostConnection['onAttachmentEnvelope']>(
				(_attachmentId, listener) => {
					hostListener = listener;
					return vi.fn<() => void>();
				},
			),
			onEnvelope: vi.fn<HostConnection['onEnvelope']>(() => vi.fn<() => void>()),
			requestControl,
		} as unknown as HostConnection;
		const emitter = new EventEmitter();
		const postMessage = vi.fn<(...arguments_: unknown[]) => void>();
		const port = Object.assign(emitter, {
			close: vi.fn<() => void>(),
			postMessage,
			start: vi.fn<() => void>(),
		}) as unknown as MessagePortMain;
		const writeClipboardText = vi.fn<(text: string) => void>();
		const broker = new ProjectedTerminalBroker(host, port, bootstrap, {
			openExternal: vi.fn<(target: string) => Promise<void>>().mockResolvedValue(undefined),
			writeClipboardText,
		});
		broker.start();

		emitter.emit('message', {
			data: {
				kind: 'selection-copy',
				payload: { attachmentId: bootstrap.attachmentId, requestSequence: '7' },
			},
		});
		await vi.waitFor(() =>
			expect(requestControl).toHaveBeenCalledWith(
				MessageKind.selectionExport,
				expect.objectContaining({ exportSequence: firstStarted.exportSequence }),
				expect.anything(),
			),
		);
		expect(resolveFirstStart).toBeTypeOf('function');
		resolveFirstStart?.(firstStarted);
		await vi.waitFor(() =>
			expect(broker.metrics.lastSelectionExportStartMilliseconds).toBeGreaterThanOrEqual(0),
		);
		cancelledFirstExport();
		await vi.waitFor(() =>
			expect(postMessage).toHaveBeenCalledWith({
				failure: expect.objectContaining({
					code: 'copy-cancelled',
					operation: 'copy',
					requestSequence: '7',
					retryable: true,
				}),
				kind: 'failure',
			}),
		);
		expect(failureCount()).toBe(1);

		emitter.emit('message', {
			data: {
				kind: 'selection-copy',
				payload: { attachmentId: bootstrap.attachmentId, requestSequence: '8' },
			},
		});
		await vi.waitFor(() => expect(selectionExportRequests).toBe(2));
		await Promise.resolve();
		expect(failureCount()).toBe(1);
		expect(resolveSecondStart).toBeTypeOf('function');
		resolveSecondStart?.(secondStarted);
		const secondChunk = encodeSelectionExportChunk({
			attachmentId: bootstrap.attachmentId,
			bytes: textEncoder.encode('selected'),
			chunkSequence: 0,
			exportSequence: 2n,
			final: true,
		});
		if (!secondChunk.ok) throw new Error(secondChunk.detail);
		await vi.waitFor(() =>
			expect(requestControl).toHaveBeenCalledWith(
				MessageKind.selectionExport,
				expect.objectContaining({ exportSequence: secondStarted.exportSequence }),
				expect.anything(),
			),
		);
		hostListener?.(
			eventEnvelope(
				MessageKind.selectionExportChunk,
				PayloadEncoding.rawBytes,
				secondChunk.value,
				bootstrap.hostEpoch,
			),
		);
		await vi.waitFor(() =>
			expect(requestControl).toHaveBeenCalledWith(
				MessageKind.selectionExportChunkApplied,
				expect.objectContaining({ exportSequence: secondStarted.exportSequence }),
				expect.anything(),
			),
		);
		hostListener?.(
			eventEnvelope(
				MessageKind.selectionExportFinished,
				PayloadEncoding.controlJson,
				textEncoder.encode(
					JSON.stringify({
						attachmentId: bootstrap.attachmentId,
						byteLength: secondStarted.byteLength,
						exportSequence: secondStarted.exportSequence,
						status: 'completed',
					}),
				),
				bootstrap.hostEpoch,
			),
		);
		await vi.waitFor(() => expect(writeClipboardText).toHaveBeenCalledWith('selected'));
		expect(writeClipboardText).toHaveBeenCalledTimes(1);
		expect(failureCount()).toBe(1);
	});

	it('returns Host selection bytes to a Browser attachment without granting Main effects', async () => {
		let hostListener: ((envelope: DecodedProtocolEnvelope) => void) | undefined;
		const bootstrap = {
			attachmentId: AttachmentIdSchema.parse('11'.repeat(16)),
			canInput: true,
			canResize: true,
			creationId: CreationIdSchema.parse('22'.repeat(16)),
			hostEpoch: HostEpochSchema.parse('33'.repeat(16)),
			terminalId: TerminalIdSchema.parse('44'.repeat(16)),
			terminalOutcome: null,
		};
		const requestControl = vi.fn<(kind: number, value: unknown) => Promise<unknown>>(
			async (kind, value) => {
				if (kind === MessageKind.selectionExport) {
					const exportRequest = value as { exportSequence: string };
					return {
						attachmentId: bootstrap.attachmentId,
						byteLength: 8,
						exportSequence: exportRequest.exportSequence,
						format: 'plain',
					};
				}
				if (kind === MessageKind.selectionExportChunkApplied) return value;
				throw new Error(`unexpected Host control kind ${kind}`);
			},
		);
		const host = {
			onAttachmentEnvelope: vi.fn<HostConnection['onAttachmentEnvelope']>(
				(_attachmentId, listener) => {
					hostListener = listener;
					return vi.fn<() => void>();
				},
			),
			onEnvelope: vi.fn<HostConnection['onEnvelope']>(() => vi.fn<() => void>()),
			requestControl,
		} as unknown as HostConnection;
		const emitter = new EventEmitter();
		const postMessage = vi.fn<(...arguments_: unknown[]) => void>();
		const port = Object.assign(emitter, {
			close: vi.fn<() => void>(),
			postMessage,
			start: vi.fn<() => void>(),
		}) as unknown as MessagePortMain;
		const openExternal = vi.fn<(_target: string) => Promise<void>>().mockResolvedValue();
		const writeClipboardText = vi.fn<(text: string) => void>();
		const broker = new ProjectedTerminalBroker(
			host,
			port,
			bootstrap,
			{ openExternal, writeClipboardText },
			'client',
		);
		broker.start();
		emitter.emit('message', {
			data: {
				kind: 'selection-copy',
				payload: { attachmentId: bootstrap.attachmentId, requestSequence: '7' },
			},
		});
		await vi.waitFor(() => expect(requestControl).toHaveBeenCalled());
		const chunk = encodeSelectionExportChunk({
			attachmentId: bootstrap.attachmentId,
			bytes: textEncoder.encode('selected'),
			chunkSequence: 0,
			exportSequence: 1n,
			final: true,
		});
		if (!chunk.ok) throw new Error(chunk.detail);
		hostListener?.(
			eventEnvelope(
				MessageKind.selectionExportChunk,
				PayloadEncoding.rawBytes,
				chunk.value,
				bootstrap.hostEpoch,
			),
		);
		await vi.waitFor(() =>
			expect(requestControl).toHaveBeenCalledWith(
				MessageKind.selectionExportChunkApplied,
				expect.anything(),
				expect.anything(),
			),
		);
		hostListener?.(
			eventEnvelope(
				MessageKind.selectionExportFinished,
				PayloadEncoding.controlJson,
				textEncoder.encode(
					JSON.stringify({
						attachmentId: bootstrap.attachmentId,
						byteLength: 8,
						exportSequence: '1',
						status: 'completed',
					}),
				),
				bootstrap.hostEpoch,
			),
		);

		await vi.waitFor(() =>
			expect(postMessage).toHaveBeenCalledWith(
				expect.objectContaining({ kind: 'selection-copy-content', requestSequence: '7' }),
			),
		);
		const copyEvent = postMessage.mock.calls.find(
			(call) => (call[0] as { kind?: string }).kind === 'selection-copy-content',
		)?.[0] as { content: ArrayBuffer } | undefined;
		expect(new TextDecoder().decode(copyEvent?.content)).toBe('selected');
		expect(writeClipboardText).not.toHaveBeenCalled();
		expect(broker.metrics.clipboardCommits).toBe(0);

		emitter.emit('message', {
			data: {
				kind: 'link-open',
				payload: {
					attachmentId: bootstrap.attachmentId,
					requestSequence: '8',
					target: 'https://orchbee.dev',
				},
			},
		});
		await vi.waitFor(() =>
			expect(postMessage).toHaveBeenCalledWith({
				failure: expect.objectContaining({ code: 'link-client-effect-required' }),
				kind: 'failure',
			}),
		);
		expect(openExternal).not.toHaveBeenCalled();
	});

	it('allows only explicit web and mail schemes through the external-link effect', async () => {
		const bootstrap = {
			attachmentId: AttachmentIdSchema.parse('11'.repeat(16)),
			canInput: true,
			canResize: true,
			creationId: CreationIdSchema.parse('22'.repeat(16)),
			hostEpoch: HostEpochSchema.parse('33'.repeat(16)),
			terminalId: TerminalIdSchema.parse('44'.repeat(16)),
			terminalOutcome: null,
		};
		const host = {
			onAttachmentEnvelope: vi.fn<HostConnection['onAttachmentEnvelope']>(() =>
				vi.fn<() => void>(),
			),
			onEnvelope: vi.fn<HostConnection['onEnvelope']>(() => vi.fn<() => void>()),
		} as unknown as HostConnection;
		const emitter = new EventEmitter();
		const postMessage = vi.fn<(...arguments_: unknown[]) => void>();
		const port = Object.assign(emitter, {
			close: vi.fn<() => void>(),
			postMessage,
			start: vi.fn<() => void>(),
		}) as unknown as MessagePortMain;
		const openExternal = vi.fn<(_target: string) => Promise<void>>().mockResolvedValue(undefined);
		const broker = new ProjectedTerminalBroker(host, port, bootstrap, {
			openExternal,
			writeClipboardText: vi.fn<(text: string) => void>(),
		});
		broker.start();
		for (const [requestSequence, target] of [
			['1', 'https://orchbee.dev/终端'],
			['2', 'file:///tmp/secret'],
		] as const) {
			emitter.emit('message', {
				data: {
					kind: 'link-open',
					payload: { attachmentId: bootstrap.attachmentId, requestSequence, target },
				},
			});
		}

		await vi.waitFor(() => expect(openExternal).toHaveBeenCalledTimes(1));
		expect(openExternal).toHaveBeenCalledWith('https://orchbee.dev/%E7%BB%88%E7%AB%AF');
		await vi.waitFor(() => {
			expect(postMessage).toHaveBeenCalledWith({
				kind: 'link-opened',
				requestSequence: CanonicalUint64TextSchema.parse('1'),
			});
		});
		await vi.waitFor(() => {
			expect(postMessage).toHaveBeenCalledWith({
				failure: expect.objectContaining({ code: 'link-policy-blocked' }),
				kind: 'failure',
			});
		});
		expect(broker.metrics.externalTargetsOpened).toBe(1);
	});

	it('hands renderer postMessage failure to the cleanup owner before detaching', async () => {
		const bootstrap = {
			attachmentId: AttachmentIdSchema.parse('11'.repeat(16)),
			canInput: true,
			canResize: true,
			creationId: CreationIdSchema.parse('22'.repeat(16)),
			hostEpoch: HostEpochSchema.parse('33'.repeat(16)),
			terminalId: TerminalIdSchema.parse('44'.repeat(16)),
			terminalOutcome: null,
		};
		const requestControl = vi
			.fn<(...arguments_: unknown[]) => Promise<unknown>>()
			.mockResolvedValue({ attachmentId: bootstrap.attachmentId });
		const host = {
			onAttachmentEnvelope: vi.fn<HostConnection['onAttachmentEnvelope']>(() =>
				vi.fn<() => void>(),
			),
			onEnvelope: vi.fn<HostConnection['onEnvelope']>(() => vi.fn<() => void>()),
			requestControl,
		} as unknown as HostConnection;
		const emitter = new EventEmitter();
		const postMessage = vi.fn<(...arguments_: unknown[]) => void>().mockImplementationOnce(() => {
			throw new Error('renderer port failed');
		});
		const port = Object.assign(emitter, {
			close: vi.fn<() => void>(),
			postMessage,
			start: vi.fn<() => void>(),
		}) as unknown as MessagePortMain;
		let broker: ProjectedTerminalBroker;
		const onCleanupRequested = vi.fn<() => void>(() => {
			void broker.dispose().catch(() => undefined);
		});
		broker = new ProjectedTerminalBroker(host, port, bootstrap, undefined, 'desktop', {
			onCleanupRequested,
		});
		broker.start();

		emitter.emit('message', { data: { malformed: true } });

		await vi.waitFor(() => expect(requestControl).toHaveBeenCalledOnce());
		expect(onCleanupRequested).toHaveBeenCalledOnce();
		expect(requestControl).toHaveBeenCalledWith(
			MessageKind.detachTerminal,
			{ attachmentId: bootstrap.attachmentId },
			expect.anything(),
		);
	});

	it('awaits Host detach exactly once before completing attachment retirement', async () => {
		const bootstrap = {
			attachmentId: AttachmentIdSchema.parse('11'.repeat(16)),
			canInput: true,
			canResize: true,
			creationId: CreationIdSchema.parse('22'.repeat(16)),
			hostEpoch: HostEpochSchema.parse('33'.repeat(16)),
			terminalId: TerminalIdSchema.parse('44'.repeat(16)),
			terminalOutcome: null,
		};
		let finishDetach: (() => void) | undefined;
		const requestControl = vi.fn<(...arguments_: unknown[]) => Promise<unknown>>(
			() =>
				new Promise((resolve) => {
					finishDetach = () => resolve({ attachmentId: bootstrap.attachmentId });
				}),
		);
		const host = {
			onAttachmentEnvelope: vi.fn<HostConnection['onAttachmentEnvelope']>(() =>
				vi.fn<() => void>(),
			),
			onEnvelope: vi.fn<HostConnection['onEnvelope']>(() => vi.fn<() => void>()),
			requestControl,
		} as unknown as HostConnection;
		const emitter = new EventEmitter();
		const port = Object.assign(emitter, {
			close: vi.fn<() => void>(),
			postMessage: vi.fn<(...arguments_: unknown[]) => void>(),
			start: vi.fn<() => void>(),
		}) as unknown as MessagePortMain;
		const broker = new ProjectedTerminalBroker(host, port, bootstrap);
		broker.start();

		const first = broker.dispose();
		const second = broker.dispose();
		expect(first).toBe(second);
		expect(requestControl).toHaveBeenCalledWith(
			MessageKind.detachTerminal,
			{ attachmentId: bootstrap.attachmentId },
			expect.anything(),
		);
		let completed = false;
		void first.then(() => {
			completed = true;
		});
		await Promise.resolve();
		expect(completed).toBe(false);
		finishDetach?.();
		await first;
		expect(completed).toBe(true);
		expect(requestControl).toHaveBeenCalledOnce();
	});

	it('surfaces a failed detach and retries the same AttachmentId', async () => {
		const bootstrap = {
			attachmentId: AttachmentIdSchema.parse('11'.repeat(16)),
			canInput: true,
			canResize: true,
			creationId: CreationIdSchema.parse('22'.repeat(16)),
			hostEpoch: HostEpochSchema.parse('33'.repeat(16)),
			terminalId: TerminalIdSchema.parse('44'.repeat(16)),
			terminalOutcome: null,
		};
		const requestControl = vi
			.fn<(...arguments_: unknown[]) => Promise<unknown>>()
			.mockRejectedValueOnce(new Error('detach unavailable'))
			.mockResolvedValue({ attachmentId: bootstrap.attachmentId });
		const host = {
			onAttachmentEnvelope: vi.fn<HostConnection['onAttachmentEnvelope']>(() =>
				vi.fn<() => void>(),
			),
			onEnvelope: vi.fn<HostConnection['onEnvelope']>(() => vi.fn<() => void>()),
			requestControl,
		} as unknown as HostConnection;
		const emitter = new EventEmitter();
		const port = Object.assign(emitter, {
			close: vi.fn<() => void>(),
			postMessage: vi.fn<(...arguments_: unknown[]) => void>(),
			start: vi.fn<() => void>(),
		}) as unknown as MessagePortMain;
		const broker = new ProjectedTerminalBroker(host, port, bootstrap);
		broker.start();

		await expect(broker.dispose()).rejects.toThrow('detach unavailable');
		await expect(broker.dispose()).resolves.toBeUndefined();
		expect(requestControl).toHaveBeenCalledTimes(2);
		expect(requestControl).toHaveBeenNthCalledWith(
			1,
			MessageKind.detachTerminal,
			{ attachmentId: bootstrap.attachmentId },
			expect.anything(),
		);
	});

	it('treats a mismatched detach ACK as a retryable cleanup failure', async () => {
		const bootstrap = {
			attachmentId: AttachmentIdSchema.parse('11'.repeat(16)),
			canInput: true,
			canResize: true,
			creationId: CreationIdSchema.parse('22'.repeat(16)),
			hostEpoch: HostEpochSchema.parse('33'.repeat(16)),
			terminalId: TerminalIdSchema.parse('44'.repeat(16)),
			terminalOutcome: null,
		};
		const requestControl = vi
			.fn<(...arguments_: unknown[]) => Promise<unknown>>()
			.mockResolvedValueOnce({ attachmentId: AttachmentIdSchema.parse('ff'.repeat(16)) })
			.mockResolvedValue({ attachmentId: bootstrap.attachmentId });
		const host = {
			onAttachmentEnvelope: vi.fn<HostConnection['onAttachmentEnvelope']>(() =>
				vi.fn<() => void>(),
			),
			onEnvelope: vi.fn<HostConnection['onEnvelope']>(() => vi.fn<() => void>()),
			requestControl,
		} as unknown as HostConnection;
		const emitter = new EventEmitter();
		const port = Object.assign(emitter, {
			close: vi.fn<() => void>(),
			postMessage: vi.fn<(...arguments_: unknown[]) => void>(),
			start: vi.fn<() => void>(),
		}) as unknown as MessagePortMain;
		const broker = new ProjectedTerminalBroker(host, port, bootstrap);
		broker.start();

		await expect(broker.dispose()).rejects.toMatchObject({ code: 'attachment-ack-mismatch' });
		await expect(broker.dispose()).resolves.toBeUndefined();
		expect(requestControl).toHaveBeenCalledTimes(2);
	});
});
