import { EventEmitter } from 'node:events';

import {
	AttachmentIdSchema,
	CanonicalUint64TextSchema,
	CreationIdSchema,
	encodeSelectionExportChunk,
	HostEpochSchema,
	MessageKind,
	PayloadEncoding,
	TerminalIdSchema,
	type DecodedProtocolEnvelope,
} from '@orchbee/terminal-protocol';
import type { MessagePortMain } from 'electron';
import { describe, expect, it, vi } from 'vitest';

import type { HostConnection } from '../../host';
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
		protocol: { major: 1, minor: 2 },
		response: false,
	};
}

describe('ProjectedTerminalBroker', () => {
	it('copies exact frame bytes once and never interprets terminal cells in Main', () => {
		let hostListener: ((envelope: DecodedProtocolEnvelope) => void) | undefined;
		const host = {
			onEnvelope: vi.fn<HostConnection['onEnvelope']>((listener) => {
				hostListener = listener;
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
			canResize: true,
			creationId: CreationIdSchema.parse('22'.repeat(16)),
			hostEpoch: HostEpochSchema.parse('33'.repeat(16)),
			terminalId: TerminalIdSchema.parse('44'.repeat(16)),
		};
		const broker = new ProjectedTerminalBroker(host, port, bootstrap);
		broker.start();
		const backing = new Uint8Array([99, 1, 2, 3, 88]);
		hostListener?.({
			correlationId: null,
			event: true,
			hostEpoch: bootstrap.hostEpoch,
			ignoredOptionalFlags: 0,
			kind: MessageKind.snapshotFrame,
			knownMessage: true,
			optionalMessage: false,
			payload: backing.subarray(1, 4),
			payloadEncoding: PayloadEncoding.terminalFrame,
			protocol: { major: 1, minor: 1 },
			response: false,
		});

		const posted = postMessage.mock.calls[0]?.[0] as { frame: ArrayBuffer; kind: string };
		expect(posted.kind).toBe('frame');
		expect(new Uint8Array(posted.frame)).toEqual(new Uint8Array([1, 2, 3]));
		expect(posted.frame).not.toBe(backing.buffer);
		expect(broker.metrics).toMatchObject({ frameBytesForwarded: 3, framesForwarded: 1 });
	});

	it('rejects commands for another attachment before calling the Host', () => {
		const host = {
			onEnvelope: vi.fn<HostConnection['onEnvelope']>(() => vi.fn<() => void>()),
			request: vi.fn<(...arguments_: unknown[]) => Promise<unknown>>(),
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
			canResize: true,
			creationId: CreationIdSchema.parse('66'.repeat(16)),
			hostEpoch: HostEpochSchema.parse('77'.repeat(16)),
			terminalId: TerminalIdSchema.parse('88'.repeat(16)),
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

		expect(host.request).not.toHaveBeenCalled();
		expect(postMessage).toHaveBeenCalledWith({
			failure: expect.objectContaining({ code: 'renderer-capability-rejected' }),
			kind: 'failure',
		});
		expect(broker.metrics.commandsRejected).toBe(1);
	});

	it('routes one bounded row response without interpreting its cells in Main', async () => {
		const bootstrap = {
			attachmentId: AttachmentIdSchema.parse('11'.repeat(16)),
			canResize: true,
			creationId: CreationIdSchema.parse('22'.repeat(16)),
			hostEpoch: HostEpochSchema.parse('33'.repeat(16)),
			terminalId: TerminalIdSchema.parse('44'.repeat(16)),
		};
		const request = vi
			.fn<
				(...arguments_: unknown[]) => Promise<{
					readonly payload: Uint8Array;
					readonly payloadEncoding: typeof PayloadEncoding.rowWindow;
				}>
			>()
			.mockResolvedValue({
				payload: new Uint8Array([7, 8, 9]),
				payloadEncoding: PayloadEncoding.rowWindow,
			});
		const host = {
			onEnvelope: vi.fn<HostConnection['onEnvelope']>(() => vi.fn<() => void>()),
			request,
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
		const posted = postMessage.mock.calls[0]?.[0] as { rows: ArrayBuffer };
		expect(new Uint8Array(posted.rows)).toEqual(new Uint8Array([7, 8, 9]));
		expect(broker.metrics).toMatchObject({
			rowWindowBytesForwarded: 3,
			rowWindowsForwarded: 1,
		});
	});

	it('streams a bounded Host selection directly into Main clipboard exactly once', async () => {
		let hostListener: ((envelope: DecodedProtocolEnvelope) => void) | undefined;
		const bootstrap = {
			attachmentId: AttachmentIdSchema.parse('11'.repeat(16)),
			canResize: true,
			creationId: CreationIdSchema.parse('22'.repeat(16)),
			hostEpoch: HostEpochSchema.parse('33'.repeat(16)),
			terminalId: TerminalIdSchema.parse('44'.repeat(16)),
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
			onEnvelope: vi.fn<HostConnection['onEnvelope']>((listener) => {
				hostListener = listener;
				return vi.fn<() => void>();
			}),
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
				MessageKind.selectionExportChunkApplied,
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

	it('allows only explicit web and mail schemes through the external-link effect', async () => {
		const bootstrap = {
			attachmentId: AttachmentIdSchema.parse('11'.repeat(16)),
			canResize: true,
			creationId: CreationIdSchema.parse('22'.repeat(16)),
			hostEpoch: HostEpochSchema.parse('33'.repeat(16)),
			terminalId: TerminalIdSchema.parse('44'.repeat(16)),
		};
		const host = {
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

	it('awaits Host detach exactly once before completing attachment retirement', async () => {
		const bootstrap = {
			attachmentId: AttachmentIdSchema.parse('11'.repeat(16)),
			canResize: true,
			creationId: CreationIdSchema.parse('22'.repeat(16)),
			hostEpoch: HostEpochSchema.parse('33'.repeat(16)),
			terminalId: TerminalIdSchema.parse('44'.repeat(16)),
		};
		let finishDetach: (() => void) | undefined;
		const requestControl = vi.fn<(...arguments_: unknown[]) => Promise<unknown>>(
			() =>
				new Promise((resolve) => {
					finishDetach = () => resolve({ attachmentId: bootstrap.attachmentId });
				}),
		);
		const host = {
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
			MessageKind.detachTerminal,
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
});
