import { describe, expect, it } from 'vitest';

import {
	SSH_ENVIRONMENT_FRAME_HEADER_BYTES,
	SSH_ENVIRONMENT_MAX_FRAME_BYTES,
	SSH_ENVIRONMENT_PROTOCOL_VERSION,
	SSH_PROJECT_ROOT_SERVICE_ID,
	SSH_TERMINAL_HOST_SERVICE_ID,
	SshEnvironmentBuildDigestSchema,
	SshEnvironmentChannelIdSchema,
	SshEnvironmentClientNonceSchema,
	SshEnvironmentLoginShellSchema,
	SshEnvironmentReleaseCohortDigestSchema,
	SshEnvironmentRuntimeEpochSchema,
	SshEnvironmentRuntimeInstanceIdSchema,
	SshEnvironmentRuntimeVersionSchema,
	SshEnvironmentServiceIdSchema,
	SshEnvironmentTerminalHostVersionSchema,
	decodeSshEnvironmentFrame,
	encodeSshEnvironmentMessage,
	type SshEnvironmentDecodeError,
	type SshEnvironmentDecodeResult,
	type SshEnvironmentMessage,
} from '../ssh-environment-protocol';

const runtimeInstanceId = SshEnvironmentRuntimeInstanceIdSchema.parse('1'.repeat(32));
const runtimeEpoch = SshEnvironmentRuntimeEpochSchema.parse('2'.repeat(32));
const clientNonce = SshEnvironmentClientNonceSchema.parse('3'.repeat(32));
const buildDigest = SshEnvironmentBuildDigestSchema.parse('4'.repeat(64));
const terminalHostBuildDigest = SshEnvironmentBuildDigestSchema.parse('5'.repeat(64));
const releaseCohortDigest = SshEnvironmentReleaseCohortDigestSchema.parse('6'.repeat(64));
const runtimeVersion = SshEnvironmentRuntimeVersionSchema.parse('0.2.0');
const terminalHostVersion = SshEnvironmentTerminalHostVersionSchema.parse('0.2.0');
const loginShell = SshEnvironmentLoginShellSchema.parse('/bin/zsh');
const channelId = SshEnvironmentChannelIdSchema.parse(7);
const binaryChannelId = SshEnvironmentChannelIdSchema.parse(9);
const serviceId = SshEnvironmentServiceIdSchema.parse(SSH_PROJECT_ROOT_SERVICE_ID);

const helloPayload = {
	protocolVersion: SSH_ENVIRONMENT_PROTOCOL_VERSION,
	runtimeInstanceId,
	clientNonce,
	maxFrameBytes: SSH_ENVIRONMENT_MAX_FRAME_BYTES,
	initialChannelCreditBytes: 65_536,
} as const;

const hello: SshEnvironmentMessage = {
	type: 'hello',
	payload: helloPayload,
};

describe('SSH Environment protocol framing', () => {
	it('round-trips every JSON control message kind', () => {
		const messages: readonly SshEnvironmentMessage[] = [
			hello,
			{
				type: 'hello-accepted',
				payload: {
					protocolVersion: SSH_ENVIRONMENT_PROTOCOL_VERSION,
					runtimeInstanceId,
					runtimeEpoch,
					runtimeVersion,
					runtimeBuildDigest: buildDigest,
					releaseCohortDigest,
					loginShell,
					terminalHostVersion,
					terminalHostBuildDigest,
					platform: 'darwin-arm64',
					capabilities: [SSH_PROJECT_ROOT_SERVICE_ID, SSH_TERMINAL_HOST_SERVICE_ID],
					heartbeatIntervalMs: 15_000,
					heartbeatMissLimit: 3,
					maxFrameBytes: SSH_ENVIRONMENT_MAX_FRAME_BYTES,
					initialChannelCreditBytes: 65_536,
				},
			},
			{ type: 'heartbeat', payload: { runtimeEpoch, sequence: 1 } },
			{ type: 'heartbeat-acknowledged', payload: { runtimeEpoch, sequence: 1 } },
			{ type: 'failure', payload: { code: 'service-unsupported' } },
			{
				type: 'channel-open',
				payload: {
					runtimeEpoch,
					channelId,
					serviceId,
					initialCreditBytes: 4096,
				},
			},
			{
				type: 'channel-opened',
				payload: { runtimeEpoch, channelId, initialCreditBytes: 4096 },
			},
			{
				type: 'channel-close',
				payload: { runtimeEpoch, channelId, reason: 'completed' },
			},
			{
				type: 'channel-closed',
				payload: { runtimeEpoch, channelId, reason: 'completed' },
			},
		];

		for (const message of messages) {
			expect(decodeSshEnvironmentFrame(encodeSshEnvironmentMessage(message))).toEqual({
				ok: true,
				message,
			});
		}
	});

	it('round-trips binary channel data and credit without JSON conversion', () => {
		const dataMessage: SshEnvironmentMessage = {
			type: 'channel-data',
			channelId: binaryChannelId,
			data: Uint8Array.of(0, 255, 1, 128),
		};
		const creditMessage: SshEnvironmentMessage = {
			type: 'channel-credit',
			channelId: binaryChannelId,
			bytes: 8192,
		};

		expect(decodeSshEnvironmentFrame(encodeSshEnvironmentMessage(dataMessage))).toEqual({
			ok: true,
			message: dataMessage,
		});
		expect(decodeSshEnvironmentFrame(encodeSshEnvironmentMessage(creditMessage))).toEqual({
			ok: true,
			message: creditMessage,
		});
	});

	it('rejects malformed headers, unsupported kinds, and inconsistent lengths', () => {
		expect.hasAssertions();
		const cases: Array<readonly [number, number, SshEnvironmentDecodeError['code']]> = [
			[0, 'X'.charCodeAt(0), 'magic-invalid'],
			[4, 1, 'version-unsupported'],
			[7, 255, 'kind-unsupported'],
			[12, 1, 'reserved-bytes-nonzero'],
		];
		for (const [offset, value, expectedCode] of cases) {
			const frame = encodeSshEnvironmentMessage(hello);
			frame[offset] = value;
			expectDecodeFailure(decodeSshEnvironmentFrame(frame), expectedCode);
		}

		const truncated = encodeSshEnvironmentMessage(hello).slice(0, -1);
		expectDecodeFailure(decodeSshEnvironmentFrame(truncated), 'frame-length-mismatch');
	});

	it('rejects non-strict or invalid JSON payloads', () => {
		expect.hasAssertions();
		const frame = encodeSshEnvironmentMessage(hello);
		const payload = new TextEncoder().encode(JSON.stringify({ ...helloPayload, unexpected: true }));
		const malformed = new Uint8Array(SSH_ENVIRONMENT_FRAME_HEADER_BYTES + payload.byteLength);
		malformed.set(frame.slice(0, SSH_ENVIRONMENT_FRAME_HEADER_BYTES));
		new DataView(malformed.buffer).setUint32(8, payload.byteLength, false);
		malformed.set(payload, SSH_ENVIRONMENT_FRAME_HEADER_BYTES);

		expectDecodeFailure(decodeSshEnvironmentFrame(malformed), 'payload-invalid');
	});

	it('rejects exact and escaped-equivalent duplicate JSON members', () => {
		expect.hasAssertions();
		const canonicalFrame = encodeSshEnvironmentMessage(hello);
		const canonicalPayload = JSON.stringify(helloPayload);
		for (const duplicateMember of [
			'"initialChannelCreditBytes":1',
			'"initialChannelCredit\\u0042ytes":1',
		]) {
			const text = `${canonicalPayload.slice(0, -1)},${duplicateMember}}`;
			const payload = new TextEncoder().encode(text);
			const frame = new Uint8Array(SSH_ENVIRONMENT_FRAME_HEADER_BYTES + payload.byteLength);
			frame.set(canonicalFrame.slice(0, SSH_ENVIRONMENT_FRAME_HEADER_BYTES));
			new DataView(frame.buffer).setUint32(8, payload.byteLength, false);
			frame.set(payload, SSH_ENVIRONMENT_FRAME_HEADER_BYTES);

			expectDecodeFailure(decodeSshEnvironmentFrame(frame), 'payload-invalid');
		}
	});

	it('enforces the one MiB frame bound before allocation or parsing', () => {
		expect.hasAssertions();
		const header = encodeSshEnvironmentMessage(hello).slice(0, SSH_ENVIRONMENT_FRAME_HEADER_BYTES);
		new DataView(header.buffer).setUint32(8, SSH_ENVIRONMENT_MAX_FRAME_BYTES, false);
		expectDecodeFailure(decodeSshEnvironmentFrame(header), 'frame-too-large');
	});
});

function expectDecodeFailure(
	result: SshEnvironmentDecodeResult,
	code: SshEnvironmentDecodeError['code'],
): void {
	expect(result).toMatchObject({ ok: false, error: { code } });
}
