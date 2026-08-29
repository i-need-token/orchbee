import { describe, expect, it } from 'vitest';

import {
	SSH_ENVIRONMENT_MAX_FRAME_BYTES,
	SSH_ENVIRONMENT_PROTOCOL_VERSION,
	SshEnvironmentChannelIdSchema,
	SshEnvironmentClientNonceSchema,
	SshEnvironmentRuntimeEpochSchema,
	SshEnvironmentRuntimeInstanceIdSchema,
	encodeSshEnvironmentMessage,
	type SshEnvironmentMessage,
} from '../ssh-environment-protocol';
import {
	SshEnvironmentFrameDecoder,
	type SshEnvironmentFrameDecoderResult,
} from '../SshEnvironmentFrameDecoder';

const runtimeInstanceId = SshEnvironmentRuntimeInstanceIdSchema.parse('1'.repeat(32));
const runtimeEpoch = SshEnvironmentRuntimeEpochSchema.parse('2'.repeat(32));
const clientNonce = SshEnvironmentClientNonceSchema.parse('3'.repeat(32));
const binaryChannelId = SshEnvironmentChannelIdSchema.parse(9);
const hello: SshEnvironmentMessage = {
	type: 'hello',
	payload: {
		protocolVersion: SSH_ENVIRONMENT_PROTOCOL_VERSION,
		runtimeInstanceId,
		clientNonce,
		maxFrameBytes: SSH_ENVIRONMENT_MAX_FRAME_BYTES,
		initialChannelCreditBytes: 65_536,
	},
};

describe('SshEnvironmentFrameDecoder', () => {
	it('assembles split frames and emits coalesced frames in order', () => {
		const first = encodeSshEnvironmentMessage(hello);
		const secondMessage: SshEnvironmentMessage = {
			type: 'heartbeat',
			payload: { runtimeEpoch, sequence: 11 },
		};
		const second = encodeSshEnvironmentMessage(secondMessage);
		const decoder = new SshEnvironmentFrameDecoder();

		expect(decoder.push(first.slice(0, 7))).toEqual({ ok: true, messages: [] });
		const combined = new Uint8Array(first.byteLength - 7 + second.byteLength);
		combined.set(first.slice(7));
		combined.set(second, first.byteLength - 7);
		expect(decoder.push(combined)).toEqual({
			ok: true,
			messages: [hello, secondMessage],
		});
		expect(decoder.finish()).toEqual({ ok: true, messages: [] });
	});

	it('decodes multiple bounded frames from a transport chunk larger than one frame', () => {
		const dataBytes = 600 * 1024;
		const first = encodeSshEnvironmentMessage({
			type: 'channel-data',
			channelId: binaryChannelId,
			data: new Uint8Array(dataBytes).fill(1),
		});
		const second = encodeSshEnvironmentMessage({
			type: 'channel-data',
			channelId: binaryChannelId,
			data: new Uint8Array(dataBytes).fill(2),
		});
		const combined = new Uint8Array(first.byteLength + second.byteLength);
		combined.set(first);
		combined.set(second, first.byteLength);
		expect(combined.byteLength).toBeGreaterThan(SSH_ENVIRONMENT_MAX_FRAME_BYTES);

		const messages = requireMessages(new SshEnvironmentFrameDecoder().push(combined));
		const channelMessages = messages.filter(
			(message): message is Extract<SshEnvironmentMessage, { readonly type: 'channel-data' }> =>
				message.type === 'channel-data',
		);
		expect(channelMessages).toHaveLength(2);
		expect(channelMessages.map((message) => message.data.byteLength)).toEqual([
			dataBytes,
			dataBytes,
		]);
		expect(channelMessages.map((message) => message.data[0])).toEqual([1, 2]);
	});

	it('fails closed and remains failed after a malformed or partial stream', () => {
		const malformed = encodeSshEnvironmentMessage(hello);
		malformed[0] = 0;
		const decoder = new SshEnvironmentFrameDecoder();
		const first = decoder.push(malformed);
		expect(first).toMatchObject({ ok: false, error: { code: 'magic-invalid' } });
		expect(decoder.push(encodeSshEnvironmentMessage(hello))).toEqual(first);

		const partialDecoder = new SshEnvironmentFrameDecoder();
		partialDecoder.push(encodeSshEnvironmentMessage(hello).slice(0, -1));
		expect(partialDecoder.finish()).toMatchObject({
			ok: false,
			error: { code: 'frame-length-mismatch' },
		});
	});
});

function requireMessages(
	result: SshEnvironmentFrameDecoderResult,
): readonly SshEnvironmentMessage[] {
	expect(result.ok).toBe(true);
	if (!result.ok) throw new Error(`expected decoded messages, received ${result.error.code}`);
	return result.messages;
}
