import {
	SSH_TERMINAL_HOST_INITIAL_CREDIT_BYTES,
	SSH_TERMINAL_HOST_MAX_CHUNK_BYTES,
	SSH_TERMINAL_HOST_SERVICE_ID,
	SshEnvironmentChannelIdSchema,
	SshEnvironmentRuntimeEpochSchema,
	SshEnvironmentTerminalHostVersionSchema,
	decodeSshEnvironmentFrame,
	type SshEnvironmentChannelId,
	type SshEnvironmentMessage,
} from '@orchbee/ssh-environment-protocol';
import {
	CURRENT_PROTOCOL_VERSION,
	HostEpochSchema,
	MessageKind,
	PROTOCOL_LIMITS,
	PayloadEncoding,
	decodeProtocolEnvelope,
	encodeProtocolEnvelope,
} from '@orchbee/terminal-protocol';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { SshTerminalHostChannelClient } from '../SshTerminalHostChannelClient';

const runtimeEpoch = SshEnvironmentRuntimeEpochSchema.parse('2'.repeat(32));
const expectedHostVersion = SshEnvironmentTerminalHostVersionSchema.parse('0.1.0');
const hostEpoch = HostEpochSchema.parse('a'.repeat(32));

afterEach(() => vi.useRealTimers());

describe('SSH Terminal Host channel client', () => {
	it('negotiates a real HostConnection over the relay and replenishes consumed Host output credit', async () => {
		const harness = createHarness();
		const opening = harness.client.open();
		const channelId = requireOpenChannelId(harness.messages[0]);
		expect(harness.messages[0]).toEqual({
			type: 'channel-open',
			payload: {
				runtimeEpoch,
				channelId,
				serviceId: SSH_TERMINAL_HOST_SERVICE_ID,
				initialCreditBytes: SSH_TERMINAL_HOST_INITIAL_CREDIT_BYTES,
			},
		});

		expect(harness.client.handle(opened(channelId))).toBe('handled');
		await vi.waitFor(() => expect(findLastData(harness.messages)).toBeDefined());
		const hostHello = findLastData(harness.messages);
		if (hostHello === undefined) throw new Error('missing relayed Terminal Host Hello');
		const response = createHelloAccepted(hostHello.data, expectedHostVersion);
		expect(harness.client.handle({ type: 'channel-data', channelId, data: response })).toBe(
			'handled',
		);

		const outcome = await opening;
		expect(outcome.kind).toBe('ready');
		if (outcome.kind !== 'ready') throw new Error('expected negotiated HostConnection');
		expect(outcome.connection.hostEpoch).toBe(hostEpoch);
		expect(harness.messages).toContainEqual({
			type: 'channel-credit',
			channelId,
			bytes: response.byteLength,
		});
		expect(harness.onAttachmentInvalid).not.toHaveBeenCalled();
		harness.client.retire();
	});

	it('enforces one relay and maps Runtime open failure without retiring the attachment', async () => {
		const harness = createHarness();
		const first = harness.client.open();
		const channelId = requireOpenChannelId(harness.messages[0]);
		await expect(harness.client.open()).resolves.toEqual({ kind: 'failed', reason: 'busy' });
		expect(
			harness.client.handle({
				type: 'channel-closed',
				payload: { runtimeEpoch, channelId, reason: 'service-unavailable' },
			}),
		).toBe('handled');
		await expect(first).resolves.toEqual({ kind: 'failed', reason: 'unavailable' });
		expect(harness.onAttachmentInvalid).not.toHaveBeenCalled();
	});

	it('closes only the relay when the negotiated Host version is outside the release cohort', async () => {
		const harness = createHarness();
		const opening = harness.client.open();
		const channelId = requireOpenChannelId(harness.messages[0]);
		harness.client.handle(opened(channelId));
		await vi.waitFor(() => expect(findLastData(harness.messages)).toBeDefined());
		const hostHello = findLastData(harness.messages);
		if (hostHello === undefined) throw new Error('missing relayed Terminal Host Hello');
		const incompatibleVersion = SshEnvironmentTerminalHostVersionSchema.parse('9.9.9');
		const response = createHelloAccepted(hostHello.data, incompatibleVersion);
		harness.client.handle({ type: 'channel-data', channelId, data: response });

		await expect(opening).resolves.toEqual({
			kind: 'failed',
			reason: 'compatibility-unsupported',
		});
		await vi.waitFor(() =>
			expect(harness.messages).toContainEqual({
				type: 'channel-close',
				payload: { runtimeEpoch, channelId, reason: 'compatibility-unsupported' },
			}),
		);
		expect(harness.onAttachmentInvalid).not.toHaveBeenCalled();
		harness.client.retire();
	});

	it('treats oversized relay chunks and credit overflow as attachment corruption', async () => {
		const harness = createHarness();
		void harness.client.open();
		const channelId = requireOpenChannelId(harness.messages[0]);
		harness.client.handle(opened(channelId));
		expect(
			harness.client.handle({
				type: 'channel-data',
				channelId,
				data: new Uint8Array(SSH_TERMINAL_HOST_MAX_CHUNK_BYTES + 1),
			}),
		).toBe('attachment-invalid');
		expect(
			harness.client.handle({
				type: 'channel-credit',
				channelId,
				bytes: SSH_TERMINAL_HOST_INITIAL_CREDIT_BYTES,
			}),
		).toBe('attachment-invalid');
		harness.client.retire();
	});
});

function createHarness() {
	const messages: SshEnvironmentMessage[] = [];
	const onAttachmentInvalid = vi.fn<() => void>();
	let nextChannelId = 1;
	const client = new SshTerminalHostChannelClient({
		allocateChannelId: () => SshEnvironmentChannelIdSchema.parse(nextChannelId++),
		expectedHostVersion,
		onAttachmentInvalid,
		runtimeEpoch,
		write: async (bytes) => {
			const decoded = decodeSshEnvironmentFrame(bytes);
			if (!decoded.ok) throw new Error(decoded.error.code);
			messages.push(decoded.message);
		},
	});
	return { client, messages, onAttachmentInvalid };
}

function requireOpenChannelId(message: SshEnvironmentMessage | undefined): SshEnvironmentChannelId {
	if (message?.type !== 'channel-open') throw new Error('missing Terminal Host channel-open');
	return message.payload.channelId;
}

function opened(channelId: SshEnvironmentChannelId): SshEnvironmentMessage {
	return {
		type: 'channel-opened',
		payload: {
			runtimeEpoch,
			channelId,
			initialCreditBytes: SSH_TERMINAL_HOST_INITIAL_CREDIT_BYTES,
		},
	};
}

function findLastData(
	messages: readonly SshEnvironmentMessage[],
): Extract<SshEnvironmentMessage, { readonly type: 'channel-data' }> | undefined {
	for (let index = messages.length - 1; index >= 0; index -= 1) {
		const message = messages[index];
		if (message?.type === 'channel-data') return message;
	}
	return undefined;
}

function createHelloAccepted(
	helloRecord: Uint8Array,
	hostVersion: typeof expectedHostVersion,
): Uint8Array {
	const request = decodeProtocolEnvelope(helloRecord);
	if (!request.ok) throw new Error(request.detail);
	if (request.value.kind !== MessageKind.hello) throw new Error('expected Terminal Host Hello');
	const response = encodeProtocolEnvelope({
		correlationId: request.value.correlationId,
		event: false,
		hostEpoch,
		kind: MessageKind.helloAccepted,
		optionalMessage: false,
		payload: new TextEncoder().encode(
			JSON.stringify({
				frameBytes: PROTOCOL_LIMITS.maxFrameBytes,
				hostEpoch,
				hostVersion,
				inputBytes: PROTOCOL_LIMITS.maxInputBytes,
				protocol: CURRENT_PROTOCOL_VERSION,
				rowWindowBytes: PROTOCOL_LIMITS.maxRowWindowBytes,
				supportedOptionalFeatures: [],
			}),
		),
		payloadEncoding: PayloadEncoding.controlJson,
		protocol: CURRENT_PROTOCOL_VERSION,
		response: true,
	});
	if (!response.ok) throw new Error(response.detail);
	return response.value;
}
