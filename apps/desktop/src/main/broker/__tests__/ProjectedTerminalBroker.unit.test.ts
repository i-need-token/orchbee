import { EventEmitter } from 'node:events';

import {
	AttachmentIdSchema,
	CreationIdSchema,
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
});
